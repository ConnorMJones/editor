use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::Ordering;

use helix_core::{Range, Selection};
use model::command::{Command, Cycle, PickerCommand, TreeCommand, WindowCommand};
use model::notice::Notice;
use model::window::PickerKind;
use slotmap::SlotMap;

use crate::apply::apply;
use crate::buffer::{Buffer, BufferId, Content};
use crate::files::Files;
use crate::job::{FsOp, Job};
use crate::layout::{LastPane, Layout, Pane, PaneId, Scroll, TextView, View};
use crate::path::RelPath;
use crate::picker::{self, Grep, Items, Narrowing, PALETTE, Picker, Search};
use crate::tree::{self, Dir, EntryKind, Fold, Listing, RowKind, TreeEdit};
use crate::{Effect, Focus, Overlay, Sidebar, State};

/// Shows `buffer` in `pane`, parking the pane's current view and restoring a parked one.
pub fn show(
    layout: &mut Layout,
    buffers: &SlotMap<BufferId, Buffer>,
    pane: PaneId,
    buffer: BufferId,
) {
    let (Some(pane), Some(target)) = (layout.panes.get_mut(pane), buffers.get(buffer)) else {
        return;
    };
    match &pane.view {
        View::Text(view) if view.buffer == buffer => return,
        View::Text(_) | View::Empty => {}
    }
    let len = match &target.content {
        Content::Text(doc) => doc.text.len_chars(),
    };
    let view = match pane.parked.iter().position(|view| view.buffer == buffer) {
        // Parked selections are not mapped through edits made elsewhere, so clamp them.
        Some(index) => {
            let mut view = pane.parked.swap_remove(index);
            view.selection = view
                .selection
                .transform(|range| Range::new(range.anchor.min(len), range.head.min(len)));
            view
        }
        None => TextView {
            buffer,
            selection: Selection::point(0),
            scroll: Scroll::default(),
        },
    };
    match std::mem::replace(&mut pane.view, View::Text(view)) {
        View::Text(old) => pane.parked.push(old),
        View::Empty => {}
    }
}

pub fn window(state: &mut State, command: WindowCommand, out: &mut Vec<Effect>) {
    let layout = &mut state.layout;
    match command {
        WindowCommand::Split(axis) => {
            let view = match layout.panes.get(layout.focus).map(|pane| &pane.view) {
                Some(View::Text(view)) => View::Text(view.clone()),
                Some(View::Empty) | None => View::Empty,
            };
            layout.split(
                axis,
                Pane {
                    view,
                    parked: Vec::new(),
                },
            );
        }
        WindowCommand::Focus(side) => match layout.neighbour(side) {
            Some(pane) => layout.focus = pane,
            None => {}
        },
        WindowCommand::FocusNext => layout.focus = layout.next(),
        WindowCommand::Close => match layout.close(layout.focus) {
            Ok(()) => {}
            Err(LastPane) => apply(state, Command::Quit, out),
        },
        WindowCommand::Only => layout.only(),
    }
    focus_editor(state);
}

pub fn cycle_buffer(state: &mut State, cycle: Cycle) {
    let focus = state.layout.focus;
    let current = match state.layout.panes.get(focus).map(|pane| &pane.view) {
        Some(View::Text(view)) => state.tabs.iter().position(|&id| id == view.buffer),
        Some(View::Empty) | None => None,
    };
    let count = state.tabs.len();
    let next = match (current, cycle, count) {
        (_, _, 0) => return,
        (None, _, _) => 0,
        (Some(i), Cycle::Next, _) => (i + 1) % count,
        (Some(i), Cycle::Previous, _) => (i + count - 1) % count,
    };
    show(&mut state.layout, &state.buffers, focus, state.tabs[next]);
}

fn focus_editor(state: &mut State) {
    match state.sidebar {
        Sidebar::Files(_) => state.sidebar = Sidebar::Files(Focus::Editor),
        Sidebar::Hidden => {}
    }
}

