//! Per-buffer diff jobs, hunk and file staging, and `]g` navigation across changed files.

use std::path::{Path, PathBuf};

use helix_core::{Range, RopeSlice, Selection, Tendril, Transaction};
use model::command::{Answer, FileOp, HunkOp, HunkTarget};
use model::config::OpenAs;
use model::git::Change;
use model::mode::Mode;
use model::notice::Notice;

use crate::apply::{count_of, edit, exit_select_mode};
use crate::buffer::{BufferId, Content, Target};
use crate::diff::{
    Base, BaseRead, Bases, Compute, Edge, Hunk, Landing, Lines, Outcome, Side, Stamp, Tracked,
    intersecting, next_hunk, prev_hunk, splice,
};
use crate::git::{IndexError, IndexOp, RepoPath};
use crate::job::{FsOp, Job};
use crate::layout::{Pane, View};
use crate::path::RelPath;
use crate::tree::EntryKind;
use crate::{Effect, Refresh, Repo, State, request_git, scroll, watch};

/// A destructive git operation waiting for `Command::Confirm`; a sidebar shows it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Confirm {
    Discard { path: PathBuf, how: Discard },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Discard {
    /// Restores the worktree file from the index.
    Checkout,
    /// Deletes the untracked file at this root-relative path.
    Delete(RelPath),
}

/// What `]g` shows of the next changed file; the diff view adds its own variant.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Arrive {
    Text,
}

/// Git dir and repository path of `path`; `None` outside the repository's worktree.
fn locate(repo: &Repo, path: &Path) -> Option<(PathBuf, RepoPath)> {
    match repo {
        Repo::Found {
            git_dir, workdir, ..
        } => RelPath::new(workdir, path)
            .ok()
            .map(|file| (git_dir.clone(), RepoPath(file))),
        Repo::Unknown | Repo::None => None,
    }
}

/// Starts a base read for `buffer`, or queues one behind the running read.
pub fn request_bases(state: &mut State, buffer: BufferId, out: &mut Vec<Effect>) {
    let Some(entry) = state.buffers.get_mut(buffer) else {
        return;
    };
    let Content::Text { diff, .. } = &mut entry.content else {
        return;
    };
    match (&state.git.repo, locate(&state.git.repo, &entry.path)) {
        // The first status refresh requests every buffer's bases.
        (Repo::Unknown, _) => {}
        (Repo::None | Repo::Found { .. }, None) => {
            diff.bases = Bases::NoRepo;
            diff.landing = None;
        }
        (Repo::None | Repo::Found { .. }, Some((git_dir, path))) => {
            diff.read = match diff.read {
                Refresh::Idle => {
                    let known = match &diff.bases {
                        Bases::Tracked(tracked) => [tracked.head.oid(), tracked.index.oid()],
                        Bases::NoRepo | Bases::Loading | Bases::Untracked => [None, None],
                    };
                    out.push(Effect::Job(Job::DiffBase {
                        buffer,
                        git_dir,
                        path,
                        known,
                    }));
                    Refresh::Running
                }
                Refresh::Running | Refresh::Queued => Refresh::Queued,
            };
        }
    }
}

/// Re-reads the bases of every text buffer, as after a status refresh.
pub fn request_all_bases(state: &mut State, out: &mut Vec<Effect>) {
    let ids: Vec<BufferId> = state.buffers.keys().collect();
    for buffer in ids {
        request_bases(state, buffer, out);
    }
}

pub fn bases_read(
    state: &mut State,
    buffer: BufferId,
    result: Result<[BaseRead; 2], git2::Error>,
    out: &mut Vec<Effect>,
) {
    let Some(Content::Text { diff, .. }) = state.buffers.get_mut(buffer).map(|b| &mut b.content)
    else {
        return;
    };
    let again = std::mem::replace(&mut diff.read, Refresh::Idle);
    match result {
        Err(error) => tracing::warn!(%error, "reading diff bases failed"),
        Ok([head, index]) => {
            // `Kept` means the id the request knew: the tracked base, or absent.
            let tracked = match std::mem::replace(&mut diff.bases, Bases::Loading) {
                Bases::Tracked(tracked) => Some(tracked),
                Bases::NoRepo | Bases::Loading | Bases::Untracked => None,
            };
            let pick = |read: BaseRead, kept: Option<Base>| match read {
                BaseRead::Fresh(base) => base,
                BaseRead::Kept => kept.unwrap_or(Base::Absent),
            };
            diff.bases = match tracked {
                Some(mut tracked) => {
                    tracked.head = pick(head, Some(tracked.head));
                    tracked.index = pick(index, Some(tracked.index));
                    match (&tracked.head, &tracked.index) {
                        (Base::Absent, Base::Absent) => Bases::Untracked,
                        (Base::Blob { .. }, _) | (_, Base::Blob { .. }) => Bases::Tracked(tracked),
                    }
                }
                None => match (pick(head, None), pick(index, None)) {
                    (Base::Absent, Base::Absent) => Bases::Untracked,
                    (head, index) => Bases::Tracked(Tracked::new(head, index)),
                },
            };
        }
    }
    match diff.bases {
        Bases::NoRepo | Bases::Untracked => diff.landing = None,
        Bases::Loading | Bases::Tracked(_) => {}
    }
    match again {
        Refresh::Queued => request_bases(state, buffer, out),
        Refresh::Running | Refresh::Idle => {}
    }
}

