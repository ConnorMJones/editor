use std::fs::File;
use std::io::{self, BufWriter, Write as _};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::AtomicU64;
use std::time::SystemTime;

use git2::Oid;
use helix_core::regex::bytes::Regex;
use helix_core::syntax::HighlighterError;
use helix_core::syntax::Loader;
use helix_core::{Language, Rope, Syntax};
use model::command::Dirs;

use crate::git::diff::{Base, BaseRead, Lines, Outcome, Stamp};
use crate::git::{self, IndexError, IndexOp, RepoPath};
use crate::input::picker::{self, Hit, Preview, Search};
use crate::project::files::{self, Rescan, Walked};
use crate::project::path::{Folder, RelPath};
use crate::sidebar::tree::{self, Entry, EntryKind};
use crate::text::buffer::{BufferId, Disk, ReadId, TextDoc};
use crate::text::highlight;
use crate::{Effect, State, apply, git::stage, git::track, project::backlinks, render::typeset};

/// Blocking work run off the UI thread.
pub enum Job {
    Open {
        read: ReadId,
        path: PathBuf,
        languages: Arc<Loader>,
    },
    Write {
        buffer: BufferId,
        path: PathBuf,
        text: Rope,
        revision: usize,
        dirs: Dirs,
    },
    Export {
        path: PathBuf,
        bytes: Vec<u8>,
    },

    Index {
        root: PathBuf,
    },
    ListDir {
        root: PathBuf,
        dir: Folder,
    },
    Rescan {
        root: PathBuf,
        paths: Vec<RelPath>,
    },
    GitStatus {
        root: PathBuf,
    },
    Reload {
        buffer: BufferId,
        path: PathBuf,
        when: ReloadIf,
        policy: ReloadPolicy,
    },
    Fs {
        root: PathBuf,
        op: FsOp,
    },
    Grep {
        root: PathBuf,
        files: Arc<Vec<RelPath>>,
        regex: Regex,
        generation: u64,
        current: Arc<AtomicU64>,
    },
    Preview {
        root: PathBuf,
        path: RelPath,
        languages: Arc<Loader>,
    },
    /// Reads the outgoing links of notes for the backlinks index.
    Links {
        root: PathBuf,
        paths: Vec<RelPath>,
        stamp: u64,
    },

    /// Reads a buffer's HEAD and index blobs, skipping sides still at `known`.
    DiffBase {
        buffer: BufferId,
        git_dir: PathBuf,
        path: RepoPath,
        known: [Option<Oid>; 2],
    },
    /// Diffs a snapshot of the buffer against both bases; `lines` comes back in the result.
    Diff {
        buffer: BufferId,
        stamp: Stamp,
        head: Base,
        index: Base,
        text: Rope,
        lines: Box<Lines>,
    },
    GitIndex {
        git_dir: PathBuf,
        file: RepoPath,
        /// The buffer path, for notices and reloads.
        path: PathBuf,
        op: IndexOp,
    },
    Parse {
        buffer: BufferId,
        version: u64,
        text: Rope,
        language: Language,
        languages: Arc<Loader>,
    },
    /// Builds the highlighted HEAD copy a diff view shows on its left.
    ParseHead {
        buffer: BufferId,
        oid: Oid,
        text: Rope,
        /// Picks the language.
        path: PathBuf,
        languages: Arc<Loader>,
    },
}

/// What a reload does to a buffer with unsaved edits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReloadPolicy {
    /// Disk changes noticed by the watcher only flag a modified buffer.
    KeepEdits,
    /// `:reload` replaces the text, as an undoable change.
    Discard,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OnChange {
    Replace,
    Flag,
}

