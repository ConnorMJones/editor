//! Side-by-side diffs: HEAD on the left, the live buffer on the right, kept row-aligned by
//! filler rows so soft-wrapped lines line up and both sides scroll together.

use std::ops::Range;
use std::sync::Arc;

use git2::Oid;
use helix_core::doc_formatter::{DocumentFormatter, TextFormat};
use helix_core::text_annotations::TextAnnotations;
use helix_core::{RopeSlice, Selection, SmallVec, visual_offset_from_block};
use model::theme::Background;
use slotmap::SlotMap;

use crate::buffer::{Buffer, BufferId, Content, Disk, TextDoc};
use crate::diff::{Base, Bases, Diff, Hunk, Side, Stamp, Tracked};
use crate::display::{Indent, Shade, text_format};
use crate::job::Job;
use crate::layout::{Layout, Pane, PaneId, Scroll, TextView, View, Viewport};
use crate::scroll::{self, Padding};
use crate::{Effect, State};

/// Index of the HEAD side in per-side arrays.
pub const LEFT: usize = 0;
/// Index of the buffer side in per-side arrays.
pub const RIGHT: usize = 1;

#[derive(Clone)]
pub struct DiffView {
    /// The buffer side; its selection takes every key and its scroll drives both sides.
    pub text: TextView,
    pub left: Left,
    pub align: Align,
}

impl DiffView {
    pub(crate) fn new(buffer: BufferId) -> DiffView {
        DiffView {
            text: TextView::new(buffer),
            left: Left::Pending,
            align: Align::default(),
        }
    }
}

/// The HEAD side.
#[derive(Clone)]
pub enum Left {
    /// The buffer's bases are still being read.
    Pending,
    /// No HEAD copy: untracked, new in the index, or outside a repository.
    Absent,
    /// HEAD's blob waits for its parse.
    Parsing { oid: Oid },
    /// `scroll` follows the right side's through the alignment.
    Head {
        oid: Oid,
        doc: Arc<TextDoc>,
        scroll: Scroll,
    },
}

/// Whether the buffer side takes edits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Right {
    Live,
    /// Deleted from the worktree: drawn blank and inert.
    Deleted,
}

/// The buffer side of `content`: deleted when HEAD has the file, the worktree does not and
/// nothing was typed since.
pub fn right(content: &Content) -> Right {
    match content {
        Content::Text {
            doc,
            disk: Disk::New | Disk::Deleted,
            diff:
                Diff {
                    bases:
                        Bases::Tracked(Tracked {
                            head: Base::Blob { .. },
                            ..
                        }),
                    ..
                },
            ..
        } if doc.text.len_chars() == 0 => Right::Deleted,
        Content::Text { .. } | Content::Loading { .. } => Right::Live,
    }
}

/// Rows keeping the sides aligned; empty unless the left shows HEAD.
#[derive(Clone, Default)]
pub struct Align {
    /// What the rows were computed from; `None` while empty.
    pub key: Option<AlignKey>,
    /// Indexed by `LEFT` and `RIGHT`.
    pub sides: [SideRows; 2],
    /// Visual rows of each hunk's lines per side, parallel to the buffer's hunks.
    pub hunks: Vec<[u32; 2]>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AlignKey {
    pub stamp: Stamp,
    pub head: Oid,
    pub cols: u16,
}

/// One side's rows besides its text, and its hunk backgrounds.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SideRows {
    /// `(line, count)`: filler rows after a document line, sorted by line.
    pub fillers: Vec<(usize, u16)>,
    /// Filler rows above the first line.
    pub lead: u32,
    pub shades: Vec<Shade>,
}

impl SideRows {
    pub fn padding(&self) -> Padding<'_> {
        Padding {
            fillers: &self.fillers,
            lead: self.lead,
        }
    }
}

/// Visual rows of document `lines` under `format`, without annotations.
pub fn rows(text: RopeSlice, lines: Range<usize>, format: &TextFormat) -> u32 {
    match lines.is_empty() {
        true => return 0,
        false => {}
    }
    let start = text.line_to_char(lines.start);
    let end = match lines.end < text.len_lines() {
        true => text.line_to_char(lines.end),
        false => usize::MAX,
    };
    let annotations = TextAnnotations::default();
    let mut last = 0;
    for grapheme in DocumentFormatter::new_at_prev_checkpoint(text, format, &annotations, start) {
        match grapheme.char_idx >= end {
            true => break,
            false => last = grapheme.visual_pos.row,
        }
    }
    last as u32 + 1
}