/// Starts a diff for every tracked buffer whose hunks no longer match its text or bases.
pub fn sync(state: &mut State, out: &mut Vec<Effect>) {
    for (buffer, entry) in state.buffers.iter_mut() {
        let (doc, tracked) = match &mut entry.content {
            Content::Text {
                doc,
                diff:
                    crate::diff::Diff {
                        bases: Bases::Tracked(tracked),
                        ..
                    },
                ..
            } => (doc, tracked),
            Content::Text { .. } | Content::Loading { .. } => continue,
        };
        let stamp = tracked.current(doc.version);
        match tracked.stamp == Some(stamp) {
            true => continue,
            false => {}
        }
        match std::mem::replace(&mut tracked.compute, Compute::Running) {
            Compute::Idle(lines) => out.push(Effect::Job(Job::Diff {
                buffer,
                stamp,
                head: tracked.head.clone(),
                index: tracked.index.clone(),
                text: doc.text.clone(),
                lines,
            })),
            Compute::Running => {}
        }
    }
}

/// Takes a finished diff; a result for older text or bases is dropped and `sync` re-queues.
pub fn diffed(
    state: &mut State,
    buffer: BufferId,
    stamp: Stamp,
    mut lines: Box<Lines>,
    outcome: Outcome,
) {
    let Some(Content::Text { doc, diff, .. }) =
        state.buffers.get_mut(buffer).map(|b| &mut b.content)
    else {
        return;
    };
    let Bases::Tracked(tracked) = &mut diff.bases else {
        return;
    };
    let current = tracked.current(doc.version);
    // The line cache now holds these bases, so their staged hunks are not computed again.
    match (outcome, stamp.bases == current.bases) {
        (
            Outcome::Diffed {
                staged: Some(staged),
            },
            true,
        ) => tracked.staged = staged,
        (Outcome::TooLarge, true) => tracked.staged.clear(),
        (Outcome::Diffed { .. } | Outcome::TooLarge, _) => {}
    }
    let fresh = stamp == current;
    match fresh {
        true => {
            std::mem::swap(&mut tracked.hunks, &mut lines.hunks);
            std::mem::swap(&mut tracked.unstaged, &mut lines.unstaged);
            tracked.stamp = Some(stamp);
        }
        false => {}
    }
    tracked.compute = Compute::Idle(lines);
    match fresh {
        true => land(state, buffer),
        false => {}
    }
}

/// Selection covering a hunk's buffer lines; a removal is the character after it.
fn hunk_range(hunk: &Hunk, text: RopeSlice) -> Range {
    let anchor = text.line_to_char(hunk.after.start as usize);
    let head = match hunk.after.is_empty() {
        true => (anchor + 1).min(text.len_chars()),
        false => text.line_to_char(hunk.after.end as usize),
    };
    Range::new(anchor, head)
}

