//! Merge conflict markers in buffer text: parsing, display data and resolution.

use std::fmt::Write as _;
use std::ops::Range;

use helix_core::line_ending::{get_line_ending, get_line_ending_of_str};
use helix_core::text_annotations::TextAnnotations;
use helix_core::{RopeSlice, Tendril, Transaction};
use helix_stdx::rope::RopeSliceExt;
use model::command::{Command, Direction, Take};
use model::mode::Mode;
use model::notice::Notice;
use model::theme::{self, Background};

use crate::State;
use crate::apply::{count_of, edit, exit_select_mode};
use crate::input::keymap::{self, Context};
use crate::text::buffer::{Content, TextDoc};
use crate::view::display::{Frame, LineMark, Row, RowKind, Run, Shade};
use crate::view::layout::TextView;
use crate::view::scroll::Padding;

/// Length of every marker: `<<<<<<<`, `|||||||`, `=======`, `>>>>>>>`.
const MARKER: usize = 7;

/// A conflict by its marker lines; each section lies between two of them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Conflict {
    /// `<<<<<<<` line; the rest of it labels ours.
    pub open: usize,
    /// `|||||||` line of a diff3 conflict; the rest of it labels the base.
    pub base: Option<usize>,
    /// `=======` line.
    pub separator: usize,
    /// `>>>>>>>` line; the rest of it labels theirs.
    pub close: usize,
}

impl Conflict {
    pub fn ours(&self) -> Range<usize> {
        self.open + 1..self.base.unwrap_or(self.separator)
    }

    pub fn base_lines(&self) -> Option<Range<usize>> {
        self.base.map(|base| base + 1..self.separator)
    }

    pub fn theirs(&self) -> Range<usize> {
        self.separator + 1..self.close
    }

    /// Every line of the conflict, markers included.
    pub fn lines(&self) -> Range<usize> {
        self.open..self.close + 1
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    Ours,
    Theirs,
}

/// A label row drawn after document `line`, naming `side` from the text of its `marker` line.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Label {
    pub line: usize,
    pub side: Side,
    pub marker: usize,
}

/// A buffer's conflicts and their display data, parsed again whenever the text version moves.
#[derive(Debug)]
pub struct Conflicts {
    /// `TextDoc::version` the lists describe.
    pub version: u64,
    pub list: Vec<Conflict>,
    /// Row backgrounds, sorted and non-overlapping as `display::layout` takes them.
    pub shades: Vec<Shade>,
    /// Label rows per document line, as `Fillers` takes them.
    pub rows: Vec<(usize, u16)>,
    /// One per label row, in display order.
    pub labels: Vec<Label>,
}

impl Conflicts {
    pub const EMPTY: Conflicts = Conflicts {
        version: 0,
        list: Vec::new(),
        shades: Vec::new(),
        rows: Vec::new(),
        labels: Vec::new(),
    };
}

static NONE: Conflicts = Conflicts::EMPTY;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Marker {
    Open,
    Base,
    Separator,
    Close,
}

/// Markers seen so far of the conflict being read.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Open {
    None,
    Ours {
        open: usize,
    },
    Base {
        open: usize,
        base: usize,
    },
    Theirs {
        open: usize,
        base: Option<usize>,
        separator: usize,
    },
}

