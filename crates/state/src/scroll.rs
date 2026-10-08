use std::cmp::Ordering;

use helix_core::doc_formatter::TextFormat;
use helix_core::text_annotations::TextAnnotations;
use helix_core::textobject::{TextObject, textobject_word};
use helix_core::{
    Range, RopeSlice, Selection, VisualOffsetError, char_idx_at_visual_offset,
    visual_offset_from_anchor,
};
use model::mode::Mode;
use model::register::{Clipboard, Register};
use slotmap::SlotMap;

use crate::buffer::{Buffer, BufferId, Content, TextDoc};
use crate::display::{Indent, text_format};
use crate::layout::{Layout, PaneId, TextView, View, Viewport};
use crate::register::ClipboardUse;
use crate::{Effect, State};

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
    let moved = into_band(
        text,
        selection,
        anchor,
        viewport,
        rows.cmp(&0),
        mode,
        format,
    );
    (anchor, moved)
}

/// The selection with the primary cursor dragged inside the scrolloff band of a view moved in `direction`.
fn into_band(
    text: RopeSlice,
    selection: &Selection,
    anchor: usize,
    viewport: Viewport,
    direction: Ordering,
    mode: Mode,
    format: &TextFormat,
) -> Option<Selection> {
    let annotations = TextAnnotations::default();
    let height = viewport.rows as usize;
    let scrolloff = SCROLLOFF.min(height.saturating_sub(1) / 2);
    let primary = selection.primary();
    let cursor = primary.cursor(text);
    let edge = |row: usize| {
        char_idx_at_visual_offset(text, anchor, row as isize, 0, format, &annotations).0
    };
    let head = match direction {
        Ordering::Greater => Some(edge(scrolloff)).filter(|&head| head > cursor),
        Ordering::Less => {
            Some(edge(height.saturating_sub(scrolloff + 1))).filter(|&head| head < cursor)
        }
        Ordering::Equal => None,
    };
    head.map(|head| {
        let from = match mode {
            Mode::Select => primary.anchor,
            Mode::Normal | Mode::Insert => head,
        };
        selection
            .clone()
            .replace(selection.primary_index(), Range::new(from, head))
    })
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

/// Text a press selects; dragging afterwards extends by whole units, as zed does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Unit {
    Char,
    Word,
    Line,
}

impl Unit {
    pub fn from_clicks(count: usize) -> Unit {
        match count {
            0 | 1 => Unit::Char,
            2 => Unit::Word,
            _ => Unit::Line,
        }
    }

