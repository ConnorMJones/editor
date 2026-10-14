use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::Ordering;

use model::command::{ChangesCommand, Command, Cycle, PickerCommand, TreeCommand, WindowCommand};
use model::config::OpenAs;
use model::notice::Notice;
use model::window::PickerKind;

use crate::apply::{Force, apply};
use crate::backlinks::{self, Build};
use crate::buffer::{BufferId, Content, Position, Target};
use crate::changes::{self, Arrive, ListRow};
use crate::diffview::DiffView;
use crate::files::Files;
use crate::job::{FsOp, Job};
use crate::layout::{CloseError, Layout, Pane, PaneId, TabKey, Tabs, TextView, Tick, View};
use crate::markdown::Format;
use crate::path::RelPath;
use crate::picker::{
    self, Grep, Items, Narrowing, PALETTE, PREVIEW_SLOTS, Picker, Preview, PreviewSource,
    Referrers, Search,
};
use crate::rendered::{Link, RenderedView, Scroll};
use crate::tree::{
    self, Dir, EntryKind, Fold, Listing, NewEntry, RowKind, TreeAction, TreeEdit, TreeTarget,
};
use crate::{Effect, Focus, Overlay, Repo, Sidebar, State, scroll};

/// Shows `buffer` in `pane` by zed's preview rules, committing the pane's current edits; a
/// modified buffer always opens permanently and a modified preview is never replaced.
pub fn show(state: &mut State, pane: PaneId, key: TabKey, open: OpenAs) {
    let buffer = key.buffer();
    let open = match state.buffers.get(buffer).map(|b| b.content.is_modified()) {
        None => return,
        Some(true) => OpenAs::Permanent,
        Some(false) => open,
    };
    let now = state.layout.tick();
    let State {
        layout, buffers, ..
    } = state;
    let Some(target) = layout.panes.get_mut(pane) else {
        return;
    };
    match target.active().map(View::text) {
        Some(view) => match buffers.get_mut(view.buffer()).map(|b| &mut b.content) {
            Some(Content::Text { doc, .. }) => doc.commit(&view.selection),
            Some(Content::Loading { .. }) | None => {}
        },
        None => {}
    }
    let view = match key {
        TabKey::Text(buffer) => View::Text(TextView::new(buffer)),
        TabKey::Diff(buffer) => View::Diff(DiffView::new(buffer)),
        TabKey::Rendered(buffer) => {
            View::Rendered(RenderedView::new(buffer, Link::Standalone, Scroll::item(0)))
        }
    };
    let displaced = match &mut target.tabs {
        Some(tabs) => {
            let preview = tabs.preview().map(|p| tabs.items()[p].view.buffer());
            match preview
                .and_then(|b| buffers.get(b))
                .map(|b| b.content.is_modified())
            {
                Some(true) => tabs.keep(),
                Some(false) | None => {}
            }
            tabs.open(view, open, now)
        }
        None => {
            target.tabs = Some(Tabs::new(view, open, now));
            None
        }
    };
    // The replaced preview's buffer goes once no pane shows it.
    let Some(displaced) = displaced.map(|item| item.view.buffer()) else {
        return;
    };
    let shown = (layout.panes.values()).any(|pane| {
        pane.items()
            .iter()
            .any(|item| item.view.buffer() == displaced)
    });
    match (
        shown,
        buffers.get(displaced).map(|b| b.content.is_modified()),
    ) {
        (false, Some(false)) => {
            buffers.remove(displaced);
        }
        (false, Some(true) | None) | (true, _) => {}
    }
}

/// Closes tab `index` of `pane`, dropping the buffer with its last view unless unsaved edits
/// refuse it.
pub fn close_tab(state: &mut State, pane: PaneId, index: usize, force: Force) {
    let Some(buffer) = (state.layout.panes.get(pane))
        .and_then(|target| target.items().get(index))
        .map(|item| item.view.buffer())
    else {
        return;
    };
    // The same pane may also show the buffer, as text beside its diff.
    let elsewhere = state.layout.panes.iter().any(|(id, other)| {
        other
            .items()
            .iter()
            .enumerate()
            .any(|(at, item)| item.view.buffer() == buffer && (id, at) != (pane, index))
    });
    let modified = (state.buffers.get(buffer)).is_some_and(|b| b.content.is_modified());
    match (elsewhere, modified, force) {
        (false, true, Force::No) => {
            state.notice = Some(Notice::UnsavedBuffers { count: 1 });
            return;
        }
        (true, _, _) | (false, false, _) | (false, true, Force::Yes) => {}
    }
    state.layout.remove_tab(pane, index);
    match elsewhere {
        true => {}
        false => {
            state.buffers.remove(buffer);
        }
    }
}