/// Moves a pending `]g` landing onto the buffer's first or last hunk once its diff is fresh.
pub fn land(state: &mut State, buffer: BufferId) {
    let State {
        buffers, layout, ..
    } = state;
    let Some(Content::Text { doc, diff, .. }) = buffers.get_mut(buffer).map(|b| &mut b.content)
    else {
        return;
    };
    let (landing, tracked) = match (diff.landing, &diff.bases) {
        (Some(landing), Bases::Tracked(tracked)) if tracked.is_fresh(doc.version) => {
            (landing, tracked)
        }
        (Some(_), Bases::NoRepo | Bases::Untracked) => {
            diff.landing = None;
            return;
        }
        (Some(_), Bases::Loading | Bases::Tracked(_)) | (None, _) => return,
    };
    diff.landing = None;
    let hunk = match landing.edge {
        Edge::First => tracked.hunks.first(),
        Edge::Last => tracked.hunks.last(),
    };
    let view = (layout.panes.get_mut(landing.pane)).and_then(|pane| {
        let index = pane.tabs.as_ref()?.find(buffer)?;
        Some(&mut pane.items_mut()[index].view)
    });
    match (hunk, view) {
        (Some(hunk), Some(View::Text(view))) => {
            let range = hunk_range(hunk, doc.text.slice(..));
            view.selection = Selection::single(range.anchor, range.head);
        }
        (None, _) | (_, None) => return,
    }
    scroll::follow_cursor(state, landing.pane);
}

/// `]g [g ]G [G`, after helix's `goto_next_change`; past the ends it opens the next changed file.
pub fn goto_hunk(state: &mut State, target: HunkTarget, out: &mut Vec<Effect>) {
    let count = count_of(state);
    let mode = state.input.mode;
    let focus = state.layout.focus;
    let State {
        layout, buffers, ..
    } = &mut *state;
    let Some(View::Text(view)) = layout.panes.get_mut(focus).and_then(Pane::active_mut) else {
        state.notice = Some(Notice::NoBuffer);
        return;
    };
    let buffer = view.buffer();
    let Some(Content::Text { doc, diff, .. }) = buffers.get(buffer).map(|b| &b.content) else {
        return;
    };
    let hunks: &[Hunk] = match &diff.bases {
        Bases::Tracked(tracked) => &tracked.hunks,
        Bases::NoRepo | Bases::Loading | Bases::Untracked => &[],
    };
    let text = doc.text.slice(..);
    let last = hunks.len().saturating_sub(1);
    let pick = |line: usize| -> Option<usize> {
        match target {
            HunkTarget::Next => {
                next_hunk(hunks, Side::After, line as u32).map(|i| (i + count - 1).min(last))
            }
            HunkTarget::Previous => {
                prev_hunk(hunks, Side::After, line as u32).map(|i| i.saturating_sub(count - 1))
            }
            HunkTarget::First => (!hunks.is_empty()).then_some(0),
            HunkTarget::Last => (!hunks.is_empty()).then_some(last),
        }
    };
    let primary = view.selection.primary().cursor_line(text);
    let arrive = Arrive::Text;
    let (direction, edge) = match target {
        HunkTarget::Next => (helix_core::movement::Direction::Forward, Edge::First),
        HunkTarget::Previous => (helix_core::movement::Direction::Backward, Edge::Last),
        HunkTarget::First | HunkTarget::Last => match pick(primary) {
            Some(index) => {
                let range = hunk_range(&hunks[index], text);
                view.selection = Selection::single(range.anchor, range.head);
                return;
            }
            None => {
                state.notice = Some(Notice::NoHunks);
                return;
            }
        },
    };
    match pick(primary) {
        Some(_) => {
            view.selection = view.selection.clone().transform(|range| {
                let Some(index) = pick(range.cursor_line(text)) else {
                    return range;
                };
                let next = hunk_range(&hunks[index], text);
                match mode {
                    Mode::Select => Range::new(
                        range.anchor,
                        match next.head < range.anchor {
                            true => next.anchor,
                            false => next.head,
                        },
                    ),
                    Mode::Normal | Mode::Insert => next.with_direction(direction),
                }
            });
        }
        // Past the ends: the next or previous changed file in sidebar order, wrapping around.
        None => {
            let Repo::Found { snapshot, .. } = &state.git.repo else {
                state.notice = Some(Notice::NoChangedFiles);
                return;
            };
            let root = state.root.path();
            let current = RelPath::new(root, &state.buffers[buffer].path).ok();
            // A partly staged file is listed twice; it is visited at its first group.
            let order = snapshot.changes.iter().filter_map(|changed| {
                match snapshot
                    .paths
                    .get(&changed.path)
                    .map(|change| change.groups())
                {
                    Some([first, ..]) if *first == changed.group => Some(&changed.path),
                    Some(_) | None => None,
                }
            });
            let (len, at) = order
                .clone()
                .enumerate()
                .fold((0, None), |(_, at), (i, path)| {
                    (i + 1, at.or((Some(path) == current.as_ref()).then_some(i)))
                });
            let index = match (len, edge, at) {
                (0, ..) => {
                    state.notice = Some(Notice::NoChangedFiles);
                    return;
                }
                (len, Edge::First, Some(at)) => (at + 1) % len,
                (_, Edge::First, None) => 0,
                (len, Edge::Last, Some(at)) => (at + len - 1) % len,
                (len, Edge::Last, None) => len - 1,
            };
            let Some(path) = order.clone().nth(index).map(|path| path.to_path(root)) else {
                return;
            };
            match arrive {
                Arrive::Text => {
                    crate::apply::open(state, path, Some(Target::Hunk(edge)), OpenAs::Preview, out)
                }
            }
        }
    }
}

