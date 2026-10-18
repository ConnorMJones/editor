//! Hunk and file staging, reverts and discards.

use std::path::{Path, PathBuf};

use helix_core::{Rope, Tendril, Transaction};
use model::command::{FileOp, HunkOp};
use model::git::Change;
use model::notice::Notice;

use crate::apply::{edit, exit_select_mode};
use crate::git::diff::{Bases, Hunk, Side, intersecting, splice};
use crate::git::{IndexError, IndexOp, locate};
use crate::job::{FsOp, Job};
use crate::project::path::RelPath;
use crate::project::watch;
use crate::sidebar::tree::EntryKind;
use crate::view::layout::Pane;
use crate::view::tab::Tab;
use crate::{Confirm, Effect, Overlay, Repo, State, request_git};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Discard {
    /// Restores the worktree file from the index.
    Checkout,
    /// Deletes the untracked file at this root-relative path.
    Delete(RelPath),
}

/// Queues `op` on the index entry of `path`; `None` with a notice outside the repository.
fn index_job(state: &mut State, path: &Path, op: IndexOp) -> Option<Job> {
    match locate(&state.project.git, path) {
        Some((git_dir, file)) => Some(Job::GitIndex {
            git_dir,
            file,
            path: path.to_owned(),
            op,
        }),
        None => {
            state.notice = Some(Notice::NotTracked {
                path: path.to_owned(),
            });
            None
        }
    }
}

/// Stages, unstages or reverts the hunks touched by the focused view's selections.
pub fn hunk_op(state: &mut State, op: HunkOp, out: &mut Vec<Effect>) {
    let focus = state.layout.focus();
    let Some(view) = (state.layout.panes().get(focus))
        .and_then(Pane::active)
        .map(Tab::text)
    else {
        state.notice = Some(Notice::NoBuffer);
        return;
    };
    let entry = &state.buffers[view.buffer()];
    let doc = &entry.doc;
    let tracked = match &entry.diff.bases {
        Bases::Tracked(tracked) if tracked.is_fresh(doc.version) => tracked,
        Bases::Tracked(_) | Bases::Loading { .. } => {
            state.notice = Some(Notice::DiffPending);
            return;
        }
        Bases::NoRepo | Bases::Untracked => {
            state.notice = Some(Notice::NotTracked {
                path: entry.path.clone(),
            });
            return;
        }
    };
    let lines = view.selection.line_ranges(doc.text.slice(..));
    let picked: Vec<Hunk> = match op {
        HunkOp::Stage => intersecting(&tracked.unstaged, Side::After, lines)
            .cloned()
            .collect(),
        HunkOp::Revert => intersecting(&tracked.hunks, Side::After, lines)
            .cloned()
            .collect(),
        HunkOp::Unstage => {
            // Shown hunks select the HEAD lines whose staged changes are taken back.
            let head_lines: Vec<(usize, usize)> = intersecting(&tracked.hunks, Side::After, lines)
                .map(|hunk| {
                    let start = hunk.before.start as usize;
                    (start, (hunk.before.end as usize).max(start + 1) - 1)
                })
                .collect();
            intersecting(&tracked.staged, Side::Before, head_lines.into_iter())
                .cloned()
                .collect()
        }
    };
    let path = entry.path.clone();
    let write = |onto: &Rope, side: Side| IndexOp::Write {
        expected: tracked.index.oid(),
        text: splice(&tracked.index.text(), onto, picked.iter(), side),
    };
    match (op, picked.as_slice()) {
        (HunkOp::Stage | HunkOp::Unstage | HunkOp::Revert, []) => {
            state.notice = Some(Notice::NoHunks)
        }
        (HunkOp::Stage, _) => {
            let write = write(&doc.text, Side::Before);
            out.extend(index_job(state, &path, write).map(Effect::Job))
        }
        (HunkOp::Unstage, _) => {
            let write = write(&tracked.head.text(), Side::After);
            out.extend(index_job(state, &path, write).map(Effect::Job))
        }
        (HunkOp::Revert, _) => {
            let head = tracked.head.text();
            edit(state, |doc, _, _| {
                let text = &doc.text;
                Transaction::change(
                    text,
                    picked.iter().map(|hunk| {
                        let from = head.line_to_char(hunk.before.start as usize);
                        let to = head.line_to_char(hunk.before.end as usize);
                        let lines: Tendril = head.slice(from..to).chunks().collect();
                        (
                            text.line_to_char(hunk.after.start as usize),
                            text.line_to_char(hunk.after.end as usize),
                            Some(lines),
                        )
                    }),
                )
            });
            exit_select_mode(state);
        }
    }
}

/// Whole-file staging; a discard waits in `Overlay::Confirm` for `Command::Confirm`.
pub fn file_op(state: &mut State, op: FileOp, out: &mut Vec<Effect>) {
    let resolve = |path: &Path| helix_stdx::path::normalize(state.project.root.path().join(path));
    match op {
        FileOp::Stage(path) => {
            let path = resolve(&path);
            out.extend(index_job(state, &path, IndexOp::Stage).map(Effect::Job))
        }
        FileOp::Unstage(path) => {
            let path = resolve(&path);
            out.extend(index_job(state, &path, IndexOp::Unstage).map(Effect::Job))
        }
        FileOp::Discard(path) => {
            let path = resolve(&path);
            let change = match (
                &state.project.git,
                RelPath::new(state.project.root.path(), &path),
            ) {
                (Repo::Found { snapshot, .. }, Ok(file)) => {
                    snapshot.paths.get(&file).map(|change| (file, *change))
                }
                (Repo::Found { .. } | Repo::None | Repo::Unknown, _) => None,
            };
            match change {
                None | Some((_, Change::Ignored)) => {
                    state.notice = Some(Notice::Unchanged { path })
                }
                Some((file, Change::Untracked)) => {
                    state.overlay = Overlay::Confirm(Confirm::Discard {
                        path,
                        how: Discard::Delete(file),
                    })
                }
                Some((
                    _,
                    Change::Conflicted
                    | Change::Staged(_)
                    | Change::Unstaged(_)
                    | Change::Both { .. },
                )) => {
                    state.overlay = Overlay::Confirm(Confirm::Discard {
                        path,
                        how: Discard::Checkout,
                    })
                }
            }
        }
    }
}

/// Runs a confirmed discard.
pub fn discard(state: &mut State, path: PathBuf, how: Discard, out: &mut Vec<Effect>) {
    match how {
        Discard::Checkout => {
            out.extend(index_job(state, &path, IndexOp::Checkout).map(Effect::Job))
        }
        Discard::Delete(file) => out.push(Effect::Job(Job::Fs {
            root: state.project.root.path().to_owned(),
            op: FsOp::Remove {
                path: file,
                kind: EntryKind::File,
            },
        })),
    }
}

/// A finished index change refreshes git; a checkout also reloads the file like a disk change.
pub fn indexed(
    state: &mut State,
    path: PathBuf,
    op: IndexOp,
    result: Result<(), IndexError>,
    out: &mut Vec<Effect>,
) {
    match (result, op) {
        (Ok(()), IndexOp::Checkout) => watch::changed(state, vec![path], out),
        (Ok(()), IndexOp::Write { .. } | IndexOp::Stage | IndexOp::Unstage) => {
            request_git(state, out)
        }
        (Err(IndexError::Changed), _) => {
            state.notice = Some(Notice::IndexChanged { path });
            request_git(state, out);
        }
        (Err(IndexError::Git(error)), _) => {
            tracing::warn!(?path, %error, "index change failed");
            state.notice = Some(Notice::GitFailed {
                path,
                error: error.message().to_owned(),
            });
        }
    }
}
