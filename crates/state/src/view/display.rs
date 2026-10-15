use std::ops::Range;

use helix_core::doc_formatter::{DocumentFormatter, GraphemeSource, TextFormat};
use helix_core::graphemes::Grapheme;
use helix_core::indent::IndentStyle;
use helix_core::syntax::{HighlightEvent, Highlighter, Loader};
use helix_core::text_annotations::{LineAnnotation, TextAnnotations};
use helix_core::{Position, RopeSlice, Selection, visual_offset_from_block};
use helix_stdx::rope::RopeSliceExt;
use model::mode::Mode;
use model::theme::{self, Background, Style};

use crate::text::buffer::TextDoc;
use crate::view::layout::{Scroll, Viewport};

/// Tab stops and indent-guide spacing of a document, in columns.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Indent {
    pub tab_width: u16,
    pub unit: u16,
}

impl Indent {
    pub fn of(doc: &TextDoc, loader: &Loader) -> Indent {
        let config = doc.syntax.as_ref().and_then(|syntax| {
            loader
                .language(syntax.root_language())
                .config()
                .indent
                .as_ref()
        });
        match config {
            Some(config) => {
                let tab_width = config.tab_width.max(1) as u16;
                let unit = match IndentStyle::from_str(&config.unit) {
                    IndentStyle::Tabs => tab_width,
                    IndentStyle::Spaces(width) => width as u16,
                };
                Indent { tab_width, unit }
            }
            None => Indent {
                tab_width: 4,
                unit: 4,
            },
        }
    }
}

/// Soft-wrapping format for a viewport, matching helix's defaults.
pub fn text_format(viewport: Viewport, indent: Indent) -> TextFormat {
    let width = viewport.cols;
    TextFormat {
        soft_wrap: width > 10,
        tab_width: indent.tab_width,
        max_wrap: 20.min(width / 4),
        max_indent_retain: 40.min(width * 2 / 5),
        wrap_indicator: Box::from("↪ "),
        wrap_indicator_highlight: None,
        viewport_width: width,
        soft_wrap_at_text_width: false,
    }
}

/// Gutter columns in helix's default order: diagnostic, spacer, line number, spacer, diff sign.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Gutter {
    pub digits: u16,
    pub cols: u16,
}

impl Gutter {
    pub const NUMBER_COL: u16 = 2;

