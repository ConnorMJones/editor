//! Outgoing links of every markdown note, for the backlinks picker: read off the UI thread on
//! first use, then kept current by watcher events and by open buffers.

use std::collections::HashMap;
use std::io::{self, Read as _};
use std::path::Path;

use pulldown_cmark::{Event, LinkType, Parser, Tag};

use crate::input::picker::{Hit, Items, Narrowing, Search};
use crate::job::Job;
use crate::project::Project;
use crate::project::files::{FileIndex, Indexed, OnIndexed};
use crate::project::links::{self, Dest, Resolution};
use crate::project::path::RelPath;
use crate::render::document::Documents;
use crate::render::markdown::{self, Format, LinkTarget, WikiLink};
use crate::text::buffer::{Buffer, BufferId};
use crate::{Effect, Overlay, State, render::document};
use slotmap::SlotMap;

#[derive(Default)]
pub struct Backlinks {
    pub build: Build,
    pub notes: HashMap<RelPath, Note>,
    /// Stamp of the newest read; each note keeps the stamp of the read it came from, so
    /// older reads finishing late are dropped.
    pub stamp: u64,
}

#[derive(Default)]
pub enum Build {
    #[default]
    Unbuilt,
    /// The first read of every note runs; notes that change meanwhile are read again after it.
    Building {
        stamp: u64,
        dirty: Vec<RelPath>,
    },
    Ready,
}

pub struct Note {
    pub stamp: u64,
    pub source: Source,
    pub links: Vec<OutLink>,
}

