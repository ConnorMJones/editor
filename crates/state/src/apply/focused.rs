//! The focused text that commands act on: finding it, selecting in it, editing it.

use std::num::NonZeroUsize;

use helix_core::movement::Movement;
use helix_core::syntax::Loader;
use helix_core::{Assoc, ChangeSet, Range, Rope, RopeSlice, Selection, Syntax, Transaction};
use model::command::Shown;
use model::mode::Mode;
use model::notice::Notice;
use slotmap::SlotMap;

use crate::State;
use crate::git::diffview::{self, Right};
use crate::text::buffer::{Buffer, BufferId, HistoryStep, TextDoc, Uncommitted};
use crate::view::layout::{Edited, Editing, Layout};
use crate::view::tab::{Opened, Tab, TextTabs};

/// Sorted, deduplicated lines touched by any range.
pub fn selected_lines(text: &Rope, selection: &Selection) -> Vec<usize> {
    let mut lines: Vec<usize> = selection
        .iter()
        .flat_map(|range| {
            let (start, end) = range.line_range(text.slice(..));
            start..=end
        })
        .collect();
    lines.sort_unstable();
    lines.dedup();
    lines
}

pub fn count_of(state: &State) -> usize {
    state.input.count.map_or(1, NonZeroUsize::get)
}

/// Select mode extends selections where normal mode moves them.
pub fn movement_of(state: &State) -> Movement {
    match state.input.mode {
        Mode::Select => Movement::Extend,
        Mode::Normal | Mode::Insert => Movement::Move,
    }
}

pub fn exit_select_mode(state: &mut State) {
    match state.input.mode {
        Mode::Select => state.input.mode = Mode::Normal,
        Mode::Normal | Mode::Insert => {}
    }
}

pub enum Focused<'a> {
    Text(FocusedText<'a>),
    Empty,
    NotText,
    /// A rendering, or the deleted side of a diff.
    ReadOnly,
}

/// The text commands act on: a view of a buffer, or a drawing's text being typed.
pub struct FocusedText<'a> {
    pub view: Editing<'a>,
    pub doc: &'a mut TextDoc,
}

impl Edited<'_> {
    /// The buffer whose other views follow an edit; a mark has none.
    pub fn buffer(&self) -> Option<BufferId> {
        match self {
            Edited::Buffer { buffer, .. } => Some(*buffer),
            Edited::Mark => None,
        }
    }
}

pub fn focused<'a>(
    layout: &'a mut Layout,
    buffers: &'a mut SlotMap<BufferId, Buffer>,
) -> Focused<'a> {
    let tab = match layout.focused_mut().active_mut() {
        None => return Focused::Empty,
        Some(Tab::Image(_)) => return Focused::NotText,
        Some(Tab::Ink(ink)) => {
            return match ink.text_box() {
                Some(text) => Focused::Text(FocusedText {
                    view: Editing {
                        selection: &mut text.selection,
                        edited: Edited::Mark,
                    },
                    doc: &mut text.doc,
                }),
                None => Focused::NotText,
            };
        }
        Some(Tab::Text(tab)) => tab,
    };
    let buffer = &mut buffers[tab.buffer()];
    let right = match tab.shown() {
        Shown::Source | Shown::Split => Right::Live,
        Shown::Diff => diffview::right(buffer),
        Shown::Render => return Focused::ReadOnly,
    };
    match right {
        Right::Live => Focused::Text(FocusedText {
            view: tab.text_mut().editing(),
            doc: &mut buffer.doc,
        }),
        Right::Deleted => Focused::ReadOnly,
    }
}

/// The focused text, or `notice` set to why a text command cannot run.
pub fn focused_text<'a>(
    layout: &'a mut Layout,
    buffers: &'a mut SlotMap<BufferId, Buffer>,
    notice: &mut Option<Notice>,
) -> Option<FocusedText<'a>> {
    let refused = match focused(layout, buffers) {
        Focused::Text(text) => return Some(text),
        Focused::Empty => Notice::NoBuffer,
        Focused::NotText => Notice::NotText,
        Focused::ReadOnly => Notice::ReadOnly,
    };
    *notice = Some(refused);
    None
}

/// Maps every view of `buffer`, in every tab, through `changes`.
pub fn map_views(layout: &mut Layout, buffer: BufferId, changes: &ChangeSet, text: RopeSlice) {
    for pane in layout.panes_mut().map(|(_, pane)| pane) {
        let views = (pane.items_mut().iter_mut())
            .filter_map(|item| item.tab.as_text_mut())
            .map(TextTabs::text_mut);
        for view in views {
            match view.buffer() == buffer {
                true => {
                    view.selection = view.selection.clone().map(changes).ensure_invariants(text);
                    view.scroll.anchor = changes.map_pos(view.scroll.anchor, Assoc::Before);
                }
                false => {}
            }
        }
    }
}

