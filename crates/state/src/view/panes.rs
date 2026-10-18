use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use helix_core::Selection;
use model::command::{
    ChangesCommand, Command, Cycle, PickerCommand, PickerKind, Shown, TreeCommand, WindowCommand,
};
use model::config::OpenAs;
use model::notice::Notice;
use nucleo::Matcher;

use crate::apply::{Force, apply};
use crate::input::picker::{
    self, Grep, Items, Narrowing, PALETTE, PREVIEW_SLOTS, Picker, Preview, PreviewSource,
    Referrers, Search,
};
use crate::job::{FsOp, Job};
use crate::project::Project;
use crate::project::backlinks::{self, Build};
use crate::project::files::{FileIndex, Indexed};
use crate::project::path::{Folder, RelPath};
use crate::render::Opening;
use crate::render::markdown::Format;
use crate::sidebar::changes::{self, ListRow};
use crate::sidebar::tree::{
    self, Dir, EntryKind, Fold, Listing, NewEntry, RowKind, TreeAction, TreeEdit, TreeTarget,
};
use crate::text::buffer::{BufferId, Position, Target};
use crate::view::layout::{CloseError, Layout, NotInLayout, Pane, PaneId, Tabs, TextView, Tick};
use crate::view::tab::{NoMode, Tab};
use crate::{Effect, Files, Focus, Overlay, Sidebar, State, view::scroll};

/// The mode a shown tab arrives in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Arrive {
    /// As it was; a new tab opens in its kind's first mode.
    AsIs,
    /// In this mode when the file's kind has it, else as it was.
    In(Shown),
}

/// Zed's preview rules; a modified buffer opens permanently and a modified preview stays.
pub fn show(
    state: &mut State,
    pane: PaneId,
    buffer: BufferId,
    arrive: Arrive,
    open: OpenAs,
) -> &mut Tab {
    let open = match state.buffers[buffer].doc.is_modified() {
        true => OpenAs::Permanent,
        false => open,
    };
    let now = state.layout.tick();
    let State {
        layout, buffers, ..
    } = state;
    let target = layout.pane(pane);
    match target.active().map(Tab::text) {
        Some(view) => buffers[view.buffer()].doc.commit(&view.selection),
        None => {}
    }
    let tab = Tab::open(buffer, &buffers[buffer].path);
    let displaced = match &mut target.tabs {
        Some(tabs) => {
            let preview = tabs.preview().map(|p| tabs.items()[p].tab.buffer());
            match preview.map(|b| buffers[b].doc.is_modified()) {
                Some(true) => tabs.keep(),
                Some(false) | None => {}
            }
            tabs.open(tab, open, now)
        }
        None => {
            target.tabs = Some(Tabs::new(tab, open, now));
            None
        }
    };
    // The replaced preview's buffer goes once no pane shows it.
    match displaced.map(|item| item.tab.buffer()) {
        Some(displaced) => {
            let elsewhere = (layout.panes().values()).any(|pane| {
                pane.items()
                    .iter()
                    .any(|item| item.tab.buffer() == displaced)
            });
            match (elsewhere, buffers[displaced].doc.is_modified()) {
                (false, false) => {
                    buffers.remove(displaced);
                }
                (false, true) | (true, _) => {}
            }
        }
        None => {}
    }
    let tab = (layout.pane(pane).active_mut()).expect("the pane shows the tab it just opened");
    match arrive {
        Arrive::In(shown) if shown != tab.shown() => {
            let rope = buffers[buffer].doc.text.slice(..);
            let line = rope.char_to_line(tab.text().selection.primary().cursor(rope));
            match tab.enter(shown, Opening::Line(line)) {
                Ok(()) | Err(NoMode) => {}
            }
        }
        Arrive::In(_) | Arrive::AsIs => {}
    }
    tab
}

/// Where the cursor of a shown text tab goes.
pub struct Place {
    pub selection: Selection,
    /// Char to keep at the top; `None` scrolls just enough to show the cursor.
    pub anchor: Option<usize>,
}

