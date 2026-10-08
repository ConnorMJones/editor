use std::fs::File;
use std::io::{self, BufWriter, Write as _};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicU64;
use std::time::SystemTime;

use helix_core::regex::bytes::Regex;
use helix_core::{Rope, Selection};
use model::command::{Command, Dirs};
use model::notice::Notice;

use crate::apply::map_views;
use crate::buffer::{BufferId, Content, Disk};
use crate::files::{self, Files, Rescan, Walked};
use crate::git::{self, GitSnapshot};
use crate::layout::View;
use crate::path::RelPath;
use crate::picker::{self, Hit, Items, Search};
use crate::tree::{self, Dir, Entry, EntryKind, Fold, Listing};
use crate::{Effect, Exit, Focus, Overlay, Refresh, Repo, Sidebar, State, request_git, watch};

/// Blocking work run off the UI thread.
pub enum Job {
    Write {
        buffer: BufferId,
        path: PathBuf,
        text: Rope,
        revision: usize,
        dirs: Dirs,
    },

    // Workspace: file index, tree, git, disk sync, file operations, search.
    Index {
        root: PathBuf,
    },
    ListDir {
        root: PathBuf,
        dir: RelPath,
    },
    Rescan {
        root: PathBuf,
        paths: Vec<RelPath>,
    },
    GitStatus {
        root: PathBuf,
    },
    /// Re-reads a buffer's file unless its mtime still matches.
    Reload {
        buffer: BufferId,
        path: PathBuf,
        mtime: Option<SystemTime>,
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
}

/// What a reload does to a buffer with unsaved edits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReloadPolicy {
    /// Disk changes noticed by the watcher only flag a modified buffer.
    KeepEdits,
    /// `:reload` replaces the text, as an undoable change.
    Discard,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FsOp {
    Create { path: RelPath, kind: EntryKind },
    Rename { from: RelPath, to: RelPath },
    Remove { path: RelPath, kind: EntryKind },
}

pub enum JobResult {
    Written {
        buffer: BufferId,
        path: PathBuf,
        revision: usize,
        result: io::Result<usize>,
        mtime: Option<SystemTime>,
    },