pub fn try_select(
    state: &mut State,
    f: impl FnOnce(&TextDoc, &Selection, &Loader) -> Result<Selection, Notice>,
) {
    let State {
        layout,
        buffers,
        languages,
        notice,
        ..
    } = state;
    let Some(FocusedText { view, doc }) = focused_text(layout, buffers, notice) else {
        return;
    };
    match f(doc, &view.selection, languages) {
        Ok(selection) => *view.selection = selection.ensure_invariants(doc.text.slice(..)),
        Err(error) => *notice = Some(error),
    }
}

/// Moves the selection along the syntax tree, or says why there is none.
pub fn tree_select(state: &mut State, f: impl FnOnce(&Syntax, RopeSlice, Selection) -> Selection) {
    try_select(state, |doc, selection, _| {
        Ok(f(
            doc.highlight.tree()?,
            doc.text.slice(..),
            selection.clone(),
        ))
    });
}

pub fn select(state: &mut State, f: impl FnOnce(&TextDoc, &Selection, &Loader) -> Selection) {
    try_select(state, |doc, selection, loader| {
        Ok(f(doc, selection, loader))
    });
}

pub fn reselect(state: &mut State, mut f: impl FnMut(RopeSlice, Range) -> Range) {
    select(state, |doc, selection, _| {
        let text = doc.text.slice(..);
        selection.clone().transform(|range| f(text, range))
    });
}

/// Also maps every view of the buffer and keeps its preview tabs.
pub fn try_edit(
    state: &mut State,
    f: impl FnOnce(&TextDoc, &Selection, &Loader) -> Result<Transaction, Notice>,
) {
    let State {
        layout,
        buffers,
        languages,
        notice,
        ..
    } = state;
    let Some(FocusedText { view, doc }) = focused_text(layout, buffers, notice) else {
        return;
    };
    let tx = match f(doc, view.selection, languages) {
        Ok(tx) => tx,
        Err(error) => {
            *notice = Some(error);
            return;
        }
    };
    doc.apply(&tx, view.selection, languages);
    let text = doc.text.clone();
    match view.edited.buffer() {
        Some(buffer) => {
            map_views(layout, buffer, tx.changes(), text.slice(..));
            match tx.changes().is_empty() {
                true => {}
                false => layout.keep(Opened::Text(buffer)),
            }
            take_selection(layout, &tx, text.slice(..));
        }
        None => *view.selection = moved(view.selection, &tx, text.slice(..)),
    }
}

pub fn edit(state: &mut State, f: impl FnOnce(&TextDoc, &Selection, &Loader) -> Transaction) {
    try_edit(state, |doc, selection, loader| {
        Ok(f(doc, selection, loader))
    });
}

/// Moves the focused view's selection to where `tx` leaves it, when it says.
fn take_selection(layout: &mut Layout, tx: &Transaction, text: RopeSlice) {
    match tx.selection() {
        Some(selection) => {
            let view =
                (layout.focused_mut().active_text_mut()).expect("an edit keeps its view focused");
            view.text_mut().selection = selection.clone().ensure_invariants(text);
        }
        None => {}
    }
}

/// A selection no other view shares, after `tx`: where it says, else mapped through it.
fn moved(selection: &Selection, tx: &Transaction, text: RopeSlice) -> Selection {
    match tx.selection() {
        Some(selection) => selection.clone(),
        None => selection.clone().map(tx.changes()),
    }
    .ensure_invariants(text)
}

/// Groups the focused buffer's staged edits into an undo revision.
pub fn commit_focused(state: &mut State) {
    match focused(&mut state.layout, &mut state.buffers) {
        Focused::Text(FocusedText { view, doc }) => match doc.uncommitted {
            Uncommitted::Pending { .. } => doc.commit(view.selection),
            Uncommitted::Clean => {}
        },
        Focused::Empty | Focused::NotText | Focused::ReadOnly => {}
    }
}

pub fn step_history(state: &mut State, step: HistoryStep, exhausted: Notice) {
    let count = count_of(state);
    let State {
        layout,
        buffers,
        languages,
        notice,
        ..
    } = state;
    for _ in 0..count {
        let Some(FocusedText { view, doc }) = focused_text(layout, buffers, notice) else {
            return;
        };
        doc.commit(view.selection);
        let txs = doc.step_history(step, languages);
        if txs.is_empty() {
            *notice = Some(exhausted);
            return;
        }
        let text = doc.text.clone();
        match view.edited.buffer() {
            Some(buffer) => {
                layout.keep(Opened::Text(buffer));
                for tx in &txs {
                    map_views(layout, buffer, tx.changes(), text.slice(..));
                    take_selection(layout, tx, text.slice(..));
                }
            }
            None => {
                for tx in &txs {
                    *view.selection = moved(view.selection, tx, text.slice(..));
                }
            }
        }
    }
}