/// Where a note's links were read from; an open buffer's text wins over the disk.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    Disk,
    Buffer { buffer: BufferId, version: u64 },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OutLink {
    pub dest: Dest<String>,
    /// `links::link_key` of the file name `dest` names.
    pub key: u64,
    /// Zero-based.
    pub line: u32,
    /// Zero-based character offset within the line.
    pub column: u32,
    /// The trimmed line holding the link.
    pub text: String,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Read {
    Links(Vec<OutLink>),
    /// Missing or unreadable as UTF-8.
    Gone,
}

const MAX_LINE: usize = 240;

/// Appends the wikilinks, embeds and relative links of markdown `src` to `out`.
pub fn extract(src: &str, out: &mut Vec<OutLink>) {
    let mut line = 0u32;
    let mut line_start = 0usize;
    let mut scanned = 0usize;
    for (event, range) in Parser::new_ext(src, markdown::OPTIONS).into_offset_iter() {
        let (link_type, dest) = match event {
            Event::Start(Tag::Link {
                link_type,
                dest_url,
                ..
            })
            | Event::Start(Tag::Image {
                link_type,
                dest_url,
                ..
            }) => (link_type, dest_url),
            _ => continue,
        };
        let dest = match markdown::link_target(link_type, &dest, None) {
            LinkTarget::Wiki(WikiLink { target, .. }) if !target.is_empty() => Dest::Wiki(target),
            LinkTarget::Path { path, .. } if !path.is_empty() => match link_type {
                LinkType::Autolink | LinkType::Email => continue,
                _ => Dest::Path(path),
            },
            LinkTarget::Wiki(_)
            | LinkTarget::Path { .. }
            | LinkTarget::Url(_)
            | LinkTarget::Heading(_) => continue,
        };
        // Links arrive in source order; a step back recounts from the top.
        match range.start < scanned {
            true => (line, line_start, scanned) = (0, 0, 0),
            false => {}
        }
        for (offset, byte) in src.as_bytes()[scanned..range.start].iter().enumerate() {
            match byte {
                b'\n' => {
                    line += 1;
                    line_start = scanned + offset + 1;
                }
                _ => {}
            }
        }
        scanned = range.start;
        let end = src[line_start..]
            .find('\n')
            .map_or(src.len(), |at| line_start + at);
        let (Dest::Wiki(name) | Dest::Path(name)) = &dest;
        let key = links::link_key(name.rsplit('/').next().unwrap_or(name));
        out.push(OutLink {
            key,
            line,
            column: src[line_start..range.start].chars().count() as u32,
            text: src[line_start..end].trim().chars().take(MAX_LINE).collect(),
            dest,
        });
    }
}

/// Reads and extracts each note.
pub fn read(root: &Path, paths: Vec<RelPath>) -> Vec<(RelPath, Read)> {
    let mut text = String::new();
    paths
        .into_iter()
        .map(|path| {
            text.clear();
            let read = std::fs::File::open(path.to_path(root))
                .and_then(|mut file| file.read_to_string(&mut text));
            let read = match read {
                Ok(_) => {
                    let mut links = Vec::new();
                    extract(&text, &mut links);
                    Read::Links(links)
                }
                Err(error) => {
                    match error.kind() {
                        io::ErrorKind::NotFound => {}
                        _ => tracing::warn!(%error, %path, "cannot read note links"),
                    }
                    Read::Gone
                }
            };
            (path, read)
        })
        .collect()
}

fn is_note(path: &RelPath) -> bool {
    Format::of(Path::new(path.as_str())) == Some(Format::Markdown)
}

fn request(
    backlinks: &mut Backlinks,
    root: &Path,
    paths: Vec<RelPath>,
    out: &mut Vec<Effect>,
) -> u64 {
    backlinks.stamp += 1;
    out.push(Effect::Job(Job::Links {
        root: root.to_owned(),
        paths,
        stamp: backlinks.stamp,
    }));
    backlinks.stamp
}

/// Starts the first read of every note unless it ran already; before the first walk ends, asks
/// for it to start then.
pub fn ensure(state: &mut State, out: &mut Vec<Effect>) {
    let root = state.project.root.path();
    match &mut state.project.index {
        FileIndex::Loading { then } => *then = OnIndexed::BuildBacklinks,
        FileIndex::Ready(Indexed {
            files, backlinks, ..
        }) => match backlinks.build {
            Build::Unbuilt => {
                let notes = files.iter().filter(|file| is_note(file)).cloned().collect();
                let stamp = request(backlinks, root, notes, out);
                backlinks.build = Build::Building {
                    stamp,
                    dirty: Vec::new(),
                };
            }
            Build::Building { .. } | Build::Ready => {}
        },
    }
}

/// Notes the watcher saw change.
pub fn changed(backlinks: &mut Backlinks, root: &Path, paths: &[RelPath], out: &mut Vec<Effect>) {
    let notes: Vec<RelPath> = paths.iter().filter(|path| is_note(path)).cloned().collect();
    match (&mut backlinks.build, notes.is_empty()) {
        (_, true) | (Build::Unbuilt, false) => {}
        (Build::Building { dirty, .. }, false) => dirty.extend(notes),
        (Build::Ready, false) => {
            request(backlinks, root, notes, out);
        }
    }
}

/// The file index changed: drops notes that are gone and reads ones it does not know.
pub fn reconcile(indexed: &mut Indexed, root: &Path, out: &mut Vec<Effect>) {
    let Indexed {
        files, backlinks, ..
    } = indexed;
    match backlinks.build {
        Build::Ready => {}
        Build::Unbuilt | Build::Building { .. } => return,
    }
    let notes = &mut backlinks.notes;
    notes.retain(|path, _| files.binary_search(path).is_ok());
    let unknown: Vec<RelPath> = files
        .iter()
        .filter(|file| is_note(file) && !notes.contains_key(*file))
        .cloned()
        .collect();
    match unknown.is_empty() {
        true => {}
        false => {
            request(backlinks, root, unknown, out);
        }
    }
}

/// A finished read: applies notes no newer source replaced, then fills a waiting picker.
pub fn linked(state: &mut State, stamp: u64, reads: Vec<(RelPath, Read)>, out: &mut Vec<Effect>) {
    let State {
        project: Project { index, root, .. },
        buffers,
        documents,
        overlay,
        matcher,
        ..
    } = state;
    let indexed = match index {
        FileIndex::Ready(indexed) => indexed,
        FileIndex::Loading { .. } => unreachable!("notes are read only from a built index"),
    };
    let backlinks = &mut indexed.backlinks;
    for (path, read) in reads {
        let newer = backlinks.notes.get(&path).is_some_and(|note| {
            note.stamp > stamp
                || match note.source {
                    Source::Buffer { buffer, .. } => buffers.contains_key(buffer),
                    Source::Disk => false,
                }
        });
        match (newer, read) {
            (true, _) => {}
            (false, Read::Links(links)) => {
                backlinks.notes.insert(
                    path,
                    Note {
                        stamp,
                        source: Source::Disk,
                        links,
                    },
                );
            }
            (false, Read::Gone) => {
                backlinks.notes.remove(&path);
            }
        }
    }
    let dirty = match &mut backlinks.build {
        Build::Building {
            stamp: started,
            dirty,
        } if *started == stamp => std::mem::take(dirty),
        Build::Building { .. } | Build::Unbuilt | Build::Ready => return,
    };
    backlinks.build = Build::Ready;
    match dirty.is_empty() {
        true => {}
        false => {
            request(backlinks, root.path(), dirty, out);
        }
    }
    reconcile(indexed, root.path(), out);
    // A backlinks picker opened before the first read fills now.
    let Overlay::Picker(picker) = overlay else {
        return;
    };
    match &mut picker.items {
        Items::Backlinks(referrers) => match referrers.search {
            Search::Running => {
                sync_buffers(&mut indexed.backlinks, root.path(), buffers, documents, out);
                referrers.hits = hits(indexed, &referrers.note);
                referrers.search = Search::Done;
            }
            Search::Idle | Search::Done | Search::Truncated => return,
        },
        Items::Files(_) | Items::Buffers(_) | Items::Commands | Items::Grep(_) => return,
    }
    crate::input::picker::filter(picker, matcher, Narrowing::All);
    crate::view::panes::request_preview(state, out);
}

/// Reads the links of open notes whose text moved since, and re-reads notes whose buffer
/// closed.
pub fn sync_buffers(
    backlinks: &mut Backlinks,
    root: &Path,
    buffers: &SlotMap<BufferId, Buffer>,
    documents: &Documents,
    out: &mut Vec<Effect>,
) {
    let mut scratch = String::new();
    for (id, buffer) in buffers.iter() {
        let doc = &buffer.doc;
        let Ok(path) = RelPath::new(root, &buffer.path) else {
            continue;
        };
        let source = Source::Buffer {
            buffer: id,
            version: doc.version,
        };
        match (
            is_note(&path),
            backlinks.notes.get(&path).map(|note| note.source),
        ) {
            (false, _) => continue,
            (true, Some(known)) if known == source => continue,
            (true, Some(_) | None) => {}
        }
        let text = match document::markdown(documents, id, doc) {
            Some(parsed) => parsed.source.as_str(),
            None => {
                scratch.clear();
                for chunk in doc.text.chunks() {
                    scratch.push_str(chunk);
                }
                scratch.as_str()
            }
        };
        let mut links = Vec::new();
        extract(text, &mut links);
        backlinks.stamp += 1;
        backlinks.notes.insert(
            path,
            Note {
                stamp: backlinks.stamp,
                source,
                links,
            },
        );
    }
    let closed: Vec<RelPath> = backlinks
        .notes
        .iter()
        .filter(|(_, note)| match note.source {
            Source::Buffer { buffer, .. } => !buffers.contains_key(buffer),
            Source::Disk => false,
        })
        .map(|(path, _)| path.clone())
        .collect();
    match closed.is_empty() {
        true => {}
        false => {
            request(backlinks, root, closed, out);
        }
    }
}

/// Every link to `note`, in path and line order.
pub fn hits(indexed: &mut Indexed, note: &RelPath) -> Vec<Hit> {
    let key = links::link_key(note.name());
    let (index, backlinks) = indexed.links();
    let mut hits: Vec<Hit> = backlinks
        .notes
        .iter()
        .flat_map(|(path, entry)| {
            let dir = path.folder_path();
            entry
                .links
                .iter()
                .filter(move |link| {
                    link.key == key
                        && links::resolve(index, dir, link.dest.as_str())
                            == Resolution::File(note.clone())
                })
                .map(move |link| Hit {
                    path: path.clone(),
                    line: link.line,
                    column: link.column,
                    text: link.text.clone(),
                })
        })
        .collect();
    hits.sort_unstable_by(|a, b| (&a.path, a.line, a.column).cmp(&(&b.path, b.line, b.column)));
    hits
}