/// Stamps the focused pane's active tab as most recently used.
fn touch(layout: &mut Layout) {
    let now = layout.tick();
    match layout
        .panes
        .get_mut(layout.focus)
        .and_then(|p| p.tabs.as_mut())
    {
        Some(tabs) => tabs.activate(tabs.active(), now),
        None => {}
    }
}

pub fn window(state: &mut State, command: WindowCommand, out: &mut Vec<Effect>) {
    let layout = &mut state.layout;
    match command {
        // Like zed, the new pane holds only the active item; see `Tabs::split_copy`.
        WindowCommand::Split(axis) => {
            let now = layout.tick();
            let tabs = (layout.panes.get(layout.focus))
                .and_then(Pane::active)
                .map(|view| Tabs::split_copy(view.clone(), now));
            layout.split(axis, Pane { tabs });
        }
        WindowCommand::Focus(side) => match layout.neighbour(side) {
            Some(pane) => layout.focus = pane,
            None => {}
        },
        WindowCommand::FocusNext => layout.focus = layout.next(),
        WindowCommand::Close => match layout.close(layout.focus) {
            Ok(()) => {}
            Err(CloseError::LastPane) => apply(state, Command::Quit, out),
            Err(CloseError::NotInLayout) => unreachable!("the focus is a leaf of the layout"),
        },
        WindowCommand::Only => layout.only(),
    }
    touch(&mut state.layout);
    focus_editor(state);
}

/// Activates the next or previous tab of the focused pane, wrapping around.
pub fn cycle_tab(state: &mut State, cycle: Cycle) {
    let now = state.layout.tick();
    let focus = state.layout.focus;
    let Some(tabs) = (state.layout.panes.get_mut(focus)).and_then(|pane| pane.tabs.as_mut()) else {
        return;
    };
    let (active, count) = (tabs.active(), tabs.items().len());
    let next = match cycle {
        Cycle::Next => (active + 1) % count,
        Cycle::Previous => (active + count - 1) % count,
    };
    tabs.activate(next, now);
}

pub(crate) fn focus_editor(state: &mut State) {
    state.sidebar = match state.sidebar {
        Sidebar::Files(_) => Sidebar::Files(Focus::Editor),
        Sidebar::Changes(_) => Sidebar::Changes(Focus::Editor),
        Sidebar::Hidden => Sidebar::Hidden,
    };
}