/// Fills `conflicts` from `text`. Only lines starting with a marker character are looked at;
/// a new `<<<<<<<` abandons an unfinished conflict, as zed does, and unterminated ones are dropped.
pub fn parse(text: RopeSlice, version: u64, conflicts: &mut Conflicts) {
    let Conflicts {
        list,
        shades,
        rows,
        labels,
        ..
    } = conflicts;
    conflicts.version = version;
    list.clear();
    shades.clear();
    rows.clear();
    labels.clear();

    // Every conflict has a `=======` line, and a substring search rules most texts out quickly.
    let mut run = 0;
    let separated = text.chunks().any(|chunk| {
        let bytes = chunk.as_bytes();
        let leading = bytes.iter().take_while(|&&b| b == b'=').count();
        let found = run + leading >= MARKER || chunk.contains("=======");
        run = match leading == bytes.len() {
            true => run + leading,
            false => bytes.iter().rev().take_while(|&&b| b == b'=').count(),
        };
        found
    });
    match separated {
        true => {}
        false => return,
    }

    let mut open = Open::None;
    let mut offset = 0;
    let mut line_start = true;
    for chunk in text.chunks() {
        let mut at = 0;
        loop {
            match (line_start, chunk.as_bytes().get(at)) {
                (true, Some(b'<' | b'|' | b'=' | b'>')) => {
                    let line = text.byte_to_line(offset + at);
                    let mut chars = text.line(line).chars();
                    let first = chars.next();
                    let repeated = chars
                        .by_ref()
                        .take(MARKER - 1)
                        .filter(|&c| Some(c) == first);
                    let marker = match (
                        first,
                        repeated.count() == MARKER - 1,
                        chars.next().is_none_or(char::is_whitespace),
                    ) {
                        (Some('<'), true, true) => Some(Marker::Open),
                        (Some('|'), true, true) => Some(Marker::Base),
                        (Some('='), true, true) => Some(Marker::Separator),
                        (Some('>'), true, true) => Some(Marker::Close),
                        (_, _, _) => None,
                    };
                    open = match (open, marker) {
                        (_, Some(Marker::Open)) => Open::Ours { open: line },
                        (Open::Ours { open }, Some(Marker::Base)) => {
                            Open::Base { open, base: line }
                        }
                        (Open::Ours { open }, Some(Marker::Separator)) => Open::Theirs {
                            open,
                            base: None,
                            separator: line,
                        },
                        (Open::Base { open, base }, Some(Marker::Separator)) => Open::Theirs {
                            open,
                            base: Some(base),
                            separator: line,
                        },
                        (
                            Open::Theirs {
                                open,
                                base,
                                separator,
                            },
                            Some(Marker::Close),
                        ) => {
                            list.push(Conflict {
                                open,
                                base,
                                separator,
                                close: line,
                            });
                            Open::None
                        }
                        (open, Some(_) | None) => open,
                    };
                }
                (true, _) | (false, _) => {}
            }
            match chunk[at..].find('\n') {
                Some(end) => {
                    at += end + 1;
                    line_start = true;
                }
                None => {
                    line_start = line_start && at == chunk.len();
                    break;
                }
            }
        }
        offset += chunk.len();
    }

    for conflict in list.iter() {
        let mut shade = |lines: Range<usize>, role: Background| match lines.is_empty() {
            true => {}
            false => shades.push(Shade { lines, role }),
        };
        shade(conflict.open..conflict.open + 1, Background::OursMarker);
        shade(conflict.ours(), Background::Ours);
        match (conflict.base, conflict.base_lines()) {
            (Some(base), Some(lines)) => {
                shade(base..base + 1, Background::BaseMarker);
                shade(lines, Background::Base);
            }
            (_, _) => {}
        }
        shade(
            conflict.separator..conflict.separator + 1,
            Background::TheirsMarker,
        );
        shade(conflict.theirs(), Background::Theirs);
        shade(conflict.close..conflict.close + 1, Background::TheirsMarker);

        // No row fits above line 0, so that label goes below the marker instead.
        let above = conflict.open.saturating_sub(1);
        for label in [
            Label {
                line: above,
                side: Side::Ours,
                marker: conflict.open,
            },
            Label {
                line: conflict.close,
                side: Side::Theirs,
                marker: conflict.close,
            },
        ] {
            match rows.last_mut() {
                Some((line, count)) if *line == label.line => *count += 1,
                Some(_) | None => rows.push((label.line, 1)),
            }
            labels.push(label);
        }
    }
}

/// Parses the buffer's conflicts again when its text changed since the last parse.
pub fn refresh(doc: &mut TextDoc) {
    match doc.conflicts.version == doc.version {
        true => {}
        false => parse(doc.text.slice(..), doc.version, &mut doc.conflicts),
    }
}

