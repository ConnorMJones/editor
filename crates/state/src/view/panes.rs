use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use model::command::{
    ChangesCommand, Command, Cycle, PickerCommand, PickerKind, Shown, TreeCommand, WindowCommand,
};
use model::config::OpenAs;
use model::geom::{self, Point};
use model::notice::Notice;

use model::sheet::Viewport;
use model::window::{Axis, Side};
use nucleo::Matcher;

use ink::doc::{PageIndex, Underlay};

use crate::apply::{Force, apply};
use crate::drawing::InkTabs;
use crate::input::picker::{
    self, Grep, Items, Narrowing, PALETTE, PREVIEW_SLOTS, Picker, Preview, PreviewSource,
    Referrers, Search,
};
use crate::insert::{self, ImageFrom};
use crate::job::{FsOp, Job};
use crate::project::Project;
use crate::project::backlinks::{self, Build};
use crate::project::files::{FileIndex, Indexed};
use crate::project::path::TextKind;
use crate::project::path::{Folder, RelPath};
use crate::render::Opening;
use crate::sidebar::changes::{self, ListRow};
use crate::sidebar::tree::{
    self, Dir, EntryKind, Fold, Listing, NewEntry, RowKind, TreeAction, TreeEdit, TreeTarget,
};
use crate::text::buffer::{BufferId, Position, Target};
use crate::view::layout::{CloseError, Layout, NotInLayout, Pane, PaneId, Place, Tabs, Tick};
use crate::view::tab::{NoMode, Opened, Tab, TextTabs};
use crate::{Effect, Files, Focus, Overlay, Sidebar, State};

/// How far from an edge a drop splits, as a part of the pane along that axis.
const EDGE: f32 = 0.25;

/// The mode a shown tab arrives in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Arrive {
    /// As it was; a new tab opens in its kind's first mode.
    AsIs,
    /// In this mode when the file's kind has it, else as it was.
    In(Shown),
}

/// Zed's preview rules; a modified file opens permanently and a modified preview stays.
pub fn show(
    state: &mut State,
    pane: PaneId,
    opened: Opened,
    arrive: Arrive,
    open: OpenAs,
) -> &mut Tab {
    let open = match state.is_modified(opened) {
        true => OpenAs::Permanent,
        false => open,
    };
    let now = state.layout.tick();
    let target = state.layout.pane(pane);
    match target.active_text().map(TextTabs::text) {
        Some(view) => state.buffers[view.buffer()].doc.commit(&view.selection),
        None => {}
    }
    let tab = Tab::open(opened, state);
    let preview = (state.layout.pane(pane).tabs.as_ref())
        .and_then(|tabs| tabs.preview().map(|index| tabs.items()[index].tab.opened()));
    let keep = preview.is_some_and(|preview| state.is_modified(preview));
    let target = state.layout.pane(pane);
    let displaced = match &mut target.tabs {
        Some(tabs) => {
            match keep {
                true => tabs.keep(),
                false => {}
            }
            tabs.open(tab, open, now)
        }
        None => {
            target.tabs = Some(Tabs::new(tab, open, now));
            None
        }
    };
    // The replaced preview's file goes once no pane shows it.
    match displaced.map(|item| item.tab.opened()) {
        Some(displaced) => match state.is_modified(displaced) {
            false => state.release(displaced),
            true => {}
        },
        None => {}
    }
    let State {
        layout, buffers, ..
    } = state;
    let tab = (layout.pane(pane).active_mut()).expect("the pane shows the tab it just opened");
    match (arrive, &mut *tab) {
        (Arrive::In(shown), Tab::Text(text)) if shown != text.shown() => {
            let rope = buffers[text.buffer()].doc.text.slice(..);
            let line = rope.char_to_line(text.text().selection.primary().cursor(rope));
            match text.enter(shown, Opening::Line(line)) {
                Ok(()) | Err(NoMode) => {}
            }
        }
        (Arrive::In(_) | Arrive::AsIs, Tab::Text(_) | Tab::Ink(_) | Tab::Image(_)) => {}
    }
    tab
}

