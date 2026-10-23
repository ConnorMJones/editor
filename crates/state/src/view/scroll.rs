use std::cmp::Ordering;

use helix_core::doc_formatter::TextFormat;
use helix_core::syntax::Loader;
use helix_core::text_annotations::TextAnnotations;
use helix_core::textobject::{TextObject, textobject_word};
use helix_core::{
    Range, RopeSlice, Selection, VisualOffsetError, char_idx_at_visual_offset,
    visual_offset_from_anchor,
};
use model::mode::Mode;
use model::register::{Clipboard, Register};
use slotmap::SlotMap;

use crate::git::diffview::{self, RIGHT, Right};
use crate::text::buffer::{Buffer, BufferId, TextDoc};
use crate::text::registers::ClipboardUse;
use crate::view::display::{Fillers, Indent, text_format};
use crate::view::layout::{Layout, NotInLayout, Pane, PaneId, Scroll, TextView, Viewport};
use crate::view::tab::{BesideMut, ShowingMut};
use crate::{Effect, State};

/// Rows kept between the primary cursor and the viewport edges.
pub const SCROLLOFF: usize = 5;

/// Rows a mouse wheel notch scrolls, as helix's `scroll-lines`.
pub const WHEEL_LINES: isize = 3;

/// Rows a view shows besides its text: filler rows after lines and above the first line.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Padding<'a> {
    pub fillers: &'a [(usize, u16)],
    pub lead: u32,
}

impl Padding<'_> {
    pub const NONE: Padding<'static> = Padding {
        fillers: &[],
        lead: 0,
    };
}

/// Line annotations drawing `padding`'s fillers; allocates only when there are any.
pub fn annotations<'a>(text: RopeSlice<'a>, padding: Padding<'a>) -> TextAnnotations<'a> {
    let mut annotations = TextAnnotations::default();
    match padding.fillers.is_empty() {
        true => {}
        false => {
            annotations.add_line_annotation(Box::new(Fillers::new(text, padding.fillers)));
        }
    }
    annotations
}

/// `delta` rows from `anchor` and `rows`, through annotation rows and into the `lead`.
pub fn offset(
    text: RopeSlice,
    anchor: usize,
    rows: isize,
    delta: isize,
    lead: u32,
    format: &TextFormat,
    annotations: &TextAnnotations,
) -> (usize, isize) {
    let anchor = anchor.min(text.len_chars());
    let target = rows + delta;
    // Rows below the first line's top, needed only when the move may cross it.
    let from_top = match (lead, rows < 0, target < 0) {
        (0, ..) | (_, false, false) => None,
        (_, true, _) => Some(rows),
        (_, false, true) => visual_offset_from_anchor(
            text,
            0,
            anchor,
            format,
            annotations,
            target.unsigned_abs() + 1,
        )
        .ok()
        .map(|(pos, _)| pos.row as isize + rows),
    };
    let helix = |anchor: usize, rows: isize| {
        let (anchor, virtual_rows) =
            char_idx_at_visual_offset(text, anchor, rows, 0, format, annotations);
        (anchor, virtual_rows as isize)
    };
    match from_top.map(|top| top + delta) {
        Some(top) if top < 0 => (0, top.max(-(lead as isize))),
        Some(top) => helix(0, top),
        None => helix(anchor, target),
    }
}

/// Scroll position that keeps `cursor` at least `SCROLLOFF` rows inside the viewport.
pub fn follow(
    text: RopeSlice,
    cursor: usize,
    scroll: Scroll,
    lead: u32,
    format: &TextFormat,
    annotations: &TextAnnotations,
) -> (usize, isize) {
    let height = scroll.viewport.rows as usize;
    let anchor = scroll.anchor.min(text.len_chars());
    let bottom_row = match height {
        0 => return (anchor, scroll.rows),
        height => height - SCROLLOFF.min(height / 2) - 1,
    };
    let top_row = SCROLLOFF.min((height - 1) / 2);
    let max_rows = (height as isize + scroll.rows).max(0) as usize;
    let target =
        match visual_offset_from_anchor(text, anchor, cursor, format, annotations, max_rows) {
            Ok((pos, _)) => match pos.row as isize - scroll.rows {
                row if row < top_row as isize => top_row,
                row if row > bottom_row as isize => bottom_row,
                _ => return (anchor, scroll.rows),
            },
            Err(VisualOffsetError::PosBeforeAnchorRow) => top_row,
            Err(VisualOffsetError::PosAfterMaxRow) => bottom_row,
        };
    offset(
        text,
        cursor,
        0,
        -(target as isize),
        lead,
        format,
        annotations,
    )
}