/// Recomputes `align` for `hunks` between `left` and `right`: per hunk the shorter side gets
/// filler rows after its last hunk line, or above line 0 when the hunk starts the file.
pub fn realign(
    align: &mut Align,
    left: RopeSlice,
    right: RopeSlice,
    hunks: &[Hunk],
    format: &TextFormat,
) {
    for side in &mut align.sides {
        side.fillers.clear();
        side.lead = 0;
        side.shades.clear();
    }
    align.hunks.clear();
    // Hunks may lag one edit behind the text until the next diff lands.
    let clamp = |range: &Range<u32>, text: RopeSlice| {
        let len = text.len_lines();
        (range.start as usize).min(len)..(range.end as usize).min(len)
    };
    let pad = |side: &mut SideRows, lines: &Range<usize>, count: u32| match (count, lines.end) {
        (0, _) => {}
        (count, 0) => side.lead += count,
        (count, end) => {
            let count = u16::try_from(count).unwrap_or_else(|_| {
                tracing::warn!(count, "diff hunk taller than the filler limit");
                u16::MAX
            });
            match side.fillers.last_mut() {
                Some((line, existing)) if *line == end - 1 => {
                    *existing = existing.saturating_add(count)
                }
                Some(_) | None => side.fillers.push((end - 1, count)),
            }
        }
    };
    for hunk in hunks {
        let lines = [clamp(&hunk.before, left), clamp(&hunk.after, right)];
        let counts = [
            rows(left, lines[LEFT].clone(), format),
            rows(right, lines[RIGHT].clone(), format),
        ];
        align.hunks.push(counts);
        let [left_side, right_side] = &mut align.sides;
        pad(
            left_side,
            &lines[LEFT],
            counts[RIGHT].saturating_sub(counts[LEFT]),
        );
        pad(
            right_side,
            &lines[RIGHT],
            counts[LEFT].saturating_sub(counts[RIGHT]),
        );
        let [before, after] = lines;
        match (before.is_empty(), after.is_empty()) {
            (true, true) => {}
            (true, false) => right_side.shades.push(Shade {
                lines: after,
                role: Background::Added,
            }),
            (false, true) => left_side.shades.push(Shade {
                lines: before,
                role: Background::Removed,
            }),
            (false, false) => {
                left_side.shades.push(Shade {
                    lines: before,
                    role: Background::Modified,
                });
                right_side.shades.push(Shade {
                    lines: after,
                    role: Background::Modified,
                });
            }
        }
    }
}

/// Where a side's top row falls in the aligned stream both sides share.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Spot {
    /// Visual row `row` of a line outside every hunk, numbered as on the other side.
    Equal { line: usize, row: usize },
    /// Row `row` of hunk `hunk`, counted over its taller side.
    Hunk { hunk: usize, row: u32 },
}

fn side_index(side: Side) -> usize {
    match side {
        Side::Before => LEFT,
        Side::After => RIGHT,
    }
}

fn other(side: Side) -> Side {
    match side {
        Side::Before => Side::After,
        Side::After => Side::Before,
    }
}

/// The line facing `line` of `side` on the other side: the same line outside hunks, the
/// matching or last line inside one.
pub fn facing(hunks: &[Hunk], side: Side, line: usize) -> usize {
    let index = hunks.partition_point(|hunk| side.of(hunk).end as usize <= line);
    match hunks.get(index) {
        Some(hunk) if side.of(hunk).start as usize <= line => {
            let (from, to) = (side.of(hunk), other(side).of(hunk));
            let within = (line - from.start as usize).min(to.len().saturating_sub(1));
            to.start as usize + within
        }
        Some(_) | None => match index.checked_sub(1).map(|prev| &hunks[prev]) {
            Some(prev) => line - side.of(prev).end as usize + other(side).of(prev).end as usize,
            None => line,
        },
    }
}

