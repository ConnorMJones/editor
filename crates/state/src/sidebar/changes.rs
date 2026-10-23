//! The Changes sidebar: grouped rows of the repository's changes.

use std::path::PathBuf;

use model::command::{ChangesCommand, FileOp, Shown};
use model::config::OpenAs;
use model::git::Group;
use model::notice::Notice;

use crate::git::Changed;
use crate::git::stage::file_op;
use crate::project::path::RelPath;
use crate::sidebar::tree::Fold;
use crate::view::panes;
use crate::view::panes::Arrive;
use crate::{Effect, Focus, Sidebar, State};

/// Entries of each sidebar group in `changes`, which is sorted by group.
pub fn groups(changes: &[Changed]) -> [(Group, std::ops::Range<usize>); 3] {
    let mut start = 0;
    [Group::Conflicts, Group::Staged, Group::Changes].map(|group| {
        let end =
            start + changes[start..].partition_point(|changed| changed.entry.group() == group);
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

pub struct ChangesList {
    pub cursor: Option<ListCursor>,
    /// Per `Group`, whether the sidebar shows its entries.
    pub folds: [Fold; Group::COUNT],
}

/// What the Changes sidebar cursor rests on, by value so it survives refreshes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ListCursor {
    Header(Group),
    Entry { group: Group, path: RelPath },
}

impl ListCursor {
    pub fn on(changed: &Changed) -> ListCursor {
        ListCursor::Entry {
            group: changed.entry.group(),
            path: changed.path.clone(),
        }
    }
}

/// Sidebar rows a group takes: its header, plus its entries unless collapsed.
fn group_rows(entries: &std::ops::Range<usize>, fold: Fold) -> usize {
    match (entries.len(), fold) {
        (0, _) => 0,
        (count, Fold::Expanded) => 1 + count,
        (_, Fold::Collapsed) => 1,
    }
}

/// Rows of the Changes sidebar: a header per non-empty group plus the entries of expanded ones.
pub fn list_len(changes: &[Changed], folds: &[Fold; Group::COUNT]) -> usize {
    groups(changes)
        .iter()
        .map(|(group, entries)| group_rows(entries, folds[*group as usize]))
        .sum()
}

/// The rows `list_len` counts, in order.
pub fn rows(
    changes: &[Changed],
    folds: &[Fold; Group::COUNT],
) -> impl Iterator<Item = ListRow> + use<> {
    let folds = *folds;
    groups(changes)
        .into_iter()
        .flat_map(move |(group, entries)| {
            let fold = folds[group as usize];
            let header = match entries.len() {
                0 => None,
                count => Some(ListRow::Header { group, count, fold }),
            };
            let shown = match fold {
                Fold::Expanded => entries,
                Fold::Collapsed => entries.end..entries.end,
            };
            header.into_iter().chain(shown.map(ListRow::Entry))
        })
}

/// An entry of a collapsed group shows as its header, a vanished entry as the row in its place.
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
    let entry = |index: usize| {
        let group = changes[index].entry.group();
        match folds[group as usize] {
            Fold::Collapsed => header(group),
            Fold::Expanded => header(group) + 1 + index - groups[group as usize].1.start,
        }
    };
    let row = match cursor {
        Some(ListCursor::Header(group)) => header(*group),
        Some(ListCursor::Entry { group, path }) => {
            match changes.binary_search_by(|changed| changed.key().cmp(&(*group, path))) {
                Ok(index) | Err(index) => entry(index.min(changes.len() - 1)),
            }
        }
        // Nothing selected yet starts on the first file.
        None => entry(0),
    };
    Some(row.min(last))
}

pub fn cursor_at(
    changes: &[Changed],
    folds: &[Fold; Group::COUNT],
    row: usize,
) -> Option<ListCursor> {
    match rows(changes, folds).nth(row)? {
        ListRow::Header { group, .. } => Some(ListCursor::Header(group)),
        ListRow::Entry(index) => Some(ListCursor::on(&changes[index])),
    }
}

/// Opens entry `index` in the focused pane: its diff, or its text when it has conflicts.
pub fn open_entry(state: &mut State, index: usize, open: OpenAs, out: &mut Vec<Effect>) {
    let Some(changed) = state.project.git.changes().get(index).cloned() else {
        return;
    };
    let path = changed.path.to_path(state.project.root.path());
    let arrive = match changed.entry.group() {
        Group::Conflicts => Arrive::In(Shown::Source),
        Group::Staged | Group::Changes => Arrive::In(Shown::Diff),
    };
    state.changes.cursor = Some(ListCursor::on(&changed));
    crate::apply::open(state, path, arrive, None, open, out);
}

pub fn list(state: &mut State, command: ChangesCommand, out: &mut Vec<Effect>) {
    let folds = state.changes.folds;
    let changes = state.project.git.changes();
    let at = cursor_row(changes, &folds, state.changes.cursor.as_ref());
    let row = at.and_then(|at| rows(changes, &folds).nth(at));
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
            state.changes.cursor = Some(ListCursor::Header(changes[index].entry.group()))
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