/// Scrolls by `rows` and drags the primary cursor along once it would leave the scrolloff band.
#[allow(clippy::too_many_arguments)]
pub fn scroll(
    text: RopeSlice,
    selection: &Selection,
    scroll: Scroll,
    rows: isize,
    mode: Mode,
    lead: u32,
    format: &TextFormat,
    annotations: &TextAnnotations,
) -> (Scroll, Option<Selection>) {
    let (anchor, top) = offset(
        text,
        scroll.anchor,
        scroll.rows,
        rows,
        lead,
        format,
        annotations,
    );
    let moved = Scroll {
        anchor,
        rows: top,
        viewport: scroll.viewport,
    };
    let dragged = into_band(
        text,
        selection,
        moved,
        rows.cmp(&0),
        mode,
        format,
        annotations,
    );
    (moved, dragged)
}

/// The selection with the primary cursor dragged into the scrolloff band.
fn into_band(
    text: RopeSlice,
    selection: &Selection,
    scroll: Scroll,
    direction: Ordering,
    mode: Mode,
    format: &TextFormat,
    annotations: &TextAnnotations,
) -> Option<Selection> {
    let height = scroll.viewport.rows as usize;
    let scrolloff = SCROLLOFF.min(height.saturating_sub(1) / 2);
    let primary = selection.primary();
    let cursor = primary.cursor(text);
    let edge = |row: usize| char_at(text, scroll, row as u16, 0, format, annotations);
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

/// Char under the cell at `row`, `col` of the text area; filler rows give the char before them.
pub fn char_at(
    text: RopeSlice,
    scroll: Scroll,
    row: u16,
    col: u16,
    format: &TextFormat,
    annotations: &TextAnnotations,
) -> usize {
    char_idx_at_visual_offset(
        text,
        scroll.anchor.min(text.len_chars()),
        scroll.rows + row as isize,
        col as usize,
        format,
        annotations,
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

/// Where a press on the track lands, from the thumb's point of view.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum OnTrack {
    Above,
    /// On the thumb, `grab` below its top.
    Thumb {
        grab: f32,
    },
    Below,
}

impl Thumb {
    pub fn hit(self, y: f32) -> OnTrack {
        match y - self.top {
            grab if grab < 0.0 => OnTrack::Above,
            grab if grab > self.height => OnTrack::Below,
            grab => OnTrack::Thumb { grab },
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Scrollbar {
    /// The whole document is on screen.
    Hidden,
    Shown(Thumb),
}

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

/// A text view, or a diff's buffer side with the diff's filler rows.
pub struct Pointed<'a> {
    pub view: &'a mut TextView,
    pub doc: &'a TextDoc,
    pub padding: Padding<'a>,
    pub right: Right,
}

pub fn pane_text<'a>(
    layout: &'a mut Layout,
    buffers: &'a SlotMap<BufferId, Buffer>,
    pane: PaneId,
) -> Option<Pointed<'a>> {
    let tab = layout.find_pane(pane)?.active_text_mut()?;
    let buffer = &buffers[tab.buffer()];
    let doc = &buffer.doc;
    Some(match tab.showing_mut() {
        ShowingMut::Source(view)
        | ShowingMut::Document(view, _, BesideMut::Source(..))
        | ShowingMut::Pages(view, _, BesideMut::Source(..)) => Pointed {
            view,
            doc,
            padding: crate::git::conflict::padding(doc),
            right: Right::Live,
        },
        ShowingMut::Diff(view, diff) => Pointed {
            view,
            doc,
            padding: diff.align.sides[RIGHT].padding(),
            right: diffview::right(buffer),
        },
        ShowingMut::Document(_, _, BesideMut::Alone)
        | ShowingMut::Pages(_, _, BesideMut::Alone) => {
            return None;
        }
    })
}

/// Keeps the primary cursor of `pane` in view; run after every command.
pub fn follow_cursor(state: &mut State, pane: PaneId) {
    let State {
        layout,
        buffers,
        languages,
        ..
    } = state;
    match pane_text(layout, buffers, pane) {
        Some(Pointed {
            view,
            doc,
            padding,
            right: Right::Live,
        }) => {
            let text = doc.text.slice(..);
            let format = text_format(view.scroll.viewport, Indent::of(doc, languages));
            let annotations = annotations(text, padding);
            (view.scroll.anchor, view.scroll.rows) = follow(
                text,
                view.selection.primary().cursor(text),
                view.scroll,
                padding.lead,
                &format,
                &annotations,
            );
        }
        // Nothing to follow on an inert side.
        Some(Pointed {
            right: Right::Deleted,
            ..
        })
        | None => {}
    }
    diffview::sync_left(state, pane);
}

/// What the host draws this frame; `measured` later gives the state the same result.
pub fn fit(
    doc: &TextDoc,
    selection: &Selection,
    scroll: Scroll,
    viewport: Viewport,
    padding: Padding,
    languages: &Loader,
) -> Scroll {
    match scroll.viewport == viewport {
        true => scroll,
        false => {
            let text = doc.text.slice(..);
            let format = text_format(viewport, Indent::of(doc, languages));
            let (anchor, rows) = follow(
                text,
                selection.primary().cursor(text),
                Scroll { viewport, ..scroll },
                padding.lead,
                &format,
                &annotations(text, padding),
            );
            Scroll {
                anchor,
                rows,
                viewport,
            }
        }
    }
}

/// Reported after the frame, so the pane may have closed since.
pub fn measured(state: &mut State, pane: PaneId, viewport: Viewport) {
    let State {
        layout,
        buffers,
        languages,
        ..
    } = state;
    match pane_text(layout, buffers, pane) {
        Some(Pointed {
            view,
            doc,
            padding,
            right: Right::Live,
        }) => {
            view.scroll = fit(
                doc,
                &view.selection,
                view.scroll,
                viewport,
                padding,
                languages,
            )
        }
        // Nothing to follow on an inert side or in text still loading.
        Some(Pointed {
            right: Right::Deleted,
            ..
        })
        | None => match layout.find_pane(pane).and_then(Pane::active_text_mut) {
            Some(view) => view.text_mut().scroll.viewport = viewport,
            None => return,
        },
    }
    diffview::sync_left(state, pane);
}

/// Wheel scrolling; a diff scrolls both sides, also when its buffer side is inert.
pub fn wheel(state: &mut State, pane: PaneId, rows: isize) {
    let mode = state.input.mode;
    let State {
        layout,
        buffers,
        languages,
        ..
    } = state;
    let Some(Pointed {
        view,
        doc,
        padding,
        right,
    }) = pane_text(layout, buffers, pane)
    else {
        return;
    };
    let text = doc.text.slice(..);
    let format = text_format(view.scroll.viewport, Indent::of(doc, languages));
    let annotations = annotations(text, padding);
    let (moved, dragged) = scroll(
        text,
        &view.selection,
        view.scroll,
        rows,
        mode,
        padding.lead,
        &format,
        &annotations,
    );
    view.scroll = moved;
    match (dragged, right) {
        (Some(selection), Right::Live) => view.selection = selection,
        (Some(_), Right::Deleted) | (None, _) => {}
    }
    drop(annotations);
    diffview::sync_left(state, pane);
}

/// The live view of `pane` with the char under its text cell `row`, `col`.
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
    let Pointed {
        view,
        doc,
        padding,
        right: Right::Live,
    } = pane_text(layout, buffers, pane)?
    else {
        return None;
    };
    let text = doc.text.slice(..);
    let format = text_format(view.scroll.viewport, Indent::of(doc, languages));
    let annotations = annotations(text, padding);
    let pos = char_at(text, view.scroll, row, col, &format, &annotations);
    Some((view, doc, pos))
}

/// Focuses `pane` and selects at a text cell; returns the drag a following move extends.
pub fn press(state: &mut State, pane: PaneId, row: u16, col: u16, press: Press) -> Option<Gesture> {
    let mode = state.input.mode;
    state.layout.set_focus(pane).ok()?;
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
    let Some(Pointed { view, doc, .. }) = pane_text(layout, buffers, pane) else {
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
    match state.layout.set_focus(pane) {
        Ok(()) => {}
        Err(NotInLayout(_)) => return,
    }
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
    let Some(Pointed {
        view,
        doc,
        padding,
        right,
    }) = pane_text(layout, buffers, pane)
    else {
        return;
    };
    let text = doc.text.slice(..);
    let line = line.min(text.len_lines().saturating_sub(1));
    let current = text.char_to_line(view.scroll.anchor.min(text.len_chars()));
    let moved = Scroll {
        anchor: text.line_to_char(line),
        rows: 0,
        viewport: view.scroll.viewport,
    };
    let format = text_format(view.scroll.viewport, Indent::of(doc, languages));
    let annotations = annotations(text, padding);
    let dragged = into_band(
        text,
        &view.selection,
        moved,
        line.cmp(&current),
        mode,
        &format,
        &annotations,
    );
    view.scroll = moved;
    match (dragged, right) {
        (Some(selection), Right::Live) => view.selection = selection,
        (Some(_), Right::Deleted) | (None, _) => {}
    }
    drop(annotations);
    diffview::sync_left(state, pane);
}