    Indexed(Walked),
    Listed {
        dir: RelPath,
        result: io::Result<Vec<Entry>>,
    },
    Rescanned(Rescan),
    Git(Result<Option<(PathBuf, GitSnapshot)>, git2::Error>),
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
}

pub enum DiskRead {
    Unchanged,
    Changed {
        text: Rope,
        mtime: Option<SystemTime>,
    },
    Missing,
    NotUtf8,
    Failed(io::Error),
}

pub fn run(job: Job) -> JobResult {
    match job {
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
                Ok(text.len_bytes())
            });
            let mtime = std::fs::metadata(&path).and_then(|m| m.modified()).ok();
            JobResult::Written {
                buffer,
                path,
                revision,
                result,
                mtime,
            }
        }
        Job::Index { root } => {
            let mut walked = files::walk(&root, &RelPath::ROOT, None);
            walked.dirs.push(RelPath::ROOT);
            JobResult::Indexed(walked)
        }
        Job::ListDir { root, dir } => {
            let result = tree::list(&root, &dir);
            JobResult::Listed { dir, result }
        }
        Job::Rescan { root, paths } => JobResult::Rescanned(files::rescan(&root, &paths)),
        Job::GitStatus { root } => JobResult::Git(git::status(&root)),
        Job::Reload {
            buffer,
            path,
            mtime,
            policy,
        } => {
            let read = match std::fs::metadata(&path) {
                Err(error) if error.kind() == io::ErrorKind::NotFound => DiskRead::Missing,
                Err(error) => DiskRead::Failed(error),
                Ok(meta) => match (meta.modified().ok(), mtime) {
                    (Some(now), Some(known)) if now == known => DiskRead::Unchanged,
                    (now, _) => match std::fs::read(&path).map(String::from_utf8) {
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
    }
}

pub fn complete(state: &mut State, result: JobResult, out: &mut Vec<Effect>) {
    match result {
        JobResult::Written {
            buffer,
            path,
            revision,
            result: Ok(bytes),
            mtime,
        } => {
            if let Some(entry) = state.buffers.get_mut(buffer) {
                match &mut entry.content {
                    Content::Text(doc) => doc.saved_revision = revision,
                }
                entry.disk = Disk::Synced;
                entry.mtime = mtime;
            }
            state.notice = Some(Notice::Written { path, bytes });
            request_git(state, out);
            if let Exit::AfterWrite { buffers } = &mut state.exit {
                buffers.retain(|waiting| *waiting != buffer);
                if buffers.is_empty() {
                    state.exit = Exit::Running;
                    crate::apply::apply(state, Command::Quit, out);
                }
            }
        }
        JobResult::Written {
            buffer,
            path,
            result: Err(error),
            ..
        } => {
            tracing::warn!(?path, %error, "write failed");
            state.notice = Some(Notice::WriteFailed { path, error });
            match &state.exit {
                Exit::AfterWrite { buffers } if buffers.contains(&buffer) => {
                    state.exit = Exit::Running;
                }
                Exit::AfterWrite { .. } | Exit::Running => {}
            }
        }

        JobResult::Indexed(walked) => {
            let root = state.root.path();
            out.push(Effect::Watch(
                walked.dirs.iter().map(|dir| dir.to_path(root)).collect(),
            ));
            let files = Arc::new(walked.files);
            match &mut state.overlay {
                Overlay::Picker(picker) => match &mut picker.items {
                    Items::Files(items) => {
                        *items = files.clone();
                        picker::filter(picker, &mut state.matcher, picker::Narrowing::All);
                    }
                    Items::Buffers(_) | Items::Commands | Items::Grep(_) => {}
                },
                Overlay::None | Overlay::Prompt(_) => {}
            }
            state.files = Files::Ready(files);
        }
        JobResult::Listed { dir, result } => {
            let entries = match result {
                Ok(entries) => entries,
                Err(error) => {
                    tracing::warn!(%error, %dir, "cannot list directory");
                    state.tree.dirs.remove(&dir);
                    tree::rebuild(&mut state.tree);
                    return;
                }
            };
            out.push(Effect::Watch(vec![dir.to_path(state.root.path())]));
            // Forget expansion state of subdirectories that disappeared.
            state.tree.dirs.retain(|path, _| match path.parent() {
                Some(parent) if parent == dir.as_str() => entries
                    .iter()
                    .any(|e| e.kind == EntryKind::Dir && e.name == path.name()),
                Some(_) | None => true,
            });
            match state.tree.dirs.get_mut(&dir) {
                Some(existing) => existing.listing = Listing::Loaded(entries),
                None => {
                    state.tree.dirs.insert(
                        dir,
                        Dir {
                            fold: Fold::Collapsed,
                            listing: Listing::Loaded(entries),
                        },
                    );
                }
            }
            tree::rebuild(&mut state.tree);
        }
        JobResult::Rescanned(rescan) => {
            let root = state.root.path();
            let new_dirs: Vec<PathBuf> = rescan
                .subtrees
                .iter()
                .flat_map(|(dir, walked)| std::iter::once(dir).chain(&walked.dirs))
                .map(|dir| dir.to_path(root))
                .collect();
            match new_dirs.is_empty() {
                true => {}
                false => out.push(Effect::Watch(new_dirs)),
            }
            match &mut state.files {
                Files::Ready(files) => files::apply_rescan(Arc::make_mut(files), rescan),
                Files::Loading => {}
            }
        }
        JobResult::Git(result) => {
            match result {
                Ok(Some((git_dir, snapshot))) => {
                    match &state.git.repo {
                        Repo::Found { git_dir: known, .. } if *known == git_dir => {}
                        Repo::Found { .. } | Repo::None | Repo::Unknown => {
                            out.push(Effect::Watch(vec![git_dir.clone()]))
                        }
                    }
                    state.git.repo = Repo::Found { git_dir, snapshot };
                }
                Ok(None) => state.git.repo = Repo::None,
                Err(error) => tracing::warn!(%error, "git status failed"),
            }
            match state.git.refresh {
                Refresh::Queued => {
                    state.git.refresh = Refresh::Idle;
                    request_git(state, out);
                }
                Refresh::Running | Refresh::Idle => state.git.refresh = Refresh::Idle,
            }
        }
        JobResult::Reloaded {
            buffer,
            read,
            policy,
        } => {
            let State {
                buffers,
                layout,
                languages,
                notice,
                ..
            } = state;
            let Some(entry) = buffers.get_mut(buffer) else {
                return;
            };
            let Content::Text(doc) = &mut entry.content;
            let keep = match (doc.is_modified(), policy) {
                (true, ReloadPolicy::KeepEdits) => true,
                (false, _) | (true, ReloadPolicy::Discard) => false,
            };
            match (read, keep) {
                (DiskRead::Unchanged, _) => {}
                (DiskRead::Missing, _) => {
                    entry.disk = Disk::Deleted;
                    *notice = Some(Notice::DeletedOnDisk {
                        path: entry.path.clone(),
                    });
                }
                (DiskRead::NotUtf8, _) => {
                    *notice = Some(Notice::NotUtf8 {
                        path: entry.path.clone(),
                    })
                }
                (DiskRead::Failed(error), _) => {
                    tracing::warn!(%error, path = ?entry.path, "reload failed");
                    *notice = Some(Notice::ReloadFailed {
                        path: entry.path.clone(),
                        error,
                    });
                }
                (DiskRead::Changed { .. }, true) => {
                    entry.disk = Disk::ChangedExternally;
                    *notice = Some(Notice::ChangedOnDisk {
                        path: entry.path.clone(),
                    });
                }
                (DiskRead::Changed { text, mtime }, false) => {
                    let tx = helix_core::diff::compare_ropes(&doc.text, &text);
                    let selection = layout
                        .panes
                        .values()
                        .find_map(|pane| match &pane.view {
                            View::Text(view) if view.buffer == buffer => {
                                Some(view.selection.clone())
                            }
                            View::Text(_) | View::Empty => None,
                        })
                        .unwrap_or_else(|| Selection::point(0));
                    // Unsaved edits stay their own undo step below the reload.
                    doc.commit(&selection);
                    doc.apply(&tx, &selection, languages);
                    map_views(layout, buffer, tx.changes(), doc.text.slice(..));
                    doc.commit(&selection.map(tx.changes()));
                    doc.saved_revision = doc.history.current_revision();
                    entry.disk = Disk::Synced;
                    entry.mtime = mtime;
                }
            }
        }
        JobResult::Fs {
            op,
            result: Err(error),
        } => {
            let path = match &op {
                FsOp::Create { path, .. } | FsOp::Remove { path, .. } => path,
                FsOp::Rename { from, .. } => from,
            };
            state.notice = Some(Notice::FileOpFailed {
                path: path.to_path(state.root.path()),
                error,
            });
        }
        JobResult::Fs { op, result: Ok(()) } => {
            let root = state.root.path().to_owned();
            let changed = match op {
                FsOp::Create { path, kind } => {
                    let changed = vec![path.to_path(&root)];
                    match kind {
                        EntryKind::File => {
                            crate::apply::apply(state, Command::Open(path.to_path(&root)), out);
                            match state.sidebar {
                                Sidebar::Files(_) => state.sidebar = Sidebar::Files(Focus::Editor),
                                Sidebar::Hidden => {}
                            }
                        }
                        EntryKind::Dir => {}
                    }
                    changed
                }
                FsOp::Rename { from, to } => {
                    for buffer in state.buffers.values_mut() {
                        let Ok(rel) = RelPath::new(&root, &buffer.path) else {
                            continue;
                        };
                        match rel.is_within(from.as_str()) {
                            true => buffer.path = rel.rebase(&from, &to).to_path(&root),
                            false => {}
                        }
                    }
                    let moved: Vec<RelPath> = state
                        .tree
                        .dirs
                        .keys()
                        .filter(|dir| dir.is_within(from.as_str()))
                        .cloned()
                        .collect();
                    for dir in moved {
                        if let Some(entry) = state.tree.dirs.remove(&dir) {
                            state.tree.dirs.insert(dir.rebase(&from, &to), entry);
                        }
                    }
                    vec![from.to_path(&root), to.to_path(&root)]
                }
                FsOp::Remove { path, .. } => {
                    state
                        .tree
                        .dirs
                        .retain(|dir, _| !dir.is_within(path.as_str()));
                    vec![path.to_path(&root)]
                }
            };
            tree::rebuild(&mut state.tree);
            watch::changed(state, changed, out);
        }
        JobResult::Grepped {
            generation,
            hits,
            search,
        } => match &mut state.overlay {
            Overlay::Picker(picker) => match &mut picker.items {
                Items::Grep(grep) if grep.generation == generation => {
                    grep.hits = hits;
                    grep.search = search;
                    picker.matches.clear();
                    picker.matches.extend(0..grep.hits.len() as u32);
                    picker.cursor = 0;
                }
                Items::Grep(_) | Items::Files(_) | Items::Buffers(_) | Items::Commands => {}
            },
            Overlay::None | Overlay::Prompt(_) => {}
        },
    }
}