pub fn tree(state: &mut State, command: TreeCommand, out: &mut Vec<Effect>) {
    let root = state.root.path().to_owned();
    let selected = state.tree.rows.get(state.tree.cursor).cloned();
    match command {
        TreeCommand::Toggle => {
            state.sidebar = match state.sidebar {
                Sidebar::Hidden | Sidebar::Files(Focus::Editor) => Sidebar::Files(Focus::Sidebar),
                Sidebar::Files(Focus::Sidebar) => Sidebar::Hidden,
            };
        }
        TreeCommand::FocusEditor => focus_editor(state),
        TreeCommand::Up => state.tree.cursor = state.tree.cursor.saturating_sub(1),
        TreeCommand::Down => {
            state.tree.cursor = (state.tree.cursor + 1).min(state.tree.rows.len().saturating_sub(1))
        }
        TreeCommand::Open => match selected {
            None => {}
            Some(row) => match row.kind {
                RowKind::File => {
                    apply(state, Command::Open(row.path.to_path(&root)), out);
                    focus_editor(state);
                }
                RowKind::Dir(Fold::Expanded) => {
                    if let Some(dir) = state.tree.dirs.get_mut(&row.path) {
                        dir.fold = Fold::Collapsed;
                    }
                    tree::rebuild(&mut state.tree);
                }
                RowKind::Dir(Fold::Collapsed) => {
                    match state.tree.dirs.get_mut(&row.path) {
                        Some(dir) => dir.fold = Fold::Expanded,
                        None => {
                            state.tree.dirs.insert(
                                row.path.clone(),
                                Dir {
                                    fold: Fold::Expanded,
                                    listing: Listing::Loading,
                                },
                            );
                            out.push(Effect::Job(Job::ListDir {
                                root,
                                dir: row.path,
                            }));
                        }
                    }
                    tree::rebuild(&mut state.tree);
                }
            },
        },
        TreeCommand::Close => match selected {
            None => {}
            Some(row) => match (row.kind, row.path.parent()) {
                (RowKind::Dir(Fold::Expanded), _) => {
                    if let Some(dir) = state.tree.dirs.get_mut(&row.path) {
                        dir.fold = Fold::Collapsed;
                    }
                    tree::rebuild(&mut state.tree);
                }
                (RowKind::File | RowKind::Dir(Fold::Collapsed), Some(parent)) => {
                    match state
                        .tree
                        .rows
                        .iter()
                        .position(|r| r.path.as_str() == parent)
                    {
                        Some(index) => state.tree.cursor = index,
                        None => {}
                    }
                }
                (RowKind::File | RowKind::Dir(Fold::Collapsed), None) => {}
            },
        },
        TreeCommand::Create => {
            let dir = match &selected {
                Some(row) => match row.kind {
                    RowKind::Dir(_) => row.path.as_str(),
                    RowKind::File => row.path.parent().unwrap_or(""),
                },
                None => "",
            };
            let line = match dir.is_empty() {
                true => String::new(),
                false => format!("{dir}/"),
            };
            state.tree.edit = TreeEdit::Create { line };
        }
        TreeCommand::Rename => match selected {
            Some(row) => {
                state.tree.edit = TreeEdit::Rename {
                    line: row.path.as_str().to_owned(),
                    from: row.path,
                }
            }
            None => {}
        },
        TreeCommand::Delete => match selected {
            Some(row) => {
                state.tree.edit = TreeEdit::Delete {
                    path: row.path,
                    kind: match row.kind {
                        RowKind::File => EntryKind::File,
                        RowKind::Dir(_) => EntryKind::Dir,
                    },
                    line: String::new(),
                }
            }
            None => {}
        },
        TreeCommand::EditInsert(c) => match &mut state.tree.edit {
            TreeEdit::Create { line }
            | TreeEdit::Rename { line, .. }
            | TreeEdit::Delete { line, .. } => line.push(c),
            TreeEdit::None => {}
        },
        TreeCommand::EditBackspace => match &mut state.tree.edit {
            TreeEdit::Create { line }
            | TreeEdit::Rename { line, .. }
            | TreeEdit::Delete { line, .. } => {
                line.pop();
            }
            TreeEdit::None => {}
        },
        TreeCommand::EditCancel => state.tree.edit = TreeEdit::None,
        TreeCommand::EditSubmit => {
            let op = match std::mem::take(&mut state.tree.edit) {
                TreeEdit::None => return,
                TreeEdit::Create { line } => {
                    let kind = match line.ends_with('/') {
                        true => EntryKind::Dir,
                        false => EntryKind::File,
                    };
                    match RelPath::parse(line.trim_end_matches('/')) {
                        Ok(path) if !path.is_root() => FsOp::Create { path, kind },
                        Ok(_) | Err(_) => {
                            state.notice = Some(Notice::InvalidPath { input: line });
                            return;
                        }
                    }
                }
                TreeEdit::Rename { from, line } => match RelPath::parse(line.trim_end_matches('/'))
                {
                    Ok(to) if to == from => return,
                    Ok(to) if !to.is_root() => FsOp::Rename { from, to },
                    Ok(_) | Err(_) => {
                        state.notice = Some(Notice::InvalidPath { input: line });
                        return;
                    }
                },
                TreeEdit::Delete { path, kind, line } => match line.as_str() {
                    "y" => FsOp::Remove { path, kind },
                    _ => {
                        state.notice = Some(Notice::DeleteNotConfirmed);
                        return;
                    }
                },
            };
            out.push(Effect::Job(Job::Fs { root, op }));
        }
    }
}

