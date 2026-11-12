use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use model::command::{
    ChangesCommand, Command, Cycle, Direction, PickerCommand, PickerKind, Shown, TabCommand,
    TreeCommand, WindowCommand,
};
use model::config::OpenAs;

use model::notice::Notice;

use model::window::{Axis, Side};
use nucleo::Matcher;

use crate::apply::{Force, apply};
use crate::draft::Edits;
use crate::drawing::InkTabs;
use crate::input::picker::{
    self, AnnotationSearch, Grep, Items, Narrowing, Notebook, Outline, OutlineEntry, PALETTE,
    PREVIEW_SLOTS, Picker, Preview, PreviewSource, Referrers, Search,
};
use crate::insert::{self, ImageFrom};
use crate::job::{FsOp, Job};
use crate::project::Project;
use crate::project::backlinks::{self, Build};
use crate::project::files::{FileIndex, Indexed};
use crate::project::path::TextKind;
use crate::project::path::{Folder, RelPath};
use crate::reading::Landing;
use crate::reading::{Notes, Reader, ReaderId, Viewing};
use crate::render::Opening;
use crate::sidebar::Pick;
use crate::sidebar::changes::{self, EntryKey, ListCursor, ListRow};
use crate::sidebar::tree::{
    self, Dir, EntryKind, Fold, Listing, NewEntry, Removal, Row, RowKind, TreeAction, TreeEdit,
    TreeTarget,
};
use crate::text::buffer::{BufferId, Position, Target};
use crate::view::layout::TabPane;
use crate::view::layout::{
    CloseError, DropOn, Layout, NotInLayout, Pane, PaneId, Place, Tabs, Tick, Zone,
};
use crate::view::tab::{NoMode, Opened, Tab, TextTabs};
use crate::{Effect, Files, Focus, Overlay, Sidebar, State};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Arrive {
    AsIs,
    In(Shown),
}

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
        (
            Arrive::In(_) | Arrive::AsIs,
            Tab::Text(_) | Tab::Ink(_) | Tab::Image(_) | Tab::Book(_) | Tab::JsonCanvas(_),
        ) => {}
    }
    tab
}

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
        WindowCommand::Swap(side) => {
            layout.swap(side);
        }
        WindowCommand::Transpose => layout.transpose(),
        WindowCommand::Resize(resize) => layout.resize(resize),
        WindowCommand::Maximise => layout.toggle_maximise(),
        WindowCommand::MoveTab(side) => match layout.move_tab_to(side) {
            Some(_) => {}
            None => state.notice = Some(Notice::NoBuffer),
        },
        WindowCommand::GotoFile(axis) => {
            let side = match axis {
                Axis::Row => Side::Right,
                Axis::Column => Side::Down,
            };
            match layout.split_copy(layout.focus(), side) {
                Some(_) => crate::project::links::goto_file(state, out),
                None => state.notice = Some(Notice::NothingToSplit),
            }
        }
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