/// The aligned spot of `scroll`'s top row on `side`.
pub fn spot(
    text: RopeSlice,
    scroll: Scroll,
    side: Side,
    hunks: &[Hunk],
    align: &Align,
    format: &TextFormat,
) -> Spot {
    let lead = align.sides[side_index(side)].lead as isize;
    match scroll.rows < 0 {
        // The lead holds the fillers of a first hunk with no lines on this side.
        true => {
            return Spot::Hunk {
                hunk: 0,
                row: (lead + scroll.rows).max(0) as u32,
            };
        }
        false => {}
    }
    let anchor = scroll.anchor.min(text.len_chars());
    let line = text.char_to_line(anchor);
    let annotations = TextAnnotations::default();
    let row = visual_offset_from_block(text, anchor, anchor, format, &annotations)
        .0
        .row
        + scroll.rows as usize;
    let line_rows = rows(text, line..line + 1, format) as usize;
    let index = hunks.partition_point(|hunk| side.of(hunk).end as usize <= line);
    let equal = |row: usize| match index.checked_sub(1).map(|prev| &hunks[prev]) {
        Some(prev) => Spot::Equal {
            line: line - side.of(prev).end as usize + other(side).of(prev).end as usize,
            row,
        },
        None => Spot::Equal { line, row },
    };
    let within = |hunk: &Hunk| rows(text, side.of(hunk).start as usize..line, format);
    match (row < line_rows, hunks.get(index)) {
        (true, Some(hunk)) if side.of(hunk).start as usize <= line => Spot::Hunk {
            hunk: index,
            row: within(hunk) + row as u32,
        },
        (true, Some(_) | None) => equal(row),
        // Filler rows after `line`: the last line of a hunk taller on the other side, or the
        // line before a hunk with no lines on this side.
        (false, Some(hunk)) if side.of(hunk).end as usize == line + 1 => Spot::Hunk {
            hunk: index,
            row: align
                .hunks
                .get(index)
                .map_or(0, |counts| counts[side_index(side)])
                + (row - line_rows) as u32,
        },
        // Past the line's rows where no fillers follow it; stays on its last row.
        (false, Some(hunk)) if side.of(hunk).start as usize <= line => Spot::Hunk {
            hunk: index,
            row: within(hunk) + line_rows.saturating_sub(1) as u32,
        },
        (false, Some(_) | None) => equal(line_rows.saturating_sub(1)),
    }
}

/// The scroll position putting `spot` at the top of `side`.
pub fn place(
    text: RopeSlice,
    spot: Spot,
    side: Side,
    hunks: &[Hunk],
    align: &Align,
    format: &TextFormat,
) -> (usize, isize) {
    let last = text.len_lines().saturating_sub(1);
    let (index, row) = match spot {
        Spot::Equal { line, row } => return (text.line_to_char(line.min(last)), row as isize),
        Spot::Hunk { hunk, row } => (hunk, row),
    };
    match hunks.get(index) {
        None => (0, 0),
        Some(hunk) => {
            let lines = side.of(hunk);
            let lines = (lines.start as usize).min(last + 1)..(lines.end as usize).min(last + 1);
            let own = align
                .hunks
                .get(index)
                .map_or(0, |counts| counts[side_index(side)]);
            match (row < own, lines.end) {
                (true, _) => {
                    let mut left = row;
                    for line in lines.clone() {
                        let height = rows(text, line..line + 1, format);
                        match left < height {
                            true => return (text.line_to_char(line), left as isize),
                            false => left -= height,
                        }
                    }
                    (text.line_to_char(lines.end.saturating_sub(1)), 0)
                }
                (false, 0) => (
                    0,
                    (row - own) as isize - align.sides[side_index(side)].lead as isize,
                ),
                (false, end) => {
                    let height = rows(text, end - 1..end, format);
                    (text.line_to_char(end - 1), (height + row - own) as isize)
                }
            }
        }
    }
}

/// The diff view active in `pane` with its buffer's text and tracked bases.
fn active<'a>(
    layout: &'a mut Layout,
    buffers: &'a SlotMap<BufferId, Buffer>,
    pane: PaneId,
) -> Option<(&'a mut DiffView, &'a TextDoc, &'a Tracked)> {
    let View::Diff(diff) = layout.panes.get_mut(pane)?.active_mut()? else {
        return None;
    };
    match &buffers.get(diff.text.buffer())?.content {
        Content::Text {
            doc,
            diff:
                Diff {
                    bases: Bases::Tracked(tracked),
                    ..
                },
            ..
        } => Some((diff, doc, tracked)),
        Content::Text { .. } | Content::Loading { .. } => None,
    }
}