pub fn picker(state: &mut State, command: PickerCommand, out: &mut Vec<Effect>) {
    match command {
        PickerCommand::Open(kind) => {
            let items = match kind {
                PickerKind::Files => Items::Files(match &state.files {
                    Files::Ready(files) => files.clone(),
                    Files::Loading => Arc::new(Vec::new()),
                }),
                PickerKind::Buffers => Items::Buffers(
                    state
                        .tabs
                        .iter()
                        .map(|&id| {
                            let path = &state.buffers[id].path;
                            let label = path.strip_prefix(state.root.path()).unwrap_or(path);
                            (id, label.display().to_string())
                        })
                        .collect(),
                ),
                PickerKind::Grep => Items::Grep(Grep {
                    generation: state.grep.load(Ordering::Relaxed),
                    search: Search::Idle,
                    hits: Vec::new(),
                }),
                PickerKind::Commands => Items::Commands,
            };
            state.overlay = Overlay::Picker(Picker::new(items));
        }
        PickerCommand::Insert(c) => edit_query(state, |query| query.push(c), out),
        PickerCommand::Backspace => edit_query(
            state,
            |query| {
                query.pop();
            },
            out,
        ),
        PickerCommand::Next => move_cursor(state, Cycle::Next),
        PickerCommand::Previous => move_cursor(state, Cycle::Previous),
        PickerCommand::Cancel => state.overlay = Overlay::None,
        PickerCommand::Submit => {
            let Overlay::Picker(picker) = std::mem::replace(&mut state.overlay, Overlay::None)
            else {
                return;
            };
            let Some(&index) = picker.matches.get(picker.cursor) else {
                return;
            };
            let index = index as usize;
            match picker.items {
                Items::Files(files) => {
                    apply(
                        state,
                        Command::Open(PathBuf::from(files[index].as_str())),
                        out,
                    );
                    focus_editor(state);
                }
                Items::Buffers(buffers) => {
                    let focus = state.layout.focus;
                    show(&mut state.layout, &state.buffers, focus, buffers[index].0);
                    focus_editor(state);
                }
                Items::Grep(grep) => {
                    let hit = &grep.hits[index];
                    apply(state, Command::Open(PathBuf::from(hit.path.as_str())), out);
                    focus_editor(state);
                    let State {
                        layout, buffers, ..
                    } = state;
                    let Some(View::Text(view)) =
                        layout.panes.get_mut(layout.focus).map(|p| &mut p.view)
                    else {
                        return;
                    };
                    let Some(Content::Text(doc)) = buffers.get(view.buffer).map(|b| &b.content)
                    else {
                        return;
                    };
                    let line = (hit.line as usize).min(doc.text.len_lines().saturating_sub(1));
                    let start = doc.text.line_to_char(line);
                    let end = doc.text.line_to_char((line + 1).min(doc.text.len_lines()));
                    view.selection = Selection::point((start + hit.column as usize).min(end));
                }
                Items::Commands => apply(state, PALETTE[index].1.clone(), out),
            }
        }
    }
}

fn move_cursor(state: &mut State, cycle: Cycle) {
    let Overlay::Picker(picker) = &mut state.overlay else {
        return;
    };
    let count = picker.matches.len().max(1);
    picker.cursor = match cycle {
        Cycle::Next => (picker.cursor + 1) % count,
        Cycle::Previous => (picker.cursor + count - 1) % count,
    };
}

fn edit_query(state: &mut State, edit: impl FnOnce(&mut String), out: &mut Vec<Effect>) {
    let Overlay::Picker(picker) = &mut state.overlay else {
        return;
    };
    let previous = picker.query.clone();
    edit(&mut picker.query);
    match &mut picker.items {
        Items::Grep(grep) => {
            let generation = state.grep.fetch_add(1, Ordering::Relaxed) + 1;
            grep.generation = generation;
            grep.hits.clear();
            picker.matches.clear();
            picker.cursor = 0;
            let files = match (&state.files, picker.query.is_empty()) {
                (Files::Ready(files), false) => files.clone(),
                (Files::Ready(_), true) | (Files::Loading, _) => {
                    grep.search = Search::Idle;
                    return;
                }
            };
            grep.search = Search::Running;
            out.push(Effect::Job(Job::Grep {
                root: state.root.path().to_owned(),
                files,
                regex: picker::grep_regex(&picker.query),
                generation,
                current: state.grep.clone(),
            }));
        }
        Items::Files(_) | Items::Buffers(_) | Items::Commands => {
            let narrowing = Narrowing::between(&previous, &picker.query);
            picker::filter(picker, &mut state.matcher, narrowing);
        }
    }
}
