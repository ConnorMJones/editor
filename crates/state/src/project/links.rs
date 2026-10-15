//! The project root as an Obsidian vault: resolves wikilinks and markdown links to files,
//! follows them, and finds the link or path under the cursor for `gf`.

use std::collections::HashMap;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use helix_core::SmallVec;
use helix_stdx::rope::RopeSliceExt;
use model::config::OpenAs;
use model::notice::Notice;

use crate::apply::{self, focused};
use crate::git::changes::Arrive;
use crate::project::files::Files;
use crate::project::path::RelPath;
use crate::render::markdown::{self, BlockKind, Format, LinkTarget, Markdown, WikiLink};
use crate::text::buffer::{BufferId, Target, TextDoc};
use crate::view::layout::PaneId;
use crate::{Confirm, Effect, State};

pub struct Vault {
    /// Bumped whenever the set of indexed files changes.
    pub generation: u64,
    /// Built on first use after the generation moved.
    pub index: Option<Index>,
}

impl Vault {
    pub const fn new() -> Vault {
        Vault {
            generation: 1,
            index: None,
        }
    }
}

impl Default for Vault {
    fn default() -> Vault {
        Vault::new()
    }
}

/// File names of the file index, for resolving links by name.
pub struct Index {
    /// The `Vault::generation` it was built for.
    pub generation: u64,
    pub files: Arc<Vec<RelPath>>,
    /// Lowercased file name to positions in `files`.
    pub names: HashMap<String, SmallVec<[u32; 1]>>,
}

impl Index {
    pub fn new(generation: u64, files: Arc<Vec<RelPath>>) -> Index {
        let mut names: HashMap<String, SmallVec<[u32; 1]>> = HashMap::with_capacity(files.len());
        for (at, file) in files.iter().enumerate() {
            names
                .entry(file.name().to_lowercase())
                .or_default()
                .push(at as u32);
        }
        Index {
            generation,
            files,
            names,
        }
    }
}

/// The index for the current generation, built from the file index when missing.
pub fn index<'a>(vault: &'a mut Vault, files: &Files) -> &'a Index {
    let Vault { generation, index } = vault;
    index.get_or_insert_with(|| {
        let files = match files {
            Files::Ready(files) => files.clone(),
            Files::Loading => Arc::default(),
        };
        Index::new(*generation, files)
    })
}

/// The file index changed: links resolve again on next use.
pub fn invalidate(vault: &mut Vault) {
    vault.generation += 1;
    vault.index = None;
}

/// A link's file part as written, without heading or alias.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Dest<S> {
    /// `[[target]]`: a name or a path suffix anywhere in the vault.
    Wiki(S),
    /// A markdown link's percent-decoded path, relative to the note or, with a leading `/`,
    /// to the vault root.
    Path(S),
}