/// Realigns the diff in `pane` if its inputs moved and scrolls the left side to face the
/// right side's top row, after anything moves the right side's scroll.
pub fn sync_left(state: &mut State, pane: PaneId) {
    let State {
        layout,
        buffers,
        languages,
        ..
    } = state;
    let Some((diff, doc, tracked)) = active(layout, buffers, pane) else {
        return;
    };
    let DiffView { text, left, align } = diff;
    let Left::Head {
        oid,
        doc: head,
        scroll,
    } = left
    else {
        return;
    };
    let viewport = text.scroll.viewport;
    let format = text_format(viewport, Indent::of(doc, languages));
    // Hunks against an older HEAD than the one shown cannot align it.
    let key = match tracked.stamp {
        Some(stamp) if stamp.bases[0] == Some(*oid) => Some(AlignKey {
            stamp,
            head: *oid,
            cols: viewport.cols,
        }),
        Some(_) | None => None,
    };
    let first = align.key.is_none();
    match (key, align.key == key) {
        (_, true) => {}
        (None, false) => *align = Align::default(),
        (Some(key), false) => {
            realign(
                align,
                head.text.slice(..),
                doc.text.slice(..),
                &tracked.hunks,
                &format,
            );
            align.key = Some(key);
        }
    }
    // A view still at the top when HEAD first shows starts above the filler rows there, and
    // no view stays above rows that are gone.
    let top = -(align.sides[RIGHT].lead as isize);
    text.scroll.rows = match (first, align.key, text.scroll.anchor, text.scroll.rows) {
        (true, Some(_), 0, 0) => top,
        (_, _, _, rows) => rows.max(top),
    };
    let hunks: &[Hunk] = match align.key {
        Some(_) => &tracked.hunks,
        None => &[],
    };
    let at = spot(
        doc.text.slice(..),
        text.scroll,
        Side::After,
        hunks,
        align,
        &format,
    );
    let (anchor, rows) = place(head.text.slice(..), at, Side::Before, hunks, align, &format);
    *scroll = Scroll {
        anchor,
        rows,
        viewport,
    };
}

/// What the left side should show for the buffer's current bases.
enum Wanted<'a> {
    Pending,
    Absent,
    Head {
        oid: Oid,
        text: &'a helix_core::Rope,
    },
}

/// Brings the left side of the diff active in `pane` up to date with its buffer's HEAD copy,
/// starting a parse when HEAD moved, then realigns.
pub fn refresh_pane(state: &mut State, pane: PaneId, out: &mut Vec<Effect>) {
    let Some(View::Diff(diff)) = state.layout.panes.get(pane).and_then(Pane::active) else {
        return;
    };
    let buffer = diff.text.buffer();
    let Some(entry) = state.buffers.get(buffer) else {
        return;
    };
    let wanted = match &entry.content {
        Content::Loading { .. } => return,
        Content::Text { diff, .. } => match &diff.bases {
            Bases::Loading => Wanted::Pending,
            Bases::NoRepo | Bases::Untracked => Wanted::Absent,
            Bases::Tracked(tracked) => match &tracked.head {
                Base::Absent => Wanted::Absent,
                Base::Blob { oid, text } => Wanted::Head { oid: *oid, text },
            },
        },
    };
    let next = match (wanted, &diff.left) {
        (Wanted::Pending, _) | (Wanted::Absent, Left::Absent) => None,
        (Wanted::Absent, Left::Pending | Left::Parsing { .. } | Left::Head { .. }) => {
            Some(Left::Absent)
        }
        (
            Wanted::Head { oid, .. },
            Left::Parsing { oid: shown } | Left::Head { oid: shown, .. },
        ) if oid == *shown => None,
        (
            Wanted::Head { oid, text },
            Left::Pending | Left::Absent | Left::Parsing { .. } | Left::Head { .. },
        ) => {
            // Another view of the same blob shares its parse, done or under way.
            let shared = (state.layout.panes.values())
                .flat_map(Pane::items)
                .find_map(|item| match &item.view {
                    View::Diff(other) if other.text.buffer() == buffer => match &other.left {
                        Left::Head {
                            oid: theirs, doc, ..
                        } if *theirs == oid => Some(Left::Head {
                            oid,
                            doc: doc.clone(),
                            scroll: Scroll::default(),
                        }),
                        Left::Parsing { oid: theirs } if *theirs == oid => {
                            Some(Left::Parsing { oid })
                        }
                        Left::Pending | Left::Absent | Left::Parsing { .. } | Left::Head { .. } => {
                            None
                        }
                    },
                    View::Diff(_) | View::Text(_) => None,
                });
            Some(match shared {
                Some(left) => left,
                None => {
                    out.push(Effect::Job(Job::ParseHead {
                        buffer,
                        oid,
                        text: text.clone(),
                        path: entry.path.clone(),
                        languages: state.languages.clone(),
                    }));
                    Left::Parsing { oid }
                }
            })
        }
    };
    match (
        next,
        state.layout.panes.get_mut(pane).and_then(Pane::active_mut),
    ) {
        (Some(left), Some(View::Diff(diff))) => {
            diff.left = left;
            diff.align = Align::default();
        }
        (Some(_), Some(View::Text(_)) | None) | (None, _) => {}
    }
    sync_left(state, pane);
}