pub fn tab(state: &mut State, command: TabCommand) {
    let now = state.layout.tick();
    let Some(tabs) = state.layout.focused_mut().tabs.as_mut() else {
        return;
    };
    match command {
        TabCommand::Shift(direction) => tabs.shift(direction, now),
        TabCommand::LastUsed => match tabs.last_used() {
            Some(index) => tabs.activate(index, now),
            None => {}
        },
    }
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
        TreeCommand::Up => {
            state.tree.selected.clear();
            state.tree.cursor = state.tree.cursor.saturating_sub(1)
        }
        TreeCommand::Down => {
            state.tree.selected.clear();
            state.tree.cursor = (state.tree.cursor + 1).min(state.tree.rows.len().saturating_sub(1))
        }
        TreeCommand::Extend(direction) => {
            let listed = &mut state.tree;
            let from = listed.cursor;
            let to = match direction {
                Direction::Forward => (from + 1).min(listed.rows.len().saturating_sub(1)),
                Direction::Backward => from.saturating_sub(1),
            };
            let (Some(left), Some(reached)) = (listed.rows.get(from), listed.rows.get(to)) else {
                return;
            };
            let (left, reached) = (left.path.clone(), reached.path.clone());
            match (left, reached) {
                (left, reached) if listed.selected.contains(&reached) => {
                    listed.selected.retain(|path| *path != left)
                }
                (left, reached) => {
                    listed
                        .selected
                        .retain(|path| *path != left && *path != reached);
                    listed.selected.extend([left, reached]);
                }
            }
            listed.cursor = to;
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
                    line: row.path.name().to_owned(),
                    from: row.path,
                }))
            }
            None => {}
        },
        TreeCommand::Delete => {
            let tree = &state.tree;
            let removal = |row: &Row| Removal {
                path: row.path.clone(),
                kind: match row.kind {
                    RowKind::File => EntryKind::File,
                    RowKind::Dir(_) => EntryKind::Dir,
                },
            };
            let within_picked_dir = |row: &Row| {
                (tree.rows.iter()).any(|outer| {
                    matches!(outer.kind, RowKind::Dir(_))
                        && outer.path != row.path
                        && tree.selected.contains(&outer.path)
                        && row.path.is_within(&outer.path)
                })
            };
            let removals: Vec<Removal> = match selected {
                Some(row) if tree.selected.contains(&row.path) => (tree.rows.iter())
                    .filter(|row| tree.selected.contains(&row.path) && !within_picked_dir(row))
                    .map(removal)
                    .collect(),
                Some(row) => vec![removal(&row)],
                None => Vec::new(),
            };
            let mut removals = removals.into_iter();
            match removals.next() {
                Some(first) => {
                    state.sidebar = Sidebar::Files(Files::Edit(TreeEdit::Delete {
                        first,
                        rest: removals.collect(),
                        line: String::new(),
                    }))
                }
                None => {}
            }
        }
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
                TreeEdit::Rename { from, line } => {
                    match RelPath::parse(from.folder().join(line.trim_end_matches('/')).as_str()) {
                        Ok(to) if to == from => return,
                        Ok(to) => FsOp::Rename { from, to },
                        Err(_) => {
                            state.notice = Some(Notice::InvalidPath { input: line });
                            return;
                        }
                    }
                }
                TreeEdit::Delete { first, rest, line } => match line.as_str() {
                    "y" => {
                        state.tree.selected.clear();
                        let removed = std::iter::once(first).chain(rest);
                        out.extend(removed.map(|Removal { path, kind }| {
                            Effect::Job(Job::Fs {
                                root: root.clone(),
                                op: FsOp::Remove { path, kind },
                            })
                        }));
                        return;
                    }
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
        library,
        ..
    } = state;
    match (command, overlay) {
        (PickerCommand::Open(kind), _) => {
            let items = match kind {
                PickerKind::Files => Items::Files(match &state.project.index {
                    FileIndex::Ready(Indexed { files, .. }) => files.clone(),
                    FileIndex::Loading { .. } => Arc::new(Vec::new()),
                }),
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
                            .map(|(_, id)| (id, state.project.root.label(&state.buffers[id].path)))
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
                    let entries: Vec<OutlineEntry> = match state.layout.focused().active() {
                        Some(Tab::Ink(InkTabs::Pdf(tab))) => {
                            (state.pdfs[tab.file()].document().outline().into_iter())
                                .map(|heading| OutlineEntry {
                                    title: heading.title,
                                    depth: heading.depth,
                                    at: heading.page.map(|page| library::Place {
                                        part: library::Part(page),
                                        char: 0,
                                    }),
                                })
                                .collect()
                        }
                        Some(Tab::Book(tab)) => {
                            let book = &state.books[tab.file()];
                            (book.contents.iter())
                                .map(|entry| OutlineEntry {
                                    title: entry.title.clone(),
                                    depth: entry.depth,
                                    at: Some(book.flow.place_of(book.flow.item(&entry.at))),
                                })
                                .collect()
                        }
                        Some(Tab::Ink(_) | Tab::Text(_) | Tab::Image(_) | Tab::JsonCanvas(_))
                        | None => {
                            return state.notice = Some(Notice::NotReading);
                        }
                    };
                    let file = (state.layout.focused().active()).and_then(ReaderId::of);
                    match (file, entries.as_slice()) {
                        (Some(file), [_, ..]) => Items::Outline(Outline { file, entries }),
                        (Some(_), []) | (None, _) => {
                            return state.notice = Some(Notice::NoOutline);
                        }
                    }
                }
                PickerKind::Notebook => {
                    let Some(file) = (state.layout.focused().active()).and_then(ReaderId::of)
                    else {
                        return state.notice = Some(Notice::NotReading);
                    };
                    let notes = file.notes(&state.pdfs, &state.books);
                    let listed = match notes.ok_or(Notice::NotReading).and_then(Notes::listed) {
                        Ok(listed) => listed,
                        Err(refused) => return state.notice = Some(refused),
                    };
                    let entries: Vec<_> = (listed.notebook.in_reading_order().into_iter())
                        .map(|annotation| {
                            let label = picker::label(&annotation, file.kind());
                            (annotation, label)
                        })
                        .collect();
                    match entries.as_slice() {
                        [] => return state.notice = Some(Notice::NoAnnotations),
                        [_, ..] => Items::Notebook(Notebook { file, entries }),
                    }
                }
                PickerKind::Annotations => Items::Annotations(AnnotationSearch {
                    search: Search::Idle,
                    hits: Vec::new(),
                }),
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
        (PickerCommand::Insert(c), Overlay::Picker(picker)) => edit_query(
            picker,
            matcher,
            grep,
            project,
            library,
            |query| query.push(c),
            out,
        ),
        (PickerCommand::Backspace, Overlay::Picker(picker)) => edit_query(
            picker,
            matcher,
            grep,
            project,
            library,
            |query| {
                query.pop();
            },
            out,
        ),
        (PickerCommand::Next, Overlay::Picker(picker)) => move_cursor(picker, Cycle::Next),
        (PickerCommand::Previous, Overlay::Picker(picker)) => move_cursor(picker, Cycle::Previous),
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
        Items::Outline(outline) => {
            let State {
                layout,
                pdfs,
                books,
                notice,
                ..
            } = state;
            let shown =
                (layout.focused_mut().active_mut()).and_then(|tab| Viewing::of(tab, pdfs, books));
            match (shown, outline.entries[index].at) {
                (Some((file, mut viewing)), Some(at)) if file == outline.file => viewing.show(at),
                (Some((file, _)), None) if file == outline.file => {
                    *notice = Some(Notice::NothingToFollow)
                }
                (Some(_), _) | (None, _) => {}
            }
            focus_editor(state);
        }
        Items::Notebook(mut notebook) => {
            let State {
                layout,
                pdfs,
                books,
                ..
            } = state;
            let reader =
                (layout.focused_mut().active_mut()).map(|tab| Reader::of(tab, pdfs, books));
            match reader {
                Some(Ok(mut reader)) if reader.id == notebook.file => {
                    let (annotation, _) = notebook.entries.swap_remove(index);
                    reader.show(annotation)
                }
                Some(Ok(_) | Err(_)) | None => {}
            }
            focus_editor(state);
        }
        Items::Annotations(found) => {
            let (hit, _) = &found.hits[index];
            let path = state.library.path_of(&hit.path);
            let open = state.tab_config.open_from_picker;
            let to = Some(Target::Reader(Landing::Annotation(hit.annotation.id())));
            crate::apply::open(state, path, Arrive::AsIs, to, open, out);
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
    library: &library::Library,
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
        Items::Annotations(found) => {
            found.hits.clear();
            picker.matches.clear();
            picker.cursor = 0;
            found.search = match library::SearchQuery::new(&picker.query) {
                Some(query) => {
                    out.push(Effect::Job(Job::SearchAnnotations {
                        library: library.clone(),
                        query,
                        typed: picker.query.clone(),
                    }));
                    Search::Running
                }
                None => Search::Idle,
            };
        }
        Items::Files(_)
        | Items::Images(_)
        | Items::Buffers(_)
        | Items::Commands
        | Items::Backlinks(_)
        | Items::Outline(_)
        | Items::Notebook(_) => {
            let narrowing = Narrowing::between(&previous, &picker.query);
            picker::filter(picker, matcher, narrowing);
        }
    }
}

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

pub fn request_close(state: &mut State) -> Close {
    let unsaved = (state.buffers.values())
        .filter(|b| b.doc.is_modified())
        .count()
        + (state.pageds.values())
            .filter(|file| file.is_modified())
            .count()
        + (state.boards.values())
            .filter(|file| file.is_modified())
            .count()
        + (state.layout.panes().values())
            .flat_map(Pane::items)
            .filter_map(|item| item.tab.drafting())
            .filter(|(draft, _)| draft.edits() == Edits::Unsaved)
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
    pub fn of(count: usize) -> Click {
        match count {
            0 | 1 => Click::Single,
            _ => Click::Double,
        }
    }
}

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

pub fn focus_pane(state: &mut State, pane: PaneId) {
    match state.layout.set_focus(pane) {
        Ok(()) => {}
        Err(NotInLayout(_)) => return,
    }
    touch(&mut state.layout);
    focus_editor(state);
}

pub fn click_tree(
    state: &mut State,
    index: usize,
    click: Click,
    pick: Pick,
    out: &mut Vec<Effect>,
) {
    let Some(kind) = state.tree.rows.get(index).map(|row| row.kind) else {
        return;
    };
    state.sidebar = Sidebar::Files(Files::Tree);
    let listed = &mut state.tree;
    match pick {
        Pick::Only => listed.selected.clear(),
        Pick::Toggle => {
            let clicked = listed.rows[index].path.clone();
            match (listed.rows.get(listed.cursor), listed.selected.as_slice()) {
                (Some(at), []) => listed.selected.push(at.path.clone()),
                (Some(_) | None, _) => {}
            }
            match listed.selected.iter().position(|path| *path == clicked) {
                Some(at) => drop(listed.selected.remove(at)),
                None => listed.selected.push(clicked),
            }
            listed.cursor = index;
            return;
        }
        Pick::Extend => {
            let from = listed.cursor.min(listed.rows.len().saturating_sub(1));
            listed.selected = listed.rows[from.min(index)..=from.max(index)]
                .iter()
                .map(|row| row.path.clone())
                .collect();
            return;
        }
    }
    state.tree.cursor = index;
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

pub fn click_changes(
    state: &mut State,
    row: usize,
    click: Click,
    pick: Pick,
    out: &mut Vec<Effect>,
) {
    let changes = state.project.git.changes();
    let folds = &state.changes.folds;
    state.sidebar = Sidebar::Changes(Focus::Sidebar);
    match (changes::rows(changes, folds).nth(row), pick) {
        (Some(ListRow::Header { .. }), _) => {
            state.changes.cursor = changes::cursor_at(changes, folds, row);
            changes::list(state, ChangesCommand::Open, out);
        }
        (Some(ListRow::Entry(index)), Pick::Only) => {
            let open = match click {
                Click::Single => OpenAs::Preview,
                Click::Double => OpenAs::Permanent,
            };
            state.changes.selected.clear();
            changes::open_entry(state, index, open, out);
        }
        (Some(ListRow::Entry(index)), Pick::Toggle) => {
            let key = EntryKey::of(&changes[index]);
            let list = &mut state.changes;
            match (&list.cursor, list.selected.as_slice()) {
                (Some(ListCursor::Entry { group, path }), []) => list.selected.push(EntryKey {
                    group: *group,
                    path: path.clone(),
                }),
                (Some(_) | None, _) => {}
            }
            match list.selected.iter().position(|picked| *picked == key) {
                Some(at) => drop(list.selected.remove(at)),
                None => list.selected.push(key),
            }
            list.cursor = Some(ListCursor::on(&changes[index]));
        }
        (Some(ListRow::Entry(_)), Pick::Extend) => {
            let anchor = changes::cursor_row(changes, folds, state.changes.cursor.as_ref());
            let from = anchor.unwrap_or(row);
            let selected = changes::rows(changes, folds)
                .skip(from.min(row))
                .take(from.abs_diff(row) + 1)
                .filter_map(|list_row| match list_row {
                    ListRow::Entry(index) => Some(EntryKey::of(&changes[index])),
                    ListRow::Header { .. } => None,
                })
                .collect();
            state.changes.selected = selected;
        }
        (None, _) => {}
    }
}

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
    let listed = &mut state.tree;
    let row = match target {
        TreeTarget::Row(index) => listed.rows.get(index),
        TreeTarget::Root => None,
    };
    match row {
        Some(row) if listed.selected.contains(&row.path) => {}
        Some(_) | None => listed.selected.clear(),
    }
    match target {
        TreeTarget::Row(index) if index < listed.rows.len() => listed.cursor = index,
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Dragged {
    File { file: RelPath, arrive: Arrive },
    Outside(PathBuf),
    Tab { pane: PaneId, index: usize },
}

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
        Dragged::File { file, arrive } => {
            let path = file.to_path(state.project.root.path());
            dropped_file(state, path, arrive, on, out)
        }
        Dragged::Outside(path) => dropped_file(state, path, Arrive::AsIs, on, out),
    }
}

fn dropped_file(
    state: &mut State,
    path: PathBuf,
    arrive: Arrive,
    on: DropOn,
    out: &mut Vec<Effect>,
) {
    let (into, point) = match on {
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
    match (point, state.layout.focused().active()) {
        (Some(point), Some(Tab::JsonCanvas(_))) => {
            match crate::json_canvas::drop_file(state, into, path, point, out) {
                Ok(()) => {}
                Err(refused) => state.notice = Some(refused),
            }
            return;
        }
        (Some(_) | None, _) => {}
    }
    let image = (point.zip(ImageFrom::file(path.clone()).ok()))
        .map(|(point, image)| insert::into_focused(state, Some(point)).map(|into| (into, image)));
    match image {
        Some(Ok((into, image))) => insert::insert(state, into, image, out),
        Some(Err(Notice::NoImagesHere)) | None => {
            crate::apply::open(state, path, arrive, None, OpenAs::Permanent, out)
        }
        Some(Err(notice)) => state.notice = Some(notice),
    }
}
