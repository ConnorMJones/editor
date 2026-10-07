use std::io;
use std::path::PathBuf;

use helix_core::doc_formatter::TextFormat;
use helix_core::movement::{self, Direction, Movement};
use helix_core::text_annotations::TextAnnotations;
use helix_core::{ChangeSet, Range, Rope, RopeSlice, Selection, Tendril, Transaction, graphemes};
use model::command::Command;
use model::mode::Mode;
use model::notice::Notice;
use slotmap::SlotMap;

use crate::buffer::{Buffer, BufferId, Content, Disk, HistoryStep, TextDoc};
use crate::job::Job;
use crate::layout::{Layout, PaneId, Scroll, TextView, View};
use crate::{Effect, Exit, Overlay, Prompt, State, prompt};

type Motion =
    fn(RopeSlice, Range, Direction, usize, Movement, &TextFormat, &mut TextAnnotations) -> Range;

pub fn apply(state: &mut State, command: Command, out: &mut Vec<Effect>) {
    match command {
        Command::NormalMode => {
            if let Some(id) = focused_buffer(state) {
                commit(state, id);
            }
            state.input.mode = Mode::Normal;
        }
        Command::InsertMode => {
            state.input.mode = Mode::Insert;
            reselect(state, |_, range| Range::new(range.to(), range.from()));
        }
        Command::AppendMode => {
            state.input.mode = Mode::Insert;
            let State {
                layout,
                buffers,
                languages,
                ..
            } = state;
            if let Some((view, doc)) = focused(layout, buffers) {
                let end = doc.text.len_chars();
                let last = view.selection.ranges().last().copied();
                if let Some(last) = last
                    && !last.is_empty()
                    && last.to() == end
                {
                    let tx = Transaction::change(
                        &doc.text,
                        [(end, end, Some(doc.line_ending.as_str().into()))].into_iter(),
                    );
                    doc.apply(&tx, &view.selection, languages);
                }
            }
            reselect(state, |text, range| {
                Range::new(
                    range.from(),
                    graphemes::next_grapheme_boundary(text, range.to()),
                )
            });
        }
        Command::SelectMode => state.input.mode = Mode::Select,

        Command::MoveCharLeft => motion(state, movement::move_horizontally, Direction::Backward),
        Command::MoveCharRight => motion(state, movement::move_horizontally, Direction::Forward),
        Command::MoveLineUp => motion(state, movement::move_vertically, Direction::Backward),
        Command::MoveLineDown => motion(state, movement::move_vertically, Direction::Forward),

        Command::InsertChar(c) => insert(state, Tendril::from_iter([c])),
        Command::InsertNewline => {
            let line_ending = match focused(&mut state.layout, &mut state.buffers) {
                Some((_, doc)) => doc.line_ending,
                None => return,
            };
            insert(state, Tendril::from(line_ending.as_str()));
        }
        Command::DeleteCharBackward => edit(state, |text, selection| {
            let slice = text.slice(..);
            Transaction::delete_by_selection(text, selection, |range| {
                let cursor = range.cursor(slice);
                (
                    graphemes::nth_prev_grapheme_boundary(slice, cursor, 1),
                    cursor,
                )
            })
        }),

        Command::Undo => step_history(state, HistoryStep::Undo),
        Command::Redo => step_history(state, HistoryStep::Redo),

        Command::CommandMode => {
            state.overlay = Overlay::Prompt(Prompt {
                line: String::new(),
            })
        }
        Command::PromptInsert(c) => match &mut state.overlay {
            Overlay::Prompt(prompt) => prompt.line.push(c),
            Overlay::None => {}
        },
        Command::PromptBackspace => match &mut state.overlay {
            Overlay::Prompt(prompt) => match prompt.line.pop() {
                Some(_) => {}
                None => state.overlay = Overlay::None,
            },
            Overlay::None => {}
        },
        Command::PromptCancel => state.overlay = Overlay::None,
        Command::PromptSubmit => match std::mem::replace(&mut state.overlay, Overlay::None) {
            Overlay::Prompt(prompt) => match prompt::parse(&prompt.line) {
                Ok(command) => apply(state, command, out),
                Err(notice) => state.notice = Some(notice),
            },
            Overlay::None => {}
        },

        Command::Open(path) => open(state, path),
        Command::Write => match focused_buffer(state) {
            Some(id) => out.extend(write_job(state, id).map(Effect::Job)),
            None => state.notice = Some(Notice::NoBuffer),
        },
        Command::WriteAll => {
            let modified: Vec<BufferId> = state
                .buffers
                .iter()
                .filter(|(_, buffer)| is_modified(buffer))
                .map(|(id, _)| id)
                .collect();
            for id in modified {
                out.extend(write_job(state, id).map(Effect::Job));
            }
        }
        Command::WriteQuit => match focused_buffer(state) {
            Some(id) => {
                out.extend(write_job(state, id).map(Effect::Job));
                state.exit = Exit::AfterWrite { buffer: id };
            }
            None => apply(state, Command::Quit, out),
        },
        Command::Quit => match state.buffers.values().filter(|b| is_modified(b)).count() {
            0 => out.push(Effect::Quit),
            count => state.notice = Some(Notice::UnsavedBuffers { count }),
        },
        Command::ForceQuit => out.push(Effect::Quit),
        Command::CloseBuffer => close_buffer(state, Force::No),
        Command::ForceCloseBuffer => close_buffer(state, Force::Yes),
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Force {
    No,
    Yes,
}

fn focused<'a>(
    layout: &'a mut Layout,
    buffers: &'a mut SlotMap<BufferId, Buffer>,
) -> Option<(&'a mut TextView, &'a mut TextDoc)> {
    let focus = layout.focus;
    match layout.panes.get_mut(focus).map(|pane| &mut pane.view) {
        Some(View::Text(view)) => match buffers.get_mut(view.buffer).map(|b| &mut b.content) {
            Some(Content::Text(doc)) => Some((view, doc)),
            None => None,
        },
        Some(View::Empty) | None => None,
    }
}

fn focused_buffer(state: &State) -> Option<BufferId> {
    match state
        .layout
        .panes
        .get(state.layout.focus)
        .map(|pane| &pane.view)
    {
        Some(View::Text(view)) => Some(view.buffer),
        Some(View::Empty) | None => None,
    }
}

fn is_modified(buffer: &Buffer) -> bool {
    match &buffer.content {
        Content::Text(doc) => doc.is_modified(),
    }
}

fn map_other_views(layout: &mut Layout, buffer: BufferId, changes: &ChangeSet) {
    let focus = layout.focus;
    for (id, pane) in layout.panes.iter_mut() {
        match &mut pane.view {
            View::Text(view) if id != focus && view.buffer == buffer => {
                view.selection = view.selection.clone().map(changes);
            }
            View::Text(_) | View::Empty => {}
        }
    }
}

fn reselect(state: &mut State, mut f: impl FnMut(RopeSlice, Range) -> Range) {
    if let Some((view, doc)) = focused(&mut state.layout, &mut state.buffers) {
        let text = doc.text.slice(..);
        view.selection = view.selection.clone().transform(|range| f(text, range));
    }
}

fn motion(state: &mut State, f: Motion, direction: Direction) {
    let behaviour = match state.input.mode {
        Mode::Select => Movement::Extend,
        Mode::Normal | Mode::Insert => Movement::Move,
    };
    let format = TextFormat::default();
    let mut annotations = TextAnnotations::default();
    reselect(state, |text, range| {
        f(
            text,
            range,
            direction,
            1,
            behaviour,
            &format,
            &mut annotations,
        )
    });
}

fn edit(state: &mut State, f: impl FnOnce(&Rope, &Selection) -> Transaction) {
    let State {
        layout,
        buffers,
        languages,
        ..
    } = state;
    let Some((view, doc)) = focused(layout, buffers) else {
        state.notice = Some(Notice::NoBuffer);
        return;
    };
    let tx = f(&doc.text, &view.selection);
    let before = view.selection.clone();
    doc.apply(&tx, &before, languages);
    view.selection = match tx.selection() {
        Some(selection) => selection.clone(),
        None => before.map(tx.changes()),
    };
    let buffer = view.buffer;
    map_other_views(layout, buffer, tx.changes());
}

fn insert(state: &mut State, text: Tendril) {
    edit(state, |doc, selection| {
        let slice = doc.slice(..);
        Transaction::change_by_selection(doc, selection, |range| {
            let cursor = range.cursor(slice);
            (cursor, cursor, Some(text.clone()))
        })
    });
}

fn step_history(state: &mut State, step: HistoryStep) {
    let State {
        layout,
        buffers,
        languages,
        ..
    } = state;
    let Some((view, doc)) = focused(layout, buffers) else {
        return;
    };
    doc.commit(&view.selection);
    let Some(tx) = doc.step_history(step, languages) else {
        return;
    };
    view.selection = match tx.selection() {
        Some(selection) => selection.clone(),
        None => view.selection.clone().map(tx.changes()),
    };
    let buffer = view.buffer;
    map_other_views(layout, buffer, tx.changes());
}

fn commit(state: &mut State, id: BufferId) {
    let selection = state
        .layout
        .panes
        .values()
        .find_map(|pane| match &pane.view {
            View::Text(view) if view.buffer == id => Some(view.selection.clone()),
            View::Text(_) | View::Empty => None,
        })
        .unwrap_or_else(|| Selection::point(0));
    match state.buffers.get_mut(id).map(|b| &mut b.content) {
        Some(Content::Text(doc)) => doc.commit(&selection),
        None => {}
    }
}

fn write_job(state: &mut State, id: BufferId) -> Option<Job> {
    commit(state, id);
    let buffer = state.buffers.get(id)?;
    match &buffer.content {
        Content::Text(doc) => Some(Job::Write {
            buffer: id,
            path: buffer.path.clone(),
            text: doc.text.clone(),
            revision: doc.history.current_revision(),
        }),
    }
}

fn open(state: &mut State, path: PathBuf) {
    let joined = helix_stdx::path::normalize(state.root.path().join(&path));
    let path = match joined.canonicalize() {
        Ok(path) => path,
        Err(error) if error.kind() == io::ErrorKind::NotFound => joined,
        Err(error) => {
            state.notice = Some(Notice::OpenFailed {
                path: joined,
                error,
            });
            return;
        }
    };
    let existing = state
        .buffers
        .iter()
        .find(|(_, buffer)| buffer.path == path)
        .map(|(id, _)| id);
    let id = match existing {
        Some(id) => id,
        None => {
            let (text, disk) = match std::fs::read(&path) {
                Ok(bytes) => match String::from_utf8(bytes) {
                    Ok(text) => (Rope::from(text), Disk::Synced),
                    Err(_) => {
                        state.notice = Some(Notice::NotUtf8 { path });
                        return;
                    }
                },
                Err(error) if error.kind() == io::ErrorKind::NotFound => (Rope::new(), Disk::New),
                Err(error) if error.kind() == io::ErrorKind::IsADirectory => {
                    state.notice = Some(Notice::NotAFile { path });
                    return;
                }
                Err(error) => {
                    state.notice = Some(Notice::OpenFailed { path, error });
                    return;
                }
            };
            let doc = TextDoc::new(text, &path, &state.languages);
            state.buffers.insert(Buffer {
                path,
                content: Content::Text(doc),
                disk,
            })
        }
    };
    if let Some(current) = focused_buffer(state) {
        commit(state, current);
    }
    let focus = state.layout.focus;
    show(&mut state.layout, focus, id);
}

fn show(layout: &mut Layout, pane: PaneId, buffer: BufferId) {
    if let Some(pane) = layout.panes.get_mut(pane) {
        pane.view = View::Text(TextView {
            buffer,
            selection: Selection::point(0),
            scroll: Scroll::default(),
        });
    }
}

fn close_buffer(state: &mut State, force: Force) {
    let Some(id) = focused_buffer(state) else {
        state.notice = Some(Notice::NoBuffer);
        return;
    };
    let modified = state.buffers.get(id).is_some_and(is_modified);
    match (modified, force) {
        (true, Force::No) => {
            state.notice = Some(Notice::UnsavedBuffers { count: 1 });
        }
        (false, _) | (true, Force::Yes) => {
            state.buffers.remove(id);
            for pane in state.layout.panes.values_mut() {
                match &pane.view {
                    View::Text(view) if view.buffer == id => pane.view = View::Empty,
                    View::Text(_) | View::Empty => {}
                }
            }
        }
    }
}