    pub fn new(len_lines: usize) -> Gutter {
        let digits = (len_lines.max(1).ilog10() + 1).max(3) as u16;
        Gutter {
            digits,
            cols: digits + 4,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Numbering {
    Absolute,
    Relative,
}

/// Number shown in the gutter for `line`; the cursor line stays absolute when relative.
pub fn line_label(line: usize, primary_line: usize, numbering: Numbering) -> usize {
    match numbering {
        Numbering::Relative if line != primary_line => line.abs_diff(primary_line),
        Numbering::Relative | Numbering::Absolute => line + 1,
    }
}

/// Git change marker for a document line.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DiffSign {
    Added,
    Modified,
    Removed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Focus {
    Focused(Mode),
    Unfocused,
    /// Draws no cursors or selections, like the HEAD side of a diff.
    ReadOnly,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rank {
    Primary,
    Secondary,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CursorShape {
    Block,
    Bar,
    Underline,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RowKind {
    First,
    Wrapped,
    /// A row inserted by a line annotation after `Row::line`, counted from 0 within its run.
    Virtual(u16),
}

/// Whether any selection's cursor sits on the row's document line.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LineMark {
    Plain,
    Cursor,
}

/// One visual row; `text` and `runs` index into the owning `Frame`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Row {
    pub line: usize,
    pub kind: RowKind,
    pub mark: LineMark,
    pub text: Range<usize>,
    pub runs: Range<usize>,
    pub background: Option<Background>,
}

/// A background role over document lines; `layout` takes them sorted and non-overlapping.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Shade {
    pub lines: Range<usize>,
    pub role: Background,
}

/// `len` bytes of row text drawn in `style`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Run {
    pub len: usize,
    pub style: Style,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CellSpan {
    pub row: u16,
    pub cols: Range<u16>,
    pub rank: Rank,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cursor {
    pub row: u16,
    pub col: u16,
    pub width: u16,
    pub rank: Rank,
    pub shape: CursorShape,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LeadKind {
    Space,
    Tab,
    Blank,
}

/// Leading whitespace cell held back until the row's indent level is known.
#[derive(Clone, Copy, Debug)]
struct Lead {
    col: u16,
    width: u16,
    kind: LeadKind,
    style: Style,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Area {
    Indent,
    Text,
}

/// Visible rows of a text view, rebuilt in place each frame.
#[derive(Debug, Default)]
pub struct Frame {
    pub rows: Vec<Row>,
    pub text: String,
    pub runs: Vec<Run>,
    pub selections: Vec<CellSpan>,
    pub cursors: Vec<Cursor>,
    pub primary_line: usize,
    lead: Vec<Lead>,
}

/// Lays out the rows that fit `scroll`'s viewport into `frame`. Line annotations in
/// `annotations` become `RowKind::Virtual` rows shaded as filler; `shades` colour text rows.
pub fn layout(
    frame: &mut Frame,
    doc: &TextDoc,
    selection: &Selection,
    scroll: Scroll,
    loader: &Loader,
    focus: Focus,
    annotations: &TextAnnotations,
    shades: &[Shade],
) {
    frame.rows.clear();
    frame.text.clear();
    frame.runs.clear();
    frame.selections.clear();
    frame.cursors.clear();
    frame.lead.clear();

    let text = doc.text.slice(..);
    let ranges: &[helix_core::Range] = match focus {
        Focus::ReadOnly => &[],
        Focus::Focused(_) | Focus::Unfocused => selection.ranges(),
    };
    let primary = selection.primary_index();
    frame.primary_line = text.char_to_line(selection.primary().cursor(text));
    let viewport = scroll.viewport;
    match viewport {
        Viewport { rows: 0, .. } | Viewport { cols: 0, .. } => return,
        Viewport { .. } => {}
    }

    let indent = Indent::of(doc, loader);
    let unit = indent.unit.max(1);
    let format = text_format(viewport, indent);
    let anchor = scroll.anchor.min(text.len_chars());
    // Negative with filler rows above the first line on screen.
    let row_off = visual_offset_from_block(text, anchor, anchor, &format, annotations)
        .0
        .row as isize
        + scroll.rows;
    let first_line = text.char_to_line(anchor);
    // Rows skipped past the anchor are its own wrapped rows or annotations, so one more line
    // covers them.
    let end_line = (first_line + viewport.rows as usize + 2).min(text.len_lines());
    let mut highlighter = doc.syntax.as_ref().map(|syntax| {
        syntax.highlighter(
            text,
            loader,
            text.line_to_byte(first_line) as u32..text.line_to_byte(end_line) as u32,
        )
    });
    let mut next_event = event_char(&highlighter, text);
    let mut syntax = theme::TEXT;

    let mut sel_i = 0;
    let mut cur_i = 0;
    let mut next_cursor = ranges.first().map_or(usize::MAX, |r| r.cursor(text));
    let mut mark_line = usize::MAX;
    let mut mark = LineMark::Plain;
    let mut level: u16 = 0;
    let mut area = Area::Indent;
    let mut text_line = usize::MAX;
    let mut shade_i = 0;

    let formatter = DocumentFormatter::new_at_prev_checkpoint(text, &format, annotations, anchor);
    for g in formatter {
        while g.char_idx >= next_event {
            match highlighter.as_mut() {
                Some(h) => {
                    let (event, active) = h.advance();
                    let base = match event {
                        HighlightEvent::Refresh => theme::TEXT,
                        HighlightEvent::Push => syntax,
                    };
                    syntax = active.fold(base, |acc, highlight| {
                        acc.patch(theme::SCOPES[highlight.idx()].1)
                    });
                }
                None => break,
            }
            next_event = event_char(&highlighter, text);
        }
        let row = match usize::try_from(g.visual_pos.row as isize - row_off) {
            Ok(row) => row,
            Err(_) => continue,
        };
        // Every text row holds a grapheme, so rows skipped over come from line annotations.
        if row >= viewport.rows as usize {
            push_virtual(frame, viewport.rows as usize, &mut area, level, unit);
            break;
        }
        push_virtual(frame, row, &mut area, level, unit);
        let row = row as u16;
        let col = g.visual_pos.col as u16;
        let width = g.width() as u16;

        if frame.rows.len() == row as usize {
            match frame.rows.last() {
                Some(last) => finish_row(frame, last.runs.start, area, level, unit),
                None => {}
            }
            if g.line_idx != mark_line {
                mark_line = g.line_idx;
                let start = text.line_to_char(g.line_idx);
                let end = text.line_to_char((g.line_idx + 1).min(text.len_lines()));
                let first = ranges.partition_point(|r| r.cursor(text) < start);
                mark = match ranges.get(first).map(|r| r.cursor(text)) {
                    Some(cursor) if cursor < end || end == text.len_chars() => LineMark::Cursor,
                    Some(_) | None => LineMark::Plain,
                };
            }
            let kind = match text_line == g.line_idx {
                true => RowKind::Wrapped,
                false => RowKind::First,
            };
            text_line = g.line_idx;
            while shades
                .get(shade_i)
                .is_some_and(|shade| shade.lines.end <= g.line_idx)
            {
                shade_i += 1;
            }
            let background = match shades.get(shade_i) {
                Some(shade) if shade.lines.start <= g.line_idx => Some(shade.role),
                Some(_) | None => None,
            };
            frame.rows.push(Row {
                line: g.line_idx,
                kind,
                mark,
                text: frame.text.len()..frame.text.len(),
                runs: frame.runs.len()..frame.runs.len(),
                background,
            });
            area = Area::Indent;
        }

        let source = match g.source {
            GraphemeSource::Document { codepoints: 0 } => Source::Eof,
            GraphemeSource::Document { .. } => Source::Text,
            GraphemeSource::VirtualText { .. } => Source::Virtual,
        };

        match source {
            Source::Text => {
                while sel_i < ranges.len() && ranges[sel_i].to() <= g.char_idx {
                    sel_i += 1;
                }
                match ranges.get(sel_i) {
                    Some(range) if range.from() <= g.char_idx => {
                        let rank = match sel_i == primary {
                            true => Rank::Primary,
                            false => Rank::Secondary,
                        };
                        match frame.selections.last_mut() {
                            Some(span)
                                if span.row == row && span.rank == rank && span.cols.end == col =>
                            {
                                span.cols.end = col + width;
                            }
                            Some(_) | None => frame.selections.push(CellSpan {
                                row,
                                cols: col..col + width,
                                rank,
                            }),
                        }
                    }
                    Some(_) | None => {}
                }
            }
            Source::Eof | Source::Virtual => {}
        }
        match source {
            Source::Text | Source::Eof => {
                while next_cursor < g.char_idx {
                    cur_i += 1;
                    next_cursor = ranges.get(cur_i).map_or(usize::MAX, |r| r.cursor(text));
                }
                if next_cursor == g.char_idx {
                    let rank = match cur_i == primary {
                        true => Rank::Primary,
                        false => Rank::Secondary,
                    };
                    let shape = match (focus, rank) {
                        (Focus::Focused(Mode::Normal), Rank::Primary) => CursorShape::Block,
                        (Focus::Focused(Mode::Insert), Rank::Primary) => CursorShape::Bar,
                        (Focus::Focused(Mode::Select), Rank::Primary) => CursorShape::Underline,
                        (Focus::Focused(_), Rank::Secondary)
                        | (Focus::Unfocused | Focus::ReadOnly, _) => CursorShape::Block,
                    };
                    frame.cursors.push(Cursor {
                        row,
                        col,
                        width: width.max(1),
                        rank,
                        shape,
                    });
                }
            }
            Source::Virtual => {}
        }

        let cell = match (&g.raw, source) {
            (Grapheme::Newline, _) | (_, Source::Eof) => continue,
            (Grapheme::Tab { width }, _) => Cell::Tab(*width),
            (Grapheme::Other { g }, Source::Virtual) => Cell::Virtual(g),
            (Grapheme::Other { g }, Source::Text) => match &**g {
                " " => Cell::Space,
                s if s.chars().all(char::is_whitespace) => Cell::Blank,
                s => Cell::Glyph(s),
            },
        };
        let runs_start = frame.rows[row as usize].runs.start;
        let whitespace = syntax.patch(theme::WHITESPACE);
        let lead = match cell {
            Cell::Space => Some(LeadKind::Space),
            Cell::Blank => Some(LeadKind::Blank),
            Cell::Tab(_) => Some(LeadKind::Tab),
            Cell::Glyph(_) | Cell::Virtual(_) => None,
        };
        match (area, lead) {
            (Area::Indent, Some(kind)) => {
                frame.lead.push(Lead {
                    col,
                    width,
                    kind,
                    style: whitespace,
                });
                continue;
            }
            (Area::Indent, None) => {
                level = col;
                flush_lead(frame, runs_start, row, col, level, unit);
                area = Area::Text;
            }
            (Area::Text, _) => {}
        }
        let (s, style) = match cell {
            Cell::Space => ("·", whitespace),
            Cell::Blank => (" ", whitespace),
            Cell::Tab(_) => ("→", whitespace),
            Cell::Glyph(s) => (s, syntax),
            Cell::Virtual(s) => (s, theme::TEXT),
        };
        let style = cell_style(&frame.cursors, row, col, style);
        emit(&mut frame.text, &mut frame.runs, runs_start, s, style);
        if let Cell::Tab(width) = cell {
            for _ in 1..width {
                emit(&mut frame.text, &mut frame.runs, runs_start, " ", style);
            }
        }
    }
    if let Some(last) = frame.rows.last() {
        finish_row(frame, last.runs.start, area, level, unit);
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Source {
    Text,
    Eof,
    Virtual,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Cell<'a> {
    Space,
    Blank,
    Tab(usize),
    Glyph(&'a str),
    Virtual(&'a str),
}

fn event_char(highlighter: &Option<Highlighter<'_>>, text: RopeSlice) -> usize {
    match highlighter.as_ref().map(|h| h.next_event_offset()) {
        Some(u32::MAX) | None => usize::MAX,
        Some(byte) => text.byte_to_char(text.ceil_char_boundary(byte as usize)),
    }
}

fn emit(text: &mut String, runs: &mut Vec<Run>, row_runs: usize, s: &str, style: Style) {
    text.push_str(s);
    let row_has_runs = runs.len() > row_runs;
    match runs.last_mut() {
        Some(run) if row_has_runs && run.style == style => run.len += s.len(),
        Some(_) | None => runs.push(Run {
            len: s.len(),
            style,
        }),
    }
}

/// Swaps the text colour for the cursor's under a block cursor.
fn cell_style(cursors: &[Cursor], row: u16, col: u16, style: Style) -> Style {
    let block = cursors
        .iter()
        .rev()
        .take_while(|c| c.row == row)
        .find(|c| c.col == col && c.shape == CursorShape::Block);
    match block.map(|c| c.rank) {
        Some(Rank::Primary) => style.patch(Style::fg(theme::CURSOR_PRIMARY.fg)),
        Some(Rank::Secondary) => style.patch(Style::fg(theme::CURSOR_SECONDARY.fg)),
        None => style,
    }
}

/// Emits held-back leading whitespace up to `end`, with guides every `unit` columns below `level`.
fn flush_lead(frame: &mut Frame, runs_start: usize, row: u16, end: u16, level: u16, unit: u16) {
    let guides_end = level / unit * unit;
    let mut lead = 0;
    for col in 0..end {
        while lead < frame.lead.len() && frame.lead[lead].col + frame.lead[lead].width <= col {
            lead += 1;
        }
        let cell = frame.lead.get(lead).filter(|l| l.col <= col).copied();
        let (s, style) = match (col % unit == 0 && col < guides_end, cell) {
            (true, _) => ("│", theme::INDENT_GUIDE),
            (false, Some(l)) => match (l.kind, l.col == col) {
                (LeadKind::Space, _) => ("·", l.style),
                (LeadKind::Tab, true) => ("→", l.style),
                (LeadKind::Tab, false) | (LeadKind::Blank, _) => (" ", l.style),
            },
            (false, None) => (" ", theme::TEXT),
        };
        let style = cell_style(&frame.cursors, row, col, style);
        emit(&mut frame.text, &mut frame.runs, runs_start, s, style);
    }
    frame.lead.clear();
}

fn finish_row(frame: &mut Frame, runs_start: usize, area: Area, level: u16, unit: u16) {
    let row = frame.rows.len() - 1;
    match area {
        Area::Indent => {
            let held = frame.lead.last().map_or(0, |l| l.col + l.width);
            let last_guide = (level / unit * unit)
                .checked_sub(unit)
                .map_or(0, |col| col + 1);
            let end = held.max(last_guide);
            flush_lead(frame, runs_start, row as u16, end, level, unit);
        }
        Area::Text => {}
    }
    let last = &mut frame.rows[row];
    last.text.end = frame.text.len();
    last.runs.end = frame.runs.len();
}

/// Pads `frame` to `end` rows with annotation rows following the last row's line.
fn push_virtual(frame: &mut Frame, end: usize, area: &mut Area, level: u16, unit: u16) {
    while frame.rows.len() < end {
        let (line, index) = match frame.rows.last() {
            Some(last) => {
                finish_row(frame, last.runs.start, *area, level, unit);
                let last = &frame.rows[frame.rows.len() - 1];
                match last.kind {
                    RowKind::Virtual(index) => (last.line, index + 1),
                    RowKind::First | RowKind::Wrapped => (last.line, 0),
                }
            }
            None => (0, 0),
        };
        frame.rows.push(Row {
            line,
            kind: RowKind::Virtual(index),
            mark: LineMark::Plain,
            text: frame.text.len()..frame.text.len(),
            runs: frame.runs.len()..frame.runs.len(),
            background: Some(Background::Filler),
        });
        *area = Area::Text;
    }
}

/// Fills `frame` with `rows` filler rows, for a diff side with no text to show.
pub fn blank(frame: &mut Frame, rows: u16) {
    frame.rows.clear();
    frame.text.clear();
    frame.runs.clear();
    frame.selections.clear();
    frame.cursors.clear();
    frame.rows.extend((0..rows).map(|index| Row {
        line: 0,
        kind: RowKind::Virtual(index),
        mark: LineMark::Plain,
        text: 0..0,
        runs: 0..0,
        background: Some(Background::Filler),
    }));
}

/// Inserts `count` virtual rows after each listed document line; entries are sorted by line.
/// Rows after a soft-wrapped line follow its last visual row.
pub struct Fillers<'a> {
    pub text: RopeSlice<'a>,
    pub after: &'a [(usize, u16)],
    next: usize,
}

impl<'a> Fillers<'a> {
    pub fn new(text: RopeSlice<'a>, after: &'a [(usize, u16)]) -> Fillers<'a> {
        Fillers {
            text,
            after,
            next: 0,
        }
    }
}

impl LineAnnotation for Fillers<'_> {
    fn reset_pos(&mut self, char_idx: usize) -> usize {
        let line = self.text.char_to_line(char_idx.min(self.text.len_chars()));
        self.next = self.after.partition_point(|&(after, _)| after < line);
        usize::MAX
    }

    fn insert_virtual_lines(
        &mut self,
        line_end_char_idx: usize,
        _line_end_visual_pos: Position,
        doc_line: usize,
    ) -> Position {
        // Called after soft wraps too; only the end of the document line counts.
        let line_end = self
            .text
            .line_to_char((doc_line + 1).min(self.text.len_lines()));
        while self
            .after
            .get(self.next)
            .is_some_and(|&(after, _)| after < doc_line)
        {
            self.next += 1;
        }
        match self.after.get(self.next) {
            Some(&(after, count)) if after == doc_line && line_end == line_end_char_idx => {
                self.next += 1;
                Position::new(count as usize, 0)
            }
            Some(_) | None => Position::new(0, 0),
        }
    }
}