/// As `show`, then puts the cursor at `place` and scrolls to it.
pub fn show_at(
    state: &mut State,
    pane: PaneId,
    buffer: BufferId,
    arrive: Arrive,
    open: OpenAs,
    place: Place,
) -> &mut TextView {
    let text = show(state, pane, buffer, arrive, open).text_mut();
    text.selection = place.selection;
    match place.anchor {
        Some(anchor) => {
            text.scroll.anchor = anchor;
            text.scroll.rows = 0;
        }
        None => {}
    }
    scroll::follow_cursor(state, pane);
    (state.layout.pane(pane).active_mut())
        .expect("the pane shows the tab it just placed")
        .text_mut()
}

/// Drops the buffer with its last view, unless unsaved edits refuse it.
pub fn close_tab(state: &mut State, pane: PaneId, index: usize, force: Force) {
    let Some(buffer) = (state.layout.panes().get(pane))
        .and_then(|target| target.items().get(index))
        .map(|item| item.tab.buffer())
    else {
        return;
    };
    // The same pane may also show the buffer, as text beside its diff.
    let elsewhere = state.layout.panes().iter().any(|(id, other)| {
        other
            .items()
            .iter()
            .enumerate()
            .any(|(at, item)| item.tab.buffer() == buffer && (id, at) != (pane, index))
    });
    let modified = state.buffers[buffer].doc.is_modified();
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

fn touch(layout: &mut Layout) {
    let now = layout.tick();
    match layout.focused_mut().tabs.as_mut() {
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
            let tabs = layout
                .focused()
                .active()
                .map(|view| Tabs::split_copy(view.clone(), now));
            layout.split(axis, Pane { tabs });
        }
        WindowCommand::Focus(side) => {
            layout.focus_side(side);
        }
        WindowCommand::FocusNext => {
            layout.focus_next();
        }
        WindowCommand::Close => match layout.close(layout.focus()) {
            Ok(()) => {}
            Err(CloseError::LastPane) => apply(state, Command::Quit, out),
            Err(CloseError::NotInLayout) => unreachable!("the focus is a leaf of the layout"),
        },
        WindowCommand::Only => layout.only(),
    }
    touch(&mut state.layout);
    focus_editor(state);
}

pub fn cycle_tab(state: &mut State, cycle: Cycle) {
    let now = state.layout.tick();
    let Some(tabs) = state.layout.focused_mut().tabs.as_mut() else {
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
        Sidebar::Files(_) => Sidebar::Files(Files::Editor),
        Sidebar::Changes(_) => Sidebar::Changes(Focus::Editor),
        Sidebar::Hidden => Sidebar::Hidden,
    };
}