impl ReloadPolicy {
    pub fn on_change(self, doc: &TextDoc) -> OnChange {
        match (self, doc.is_modified()) {
            (ReloadPolicy::KeepEdits, true) => OnChange::Flag,
            (ReloadPolicy::KeepEdits, false) | (ReloadPolicy::Discard, _) => OnChange::Replace,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FsOp {
    Create { path: RelPath, kind: EntryKind },
    Rename { from: RelPath, to: RelPath },
    Remove { path: RelPath, kind: EntryKind },
}

pub enum JobResult {
    Opened {
        read: ReadId,
        result: OpenRead,
    },
    Written {
        buffer: BufferId,
        path: PathBuf,
        revision: usize,
        result: io::Result<Written>,
    },
    Exported {
        path: PathBuf,
        result: io::Result<usize>,
    },
    Typeset(typeset::Done),

    Indexed(Walked),
    Listed {
        dir: Folder,
        result: io::Result<Vec<Entry>>,
    },
    Rescanned(Rescan),
    Git(Result<Option<git::Found>, git2::Error>),
    Reloaded {
        buffer: BufferId,
        read: DiskRead,
        policy: ReloadPolicy,
    },
    Fs {
        op: FsOp,
        result: io::Result<()>,
    },
    Grepped {
        generation: u64,
        hits: Vec<Hit>,
        search: Search,
    },
    Previewed {
        path: RelPath,
        preview: Preview,
    },
    Linked {
        stamp: u64,
        reads: Vec<(RelPath, backlinks::Read)>,
    },

    /// `path` is where the buffer was in the repository when the read started.
    Bases {
        buffer: BufferId,
        path: RepoPath,
        result: Result<[BaseRead; 2], git2::Error>,
    },
    Diffed {
        buffer: BufferId,
        stamp: Stamp,
        lines: Box<Lines>,
        outcome: Outcome,
    },
    GitIndexed {
        path: PathBuf,
        op: IndexOp,
        result: Result<(), IndexError>,
    },
    Parsed {
        buffer: BufferId,
        version: u64,
        result: Result<Syntax, HighlighterError>,
    },
    HeadParsed {
        buffer: BufferId,
        oid: Oid,
        doc: TextDoc,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReloadIf {
    ChangedSince(SystemTime),
    Always,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Written {
    pub bytes: usize,
    pub mtime: SystemTime,
}

pub enum OpenRead {
    Text { doc: TextDoc, disk: Disk },
    NotUtf8,
    NotAFile,
    Failed(io::Error),
}

pub enum DiskRead {
    Unchanged,
    Changed { text: Rope, mtime: SystemTime },
    Missing,
    NotUtf8,
    Failed(io::Error),
}

fn modified(path: &Path) -> io::Result<SystemTime> {
    std::fs::metadata(path)?.modified()
}

pub fn run(job: Job) -> JobResult {
    let kind = match &job {
        Job::Open { .. } => "open",
        Job::Write { .. } => "write",
        Job::Export { .. } => "export",
        Job::Index { .. } => "index",
        Job::ListDir { .. } => "list dir",
        Job::Rescan { .. } => "rescan",
        Job::GitStatus { .. } => "git status",
        Job::Reload { .. } => "reload",
        Job::Fs { .. } => "fs",
        Job::Grep { .. } => "grep",
        Job::Preview { .. } => "preview",
        Job::Links { .. } => "links",
        Job::DiffBase { .. } => "diff base",
        Job::Diff { .. } => "diff",
        Job::GitIndex { .. } => "git index",
        Job::Parse { .. } => "parse",
        Job::ParseHead { .. } => "parse head",
    };
    let _span = tracing::info_span!("job", kind).entered();
    match job {
        Job::Open {
            read,
            path,
            languages,
        } => JobResult::Opened {
            read,
            result: read_file(&path, &languages),
        },
        Job::Write {
            buffer,
            path,
            text,
            revision,
            dirs,
        } => {
            let parent = match (dirs, path.parent()) {
                (Dirs::Create, Some(parent)) => std::fs::create_dir_all(parent),
                (Dirs::Create, None) | (Dirs::Existing, _) => Ok(()),
            };
            let result = parent.and_then(|()| File::create(&path)).and_then(|file| {
                let mut writer = BufWriter::new(file);
                for chunk in text.chunks() {
                    writer.write_all(chunk.as_bytes())?;
                }
                writer.flush()?;
                Ok(Written {
                    bytes: text.len_bytes(),
                    mtime: modified(&path)?,
                })
            });
            JobResult::Written {
                buffer,
                path,
                revision,
                result,
            }
        }
        Job::Export { path, bytes } => JobResult::Exported {
            result: std::fs::write(&path, &bytes).map(|()| bytes.len()),
            path,
        },
        Job::Index { root } => JobResult::Indexed(files::walk(&root, &Folder::Root, None)),
        Job::ListDir { root, dir } => {
            let result = tree::list(&root, &dir);
            JobResult::Listed { dir, result }
        }
        Job::Rescan { root, paths } => JobResult::Rescanned(files::rescan(&root, &paths)),
        Job::GitStatus { root } => JobResult::Git(git::status(&root)),
        Job::Reload {
            buffer,
            path,
            when,
            policy,
        } => {
            let read = match modified(&path) {
                Err(error) if error.kind() == io::ErrorKind::NotFound => DiskRead::Missing,
                Err(error) => DiskRead::Failed(error),
                Ok(now) => match when {
                    ReloadIf::ChangedSince(known) if now == known => DiskRead::Unchanged,
                    ReloadIf::ChangedSince(_) | ReloadIf::Always => match std::fs::read(&path)
                        .map(String::from_utf8)
                    {
                        Ok(Ok(text)) => DiskRead::Changed {
                            text: Rope::from(text),
                            mtime: now,
                        },
                        Ok(Err(_)) => DiskRead::NotUtf8,
                        Err(error) if error.kind() == io::ErrorKind::NotFound => DiskRead::Missing,
                        Err(error) => DiskRead::Failed(error),
                    },
                },
            };
            JobResult::Reloaded {
                buffer,
                read,
                policy,
            }
        }
        Job::Fs { root, op } => {
            let result = match &op {
                FsOp::Create {
                    path,
                    kind: EntryKind::File,
                } => {
                    let path = path.to_path(&root);
                    path.parent()
                        .map_or(Ok(()), std::fs::create_dir_all)
                        .and_then(|()| File::create_new(&path).map(|_| ()))
                }
                FsOp::Create {
                    path,
                    kind: EntryKind::Dir,
                } => std::fs::create_dir_all(path.to_path(&root)),
                FsOp::Rename { from, to } => {
                    let (from, to) = (from.to_path(&root), to.to_path(&root));
                    match to.try_exists() {
                        Ok(true) => Err(io::Error::from(io::ErrorKind::AlreadyExists)),
                        Ok(false) => to
                            .parent()
                            .map_or(Ok(()), std::fs::create_dir_all)
                            .and_then(|()| std::fs::rename(&from, &to)),
                        Err(error) => Err(error),
                    }
                }
                FsOp::Remove {
                    path,
                    kind: EntryKind::File,
                } => std::fs::remove_file(path.to_path(&root)),
                FsOp::Remove {
                    path,
                    kind: EntryKind::Dir,
                } => std::fs::remove_dir_all(path.to_path(&root)),
            };
            JobResult::Fs { op, result }
        }
        Job::Grep {
            root,
            files,
            regex,
            generation,
            current,
        } => {
            let (hits, search) = picker::grep(&root, &files, &regex, generation, &current);
            JobResult::Grepped {
                generation,
                hits,
                search,
            }
        }
        Job::Preview {
            root,
            path,
            languages,
        } => {
            let full = path.to_path(&root);
            let bytes = std::fs::metadata(&full).and_then(|meta| {
                match meta.len() > picker::MAX_PREVIEW_BYTES {
                    true => Ok(None),
                    false => std::fs::read(&full).map(Some),
                }
            });
            // Binary files are detected as ripgrep does, by a NUL near the start.
            let preview = match bytes {
                Err(error) => Preview::Failed(error),
                Ok(None) => Preview::TooLarge,
                Ok(Some(bytes)) => match bytes[..bytes.len().min(8192)].contains(&0) {
                    true => Preview::Binary,
                    false => match String::from_utf8(bytes) {
                        Ok(text) => {
                            Preview::Text(TextDoc::new(Rope::from(text), &full, &languages))
                        }
                        Err(_) => Preview::Binary,
                    },
                },
            };
            JobResult::Previewed { path, preview }
        }
        Job::Links { root, paths, stamp } => JobResult::Linked {
            stamp,
            reads: backlinks::read(&root, paths),
        },
        Job::DiffBase {
            buffer,
            git_dir,
            path,
            known,
        } => JobResult::Bases {
            buffer,
            result: git::read_bases(&git_dir, &path, known),
            path,
        },
        Job::Diff {
            buffer,
            stamp,
            head,
            index,
            text,
            mut lines,
        } => {
            let outcome = lines.compute(stamp, &head, &index, text);
            JobResult::Diffed {
                buffer,
                stamp,
                lines,
                outcome,
            }
        }
        Job::GitIndex {
            git_dir,
            file,
            path,
            op,
        } => JobResult::GitIndexed {
            result: git::change_index(&git_dir, &file, &op),
            path,
            op,
        },
        Job::Parse {
            buffer,
            version,
            text,
            language,
            languages,
        } => JobResult::Parsed {
            buffer,
            version,
            result: tracing::info_span!("syntax parse")
                .in_scope(|| Syntax::new(text.slice(..), language, &languages)),
        },
        Job::ParseHead {
            buffer,
            oid,
            text,
            path,
            languages,
        } => JobResult::HeadParsed {
            buffer,
            oid,
            doc: TextDoc::new(text, &path, &languages),
        },
    }
}

pub fn complete(state: &mut State, result: JobResult, out: &mut Vec<Effect>) {
    let kind = match &result {
        JobResult::Opened { .. } => "opened",
        JobResult::Written { .. } => "written",
        JobResult::Exported { .. } => "exported",
        JobResult::Typeset(_) => "typeset",
        JobResult::Indexed(_) => "indexed",
        JobResult::Listed { .. } => "listed",
        JobResult::Rescanned(_) => "rescanned",
        JobResult::Git(_) => "git",
        JobResult::Reloaded { .. } => "reloaded",
        JobResult::Fs { .. } => "fs",
        JobResult::Grepped { .. } => "grepped",
        JobResult::Previewed { .. } => "previewed",
        JobResult::Linked { .. } => "linked",
        JobResult::Bases { .. } => "bases",
        JobResult::Diffed { .. } => "diffed",
        JobResult::GitIndexed { .. } => "git indexed",
        JobResult::Parsed { .. } => "parsed",
        JobResult::HeadParsed { .. } => "head parsed",
    };
    let _span = tracing::info_span!("complete", kind).entered();
    match result {
        JobResult::Opened { read, result } => apply::opened(state, read, result, out),
        JobResult::Written {
            buffer,
            path,
            revision,
            result,
        } => apply::written(state, buffer, path, revision, result, out),
        JobResult::Exported { path, result } => typeset::exported(state, path, result),
        JobResult::Typeset(done) => typeset::finish(state, done, out),
        JobResult::Indexed(walked) => files::indexed(state, walked, out),
        JobResult::Listed { dir, result } => tree::listed(state, dir, result, out),
        JobResult::Rescanned(rescan) => files::rescanned(state, rescan, out),
        JobResult::Git(result) => crate::git_refreshed(state, result, out),
        JobResult::Reloaded {
            buffer,
            read,
            policy,
        } => apply::reloaded(state, buffer, read, policy),
        JobResult::Fs { op, result } => tree::fs_done(state, op, result, out),
        JobResult::Grepped {
            generation,
            hits,
            search,
        } => picker::grepped(state, generation, hits, search, out),
        JobResult::Previewed { path, preview } => picker::previewed(state, path, preview),
        JobResult::Linked { stamp, reads } => backlinks::linked(state, stamp, reads, out),
        JobResult::Bases {
            buffer,
            path,
            result,
        } => track::bases_read(state, buffer, path, result, out),
        JobResult::Diffed {
            buffer,
            stamp,
            lines,
            outcome,
        } => track::diffed(state, buffer, stamp, lines, outcome),
        JobResult::GitIndexed { path, op, result } => stage::indexed(state, path, op, result, out),
        JobResult::Parsed {
            buffer,
            version,
            result,
        } => highlight::parsed(state, buffer, version, result),
        JobResult::HeadParsed { buffer, oid, doc } => {
            crate::git::diffview::head_parsed(state, buffer, oid, doc)
        }
    }
}

/// Reads `path` into a document; a missing file opens empty, to be created on write.
pub fn read_file(path: &Path, languages: &Loader) -> OpenRead {
    let _span = tracing::info_span!("read", ?path).entered();
    match std::fs::read(path) {
        Ok(bytes) => match (String::from_utf8(bytes), modified(path)) {
            (Ok(text), Ok(mtime)) => OpenRead::Text {
                doc: TextDoc::unparsed(Rope::from(text), path, languages),
                disk: Disk::Synced { mtime },
            },
            (Err(_), _) => OpenRead::NotUtf8,
            (Ok(_), Err(error)) => OpenRead::Failed(error),
        },
        Err(error) if error.kind() == io::ErrorKind::NotFound => OpenRead::Text {
            doc: TextDoc::unparsed(Rope::new(), path, languages),
            disk: Disk::New,
        },
        Err(error) if error.kind() == io::ErrorKind::IsADirectory => OpenRead::NotAFile,
        Err(error) => OpenRead::Failed(error),
    }
}
