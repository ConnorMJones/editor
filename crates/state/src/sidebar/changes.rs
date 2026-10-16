//! The Changes sidebar: grouped rows of the repository's changes.

use std::path::PathBuf;

use model::command::{ChangesCommand, FileOp};
use model::config::OpenAs;
use model::git::Group;
use model::notice::Notice;

use crate::git::Changed;
use crate::git::navigate::Arrive;
use crate::git::stage::file_op;
use crate::sidebar::tree::Fold;
use crate::view::panes;
use crate::{Effect, Focus, Repo, Sidebar, State};

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

/// The Changes sidebar's view of the repository's changes.
pub struct ChangesList {
    /// Row selected, kept by value so it survives refreshes.
    pub cursor: Option<ListCursor>,
    /// Per `Group`, whether the sidebar shows its entries.
    pub folds: [Fold; Group::COUNT],
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
    let Some(changed) = snapshot_changes(&state.project.git).get(index).cloned() else {
        return;
    };
    let path = changed.path.to_path(state.project.root.path());
    let arrive = match changed.group {
        Group::Conflicts => Arrive::Text,
        Group::Staged | Group::Changes => Arrive::Diff,
    };
    state.changes.cursor = Some(ListCursor::Entry(changed));
    crate::apply::open(state, path, arrive, None, open, out);
}

pub fn list(state: &mut State, command: ChangesCommand, out: &mut Vec<Effect>) {
    let folds = state.changes.folds;
    let changes = snapshot_changes(&state.project.git);
    let at = cursor_row(changes, &folds, state.changes.cursor.as_ref());
    let row = at.and_then(|at| list_row(changes, &folds, at));
    let root = state.project.root.path();
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
            state.changes.cursor = cursor_at(changes, &folds, at.saturating_sub(1))
        }
        (ChangesCommand::Down, Some(at), _) => {
            let last = list_len(changes, &folds).saturating_sub(1);
            state.changes.cursor = cursor_at(changes, &folds, (at + 1).min(last))
        }
        (ChangesCommand::Up | ChangesCommand::Down, None, _) => {}
        (ChangesCommand::Open, _, Some(ListRow::Header { group, fold, .. })) => {
            state.changes.folds[group as usize] = match fold {
                Fold::Expanded => Fold::Collapsed,
                Fold::Collapsed => Fold::Expanded,
            };
            state.changes.cursor = Some(ListCursor::Header(group));
        }
        (ChangesCommand::Open, _, Some(ListRow::Entry(index))) => {
            open_entry(state, index, OpenAs::Preview, out)
        }
        (ChangesCommand::Collapse, _, Some(ListRow::Header { group, .. })) => {
            state.changes.folds[group as usize] = Fold::Collapsed
        }
        (ChangesCommand::Collapse, _, Some(ListRow::Entry(index))) => {
            state.changes.cursor = Some(ListCursor::Header(changes[index].group))
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