/// Drops the file with its last tab, unless unsaved changes refuse it.
pub fn close_tab(state: &mut State, pane: PaneId, index: usize, force: Force) {
    let Some(opened) = (state.layout.panes().get(pane))
        .and_then(|target| target.items().get(index))
        .map(|item| item.tab.opened())
    else {
        return;
    };
    let elsewhere = state.shown(opened, Some((pane, index)));
    let modified = state.is_modified(opened);
    match (elsewhere, modified, force) {
        (false, true, Force::No) => {
            state.notice = Some(Notice::UnsavedBuffers { count: 1 });
            return;
        }
        (true, _, _) | (false, false, _) | (false, true, Force::Yes) => {}
    }
    state.layout.remove_tab(pane, index);
    state.release(opened);
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
            let side = match axis {
                Axis::Row => Side::Right,
                Axis::Column => Side::Down,
            };
            match layout.split_copy(layout.focus(), side) {
                Some(_) => {}
                None => state.notice = Some(Notice::NothingToSplit),
            }
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

pub fn focus_editor(state: &mut State) {
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
                                .filter(|item| item.tab.opened() == Opened::Text(id))
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
                PickerKind::Images => Items::Images(match &state.project.index {
                    FileIndex::Ready(Indexed { files, .. }) => picker::images(files),
                    FileIndex::Loading { .. } => Vec::new(),
                }),
                PickerKind::Grep => Items::Grep(Grep {
                    generation: state.grep.load(Ordering::Relaxed),
                    search: Search::Idle,
                    hits: Vec::new(),
                }),
                PickerKind::Commands => Items::Commands,
                PickerKind::Outline => {
                    let headings = match state.layout.focused().active() {
                        Some(Tab::Ink(InkTabs::Pdf(tab))) => {
                            match state.pdfs[tab.file()].edited.drawing().underlay() {
                                Underlay::Pdf(pdf) => pdf.document.outline(),
                                Underlay::Blank => Vec::new(),
                            }
                        }
                        Some(Tab::Ink(_) | Tab::Text(_) | Tab::Image(_)) | None => {
                            return state.notice = Some(Notice::NotAPdf);
                        }
                    };
                    match headings.is_empty() {
                        true => return state.notice = Some(Notice::NoOutline),
                        false => Items::Outline(headings),
                    }
                }
                PickerKind::Backlinks => {
                    let Some(buffer) = (state.layout.focused().active_text())
                        .map(|view| &state.buffers[view.buffer()])
                    else {
                        return state.notice = Some(Notice::NoBuffer);
                    };
                    let path = buffer.path.clone();
                    let note = match (buffer.kind, RelPath::new(state.project.root.path(), &path)) {
                        (TextKind::Markdown, Ok(note)) => note,
                        (TextKind::Markdown | TextKind::Typst | TextKind::Code, _) => {
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
        Items::Images(images) => {
            let path = images[index].to_path(state.project.root.path());
            focus_editor(state);
            match (ImageFrom::file(path), insert::into_focused(state, None)) {
                (Ok(image), Ok(into)) => insert::insert(state, into, image, out),
                (Err(notice), _) | (_, Err(notice)) => state.notice = Some(notice),
            }
        }
        Items::Buffers(buffers) => {
            let focus = state.layout.focus();
            let buffer = buffers[index].0;
            show(
                state,
                focus,
                Opened::Text(buffer),
                Arrive::AsIs,
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
        Items::Outline(headings) => {
            let State {
                layout,
                pdfs,
                notice,
                ..
            } = state;
            match (layout.focused_mut().active_mut(), headings[index].page) {
                (Some(Tab::Ink(InkTabs::Pdf(tab))), Some(page)) => {
                    let stack = pdfs[tab.file()].edited.drawing().stack();
                    let viewport = tab.hand.last.map(|measure| measure.viewport);
                    tab.view.go_to(stack, PageIndex(page), viewport);
                }
                (Some(Tab::Ink(InkTabs::Pdf(_))), None) => *notice = Some(Notice::NothingToFollow),
                (Some(Tab::Ink(_) | Tab::Text(_) | Tab::Image(_)) | None, _) => {}
            }
            focus_editor(state);
        }
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
        Items::Files(_)
        | Items::Images(_)
        | Items::Buffers(_)
        | Items::Commands
        | Items::Backlinks(_)
        | Items::Outline(_) => {
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
    let unsaved = (state.buffers.values())
        .filter(|b| b.doc.is_modified())
        .count()
        + (state.pageds.values())
            .filter(|file| file.is_modified())
            .count()
        + (state.boards.values())
            .filter(|file| file.is_modified())
            .count();
    match unsaved {
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
    /// From the host's count of quick clicks; a second click or later is a double.
    pub fn of(count: usize) -> Click {
        match count {
            0 | 1 => Click::Single,
            _ => Click::Double,
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
    // A folder toggles on every click, however quickly they follow each other.
    match (click, kind) {
        (_, RowKind::Dir(_)) | (Click::Single, RowKind::File) => {
            tree(state, TreeCommand::Open, out)
        }
        (Click::Double, RowKind::File) => {
            let path = state.tree.rows[index]
                .path
                .to_path(state.project.root.path());
            crate::apply::open(state, path, Arrive::AsIs, None, OpenAs::Permanent, out);
            focus_editor(state);
        }
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

/// Something dragged over the panes: a file from the tree or the Changes list, or a tab.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Dragged {
    /// A file in the project, from the tree or the Changes list.
    File(RelPath),
    /// A file dropped from outside the editor, such as a file manager.
    Outside(PathBuf),
    Tab {
        pane: PaneId,
        index: usize,
    },
}

/// Where a drag is dropped: on a pane's body, or on its tab bar before tab `at`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum DropOn {
    Pane { pane: PaneId, zone: Zone },
    Tabs { pane: PaneId, at: usize },
}

/// A point on a pane drawn at `viewport`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PanePoint {
    pub at: Point<geom::Pane>,
    pub viewport: Viewport,
}

/// The part of a pane's body a drop lands in: the middle, where a drop knows its point, or near
/// an edge, which splits that way.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Zone {
    Centre(PanePoint),
    Edge(Side),
}

impl Zone {
    /// The zone `point` falls in: the nearest edge within its band, else the centre.
    pub fn at(point: PanePoint) -> Zone {
        let PanePoint { at, viewport } = point;
        let (x, y) = (at.x / viewport.width.get(), at.y / viewport.height.get());
        [
            (Side::Left, x),
            (Side::Right, 1.0 - x),
            (Side::Up, y),
            (Side::Down, 1.0 - y),
        ]
        .into_iter()
        .filter(|(_, distance)| *distance < EDGE)
        .min_by(|(_, a), (_, b)| a.total_cmp(b))
        .map_or(Zone::Centre(point), |(side, _)| Zone::Edge(side))
    }
}

/// Opens a dropped file or moves a dropped tab; a pane closed since the drag began takes nothing.
pub fn dropped(state: &mut State, dragged: Dragged, on: DropOn, out: &mut Vec<Effect>) {
    match dragged {
        Dragged::Tab { pane: from, index } => {
            let to = match on {
                DropOn::Pane {
                    pane,
                    zone: Zone::Centre(_),
                } => Place::Into { pane, at: None },
                DropOn::Pane {
                    pane,
                    zone: Zone::Edge(side),
                } => Place::Beside { pane, side },
                DropOn::Tabs { pane, at } => Place::Into { pane, at: Some(at) },
            };
            match state.layout.move_tab(from, index, to) {
                Some(_) => {
                    touch(&mut state.layout);
                    focus_editor(state);
                }
                None => {}
            }
        }
        Dragged::File(file) => {
            let path = file.to_path(state.project.root.path());
            dropped_file(state, path, on, out)
        }
        Dragged::Outside(path) => dropped_file(state, path, on, out),
    }
}

/// A file dropped on a pane: an image on a drawing or a note goes into it; anything else opens.
fn dropped_file(state: &mut State, path: PathBuf, on: DropOn, out: &mut Vec<Effect>) {
    let (into, point) = match on {
        // A pane with nothing to copy beside it takes the file itself.
        DropOn::Pane {
            pane,
            zone: Zone::Edge(side),
        } => (state.layout.split_copy(pane, side).unwrap_or(pane), None),
        DropOn::Pane {
            pane,
            zone: Zone::Centre(point),
        } => (pane, Some(point)),
        DropOn::Tabs { pane, .. } => (pane, None),
    };
    match state.layout.set_focus(into) {
        Ok(()) => {}
        Err(NotInLayout(_)) => return,
    }
    focus_editor(state);
    // An image dropped on a drawing or a note goes into it; anything else opens.
    let image = (point.zip(ImageFrom::file(path.clone()).ok()))
        .map(|(point, image)| insert::into_focused(state, Some(point)).map(|into| (into, image)));
    match image {
        Some(Ok((into, image))) => insert::insert(state, into, image, out),
        Some(Err(Notice::NoImagesHere)) | None => {
            crate::apply::open(state, path, Arrive::AsIs, None, OpenAs::Permanent, out)
        }
        Some(Err(notice)) => state.notice = Some(notice),
    }
}