/// Queues `op` on the index entry of `path`; `None` with a notice outside the repository.
fn index_job(state: &mut State, path: &Path, op: IndexOp) -> Option<Job> {
    match locate(&state.git.repo, path) {
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
    let focus = state.layout.focus;
    let Some(View::Text(view)) = state.layout.panes.get(focus).and_then(Pane::active) else {
        state.notice = Some(Notice::NoBuffer);
        return;
    };
    let Some(entry) = state.buffers.get(view.buffer()) else {
        return;
    };
    let (doc, tracked) = match &entry.content {
        Content::Loading { .. } => {
            state.notice = Some(Notice::StillLoading {
                path: entry.path.clone(),
            });
            return;
        }
        Content::Text { doc, diff, .. } => match &diff.bases {
            Bases::Tracked(tracked) if tracked.is_fresh(doc.version) => (doc, tracked),
            Bases::Tracked(_) | Bases::Loading => {
                state.notice = Some(Notice::DiffPending);
                return;
            }
            Bases::NoRepo | Bases::Untracked => {
                state.notice = Some(Notice::NotTracked {
                    path: entry.path.clone(),
                });
                return;
            }
        },
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
    match picked.is_empty() {
        true => {
            state.notice = Some(Notice::NoHunks);
            return;
        }
        false => {}
    }
    let path = entry.path.clone();
    let index = tracked.index.text();
    let write = match op {
        HunkOp::Stage => Some(IndexOp::Write {
            expected: tracked.index.oid(),
            text: splice(&index, &doc.text, picked.iter(), Side::Before),
        }),
        HunkOp::Unstage => Some(IndexOp::Write {
            expected: tracked.index.oid(),
            text: splice(&index, &tracked.head.text(), picked.iter(), Side::After),
        }),
        HunkOp::Revert => None,
    };
    match write {
        Some(write) => out.extend(index_job(state, &path, write).map(Effect::Job)),
        None => {
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

/// Whole-file staging; a discard waits in `state.git.confirm` for `Command::Confirm`.
pub fn file_op(state: &mut State, op: FileOp, out: &mut Vec<Effect>) {
    let resolve = |path: &Path| helix_stdx::path::normalize(state.root.path().join(path));
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
            let change = match (&state.git.repo, RelPath::new(state.root.path(), &path)) {
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
                    state.git.confirm = Some(Confirm::Discard {
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
                    state.git.confirm = Some(Confirm::Discard {
                        path,
                        how: Discard::Checkout,
                    })
                }
            }
        }
    }
}

pub fn confirm(state: &mut State, answer: Answer, out: &mut Vec<Effect>) {
    match (state.git.confirm.take(), answer) {
        (None, _) => {}
        (Some(Confirm::Discard { .. }), Answer::No) => {
            state.notice = Some(Notice::DiscardNotConfirmed)
        }
        (
            Some(Confirm::Discard {
                path,
                how: Discard::Checkout,
            }),
            Answer::Yes,
        ) => out.extend(index_job(state, &path, IndexOp::Checkout).map(Effect::Job)),
        (
            Some(Confirm::Discard {
                how: Discard::Delete(file),
                ..
            }),
            Answer::Yes,
        ) => out.push(Effect::Job(Job::Fs {
            root: state.root.path().to_owned(),
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

/// Lands on `edge` of a text buffer once its diff allows; called when `]g` opens a file.
pub(crate) fn expect_landing(state: &mut State, buffer: BufferId, landing: Landing) {
    match state.buffers.get_mut(buffer).map(|b| &mut b.content) {
        Some(Content::Text { diff, .. }) => match diff.bases {
            Bases::Loading | Bases::Tracked(_) => diff.landing = Some(landing),
            Bases::NoRepo | Bases::Untracked => {}
        },
        Some(Content::Loading { .. }) | None => {}
    }
    land(state, buffer);
}