/// Re-parses every buffer edited since its last parse; run after each batch of commands.
pub fn sync(state: &mut State) {
    for buffer in state.buffers.values_mut() {
        match &mut buffer.content {
            Content::Text { doc, .. } => refresh(doc),
            Content::Loading { .. } => {}
        }
    }
}

/// The conflicts of `doc`'s current text; stale ones read as none until the next sync.
pub fn current(doc: &TextDoc) -> &Conflicts {
    match doc.conflicts.version == doc.version {
        true => &doc.conflicts,
        false => &NONE,
    }
}

/// Label rows as scroll padding, so cursor follow and clicks count them.
pub fn padding(doc: &TextDoc) -> Padding<'_> {
    Padding {
        fillers: &current(doc).rows,
        lead: 0,
    }
}

/// Line annotations inserting the label rows; empty without conflicts.
pub fn annotations(doc: &TextDoc) -> TextAnnotations<'_> {
    crate::view::scroll::annotations(doc.text.slice(..), padding(doc))
}

/// Writes the label text into the virtual rows `display::layout` left for `conflicts`, in a
/// view `height` rows tall.
pub fn label(frame: &mut Frame, text: RopeSlice, conflicts: &Conflicts, height: u16) {
    match conflicts.labels.is_empty() {
        true => return,
        false => {}
    }
    // Rows after the last line follow no grapheme, so layout never reaches them.
    let last = text.len_lines() - 1;
    let after_last = conflicts.labels.partition_point(|label| label.line < last);
    match frame.rows.last() {
        Some(row) if row.line == last && !matches!(row.kind, RowKind::Virtual(_)) => {
            for index in 0..conflicts.labels.len() - after_last {
                match frame.rows.len() < height as usize {
                    true => frame.rows.push(Row {
                        line: last,
                        kind: RowKind::Virtual(index as u16),
                        mark: LineMark::Plain,
                        text: frame.text.len()..frame.text.len(),
                        runs: frame.runs.len()..frame.runs.len(),
                        background: Some(Background::Filler),
                    }),
                    false => break,
                }
            }
        }
        Some(_) | None => {}
    }
    for row in frame.rows.iter_mut() {
        let RowKind::Virtual(index) = row.kind else {
            continue;
        };
        let first = conflicts
            .labels
            .partition_point(|label| label.line < row.line);
        let Some(label) = conflicts
            .labels
            .get(first + index as usize)
            .filter(|label| label.line == row.line)
        else {
            continue;
        };
        let (name, take, role) = match label.side {
            Side::Ours => ("ours", Take::Ours, Background::OursMarker),
            Side::Theirs => ("theirs", Take::Theirs, Background::TheirsMarker),
        };
        let start = frame.text.len();
        frame.text.push_str(name);
        let rest = text.line(label.marker).slice(MARKER..);
        match (
            rest.first_non_whitespace_char(),
            rest.last_non_whitespace_char(),
        ) {
            (Some(from), Some(to)) => {
                frame.text.push_str(" (");
                rest.slice(from..to + 1)
                    .chunks()
                    .for_each(|chunk| frame.text.push_str(chunk));
                frame.text.push(')');
            }
            (_, _) => {}
        }
        match keymap::keys_for(Context::Normal, &Command::Resolve(take)) {
            Some(keys) => {
                frame.text.push_str(" —");
                for key in keys {
                    let _ = write!(frame.text, " {key}");
                }
            }
            None => {}
        }
        row.text = start..frame.text.len();
        row.runs = frame.runs.len()..frame.runs.len() + 1;
        row.background = Some(role);
        frame.runs.push(Run {
            len: frame.text.len() - start,
            style: theme::CONFLICT_LABEL,
        });
    }
}

/// The focused text view and its document with conflicts parsed for the current text.
fn focused(state: &mut State) -> Option<(&mut TextView, &TextDoc)> {
    let view = state
        .layout
        .panes
        .get_mut(state.layout.focus)?
        .active_mut()?
        .text_mut();
    match state.buffers.get_mut(view.buffer()).map(|b| &mut b.content) {
        Some(Content::Text { doc, .. }) => {
            refresh(doc);
            Some((view, doc))
        }
        Some(Content::Loading { .. }) | None => None,
    }
}