pub fn tree(state: &mut State, command: TreeCommand, out: &mut Vec<Effect>) {
    let root = state.project.root.path().to_owned();
    let selected = state.tree.rows.get(state.tree.cursor).cloned();
    match command {
        TreeCommand::Toggle => {
            state.sidebar = match state.sidebar {
                Sidebar::Hidden | Sidebar::Files(Files::Editor) | Sidebar::Changes(_) => {
                    Sidebar::Files(Files::Tree)
                }
                Sidebar::Files(Files::Tree | Files::Edit(_)) => Sidebar::Hidden,
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
                    crate::apply::open(state, path, Arrive::AsIs, None, open, out);
                    focus_editor(state);
                }
                RowKind::Dir(Fold::Expanded) => {
                    if let Some(dir) = state.tree.dirs.get_mut(row.path.as_str()) {
                        dir.fold = Fold::Collapsed;
                    }
                    tree::rebuild(&mut state.tree);
                }
                RowKind::Dir(Fold::Collapsed) => {
                    match state.tree.dirs.get_mut(row.path.as_str()) {
                        Some(dir) => dir.fold = Fold::Expanded,
                        None => {
                            state.tree.dirs.insert(
                                Folder::Sub(row.path.clone()),
                                Dir {
                                    fold: Fold::Expanded,
                                    listing: Listing::Loading,
                                },
                            );
                            out.push(Effect::Job(Job::ListDir {
                                root,
                                dir: Folder::Sub(row.path),
                            }));
                        }
                    }
                    tree::rebuild(&mut state.tree);
                }
            },
        },
        TreeCommand::Close => match selected {
            None => {}
            Some(row) => match row.kind {
                RowKind::Dir(Fold::Expanded) => {
                    if let Some(dir) = state.tree.dirs.get_mut(row.path.as_str()) {
                        dir.fold = Fold::Collapsed;
                    }
                    tree::rebuild(&mut state.tree);
                }
                // A top-level row has no folder row to move to.
                RowKind::File | RowKind::Dir(Fold::Collapsed) => {
                    let folder = row.path.folder_path();
                    match state
                        .tree
                        .rows
                        .iter()
                        .position(|r| r.path.as_str() == folder)
                    {
                        Some(index) => state.tree.cursor = index,
                        None => {}
                    }
                }
            },
        },
        TreeCommand::Create => {
            begin_create(state, row_dir(selected.as_ref()).as_str(), NewEntry::Infer)
        }
        TreeCommand::CreateFolder => {
            begin_create(state, row_dir(selected.as_ref()).as_str(), NewEntry::Folder)
        }
        TreeCommand::Rename => match selected {
            Some(row) => {
                state.sidebar = Sidebar::Files(Files::Edit(TreeEdit::Rename {
                    line: row.path.as_str().to_owned(),
                    from: row.path,
                }))
            }
            None => {}
        },
        TreeCommand::Delete => match selected {
            Some(row) => {
                state.sidebar = Sidebar::Files(Files::Edit(TreeEdit::Delete {
                    path: row.path,
                    kind: match row.kind {
                        RowKind::File => EntryKind::File,
                        RowKind::Dir(_) => EntryKind::Dir,
                    },
                    line: String::new(),
                }))
            }
            None => {}
        },
        TreeCommand::EditInsert(c) => match &mut state.sidebar {
            Sidebar::Files(Files::Edit(edit)) => edit.line_mut().push(c),
            Sidebar::Hidden | Sidebar::Files(Files::Editor | Files::Tree) | Sidebar::Changes(_) => {
            }
        },
        TreeCommand::EditBackspace => match &mut state.sidebar {
            Sidebar::Files(Files::Edit(edit)) => {
                edit.line_mut().pop();
            }
            Sidebar::Hidden | Sidebar::Files(Files::Editor | Files::Tree) | Sidebar::Changes(_) => {
            }
        },
        TreeCommand::EditCancel => match state.sidebar {
            Sidebar::Files(Files::Edit(_)) => state.sidebar = Sidebar::Files(Files::Tree),
            Sidebar::Hidden | Sidebar::Files(Files::Editor | Files::Tree) | Sidebar::Changes(_) => {
            }
        },
        TreeCommand::EditSubmit => {
            let edit = match std::mem::replace(&mut state.sidebar, Sidebar::Files(Files::Tree)) {
                Sidebar::Files(Files::Edit(edit)) => edit,
                other @ (Sidebar::Hidden
                | Sidebar::Files(Files::Editor | Files::Tree)
                | Sidebar::Changes(_)) => return state.sidebar = other,
            };
            let op = match edit {
                TreeEdit::Create { line, kind } => {
                    let kind = match (kind, line.ends_with('/')) {
                        (NewEntry::Folder, _) | (NewEntry::Infer, true) => EntryKind::Dir,
                        (NewEntry::Infer, false) => EntryKind::File,
                    };
                    match RelPath::parse(line.trim_end_matches('/')) {
                        Ok(path) => FsOp::Create { path, kind },
                        Err(_) => {
                            state.notice = Some(Notice::InvalidPath { input: line });
                            return;
                        }
                    }
                }
                TreeEdit::Rename { from, line } => match RelPath::parse(line.trim_end_matches('/'))
                {
                    Ok(to) if to == from => return,
                    Ok(to) => FsOp::Rename { from, to },
                    Err(_) => {
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
    let State {
        overlay,
        matcher,
        grep,
        project,
        ..
    } = state;
    match (command, overlay) {
        (PickerCommand::Open(kind), _) => {
            let items = match kind {
                // An open file picker fills when the first walk finishes.
                PickerKind::Files => Items::Files(match &state.project.index {
                    FileIndex::Ready(Indexed { files, .. }) => files.clone(),
                    FileIndex::Loading { .. } => Arc::new(Vec::new()),
                }),
                // Most recently activated first; buffers no pane shows come last.
                PickerKind::Buffers => {
                    let mut recent: Vec<(Tick, BufferId)> = state
                        .buffers
                        .keys()
                        .map(|id| {
                            let used = (state.layout.panes().values())
                                .flat_map(Pane::items)
                                .filter(|item| item.tab.buffer() == id)
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
                                let label =
                                    path.strip_prefix(state.project.root.path()).unwrap_or(path);
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
                    let Some(path) = state
                        .layout
                        .focused()
                        .active()
                        .map(|view| state.buffers[view.buffer()].path.clone())
                    else {
                        return state.notice = Some(Notice::NoBuffer);
                    };
                    let note = match (
                        Format::of(&path),
                        RelPath::new(state.project.root.path(), &path),
                    ) {
                        (Some(Format::Markdown), Ok(note)) => note,
                        (Some(Format::Markdown | Format::Typst) | None, _) => {
                            return state.notice = Some(Notice::NotANote { path });
                        }
                    };
                    backlinks::ensure(state, out);
                    let State {
                        project: Project { index, root, .. },
                        buffers,
                        documents,
                        ..
                    } = state;
                    let (search, hits) = match index {
                        FileIndex::Ready(indexed) => {
                            backlinks::sync_buffers(
                                &mut indexed.backlinks,
                                root.path(),
                                buffers,
                                documents,
                                out,
                            );
                            match indexed.backlinks.build {
                                Build::Ready => (Search::Done, backlinks::hits(indexed, &note)),
                                Build::Unbuilt | Build::Building { .. } => {
                                    (Search::Running, Vec::new())
                                }
                            }
                        }
                        FileIndex::Loading { .. } => (Search::Running, Vec::new()),
                    };
                    Items::Backlinks(Referrers { note, search, hits })
                }
            };
            state.overlay = Overlay::Picker(Picker::new(items));
        }
        (PickerCommand::Cancel, overlay) => *overlay = Overlay::None,
        (PickerCommand::Submit, overlay) => match std::mem::replace(overlay, Overlay::None) {
            Overlay::Picker(picker) => submit(state, picker, out),
            overlay @ (Overlay::None | Overlay::Prompt(_) | Overlay::Confirm(_)) => {
                state.overlay = overlay
            }
        },
        (PickerCommand::Insert(c), Overlay::Picker(picker)) => {
            edit_query(picker, matcher, grep, project, |query| query.push(c), out)
        }
        (PickerCommand::Backspace, Overlay::Picker(picker)) => edit_query(
            picker,
            matcher,
            grep,
            project,
            |query| {
                query.pop();
            },
            out,
        ),
        (PickerCommand::Next, Overlay::Picker(picker)) => move_cursor(picker, Cycle::Next),
        (PickerCommand::Previous, Overlay::Picker(picker)) => move_cursor(picker, Cycle::Previous),
        // The picker closed before a queued key arrived.
        (
            PickerCommand::Insert(_)
            | PickerCommand::Backspace
            | PickerCommand::Next
            | PickerCommand::Previous,
            Overlay::None | Overlay::Prompt(_) | Overlay::Confirm(_),
        ) => {}
    }
    request_preview(state, out);
}

/// Opens the item under the picker's cursor; an empty list opens nothing.
fn submit(state: &mut State, picker: Picker, out: &mut Vec<Effect>) {
    let Some(&index) = picker.matches.get(picker.cursor) else {
        return;
    };
    let index = index as usize;
    match picker.items {
        Items::Files(files) => {
            let path = PathBuf::from(files[index].as_str());
            let open = state.tab_config.open_from_picker;
            crate::apply::open(state, path, Arrive::AsIs, None, open, out);
            focus_editor(state);
        }
        Items::Buffers(buffers) => {
            let focus = state.layout.focus();
            let buffer = buffers[index].0;
            show(state, focus, buffer, Arrive::AsIs, OpenAs::Permanent);
            focus_editor(state);
        }
        Items::Grep(Grep { hits, .. }) | Items::Backlinks(Referrers { hits, .. }) => {
            let hit = &hits[index];
            let open = state.tab_config.open_from_picker;
            crate::apply::open(
                state,
                PathBuf::from(hit.path.as_str()),
                Arrive::AsIs,
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

fn move_cursor(picker: &mut Picker, cycle: Cycle) {
    let count = picker.matches.len().max(1);
    picker.cursor = match cycle {
        Cycle::Next => (picker.cursor + 1) % count,
        Cycle::Previous => (picker.cursor + count - 1) % count,
    };
}

fn edit_query(
    picker: &mut Picker,
    matcher: &mut Matcher,
    current: &Arc<AtomicU64>,
    project: &Project,
    edit: impl FnOnce(&mut String),
    out: &mut Vec<Effect>,
) {
    let previous = picker.query.clone();
    edit(&mut picker.query);
    match &mut picker.items {
        Items::Grep(grep) => {
            let generation = current.fetch_add(1, Ordering::Relaxed) + 1;
            grep.generation = generation;
            grep.hits.clear();
            picker.matches.clear();
            picker.cursor = 0;
            let files = match (&project.index, picker.query.is_empty()) {
                (FileIndex::Ready(Indexed { files, .. }), false) => files.clone(),
                (FileIndex::Ready(_), true) | (FileIndex::Loading { .. }, _) => {
                    grep.search = Search::Idle;
                    return;
                }
            };
            grep.search = Search::Running;
            out.push(Effect::Job(Job::Grep {
                root: project.root.path().to_owned(),
                files,
                regex: picker::grep_regex(&picker.query),
                generation,
                current: current.clone(),
            }));
        }
        Items::Files(_) | Items::Buffers(_) | Items::Commands | Items::Backlinks(_) => {
            let narrowing = Narrowing::between(&previous, &picker.query);
            picker::filter(picker, matcher, narrowing);
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
    let full = path.to_path(state.project.root.path());
    let buffered = state.buffers.values().any(|buffer| buffer.path == full);
    match (buffered, open.previews.get(&path)) {
        (true, _) | (false, Some(_)) => {}
        (false, None) => {
            let slot = open.previews.next;
            open.previews.slots[slot] = Some((path.clone(), Preview::Loading));
            open.previews.next = (slot + 1) % PREVIEW_SLOTS;
            out.push(Effect::Job(Job::Preview {
                root: state.project.root.path().to_owned(),
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

/// Refused while buffers have unsaved edits, with a notice.
pub fn request_close(state: &mut State) -> Close {
    match state
        .buffers
        .values()
        .filter(|b| b.doc.is_modified())
        .count()
    {
        0 => Close::Allowed,
        count => {
            state.notice = Some(Notice::UnsavedBuffers { count });
            match state.overlay {
                Overlay::Picker(_) => state.overlay = Overlay::None,
                Overlay::None | Overlay::Prompt(_) | Overlay::Confirm(_) => {}
            }
            focus_editor(state);
            Close::Refused
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Click {
    Single,
    Double,
}

impl Click {
    /// From the host's count of quick clicks; only a second click is a double.
    pub fn of(count: usize) -> Click {
        match count {
            2 => Click::Double,
            _ => Click::Single,
        }
    }
}

/// Tab click: activates tab `index` and focuses its pane; a double click keeps a preview.
pub fn click_tab(state: &mut State, pane: PaneId, index: usize, click: Click) {
    let now = state.layout.tick();
    let Some(tabs) = (state.layout.find_pane(pane)).and_then(|target| target.tabs.as_mut()) else {
        return;
    };
    tabs.activate(index, now);
    match (click, tabs.preview() == Some(index)) {
        (Click::Double, true) => tabs.keep(),
        (Click::Double, false) | (Click::Single, _) => {}
    }
    state
        .layout
        .set_focus(pane)
        .expect("the pane holds the clicked tab");
    focus_editor(state);
}

/// Mouse focus of a pane, including empty panes and statuslines.
pub fn focus_pane(state: &mut State, pane: PaneId) {
    match state.layout.set_focus(pane) {
        Ok(()) => {}
        Err(NotInLayout(_)) => return,
    }
    touch(&mut state.layout);
    focus_editor(state);
}

/// A directory toggles, a file opens, and a double click keeps the file's tab.
pub fn click_tree(state: &mut State, index: usize, click: Click, out: &mut Vec<Effect>) {
    let Some(kind) = state.tree.rows.get(index).map(|row| row.kind) else {
        return;
    };
    state.tree.cursor = index;
    state.sidebar = Sidebar::Files(Files::Tree);
    match (click, kind) {
        (Click::Single, _) => tree(state, TreeCommand::Open, out),
        (Click::Double, RowKind::File) => {
            let path = state.tree.rows[index]
                .path
                .to_path(state.project.root.path());
            crate::apply::open(state, path, Arrive::AsIs, None, OpenAs::Permanent, out);
            focus_editor(state);
        }
        // The first click already toggled the directory.
        (Click::Double, RowKind::Dir(_)) => {}
    }
}

pub fn click_picker(state: &mut State, index: usize, out: &mut Vec<Effect>) {
    // The picker closed, or its matches changed, since the frame drew the row.
    match std::mem::replace(&mut state.overlay, Overlay::None) {
        Overlay::Picker(mut picker) if index < picker.matches.len() => {
            picker.cursor = index;
            submit(state, picker, out)
        }
        overlay @ (Overlay::Picker(_)
        | Overlay::None
        | Overlay::Prompt(_)
        | Overlay::Confirm(_)) => state.overlay = overlay,
    }
}

/// A header toggles its group, an entry opens like `l`; a double click keeps its tab.
pub fn click_changes(state: &mut State, row: usize, click: Click, out: &mut Vec<Effect>) {
    let changes = state.project.git.changes();
    state.sidebar = Sidebar::Changes(Focus::Sidebar);
    match changes::rows(changes, &state.changes.folds).nth(row) {
        Some(ListRow::Header { .. }) => {
            state.changes.cursor = changes::cursor_at(changes, &state.changes.folds, row);
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
    let changes = state.project.git.changes();
    state.changes.cursor = changes::cursor_at(changes, &state.changes.folds, row);
    changes::list(state, command, out);
}

/// The folder a create or drop on `row` targets: the row itself, a file's folder, or the root.
fn row_dir(row: Option<&tree::Row>) -> Folder {
    match row {
        Some(row) => match row.kind {
            RowKind::Dir(_) => Folder::Sub(row.path.clone()),
            RowKind::File => row.path.folder(),
        },
        None => Folder::Root,
    }
}

fn begin_create(state: &mut State, dir: &str, kind: NewEntry) {
    let line = match dir.is_empty() {
        true => String::new(),
        false => format!("{dir}/"),
    };
    state.sidebar = Sidebar::Files(Files::Edit(TreeEdit::Create { line, kind }));
}

pub fn tree_action(
    state: &mut State,
    target: TreeTarget,
    action: TreeAction,
    out: &mut Vec<Effect>,
) {
    state.sidebar = Sidebar::Files(Files::Tree);
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
    let to = dir.join(from.name());
    match dir {
        Folder::Sub(dir) if dir.is_within(&from) => state.notice = Some(Notice::MoveIntoItself),
        Folder::Root | Folder::Sub(_) if to == from => {}
        Folder::Root | Folder::Sub(_) => out.push(Effect::Job(Job::Fs {
            root: state.project.root.path().to_owned(),
            op: FsOp::Rename { from, to },
        })),
    }
}
