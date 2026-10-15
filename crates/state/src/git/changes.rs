//! Per-buffer diff jobs, hunk and file staging, and `]g` navigation across changed files.

use std::path::{Path, PathBuf};

use helix_core::{Range, RopeSlice, Selection, Tendril, Transaction};
use model::command::{ChangesCommand, FileOp, HunkOp, HunkTarget};
use model::config::OpenAs;
use model::git::{Change, Group};
use model::mode::Mode;
use model::notice::Notice;

use crate::apply::{count_of, edit, exit_select_mode};
use crate::git::diff::{
    Base, BaseRead, Bases, Compute, Edge, Hunk, Landing, Lines, Outcome, Side, Stamp, Tracked,
    intersecting, next_hunk, prev_hunk, splice,
};
use crate::git::{Changed, IndexError, IndexOp, RepoPath};
use crate::job::{FsOp, Job};
use crate::project::path::RelPath;
use crate::sidebar::tree::{EntryKind, Fold};
use crate::text::buffer::{BufferId, Content, Target};
use crate::view::layout::{Pane, TabKey, View};
use crate::{
    Confirm, Effect, Focus, Refresh, Repo, Sidebar, State, project::watch, request_git,
    view::panes, view::scroll,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Discard {
    /// Restores the worktree file from the index.
    Checkout,
    /// Deletes the untracked file at this root-relative path.
    Delete(RelPath),
}

/// Which view of a file a tab shows; `]g` keeps it when it moves on to the next changed file.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Arrive {
    Text,
    Diff,
    Rendered,
}

