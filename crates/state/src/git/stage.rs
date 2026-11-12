//! Hunk and file staging, reverts and discards.

use std::path::{Path, PathBuf};

use helix_core::{Rope, Tendril, Transaction};
use model::command::{FileOp, HunkOp};
use model::git::Change;
use model::notice::Notice;

use crate::apply::{edit, exit_select_mode};
use crate::git::diff::{Bases, Hunk, Side, intersecting, splice};
use crate::git::{Files, InRepo, IndexError, IndexOp, locate};
use crate::job::{FsOp, Job};
use crate::project::path::RelPath;
use crate::project::watch;
use crate::sidebar::tree::EntryKind;
use crate::view::layout::Pane;
use crate::view::layout::TabPane;
use crate::view::tab::TextTabs;
use crate::{Confirm, Effect, Overlay, Repo, State, request_git};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Discard {
    /// Restores the worktree file from the index.
    Checkout,
    /// Deletes the untracked file at this root-relative path.
    Delete(RelPath),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Staging {
    Stage,
    Unstage,
}

fn in_repo(state: &mut State, path: PathBuf) -> Option<(PathBuf, InRepo)> {
    match locate(&state.project.git, &path) {
        Some((git_dir, file)) => Some((git_dir, InRepo { file, path })),
        None => {
            state.notice = Some(Notice::NotTracked { path });
            None
        }
    }
}

pub fn stage_files(
    state: &mut State,
    paths: impl IntoIterator<Item = PathBuf>,
    staging: Staging,
    out: &mut Vec<Effect>,
) {
    let mut found = paths.into_iter().filter_map(|path| in_repo(state, path));
    let Some((git_dir, first)) = found.next() else {
        return;
    };
    let files = Files {
        first,
        rest: found.map(|(_, at)| at).collect(),
    };
    let op = match staging {
        Staging::Stage => IndexOp::Stage(files),
        Staging::Unstage => IndexOp::Unstage(files),
    };
    out.push(Effect::Job(Job::GitIndex { git_dir, op }))
}

/// Stages, unstages or reverts the hunks touched by the focused view's selections.
pub fn hunk_op(state: &mut State, op: HunkOp, out: &mut Vec<Effect>) {
    let focus = state.layout.focus();
    let Some(view) = (state.layout.panes().get(focus))
        .and_then(Pane::active_text)
        .map(TextTabs::text)
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
    let expected = tracked.index.oid();
    let write = |onto: &Rope, side: Side| splice(&tracked.index.text(), onto, picked.iter(), side);
    let write_job = |state: &mut State, text: Rope| {
        in_repo(state, path).map(|(git_dir, at)| {
            Effect::Job(Job::GitIndex {
                git_dir,
                op: IndexOp::Write { at, expected, text },
            })
        })
    };
    match (op, picked.as_slice()) {
        (HunkOp::Stage | HunkOp::Unstage | HunkOp::Revert, []) => {
            state.notice = Some(Notice::NoHunks)
        }
        (HunkOp::Stage, _) => {
            let text = write(&doc.text, Side::Before);
            out.extend(write_job(state, text))
        }
        (HunkOp::Unstage, _) => {
            let text = write(&tracked.head.text(), Side::After);
            out.extend(write_job(state, text))
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
        FileOp::Stage(path) => stage_files(state, [resolve(&path)], Staging::Stage, out),
        FileOp::Unstage(path) => stage_files(state, [resolve(&path)], Staging::Unstage, out),
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
        Discard::Checkout => out.extend(in_repo(state, path).map(|(git_dir, at)| {
            Effect::Job(Job::GitIndex {
                git_dir,
                op: IndexOp::Checkout(at),
            })
        })),
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
    op: IndexOp,
    result: Result<(), IndexError>,
    out: &mut Vec<Effect>,
) {
    let path = op.path().to_owned();
    match (result, op) {
        (Ok(()), IndexOp::Checkout(at)) => watch::changed(state, vec![at.path], out),
        (Ok(()), IndexOp::Write { .. } | IndexOp::Stage(_) | IndexOp::Unstage(_)) => {
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