    /// The range of this unit around `pos`.
    pub fn around(self, text: RopeSlice, pos: usize) -> Range {
        match self {
            Unit::Char => Range::point(pos),
            Unit::Word => {
                let word = textobject_word(text, Range::point(pos), TextObject::Inside, 1, false);
                match word.is_empty() {
                    true => Range::point(pos),
                    false => word,
                }
            }
            Unit::Line => {
                let line = text.char_to_line(pos);
                Range::new(
                    text.line_to_char(line),
                    text.line_to_char((line + 1).min(text.len_lines())),
                )
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Press {
    /// Replaces the selection with the unit under the pointer; select mode extends instead.
    Select(Unit),
    /// Alt-click adds a cursor, as in helix.
    AddCursor,
}

/// A selection drag: the unit and the range it started on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Gesture {
    pub unit: Unit,
    pub origin: Range,
}

/// Extends `origin` to the unit under `pos`, keeping the whole origin unit selected.
pub fn extend(text: RopeSlice, gesture: Gesture, pos: usize) -> Range {
    let origin = gesture.origin;
    match gesture.unit {
        Unit::Char => origin.put_cursor(text, pos, true),
        Unit::Word | Unit::Line => {
            let under = gesture.unit.around(text, pos);
            match under.from() < origin.from() {
                true => Range::new(origin.to(), under.from()),
                false => Range::new(origin.from(), under.to().max(origin.to())),
            }
        }
    }
}

/// Vertical scrollbar thumb, in the track's units.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Thumb {
    pub top: f32,
    pub height: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Scrollbar {
    /// The whole document is on screen.
    Hidden,
    Shown(Thumb),
}

/// Thumb for a view showing document lines `visible` on a `track` long track.
/// Lines stand in for visual rows, so a soft-wrapped line counts as one.
pub fn scrollbar(
    len_lines: usize,
    visible: std::ops::Range<usize>,
    track: f32,
    min_thumb: f32,
) -> Scrollbar {
    match (visible.start, visible.end >= len_lines) {
        (0, true) => Scrollbar::Hidden,
        _ => {
            let shown = visible.len().max(1);
            let height = (track * shown as f32 / len_lines.max(1) as f32)
                .max(min_thumb)
                .min(track);
            let travel = len_lines.saturating_sub(shown).max(1);
            let top = (track - height) * (visible.start.min(travel) as f32 / travel as f32);
            Scrollbar::Shown(Thumb { top, height })
        }
    }
}

/// First line to show when a `height` thumb is dragged to `top`; the inverse of `scrollbar`.
pub fn line_at_thumb(len_lines: usize, shown: usize, height: f32, track: f32, top: f32) -> usize {
    let travel = len_lines.saturating_sub(shown.max(1));
    let room = track - height;
    match room > 0.0 {
        true => ((top / room).clamp(0.0, 1.0) * travel as f32).round() as usize,
        false => 0,
    }
}

fn pane_text<'a>(
    layout: &'a mut Layout,
    buffers: &'a SlotMap<BufferId, Buffer>,
    pane: PaneId,
) -> Option<(&'a mut TextView, &'a TextDoc)> {
    match layout.panes.get_mut(pane).map(|pane| &mut pane.view) {
        Some(View::Text(view)) => match buffers.get(view.buffer).map(|b| &b.content) {
            Some(Content::Text { doc, .. }) => Some((view, doc)),
            Some(Content::Loading { .. }) | None => None,
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

/// The view of `pane` with the char under its text cell `row`, `col`.
fn pointed(
    state: &mut State,
    pane: PaneId,
    row: u16,
    col: u16,
) -> Option<(&mut TextView, &TextDoc, usize)> {
    let State {
        layout,
        buffers,
        languages,
        ..
    } = state;
    let (view, doc) = pane_text(layout, buffers, pane)?;
    let format = text_format(view.scroll.viewport, Indent::of(doc, languages));
    let pos = char_at(doc.text.slice(..), view.scroll.anchor, row, col, &format);
    Some((view, doc, pos))
}

/// Focuses `pane` and selects at a text cell; returns the drag a following move extends.
pub fn press(state: &mut State, pane: PaneId, row: u16, col: u16, press: Press) -> Option<Gesture> {
    let mode = state.input.mode;
    state.layout.focus = pane;
    let (view, doc, pos) = pointed(state, pane, row, col)?;
    let text = doc.text.slice(..);
    let (selection, gesture) = match (press, mode) {
        (Press::AddCursor, _) => (view.selection.clone().push(Range::point(pos)), None),
        (Press::Select(Unit::Char), Mode::Select) => {
            let origin = view.selection.primary().put_cursor(text, pos, true);
            let gesture = Gesture {
                unit: Unit::Char,
                origin,
            };
            (Selection::single(origin.anchor, origin.head), Some(gesture))
        }
        (Press::Select(unit), Mode::Normal | Mode::Insert | Mode::Select) => {
            let origin = unit.around(text, pos);
            let gesture = Gesture { unit, origin };
            (Selection::single(origin.anchor, origin.head), Some(gesture))
        }
    };
    view.selection = selection.ensure_invariants(text);
    follow_cursor(state, pane);
    gesture
}

pub fn drag(state: &mut State, pane: PaneId, row: u16, col: u16, gesture: Gesture) {
    let Some((view, doc, pos)) = pointed(state, pane, row, col) else {
        return;
    };
    let range = extend(doc.text.slice(..), gesture, pos);
    view.selection =
        Selection::single(range.anchor, range.head).ensure_invariants(doc.text.slice(..));
    follow_cursor(state, pane);
}

/// Ends a drag; a selection wider than one char goes to the primary clipboard, as X11 does.
pub fn release(state: &mut State, pane: PaneId, out: &mut Vec<Effect>) {
    let State {
        layout,
        buffers,
        registers,
        notice,
        ..
    } = state;
    let Some((view, doc)) = pane_text(layout, buffers, pane) else {
        return;
    };
    let primary = view.selection.primary();
    match primary.len() > 1 {
        true => {
            let value = primary.fragment(doc.text.slice(..)).into_owned();
            match registers.write(Register::Clipboard(Clipboard::Primary), vec![value], out) {
                Ok(()) => {}
                Err(error) => *notice = Some(error),
            }
        }
        false => {}
    }
}

/// Pastes the primary clipboard before the char under a text cell, as helix's middle click.
pub fn middle_click(state: &mut State, pane: PaneId, row: u16, col: u16, out: &mut Vec<Effect>) {
    state.layout.focus = pane;
    match pointed(state, pane, row, col) {
        Some((_, _, pos)) => out.push(Effect::ReadClipboard {
            clipboard: Clipboard::Primary,
            then: ClipboardUse::PasteAt { pane, pos },
        }),
        None => {}
    }
}

/// Scrolls `pane` so `line` is the top row, dragging the cursor along like the wheel does.
pub fn scroll_to_line(state: &mut State, pane: PaneId, line: usize) {
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
    let text = doc.text.slice(..);
    let line = line.min(text.len_lines().saturating_sub(1));
    let current = text.char_to_line(view.scroll.anchor.min(text.len_chars()));
    let anchor = text.line_to_char(line);
    let format = text_format(view.scroll.viewport, Indent::of(doc, languages));
    let moved = into_band(
        text,
        &view.selection,
        anchor,
        view.scroll.viewport,
        line.cmp(&current),
        mode,
        &format,
    );
    view.scroll.anchor = anchor;
    match moved {
        Some(selection) => view.selection = selection,
        None => {}
    }
}