impl<S: AsRef<str>> Dest<S> {
    pub fn as_str(&self) -> Dest<&str> {
        match self {
            Dest::Wiki(name) => Dest::Wiki(name.as_ref()),
            Dest::Path(path) => Dest::Path(path.as_ref()),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Resolution {
    File(RelPath),
    /// No such file; following the link offers to create it here.
    Missing(RelPath),
    /// Leaves the vault or names nothing.
    Invalid,
}

/// Resolves `dest` from a note in folder `dir` by Obsidian's rules: names match
/// case-insensitively as `name.md` and then as written; among several files, an exact-case
/// match wins, then the shallowest path, then one in `dir`, then the shortest and first.
pub fn resolve(index: &Index, dir: &str, dest: Dest<&str>) -> Resolution {
    match dest {
        Dest::Wiki(name) => {
            let name = name.trim().trim_start_matches('/');
            let path = match RelPath::parse(name) {
                Ok(path) if !path.is_root() => path,
                Ok(_) | Err(_) => return Resolution::Invalid,
            };
            match lookup(index, dir, path.as_str()) {
                Some(found) => Resolution::File(found),
                None => {
                    let note = with_md(path.as_str());
                    Resolution::Missing(match path.parent() {
                        Some("") | None => RelPath(dir.to_owned()).join(&note),
                        Some(_) => RelPath(note),
                    })
                }
            }
        }
        Dest::Path(path) => {
            let (base, rest) = match path.strip_prefix('/') {
                Some(rest) => ("", rest),
                None => (dir, path),
            };
            let relative = match normalize(base, rest) {
                Some(relative) if !relative.is_root() => relative,
                Some(_) | None => return Resolution::Invalid,
            };
            let exists = |path: &RelPath| index.files.binary_search(path).is_ok();
            let note = match Path::new(relative.as_str()).extension() {
                Some(_) => None,
                None => Some(RelPath(with_md(relative.as_str()))),
            };
            let from_root = match base.is_empty() {
                true => None,
                false => normalize("", rest).filter(|path| !path.is_root()),
            };
            let direct = [Some(&relative), note.as_ref(), from_root.as_ref()]
                .into_iter()
                .flatten()
                .find(|path| exists(path))
                .cloned();
            match direct.or_else(|| lookup(index, dir, relative.name())) {
                Some(found) => Resolution::File(found),
                None => Resolution::Missing(note.unwrap_or(relative)),
            }
        }
    }
}

/// The best file named `wanted` (a name or a path suffix), as a note first.
fn lookup(index: &Index, dir: &str, wanted: &str) -> Option<RelPath> {
    let note = with_md(wanted);
    let names: SmallVec<[&str; 2]> = match note == wanted {
        true => SmallVec::from_slice(&[wanted]),
        false => SmallVec::from_slice(&[note.as_str(), wanted]),
    };
    names.into_iter().find_map(|wanted| {
        let lower = wanted.to_lowercase();
        let name = lower.rsplit('/').next().unwrap_or(&lower);
        let candidates = index.names.get(name)?;
        candidates
            .iter()
            .map(|&at| &index.files[at as usize])
            .filter_map(|path| {
                let folded = path.as_str().to_lowercase();
                match folded.strip_suffix(lower.as_str()) {
                    Some(rest) if rest.is_empty() || rest.ends_with('/') => {
                        let case = match path.as_str().ends_with(wanted) {
                            true => Case::Exact,
                            false => Case::Folded,
                        };
                        Some((case, path))
                    }
                    Some(_) | None => None,
                }
            })
            .min_by_key(|(case, path)| {
                (
                    *case,
                    path.as_str().matches('/').count(),
                    path.parent() != Some(dir),
                    path.as_str().len(),
                    path.as_str(),
                )
            })
            .map(|(_, path)| path.clone())
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Case {
    Exact,
    Folded,
}

fn with_md(name: &str) -> String {
    let start = name.len().saturating_sub(3);
    match name
        .get(start..)
        .is_some_and(|ext| ext.eq_ignore_ascii_case(".md"))
    {
        true => name.to_owned(),
        false => format!("{name}.md"),
    }
}

/// `rest` applied to folder `base`; `None` when `..` climbs out of the vault.
fn normalize(base: &str, rest: &str) -> Option<RelPath> {
    let mut parts: SmallVec<[&str; 8]> = base.split('/').filter(|part| !part.is_empty()).collect();
    for part in rest.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop()?;
            }
            part => parts.push(part),
        }
    }
    Some(RelPath(parts.join("/")))
}

/// Groups links by the file name they name, case-folded and without `.md`: only a file with
/// the same key can be a link's target.
pub fn link_key(name: &str) -> u64 {
    let lower = name.to_lowercase();
    let stem = lower.strip_suffix(".md").unwrap_or(&lower);
    let mut hasher = DefaultHasher::new();
    stem.hash(&mut hasher);
    hasher.finish()
}

/// Heading text compared loosely: case, and spaces against hyphens, do not matter.
pub fn slug(heading: &str) -> String {
    heading
        .trim()
        .chars()
        .map(|c| match c {
            '-' | '_' => ' ',
            c => c.to_ascii_lowercase(),
        })
        .collect()
}

/// Source line of the heading `heading` names (its last `#` part), from the parsed note or a
/// scan of ATX headings outside code fences.
pub fn heading_line(doc: &TextDoc, parsed: Option<&Markdown>, heading: &str) -> Option<usize> {
    let wanted = slug(heading.rsplit('#').next().unwrap_or(heading));
    match parsed {
        Some(parsed) => parsed.blocks.iter().find_map(|block| match &block.kind {
            BlockKind::Heading { text, .. } if slug(&text.text) == wanted => {
                Some(block.lines.start)
            }
            _ => None,
        }),
        None => {
            let mut fenced = Fence::Out;
            doc.text.lines().enumerate().find_map(|(line, text)| {
                let text = std::borrow::Cow::<str>::from(text);
                let trimmed = text.trim_start();
                match (
                    fenced,
                    trimmed.starts_with("```") || trimmed.starts_with("~~~"),
                ) {
                    (Fence::Out, true) => {
                        fenced = Fence::In;
                        return None;
                    }
                    (Fence::In, true) => {
                        fenced = Fence::Out;
                        return None;
                    }
                    (Fence::In, false) => return None,
                    (Fence::Out, false) => {}
                }
                let level = trimmed.bytes().take_while(|&b| b == b'#').count();
                let rest = &trimmed[level..];
                match (level, rest.chars().next()) {
                    (1..=6, None | Some(' ' | '\t')) => {
                        let title = rest.trim().trim_end_matches('#').trim_end();
                        (slug(title) == wanted).then_some(line)
                    }
                    _ => None,
                }
            })
        }
    }
}

#[derive(Clone, Copy)]
enum Fence {
    Out,
    In,
}

/// Where a followed link was clicked or typed: the pane opens the target, rendered for a
/// rendered view.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Origin {
    Text(PaneId),
    Rendered(PaneId),
}

/// Follows `target` of note `note`: URLs open in the browser, headings move within the note,
/// and files open in the origin's pane at their heading. An unresolved note asks to be created.
pub fn follow(
    state: &mut State,
    note: BufferId,
    target: &LinkTarget,
    origin: Origin,
    out: &mut Vec<Effect>,
) {
    let (pane, arrive) = match origin {
        Origin::Text(pane) => (pane, Arrive::Text),
        Origin::Rendered(pane) => (pane, Arrive::Rendered),
    };
    match state.layout.panes.contains_key(pane) {
        true => state.layout.focus = pane,
        false => return,
    }
    let (dest, heading) = match target {
        LinkTarget::Url(url) => return out.push(Effect::OpenUrl(url.clone())),
        LinkTarget::Heading(heading) => {
            return apply::place(state, pane, note, arrive, Target::Heading(heading.clone()));
        }
        LinkTarget::Wiki(WikiLink {
            target, heading, ..
        }) if target.trim().is_empty() => match heading {
            Some(heading) => {
                return apply::place(state, pane, note, arrive, Target::Heading(heading.clone()));
            }
            None => return,
        },
        LinkTarget::Wiki(link) => (Dest::Wiki(link.target.as_str()), link.heading.as_deref()),
        LinkTarget::Path { path, heading } => (Dest::Path(path.as_str()), heading.as_deref()),
    };
    // Notes outside the vault resolve from its root.
    let note = state
        .buffers
        .get(note)
        .and_then(|b| RelPath::new(state.root.path(), &b.path).ok());
    let dir = note.as_ref().and_then(RelPath::parent).unwrap_or("");
    let index = index(&mut state.vault, &state.files);
    match resolve(index, dir, dest) {
        Resolution::Invalid => {
            let (Dest::Wiki(target) | Dest::Path(target)) = dest;
            state.notice = Some(Notice::LinkNotFound {
                target: target.to_owned(),
            })
        }
        Resolution::Missing(path) => state.confirm = Some(Confirm::Create(path)),
        Resolution::File(path) => {
            let full = path.to_path(state.root.path());
            let arrive = match (arrive, Format::of(&full)) {
                (Arrive::Rendered, Some(_)) => Arrive::Rendered,
                (Arrive::Rendered | Arrive::Text | Arrive::Diff, _) => Arrive::Text,
            };
            // Block references (`#^id`) only open the note.
            let to = heading
                .filter(|heading| !heading.is_empty() && !heading.starts_with('^'))
                .map(|heading| Target::Heading(heading.to_owned()));
            apply::open(state, full, arrive, to, OpenAs::Permanent, out);
        }
    }
}

/// The markdown link under byte `at` of `line`: a wikilink or embed, an inline link, an
/// autolink or a bare URL.
pub fn link_at(line: &str, at: usize) -> Option<LinkTarget> {
    let mut from = 0;
    while let Some(open) = line[from..].find("[[").map(|found| from + found) {
        let Some(close) = line[open + 2..].find("]]").map(|found| open + 2 + found) else {
            break;
        };
        let start = match line[..open].ends_with('!') {
            true => open - 1,
            false => open,
        };
        match (start..close + 2).contains(&at) {
            true => {
                let inner = &line[open + 2..close];
                let (dest, alias) = match inner.split_once('|') {
                    Some((dest, alias)) => (dest, Some(alias.trim().to_owned())),
                    None => (inner, None),
                };
                return Some(LinkTarget::Wiki(WikiLink::parse(dest, alias)));
            }
            false => from = close + 2,
        }
    }
    let mut from = 0;
    while let Some(middle) = line[from..].find("](").map(|found| from + found) {
        let dest_start = middle + 2;
        let mut depth = 0usize;
        let end = line[dest_start..]
            .char_indices()
            .find_map(|(i, c)| match c {
                '(' => {
                    depth += 1;
                    None
                }
                ')' if depth == 0 => Some(dest_start + i),
                ')' => {
                    depth -= 1;
                    None
                }
                _ => None,
            });
        let Some(end) = end else {
            break;
        };
        let start = line[..middle]
            .rfind('[')
            .map(|open| match line[..open].ends_with('!') {
                true => open - 1,
                false => open,
            });
        match start.is_some_and(|start| (start..=end).contains(&at)) {
            true => {
                let raw = line[dest_start..end].trim();
                let dest = match raw.strip_prefix('<') {
                    Some(rest) => rest.split('>').next().unwrap_or(""),
                    None => raw.split_whitespace().next().unwrap_or(""),
                };
                return Some(markdown::link_target(
                    pulldown_cmark::LinkType::Inline,
                    dest,
                    None,
                ));
            }
            false => from = end + 1,
        }
    }
    let start = line[..at].rfind(char::is_whitespace).map_or(0, |i| i + 1);
    let end = line[at..]
        .find(char::is_whitespace)
        .map_or(line.len(), |i| at + i);
    let word = line[start..end].trim_matches(|c: char| {
        matches!(
            c,
            '<' | '>' | '(' | ')' | '[' | ']' | '"' | '\'' | ',' | '.' | ';' | '!' | '?'
        )
    });
    match word.contains("://") || word.starts_with("mailto:") {
        true => Some(LinkTarget::Url(word.to_owned())),
        false => None,
    }
}

/// How far around the cursor `gf` looks for a path, as in helix.
const LOOKAROUND: usize = 1000;

/// `gf`: follows the link under the primary cursor of a markdown buffer, else opens the URL
/// or file path under it (or the selected text) relative to the buffer's folder or the root.
pub fn goto_file(state: &mut State, out: &mut Vec<Effect>) {
    let pane = state.layout.focus;
    let (buffer, rope, range) = match focused(&mut state.layout, &mut state.buffers) {
        Some((view, doc)) => (view.buffer(), doc.text.clone(), view.selection.primary()),
        None => {
            state.notice = Some(Notice::NoBuffer);
            return;
        }
    };
    let text = rope.slice(..);
    let cursor = text.char_to_byte(range.cursor(text));
    let line = text.byte_to_line(cursor);
    let line_start = text.line_to_byte(line);
    let path = state.buffers[buffer].path.clone();
    let link = match Format::of(&path) {
        Some(Format::Markdown) => {
            let content = std::borrow::Cow::<str>::from(text.line(line));
            let content = content.trim_end_matches(['\n', '\r']);
            link_at(content, (cursor - line_start).min(content.len()))
        }
        Some(Format::Typst) | None => None,
    };
    match link {
        Some(link) => return follow(state, buffer, &link, Origin::Text(pane), out),
        None => {}
    }
    let picked = match range.len() > 1 {
        true => range.fragment(text).trim().to_owned(),
        false => {
            let start = line_start.max(text.floor_char_boundary(cursor.saturating_sub(LOOKAROUND)));
            let end = text
                .line_to_byte((line + 1).min(text.len_lines()))
                .min(text.ceil_char_boundary(cursor + LOOKAROUND));
            let around = text.byte_slice(start..end);
            helix_stdx::path::find_paths(around, true)
                .take_while(|found| start + found.start <= cursor + 1)
                .find(|found| cursor <= start + found.end)
                .map(|found| String::from(around.byte_slice(found)))
                .unwrap_or_default()
        }
    };
    match (
        picked.is_empty(),
        picked.contains("://") || picked.starts_with("mailto:"),
    ) {
        (true, _) => state.notice = Some(Notice::NothingToFollow),
        (false, true) => out.push(Effect::OpenUrl(picked)),
        (false, false) => {
            let expanded = helix_stdx::path::expand(&picked).into_owned();
            let dir = path.parent().map(Path::to_owned);
            let candidates: [Option<PathBuf>; 2] = match expanded.is_absolute() {
                true => [Some(expanded), None],
                false => [
                    dir.map(|dir| dir.join(&expanded)),
                    Some(state.root.path().join(&expanded)),
                ],
            };
            match candidates.into_iter().flatten().find(|path| path.is_file()) {
                Some(found) => {
                    apply::open(state, found, Arrive::Text, None, OpenAs::Permanent, out)
                }
                None => state.notice = Some(Notice::LinkNotFound { target: picked }),
            }
        }
    }
}