impl Arrive {
    pub fn key(self, buffer: BufferId) -> TabKey {
        match self {
            Arrive::Text => TabKey::Text(buffer),
            Arrive::Diff => TabKey::Diff(buffer),
            Arrive::Rendered => TabKey::Rendered(buffer),
        }
    }
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
    crate::git::conflict::sync(state);
    crate::render::rendered::sync(state, out);
    for (buffer, entry) in state.buffers.iter_mut() {
        let (doc, tracked) = match &mut entry.content {
            Content::Text {
                doc,
                diff:
                    crate::git::diff::Diff {
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
    crate::git::diffview::refresh(state, out);
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
        let index = pane.tabs.as_ref()?.find(landing.arrive.key(buffer))?;
        Some(pane.items_mut()[index].view.text_mut())
    });
    match (hunk, view) {
        (Some(hunk), Some(view)) => {
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
    let Some(view) = layout.panes.get_mut(focus).and_then(Pane::active_mut) else {
        state.notice = Some(Notice::NoBuffer);
        return;
    };
    let arrive = match view {
        View::Text(_) | View::Rendered(_) => Arrive::Text,
        View::Diff(_) => Arrive::Diff,
    };
    let view = view.text_mut();
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
            let order = snapshot.changes.iter().filter(|changed| {
                match snapshot
                    .paths
                    .get(&changed.path)
                    .map(|change| change.groups())
                {
                    Some([first, ..]) => *first == changed.group,
                    Some([]) | None => false,
                }
            });
            let (len, at) = order
                .clone()
                .enumerate()
                .fold((0, None), |(_, at), (i, changed)| {
                    let here = Some(&changed.path) == current.as_ref();
                    (i + 1, at.or(here.then_some(i)))
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
            let Some(changed) = order.clone().nth(index) else {
                return;
            };
            let path = changed.path.to_path(root);
            // Conflicts are resolved in the text, so they never open as a diff.
            let arrive = match changed.group {
                Group::Conflicts => Arrive::Text,
                Group::Staged | Group::Changes => arrive,
            };
            crate::apply::open(
                state,
                path,
                arrive,
                Some(Target::Hunk(edge)),
                OpenAs::Preview,
                out,
            )
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
    let Some(view) = (state.layout.panes.get(focus))
        .and_then(Pane::active)
        .map(View::text)
    else {
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

/// Whole-file staging; a discard waits in `state.confirm` for `Command::Confirm`.
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
                    state.confirm = Some(Confirm::Discard {
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
                    state.confirm = Some(Confirm::Discard {
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

/// Entries of each sidebar group in `changes`, which is sorted by group.
pub fn groups(changes: &[Changed]) -> [(Group, std::ops::Range<usize>); 3] {
    let mut start = 0;
    [Group::Conflicts, Group::Staged, Group::Changes].map(|group| {
        let end = start + changes[start..].partition_point(|changed| changed.group == group);
        let range = start..end;
        start = end;
        (group, range)
    })
}

/// A row of the Changes sidebar: a group header, or entry `Entry(i)` of the snapshot's changes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ListRow {
    Header {
        group: Group,
        count: usize,
        fold: Fold,
    },
    Entry(usize),
}

/// What the Changes sidebar cursor rests on.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ListCursor {
    Header(Group),
    Entry(Changed),
}

/// Sidebar rows a group takes: its header, plus its entries unless collapsed.
fn group_rows(entries: &std::ops::Range<usize>, fold: Fold) -> usize {
    match (entries.is_empty(), fold) {
        (true, _) => 0,
        (false, Fold::Expanded) => 1 + entries.len(),
        (false, Fold::Collapsed) => 1,
    }
}

/// Rows of the Changes sidebar: a header per non-empty group plus the entries of expanded ones.
pub fn list_len(changes: &[Changed], folds: &[Fold; Group::COUNT]) -> usize {
    groups(changes)
        .iter()
        .map(|(group, entries)| group_rows(entries, folds[*group as usize]))
        .sum()
}

pub fn list_row(changes: &[Changed], folds: &[Fold; Group::COUNT], row: usize) -> Option<ListRow> {
    let mut row = row;
    for (group, entries) in groups(changes) {
        let fold = folds[group as usize];
        let rows = group_rows(&entries, fold);
        match (rows, row) {
            (0, _) => {}
            (_, 0) => {
                return Some(ListRow::Header {
                    group,
                    count: entries.len(),
                    fold,
                });
            }
            (rows, row) if row < rows => return Some(ListRow::Entry(entries.start + row - 1)),
            (rows, rest) => row = rest - rows,
        }
    }
    None
}

/// Row of `cursor`: an entry of a collapsed group shows as its header, a vanished entry as the
/// row now in its place.
pub fn cursor_row(
    changes: &[Changed],
    folds: &[Fold; Group::COUNT],
    cursor: Option<&ListCursor>,
) -> Option<usize> {
    let last = list_len(changes, folds).checked_sub(1)?;
    let groups = groups(changes);
    let header = |group: Group| -> usize {
        groups
            .iter()
            .take_while(|(other, _)| *other != group)
            .map(|(other, entries)| group_rows(entries, folds[*other as usize]))
            .sum()
    };
    let row = match cursor {
        Some(ListCursor::Header(group)) => header(*group),
        // Nothing selected yet starts on the first file.
        None | Some(ListCursor::Entry(_)) => {
            let index = match cursor {
                Some(ListCursor::Entry(changed)) => match changes.binary_search(changed) {
                    Ok(index) | Err(index) => index.min(changes.len() - 1),
                },
                Some(ListCursor::Header(_)) | None => 0,
            };
            let group = changes[index].group;
            match folds[group as usize] {
                Fold::Collapsed => header(group),
                Fold::Expanded => header(group) + 1 + index - groups[group as usize].1.start,
            }
        }
    };
    Some(row.min(last))
}

/// The cursor that rests on `row`.
pub fn cursor_at(
    changes: &[Changed],
    folds: &[Fold; Group::COUNT],
    row: usize,
) -> Option<ListCursor> {
    match list_row(changes, folds, row)? {
        ListRow::Header { group, .. } => Some(ListCursor::Header(group)),
        ListRow::Entry(index) => Some(ListCursor::Entry(changes[index].clone())),
    }
}

fn snapshot_changes(repo: &Repo) -> &[Changed] {
    match repo {
        Repo::Found { snapshot, .. } => &snapshot.changes,
        Repo::Unknown | Repo::None => &[],
    }
}

/// Opens entry `index` in the focused pane: its diff, or its text when it has conflicts.
pub(crate) fn open_entry(state: &mut State, index: usize, open: OpenAs, out: &mut Vec<Effect>) {
    let Some(changed) = snapshot_changes(&state.git.repo).get(index).cloned() else {
        return;
    };
    let path = changed.path.to_path(state.root.path());
    let arrive = match changed.group {
        Group::Conflicts => Arrive::Text,
        Group::Staged | Group::Changes => Arrive::Diff,
    };
    state.git.cursor = Some(ListCursor::Entry(changed));
    crate::apply::open(state, path, arrive, None, open, out);
}

pub fn list(state: &mut State, command: ChangesCommand, out: &mut Vec<Effect>) {
    let folds = state.git.folds;
    let changes = snapshot_changes(&state.git.repo);
    let at = cursor_row(changes, &folds, state.git.cursor.as_ref());
    let row = at.and_then(|at| list_row(changes, &folds, at));
    let root = state.root.path();
    // Paths a file operation applies to: the entry, or every entry of a header's group.
    let paths = |row: ListRow| -> Vec<PathBuf> {
        let range = match row {
            ListRow::Header { group, .. } => groups(changes)[group as usize].1.clone(),
            ListRow::Entry(index) => index..index + 1,
        };
        changes[range]
            .iter()
            .map(|changed| changed.path.to_path(root))
            .collect()
    };
    match (command, at, row) {
        (ChangesCommand::Toggle, ..) => {
            state.sidebar = match state.sidebar {
                Sidebar::Hidden | Sidebar::Files(_) | Sidebar::Changes(Focus::Editor) => {
                    Sidebar::Changes(Focus::Sidebar)
                }
                Sidebar::Changes(Focus::Sidebar) => Sidebar::Hidden,
            }
        }
        (ChangesCommand::FocusEditor, ..) => panes::focus_editor(state),
        (ChangesCommand::Up, Some(at), _) => {
            state.git.cursor = cursor_at(changes, &folds, at.saturating_sub(1))
        }
        (ChangesCommand::Down, Some(at), _) => {
            let last = list_len(changes, &folds).saturating_sub(1);
            state.git.cursor = cursor_at(changes, &folds, (at + 1).min(last))
        }
        (ChangesCommand::Up | ChangesCommand::Down, None, _) => {}
        (ChangesCommand::Open, _, Some(ListRow::Header { group, fold, .. })) => {
            state.git.folds[group as usize] = match fold {
                Fold::Expanded => Fold::Collapsed,
                Fold::Collapsed => Fold::Expanded,
            };
            state.git.cursor = Some(ListCursor::Header(group));
        }
        (ChangesCommand::Open, _, Some(ListRow::Entry(index))) => {
            open_entry(state, index, OpenAs::Preview, out)
        }
        (ChangesCommand::Collapse, _, Some(ListRow::Header { group, .. })) => {
            state.git.folds[group as usize] = Fold::Collapsed
        }
        (ChangesCommand::Collapse, _, Some(ListRow::Entry(index))) => {
            state.git.cursor = Some(ListCursor::Header(changes[index].group))
        }
        (ChangesCommand::Stage, _, Some(row)) => {
            for path in paths(row) {
                file_op(state, FileOp::Stage(path), out);
            }
        }
        (ChangesCommand::Unstage, _, Some(row)) => {
            for path in paths(row) {
                file_op(state, FileOp::Unstage(path), out);
            }
        }
        (ChangesCommand::Discard, _, Some(ListRow::Header { .. })) => {
            state.notice = Some(Notice::DiscardGroup)
        }
        (ChangesCommand::Discard, _, Some(ListRow::Entry(index))) => {
            let path = changes[index].path.to_path(root);
            file_op(state, FileOp::Discard(path), out)
        }
        (
            ChangesCommand::Open
            | ChangesCommand::Collapse
            | ChangesCommand::Stage
            | ChangesCommand::Unstage
            | ChangesCommand::Discard,
            _,
            None,
        ) => state.notice = Some(Notice::NoChangedFiles),
    }
}