/// `]m [m`, after `]g`: each selection moves to (select mode: extends to) the count-th conflict.
pub fn goto(state: &mut State, direction: Direction) {
    let count = count_of(state);
    let mode = state.input.mode;
    let Some((view, doc)) = focused(state) else {
        state.notice = Some(Notice::NoBuffer);
        return;
    };
    let text = doc.text.slice(..);
    let list = &doc.conflicts.list;
    let pick = |line: usize| match direction {
        Direction::Forward => {
            let next = list.partition_point(|conflict| conflict.open <= line);
            (next < list.len()).then(|| (next + count - 1).min(list.len() - 1))
        }
        Direction::Backward => list
            .partition_point(|conflict| conflict.close < line)
            .checked_sub(1)
            .map(|previous| previous.saturating_sub(count - 1)),
    };
    let primary = view.selection.primary().cursor_line(text);
    match pick(primary) {
        Some(_) => {}
        None => {
            state.notice = Some(Notice::NoConflicts);
            return;
        }
    }
    view.selection = view.selection.clone().transform(|range| {
        let Some(index) = pick(range.cursor_line(text)) else {
            return range;
        };
        let lines = list[index].lines();
        let (start, end) = (text.line_to_char(lines.start), text.line_to_char(lines.end));
        match (mode, direction) {
            (Mode::Select, _) => helix_core::Range::new(
                range.anchor,
                match end <= range.anchor {
                    true => start,
                    false => end,
                },
            ),
            (Mode::Normal | Mode::Insert, Direction::Forward) => helix_core::Range::new(start, end),
            (Mode::Normal | Mode::Insert, Direction::Backward) => {
                helix_core::Range::new(end, start)
            }
        }
    });
}

/// Replaces every conflict a selection touches with the sections `take` keeps, as one edit.
pub fn resolve(state: &mut State, take: Take) {
    let changes = {
        let Some((view, doc)) = focused(state) else {
            state.notice = Some(Notice::NoBuffer);
            return;
        };
        let text = doc.text.slice(..);
        let list = &doc.conflicts.list;
        let section = |lines: Range<usize>| {
            text.slice(text.line_to_char(lines.start)..text.line_to_char(lines.end))
        };
        let mut changes = Vec::new();
        let mut next = 0;
        for (start, end) in view.selection.line_ranges(text) {
            next += list[next..].partition_point(|conflict| conflict.close < start);
            while let Some(conflict) = list.get(next).filter(|conflict| conflict.open <= end) {
                next += 1;
                let mut kept: Tendril = match (take, conflict.base_lines()) {
                    (Take::Ours, _) => section(conflict.ours()).chunks().collect(),
                    (Take::Theirs, _) => section(conflict.theirs()).chunks().collect(),
                    (Take::Both, _) => section(conflict.ours())
                        .chunks()
                        .chain(section(conflict.theirs()).chunks())
                        .collect(),
                    (Take::Base, Some(base)) => section(base).chunks().collect(),
                    (Take::Base, None) => {
                        state.notice = Some(Notice::NoConflictBase);
                        return;
                    }
                };
                // A conflict closing the file without a line ending leaves none behind either.
                match (
                    get_line_ending(&text.line(conflict.close)),
                    get_line_ending_of_str(&kept),
                ) {
                    (None, Some(ending)) => kept.truncate(kept.len() - ending.as_str().len()),
                    (Some(_), _) | (None, None) => {}
                }
                let lines = conflict.lines();
                changes.push((
                    text.line_to_char(lines.start),
                    text.line_to_char(lines.end),
                    Some(kept),
                ));
            }
        }
        changes
    };
    match changes.is_empty() {
        true => state.notice = Some(Notice::NoConflictUnderSelection),
        false => {
            edit(state, |doc, _, _| {
                Transaction::change(&doc.text, changes.into_iter())
            });
            exit_select_mode(state);
        }
    }
}
