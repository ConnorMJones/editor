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
use crate::view::layout::{Layout, TextView};

/// Sorted, deduplicated lines touched by any range.
pub(crate) fn selected_lines(text: &Rope, selection: &Selection) -> Vec<usize> {
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

pub(crate) fn count_of(state: &State) -> usize {
    state.input.count.map_or(1, NonZeroUsize::get)
}

/// Select mode extends selections where normal mode moves them.
pub(crate) fn movement_of(state: &State) -> Movement {
    match state.input.mode {
        Mode::Select => Movement::Extend,
        Mode::Normal | Mode::Insert => Movement::Move,
    }
}

pub(crate) fn exit_select_mode(state: &mut State) {
    match state.input.mode {
        Mode::Select => state.input.mode = Mode::Normal,
        Mode::Normal | Mode::Insert => {}
    }
}

pub(crate) enum Focused<'a, 'b> {
    Text(&'a mut TextView, &'b mut TextDoc),
    Empty,
    /// A rendering, or the deleted side of a diff.
    ReadOnly,
}

pub(crate) fn focused<'a, 'b>(
    layout: &'a mut Layout,
    buffers: &'b mut SlotMap<BufferId, Buffer>,
) -> Focused<'a, 'b> {
    let Some(tab) = layout.focused_mut().active_mut() else {
        return Focused::Empty;
    };
    let buffer = &mut buffers[tab.buffer()];
    let right = match tab.shown() {
        Shown::Source | Shown::Split => Right::Live,
        Shown::Diff => diffview::right(buffer),
        Shown::Render => return Focused::ReadOnly,
    };
    match right {
        Right::Live => Focused::Text(tab.text_mut(), &mut buffer.doc),
        Right::Deleted => Focused::ReadOnly,
    }
}

/// The focused text, or `notice` set to why a text command cannot run.
pub(crate) fn focused_text<'a, 'b>(
    layout: &'a mut Layout,
    buffers: &'b mut SlotMap<BufferId, Buffer>,
    notice: &mut Option<Notice>,
) -> Option<(&'a mut TextView, &'b mut TextDoc)> {
    let refused = match focused(layout, buffers) {
        Focused::Text(view, doc) => return Some((view, doc)),
        Focused::Empty => Notice::NoBuffer,
        Focused::ReadOnly => Notice::ReadOnly,
    };
    *notice = Some(refused);
    None
}

/// Maps every view of `buffer`, in every tab, through `changes`.
pub(crate) fn map_views(
    layout: &mut Layout,
    buffer: BufferId,
    changes: &ChangeSet,
    text: RopeSlice,
) {
    for pane in layout.panes_mut().map(|(_, pane)| pane) {
        for item in pane.items_mut() {
            let view = item.tab.text_mut();
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

pub(crate) fn try_select(
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
    let Some((view, doc)) = focused_text(layout, buffers, notice) else {
        return;
    };
    match f(doc, &view.selection, languages) {
        Ok(selection) => view.selection = selection.ensure_invariants(doc.text.slice(..)),
        Err(error) => *notice = Some(error),
    }
}

/// Moves the selection along the syntax tree, or says why there is none.
pub(super) fn tree_select(
    state: &mut State,
    f: impl FnOnce(&Syntax, RopeSlice, Selection) -> Selection,
) {
    try_select(state, |doc, selection, _| {
        Ok(f(
            doc.highlight.tree()?,
            doc.text.slice(..),
            selection.clone(),
        ))
    });
}

pub(crate) fn select(
    state: &mut State,
    f: impl FnOnce(&TextDoc, &Selection, &Loader) -> Selection,
) {
    try_select(state, |doc, selection, loader| {
        Ok(f(doc, selection, loader))
    });
}

pub(crate) fn reselect(state: &mut State, mut f: impl FnMut(RopeSlice, Range) -> Range) {
    select(state, |doc, selection, _| {
        let text = doc.text.slice(..);
        selection.clone().transform(|range| f(text, range))
    });
}

/// Also maps every view of the buffer and keeps its preview tabs.
pub(crate) fn try_edit(
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
    let Some((view, doc)) = focused_text(layout, buffers, notice) else {
        return;
    };
    let tx = match f(doc, &view.selection, languages) {
        Ok(tx) => tx,
        Err(error) => {
            *notice = Some(error);
            return;
        }
    };
    let buffer = view.buffer();
    doc.apply(&tx, &view.selection, languages);
    let text = doc.text.slice(..);
    map_views(layout, buffer, tx.changes(), text);
    match tx.changes().is_empty() {
        true => {}
        false => layout.keep(buffer),
    }
    take_selection(layout, &tx, text);
}

pub(crate) fn edit(
    state: &mut State,
    f: impl FnOnce(&TextDoc, &Selection, &Loader) -> Transaction,
) {
    try_edit(state, |doc, selection, loader| {
        Ok(f(doc, selection, loader))
    });
}

/// Moves the focused view's selection to where `tx` leaves it, when it says.
fn take_selection(layout: &mut Layout, tx: &Transaction, text: RopeSlice) {
    match tx.selection() {
        Some(selection) => {
            let view = (layout.focused_mut().active_mut()).expect("an edit keeps its view focused");
            view.text_mut().selection = selection.clone().ensure_invariants(text);
        }
        None => {}
    }
}

/// Groups the focused buffer's staged edits into an undo revision.
pub(crate) fn commit_focused(state: &mut State) {
    match focused(&mut state.layout, &mut state.buffers) {
        Focused::Text(view, doc) => match doc.uncommitted {
            Uncommitted::Pending { .. } => doc.commit(&view.selection),
            Uncommitted::Clean => {}
        },
        Focused::Empty | Focused::ReadOnly => {}
    }
}

pub(super) fn step_history(state: &mut State, step: HistoryStep, exhausted: Notice) {
    let count = count_of(state);
    let State {
        layout,
        buffers,
        languages,
        notice,
        ..
    } = state;
    for _ in 0..count {
        let Some((view, doc)) = focused_text(layout, buffers, notice) else {
            return;
        };
        doc.commit(&view.selection);
        let buffer = view.buffer();
        let txs = doc.step_history(step, languages);
        if txs.is_empty() {
            *notice = Some(exhausted);
            return;
        }
        let text = doc.text.slice(..);
        layout.keep(buffer);
        for tx in &txs {
            map_views(layout, buffer, tx.changes(), text);
            take_selection(layout, tx, text);
        }
    }
}
