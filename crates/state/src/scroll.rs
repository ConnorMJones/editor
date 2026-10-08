use std::cmp::Ordering;

use helix_core::doc_formatter::TextFormat;
use helix_core::text_annotations::TextAnnotations;
use helix_core::{
    Range, RopeSlice, Selection, VisualOffsetError, char_idx_at_visual_offset,
    visual_offset_from_anchor,
};
use model::mode::Mode;
use slotmap::SlotMap;

use crate::State;
use crate::buffer::{Buffer, BufferId, Content, TextDoc};
use crate::display::{Indent, text_format};
use crate::layout::{Layout, PaneId, TextView, View, Viewport};

/// Rows kept between the primary cursor and the viewport edges.
pub const SCROLLOFF: usize = 5;

/// Rows a mouse wheel notch scrolls, as helix's `scroll-lines`.
pub const WHEEL_LINES: isize = 3;

/// Anchor that keeps `cursor` at least `SCROLLOFF` rows inside a `rows` tall viewport.
pub fn follow(
    text: RopeSlice,
    cursor: usize,
    anchor: usize,
    rows: u16,
    format: &TextFormat,
) -> usize {
    let height = rows as usize;
    let anchor = anchor.min(text.len_chars());
    let bottom_row = match height {
        0 => return anchor,
        height => height - SCROLLOFF.min(height / 2) - 1,
    };
    let top_row = SCROLLOFF.min((height - 1) / 2);
    let annotations = TextAnnotations::default();
    let target = match visual_offset_from_anchor(text, anchor, cursor, format, &annotations, height)
    {
        Ok((pos, _)) if pos.row < top_row => top_row,
        Ok((pos, _)) if pos.row > bottom_row => bottom_row,
        Ok(_) => return anchor,
        Err(VisualOffsetError::PosBeforeAnchorRow) => top_row,
        Err(VisualOffsetError::PosAfterMaxRow) => bottom_row,
    };
    char_idx_at_visual_offset(text, cursor, -(target as isize), 0, format, &annotations).0
}

/// Scrolls by `rows` and drags the primary cursor along once it would leave the scrolloff band.
pub fn scroll(
    text: RopeSlice,
    selection: &Selection,
    anchor: usize,
    viewport: Viewport,
    rows: isize,
    mode: Mode,
    format: &TextFormat,
) -> (usize, Option<Selection>) {
    let annotations = TextAnnotations::default();
    let anchor = char_idx_at_visual_offset(
        text,
        anchor.min(text.len_chars()),
        rows,
        0,
        format,
        &annotations,
    )
    .0;
    let height = viewport.rows as usize;
    let scrolloff = SCROLLOFF.min(height.saturating_sub(1) / 2);
    let primary = selection.primary();
    let cursor = primary.cursor(text);
    let edge = |row: usize| {
        char_idx_at_visual_offset(text, anchor, row as isize, 0, format, &annotations).0
    };
    let head = match rows.cmp(&0) {
        Ordering::Greater => Some(edge(scrolloff)).filter(|&head| head > cursor),
        Ordering::Less => {
            Some(edge(height.saturating_sub(scrolloff + 1))).filter(|&head| head < cursor)
        }
        Ordering::Equal => None,
    };
    let moved = head.map(|head| {
        let from = match mode {
            Mode::Select => primary.anchor,
            Mode::Normal | Mode::Insert => head,
        };
        selection
            .clone()
            .replace(selection.primary_index(), Range::new(from, head))
    });
    (anchor, moved)
}

/// Char under the cell at `row`, `col` of the text area.
pub fn char_at(text: RopeSlice, anchor: usize, row: u16, col: u16, format: &TextFormat) -> usize {
    char_idx_at_visual_offset(
        text,
        anchor.min(text.len_chars()),
        row as isize,
        col as usize,
        format,
        &TextAnnotations::default(),
    )
    .0
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Click {
    Place,
    Extend,
}

fn pane_text<'a>(
    layout: &'a mut Layout,
    buffers: &'a SlotMap<BufferId, Buffer>,
    pane: PaneId,
) -> Option<(&'a mut TextView, &'a TextDoc)> {
    match layout.panes.get_mut(pane).map(|pane| &mut pane.view) {
        Some(View::Text(view)) => match buffers.get(view.buffer).map(|b| &b.content) {
            Some(Content::Text(doc)) => Some((view, doc)),
            None => None,
        },
        Some(View::Empty) | None => None,
    }
}

/// Keeps the primary cursor of `pane` in view; run after every command.
pub fn follow_cursor(state: &mut State, pane: PaneId) {
    let State {
        layout,
        buffers,
        languages,
        ..
    } = state;
    let Some((view, doc)) = pane_text(layout, buffers, pane) else {
        return;
    };
    let text = doc.text.slice(..);
    let format = text_format(view.scroll.viewport, Indent::of(doc, languages));
    view.scroll.anchor = follow(
        text,
        view.selection.primary().cursor(text),
        view.scroll.anchor,
        view.scroll.viewport.rows,
        &format,
    );
}

/// Records the drawn size of `pane`; a changed size re-follows the cursor.
pub fn set_viewport(state: &mut State, pane: PaneId, viewport: Viewport) {
    match pane_text(&mut state.layout, &state.buffers, pane) {
        Some((view, _)) if view.scroll.viewport != viewport => {
            view.scroll.viewport = viewport;
            follow_cursor(state, pane);
        }
        Some(_) | None => {}
    }
}

pub fn wheel(state: &mut State, pane: PaneId, rows: isize) {
    let mode = state.input.mode;
    let State {
        layout,
        buffers,
        languages,
        ..
    } = state;
    let Some((view, doc)) = pane_text(layout, buffers, pane) else {
        return;
    };
    let format = text_format(view.scroll.viewport, Indent::of(doc, languages));
    let (anchor, moved) = scroll(
        doc.text.slice(..),
        &view.selection,
        view.scroll.anchor,
        view.scroll.viewport,
        rows,
        mode,
        &format,
    );
    view.scroll.anchor = anchor;
    match moved {
        Some(selection) => view.selection = selection,
        None => {}
    }
}

/// Places or extends the primary cursor at a text cell and focuses `pane`.
pub fn click(state: &mut State, pane: PaneId, row: u16, col: u16, click: Click) {
    let State {
        layout,
        buffers,
        languages,
        ..
    } = state;
    let Some((view, doc)) = pane_text(layout, buffers, pane) else {
        return;
    };
    let text = doc.text.slice(..);
    let format = text_format(view.scroll.viewport, Indent::of(doc, languages));
    let pos = char_at(text, view.scroll.anchor, row, col, &format);
    view.selection = match click {
        Click::Place => Selection::point(pos),
        Click::Extend => {
            let primary = view.selection.primary().put_cursor(text, pos, true);
            view.selection
                .clone()
                .replace(view.selection.primary_index(), primary)
        }
    };
    layout.focus = pane;
}