/// Refreshes the diff shown in every pane.
pub fn refresh(state: &mut State, out: &mut Vec<Effect>) {
    let panes: SmallVec<[PaneId; 8]> = state.layout.panes.keys().collect();
    for pane in panes {
        refresh_pane(state, pane, out);
    }
}

/// Hands a parsed HEAD copy to every diff view of `buffer` waiting for blob `oid`.
pub fn head_parsed(state: &mut State, buffer: BufferId, oid: Oid, doc: TextDoc) {
    let doc = Arc::new(doc);
    for pane in state.layout.panes.values_mut() {
        for item in pane.items_mut() {
            match &mut item.view {
                View::Diff(diff) if diff.text.buffer() == buffer => match diff.left {
                    Left::Parsing { oid: waiting } if waiting == oid => {
                        diff.left = Left::Head {
                            oid,
                            doc: doc.clone(),
                            scroll: Scroll::default(),
                        };
                        diff.align = Align::default();
                    }
                    Left::Pending | Left::Absent | Left::Parsing { .. } | Left::Head { .. } => {}
                },
                View::Diff(_) | View::Text(_) => {}
            }
        }
    }
}

/// Records the drawn size of the diff in `pane` for both sides, then refreshes it.
pub fn prepare(state: &mut State, pane: PaneId, viewport: Viewport, out: &mut Vec<Effect>) {
    match state.layout.panes.get_mut(pane).and_then(Pane::active_mut) {
        Some(View::Diff(diff)) if diff.text.scroll.viewport != viewport => {
            diff.text.scroll.viewport = viewport;
            scroll::follow_cursor(state, pane);
        }
        Some(View::Diff(_) | View::Text(_)) | None => {}
    }
    refresh_pane(state, pane, out);
}

/// A click on the HEAD side puts the cursor on the buffer line facing the clicked line.
pub fn click_left(state: &mut State, pane: PaneId, row: u16, col: u16) {
    state.layout.focus = pane;
    let State {
        layout,
        buffers,
        languages,
        ..
    } = state;
    let Some((diff, doc, tracked)) = active(layout, buffers, pane) else {
        return;
    };
    let content = &buffers[diff.text.buffer()].content;
    let (
        Left::Head {
            doc: head, scroll, ..
        },
        Right::Live,
    ) = (&diff.left, right(content))
    else {
        return;
    };
    let format = text_format(diff.text.scroll.viewport, Indent::of(doc, languages));
    let head_text = head.text.slice(..);
    let annotations = scroll::annotations(head_text, diff.align.sides[LEFT].padding());
    let pos = scroll::char_at(head_text, *scroll, row, col, &format, &annotations);
    let hunks: &[Hunk] = match diff.align.key {
        Some(_) => &tracked.hunks,
        None => &[],
    };
    let text = doc.text.slice(..);
    let line = facing(hunks, Side::Before, head_text.char_to_line(pos))
        .min(text.len_lines().saturating_sub(1));
    diff.text.selection = Selection::point(text.line_to_char(line));
    drop(annotations);
    scroll::follow_cursor(state, pane);
}