pub fn tree(state: &mut State, command: TreeCommand, out: &mut Vec<Effect>) {
    let root = state.root.path().to_owned();
    let selected = state.tree.rows.get(state.tree.cursor).cloned();
    match command {
        TreeCommand::Toggle => {
            state.sidebar = match state.sidebar {
                Sidebar::Hidden | Sidebar::Files(Focus::Editor) | Sidebar::Changes(_) => {
                    Sidebar::Files(Focus::Sidebar)
                }
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
                    let open = state.tab_config.open_from_tree;
                    let path = row.path.to_path(&root);
                    crate::apply::open(state, path, Arrive::Text, None, open, out);
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
        TreeCommand::Create => begin_create(state, row_dir(selected.as_ref()), NewEntry::Infer),
        TreeCommand::CreateFolder => {
            begin_create(state, row_dir(selected.as_ref()), NewEntry::Folder)
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
            TreeEdit::Create { line, .. }
            | TreeEdit::Rename { line, .. }
            | TreeEdit::Delete { line, .. } => line.push(c),
            TreeEdit::None => {}
        },
        TreeCommand::EditBackspace => match &mut state.tree.edit {
            TreeEdit::Create { line, .. }
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
                TreeEdit::Create { line, kind } => {
                    let kind = match (kind, line.ends_with('/')) {
                        (NewEntry::Folder, _) | (NewEntry::Infer, true) => EntryKind::Dir,
                        (NewEntry::Infer, false) => EntryKind::File,
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
                // Most recently activated first; buffers no pane shows come last.
                PickerKind::Buffers => {
                    let mut recent: Vec<(Tick, BufferId)> = state
                        .buffers
                        .keys()
                        .map(|id| {
                            let used = (state.layout.panes.values())
                                .flat_map(Pane::items)
                                .filter(|item| item.view.buffer() == id)
                                .map(|item| item.used)
                                .max()
                                .unwrap_or_default();
                            (used, id)
                        })
                        .collect();
                    recent.sort_unstable_by_key(|&(used, _)| std::cmp::Reverse(used));
                    Items::Buffers(
                        recent
                            .into_iter()
                            .map(|(_, id)| {
                                let path = &state.buffers[id].path;
                                let label = path.strip_prefix(state.root.path()).unwrap_or(path);
                                (id, label.display().to_string())
                            })
                            .collect(),
                    )
                }
                PickerKind::Grep => Items::Grep(Grep {
                    generation: state.grep.load(Ordering::Relaxed),
                    search: Search::Idle,
                    hits: Vec::new(),
                }),
                PickerKind::Commands => Items::Commands,
                PickerKind::Backlinks => {
                    let Some(path) = (state.layout.panes.get(state.layout.focus))
                        .and_then(Pane::active)
                        .and_then(|view| state.buffers.get(view.buffer()))
                        .map(|buffer| buffer.path.clone())
                    else {
                        return state.notice = Some(Notice::NoBuffer);
                    };
                    let note = match (Format::of(&path), RelPath::new(state.root.path(), &path)) {
                        (Some(Format::Markdown), Ok(note)) => note,
                        (Some(Format::Markdown | Format::Typst) | None, _) => {
                            return state.notice = Some(Notice::NotANote { path });
                        }
                    };
                    backlinks::ensure(state, out);
                    backlinks::sync_buffers(state, out);
                    let (search, hits) = match state.backlinks.build {
                        Build::Ready => (Search::Done, backlinks::hits(state, &note)),
                        Build::Unbuilt | Build::Waiting | Build::Building { .. } => {
                            (Search::Running, Vec::new())
                        }
                    };
                    Items::Backlinks(Referrers { note, search, hits })
                }
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
                    let path = PathBuf::from(files[index].as_str());
                    let open = state.tab_config.open_from_picker;
                    crate::apply::open(state, path, Arrive::Text, None, open, out);
                    focus_editor(state);
                }
                Items::Buffers(buffers) => {
                    let focus = state.layout.focus;
                    show(
                        state,
                        focus,
                        TabKey::Text(buffers[index].0),
                        OpenAs::Permanent,
                    );
                    focus_editor(state);
                }
                Items::Grep(Grep { hits, .. }) | Items::Backlinks(Referrers { hits, .. }) => {
                    let hit = &hits[index];
                    let open = state.tab_config.open_from_picker;
                    crate::apply::open(
                        state,
                        PathBuf::from(hit.path.as_str()),
                        Arrive::Text,
                        Some(Target::Position(Position {
                            line: hit.line as usize,
                            column: hit.column as usize,
                        })),
                        open,
                        out,
                    );
                    focus_editor(state);
                }
                Items::Commands => apply(state, PALETTE[index].1.clone(), out),
            }
        }
    }
    request_preview(state, out);
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
        Items::Files(_) | Items::Buffers(_) | Items::Commands | Items::Backlinks(_) => {
            let narrowing = Narrowing::between(&previous, &picker.query);
            picker::filter(picker, &mut state.matcher, narrowing);
        }
    }
}

/// Starts loading the highlighted picker item's file unless it is open or cached.
pub fn request_preview(state: &mut State, out: &mut Vec<Effect>) {
    let Overlay::Picker(open) = &mut state.overlay else {
        return;
    };
    let path = match picker::preview_source(open) {
        Some(PreviewSource::File { path, .. }) => path.clone(),
        Some(PreviewSource::Buffer(_)) | None => return,
    };
    let full = path.to_path(state.root.path());
    let buffered = state.buffers.values().any(|buffer| buffer.path == full);
    match (buffered, open.previews.get(&path)) {
        (true, _) | (false, Some(_)) => {}
        (false, None) => {
            let slot = open.previews.next;
            open.previews.slots[slot] = Some((path.clone(), Preview::Loading));
            open.previews.next = (slot + 1) % PREVIEW_SLOTS;
            out.push(Effect::Job(Job::Preview {
                root: state.root.path().to_owned(),
                path,
                languages: state.languages.clone(),
            }));
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Close {
    Allowed,
    Refused,
}

/// Window close from `:q`, the title bar or the compositor; refused while buffers have
/// unsaved edits, leaving the editor focused with a notice.
pub fn request_close(state: &mut State) -> Close {
    match state
        .buffers
        .values()
        .filter(|b| b.content.is_modified())
        .count()
    {
        0 => Close::Allowed,
        count => {
            state.notice = Some(Notice::UnsavedBuffers { count });
            match state.overlay {
                Overlay::Picker(_) => state.overlay = Overlay::None,
                Overlay::None | Overlay::Prompt(_) => {}
            }
            focus_editor(state);
            Close::Refused
        }
    }
}

/// Number of a mouse click within a multi-click.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Click {
    Single,
    Double,
}

/// Tab click: activates tab `index` and focuses its pane; a double click keeps a preview.
pub fn click_tab(state: &mut State, pane: PaneId, index: usize, click: Click) {
    let now = state.layout.tick();
    let Some(tabs) = (state.layout.panes.get_mut(pane)).and_then(|target| target.tabs.as_mut())
    else {
        return;
    };
    tabs.activate(index, now);
    match (click, tabs.preview() == Some(index)) {
        (Click::Double, true) => tabs.keep(),
        (Click::Double, false) | (Click::Single, _) => {}
    }
    state.layout.focus = pane;
    focus_editor(state);
}

/// Mouse focus of a pane, including empty panes and statuslines.
pub fn focus_pane(state: &mut State, pane: PaneId) {
    match state.layout.panes.contains_key(pane) {
        true => state.layout.focus = pane,
        false => return,
    }
    touch(&mut state.layout);
    focus_editor(state);
}

/// Tree click: selects row `index`; a directory toggles, a file opens in the focused pane,
/// and a double click keeps the file's tab.
pub fn click_tree(state: &mut State, index: usize, click: Click, out: &mut Vec<Effect>) {
    let Some(kind) = state.tree.rows.get(index).map(|row| row.kind) else {
        return;
    };
    state.tree.cursor = index;
    state.tree.edit = TreeEdit::None;
    state.sidebar = Sidebar::Files(Focus::Sidebar);
    match (click, kind) {
        (Click::Single, _) => tree(state, TreeCommand::Open, out),
        (Click::Double, RowKind::File) => {
            let path = state.tree.rows[index].path.to_path(state.root.path());
            crate::apply::open(state, path, Arrive::Text, None, OpenAs::Permanent, out);
            focus_editor(state);
        }
        // The first click already toggled the directory.
        (Click::Double, RowKind::Dir(_)) => {}
    }
}

/// Picker click: opens match `index`.
pub fn click_picker(state: &mut State, index: usize, out: &mut Vec<Effect>) {
    let Overlay::Picker(open) = &mut state.overlay else {
        return;
    };
    match index < open.matches.len() {
        true => open.cursor = index,
        false => return,
    }
    picker(state, PickerCommand::Submit, out);
}

/// `space d`: shows the focused tab's file as a diff, or a diff as its file, in the same pane
/// and with the same selection; an existing tab of the other kind is activated.
pub fn toggle_diff(state: &mut State) {
    let focus = state.layout.focus;
    let Some(tabs) = (state.layout.panes.get(focus)).and_then(|pane| pane.tabs.as_ref()) else {
        state.notice = Some(Notice::NoBuffer);
        return;
    };
    let item = &tabs.items()[tabs.active()];
    let (selection, anchor) = (
        item.view.text().selection.clone(),
        item.view.text().scroll.anchor,
    );
    let key = match item.view.key() {
        TabKey::Text(buffer) | TabKey::Rendered(buffer) => TabKey::Diff(buffer),
        TabKey::Diff(buffer) => TabKey::Text(buffer),
    };
    let open = match tabs.preview() == Some(tabs.active()) {
        true => OpenAs::Preview,
        false => OpenAs::Permanent,
    };
    show(state, focus, key, open);
    match state.layout.panes.get_mut(focus).and_then(Pane::active_mut) {
        Some(view) if view.key() == key => {
            let text = view.text_mut();
            text.selection = selection;
            text.scroll.anchor = anchor;
            text.scroll.rows = 0;
        }
        Some(_) | None => {}
    }
    scroll::follow_cursor(state, focus);
}

/// Changes sidebar click on row `row`: a header toggles its group, an entry opens like `l`, and a
/// double click keeps its tab.
pub fn click_changes(state: &mut State, row: usize, click: Click, out: &mut Vec<Effect>) {
    let changes = match &state.git.repo {
        Repo::Found { snapshot, .. } => &snapshot.changes[..],
        Repo::Unknown | Repo::None => &[],
    };
    state.sidebar = Sidebar::Changes(Focus::Sidebar);
    match changes::list_row(changes, &state.git.folds, row) {
        Some(ListRow::Header { .. }) => {
            state.git.cursor = changes::cursor_at(changes, &state.git.folds, row);
            changes::list(state, ChangesCommand::Open, out);
        }
        Some(ListRow::Entry(index)) => {
            let open = match click {
                Click::Single => OpenAs::Preview,
                Click::Double => OpenAs::Permanent,
            };
            changes::open_entry(state, index, open, out);
        }
        None => {}
    }
}

/// A Changes sidebar row button: runs `command` (stage, unstage, discard) on that row.
pub fn changes_action(
    state: &mut State,
    row: usize,
    command: ChangesCommand,
    out: &mut Vec<Effect>,
) {
    let changes = match &state.git.repo {
        Repo::Found { snapshot, .. } => &snapshot.changes[..],
        Repo::Unknown | Repo::None => &[],
    };
    state.git.cursor = changes::cursor_at(changes, &state.git.folds, row);
    changes::list(state, command, out);
}

/// Directory a create or drop on `row` targets: the row itself, or a file's parent.
fn row_dir(row: Option<&tree::Row>) -> &str {
    match row {
        Some(row) => match row.kind {
            RowKind::Dir(_) => row.path.as_str(),
            RowKind::File => row.path.parent().unwrap_or(""),
        },
        None => "",
    }
}

/// Opens the create prompt prefilled with `dir`.
fn begin_create(state: &mut State, dir: &str, kind: NewEntry) {
    let line = match dir.is_empty() {
        true => String::new(),
        false => format!("{dir}/"),
    };
    state.tree.edit = TreeEdit::Create { line, kind };
}

/// A file tree context menu entry chosen for `target`.
pub fn tree_action(
    state: &mut State,
    target: TreeTarget,
    action: TreeAction,
    out: &mut Vec<Effect>,
) {
    state.sidebar = Sidebar::Files(Focus::Sidebar);
    match target {
        TreeTarget::Row(index) if index < state.tree.rows.len() => state.tree.cursor = index,
        TreeTarget::Row(_) | TreeTarget::Root => {}
    }
    match (target, action) {
        (TreeTarget::Root, TreeAction::NewFile) => begin_create(state, "", NewEntry::Infer),
        (TreeTarget::Root, TreeAction::NewFolder) => begin_create(state, "", NewEntry::Folder),
        (TreeTarget::Root, TreeAction::Rename | TreeAction::Delete) => {}
        (TreeTarget::Row(_), TreeAction::NewFile) => tree(state, TreeCommand::Create, out),
        (TreeTarget::Row(_), TreeAction::NewFolder) => tree(state, TreeCommand::CreateFolder, out),
        (TreeTarget::Row(_), TreeAction::Rename) => tree(state, TreeCommand::Rename, out),
        (TreeTarget::Row(_), TreeAction::Delete) => tree(state, TreeCommand::Delete, out),
    }
}

/// Moves `from` into the directory of `onto` (a file's parent, or the root).
pub fn move_entry(state: &mut State, from: RelPath, onto: TreeTarget, out: &mut Vec<Effect>) {
    let row = match onto {
        TreeTarget::Row(index) => state.tree.rows.get(index),
        TreeTarget::Root => None,
    };
    let dir = row_dir(row);
    let to = match dir.is_empty() {
        true => RelPath::parse(from.name()),
        false => RelPath::parse(&format!("{dir}/{}", from.name())),
    };
    match to {
        Ok(to) if to == from => {}
        Ok(_) if RelPath(dir.to_owned()).is_within(from.as_str()) => {
            state.notice = Some(Notice::MoveIntoItself)
        }
        Ok(to) => out.push(Effect::Job(Job::Fs {
            root: state.root.path().to_owned(),
            op: FsOp::Rename { from, to },
        })),
        Err(_) => {
            state.notice = Some(Notice::InvalidPath {
                input: from.as_str().to_owned(),
            })
        }
    }
}
