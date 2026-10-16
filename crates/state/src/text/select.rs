use helix_core::chars::char_is_word;
use helix_core::movement::Movement;
use helix_core::{LineEnding, Position, Range, RopeSlice, Selection, SmallVec, graphemes};
use helix_stdx::rope::{self, RopeSliceExt};
use model::command::{Direction, RegexOp, WordBounds};
use model::notice::Notice;
use model::register::Register;

use crate::apply::{count_of, movement_of, select, try_select};
use crate::{Effect, Overlay, Prompt, PromptKind, State};
use crate::{input::prompt, text::registers};

/// Copies each selection onto the next or previous lines that are long enough.
#[allow(deprecated)]
pub fn copy_selection_on_line(state: &mut State, direction: Direction) {
    use helix_core::{pos_at_visual_coords, visual_coords_at_pos};

    let count = count_of(state);
    select(state, |doc, selection, _| {
        let text = doc.text.slice(..);
        let tab_width = doc.tab_width;
        let mut ranges = SmallVec::with_capacity(selection.ranges().len() * (count + 1));
        ranges.extend_from_slice(selection.ranges());
        let mut primary_index = 0;
        for range in selection.iter() {
            let is_primary = *range == selection.primary();
            let (head, anchor) = match range.anchor < range.head {
                true => (range.head - 1, range.anchor),
                false => (range.head, range.anchor.saturating_sub(1)),
            };
            let head_pos = visual_coords_at_pos(text, head, tab_width);
            let anchor_pos = visual_coords_at_pos(text, anchor, tab_width);
            let height = head_pos.row.max(anchor_pos.row) - head_pos.row.min(anchor_pos.row) + 1;
            if is_primary {
                primary_index = ranges.len();
            }
            ranges.push(*range);
            let mut copies = 0;
            let mut i = 0;
            while copies < count {
                let offset = (i + 1) * height;
                let (anchor_row, head_row) = match direction {
                    Direction::Forward => (anchor_pos.row + offset, head_pos.row + offset),
                    Direction::Backward => (
                        anchor_pos.row.saturating_sub(offset),
                        head_pos.row.saturating_sub(offset),
                    ),
                };
                if anchor_row >= text.len_lines() || head_row >= text.len_lines() {
                    break;
                }
                let anchor = pos_at_visual_coords(
                    text,
                    Position::new(anchor_row, anchor_pos.col),
                    tab_width,
                );
                let head =
                    pos_at_visual_coords(text, Position::new(head_row, head_pos.col), tab_width);
                // Lines too short to hold the column are skipped.
                if visual_coords_at_pos(text, anchor, tab_width).col == anchor_pos.col
                    && visual_coords_at_pos(text, head, tab_width).col == head_pos.col
                {
                    if is_primary {
                        primary_index = ranges.len();
                    }
                    ranges.push(Range::point(anchor).put_cursor(text, head, true));
                    copies += 1;
                }
                if anchor_row == 0 && head_row == 0 {
                    break;
                }
                i += 1;
            }
        }
        Selection::new(ranges, primary_index)
    });
}

/// Smart case: patterns without uppercase match case-insensitively.
fn build_regex(pattern: &str, line_ending: LineEnding) -> Result<rope::Regex, Notice> {
    rope::RegexBuilder::new()
        .syntax(
            rope::Config::new()
                .case_insensitive(!pattern.chars().any(char::is_uppercase))
                .multi_line(true)
                .crlf(line_ending == LineEnding::Crlf),
        )
        .build(pattern)
        .map_err(|error| Notice::InvalidRegex {
            error: error.to_string(),
        })
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Warnings {
    Show,
    Hide,
}

/// Moves or adds the primary selection to the next match, wrapping around the buffer.
fn search(
    text: RopeSlice,
    selection: &Selection,
    regex: &rope::Regex,
    movement: Movement,
    direction: Direction,
    warnings: Warnings,
) -> (Selection, Option<Notice>) {
    let primary = selection.primary();
    let start = match direction {
        Direction::Forward => {
            text.char_to_byte(graphemes::ensure_grapheme_boundary_next(text, primary.to()))
        }
        Direction::Backward => text.char_to_byte(graphemes::ensure_grapheme_boundary_prev(
            text,
            primary.from(),
        )),
    };
    let found = match direction {
        Direction::Forward => regex.find(text.regex_input_at_bytes(start..)),
        Direction::Backward => regex.find_iter(text.regex_input_at_bytes(..start)).last(),
    };
    let (found, notice) = match found {
        Some(found) => (Some(found), None),
        None => {
            let wrapped = match direction {
                Direction::Forward => regex.find(text.regex_input()),
                Direction::Backward => regex.find_iter(text.regex_input_at_bytes(start..)).last(),
            };
            let notice = match (warnings, &wrapped) {
                (Warnings::Show, Some(_)) => Some(Notice::WrappedAround),
                (Warnings::Show, None) => Some(Notice::NoMoreMatches),
                (Warnings::Hide, _) => None,
            };
            (wrapped, notice)
        }
    };
    let selection = match found {
        Some(found) if text.byte_to_char(found.end()) != 0 => {
            let range = Range::new(
                text.byte_to_char(found.start()),
                text.byte_to_char(found.end()),
            )
            .with_direction(primary.direction());
            match movement {
                Movement::Extend => selection.clone().push(range),
                Movement::Move => selection.clone().replace(selection.primary_index(), range),
            }
        }
        Some(_) | None => selection.clone(),
    };
    (selection, notice)
}

pub fn search_next(state: &mut State, direction: Direction) {
    let count = count_of(state);
    let movement = movement_of(state);
    let register = state.input.register.unwrap_or(state.registers.last_search);
    let Some(query) = registers::first(state, register) else {
        return;
    };
    try_select(state, |doc, selection, _| {
        let regex = build_regex(&query, doc.line_ending)?;
        let text = doc.text.slice(..);
        let mut selection = selection.clone();
        let mut notice = None;
        for _ in 0..count {
            (selection, notice) = search(
                text,
                &selection,
                &regex,
                movement,
                direction,
                Warnings::Show,
            );
        }
        match notice {
            Some(Notice::NoMoreMatches) => Err(Notice::NoMoreMatches),
            Some(_) | None => Ok(selection),
        }
    });
}

pub fn search_selection(state: &mut State, bounds: WordBounds, out: &mut Vec<Effect>) {
    let register = state.input.register.unwrap_or(Register::SEARCH);
    let Some((view, doc)) =
        crate::apply::focused_text(&mut state.layout, &mut state.buffers, &mut state.notice)
    else {
        return;
    };
    let text = doc.text.slice(..);
    let mut patterns: Vec<String> = Vec::with_capacity(view.selection.len());
    for range in view.selection.iter() {
        let (from, to) = (range.from(), range.to());
        let word_start = match (bounds, from == text.len_chars(), from) {
            (WordBounds::Ignore, ..) | (WordBounds::Detect, true, _) => false,
            (WordBounds::Detect, false, 0) => char_is_word(text.char(0)),
            (WordBounds::Detect, false, from) => {
                !char_is_word(text.char(from - 1)) && char_is_word(text.char(from))
            }
        };
        let word_end = match (bounds, to == 0 || to == text.len_chars()) {
            (WordBounds::Ignore, _) | (WordBounds::Detect, true) => false,
            (WordBounds::Detect, false) => {
                char_is_word(text.char(to - 1)) && !char_is_word(text.char(to))
            }
        };
        let escaped = helix_core::regex::escape(&range.fragment(text));
        let pattern = match (word_start, word_end) {
            (true, true) => format!("\\b{escaped}\\b"),
            (true, false) => format!("\\b{escaped}"),
            (false, true) => format!("{escaped}\\b"),
            (false, false) => escaped,
        };
        if !patterns.contains(&pattern) {
            patterns.push(pattern);
        }
    }
    let regex = patterns.join("|");
    match state.registers.push(register, regex.clone(), out) {
        Ok(()) => {
            state.registers.last_search = register;
            state.notice = Some(Notice::RegisterSet {
                register: register.char(),
                value: regex,
            });
        }
        Err(notice) => state.notice = Some(notice),
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum PromptEvent {
    Update,
    Validate,
}

/// Re-runs the regex prompt's operation from its snapshot after each edit.
pub fn preview_regex(state: &mut State) {
    let (op, snapshot, line) = match &state.overlay {
        Overlay::Prompt(Prompt {
            kind: PromptKind::Regex { op, snapshot, .. },
            line,
            ..
        }) => (*op, snapshot.clone(), line.clone()),
        Overlay::Prompt(Prompt {
            kind: PromptKind::Command,
            ..
        })
        | Overlay::None
        | Overlay::Picker(_)
        | Overlay::Confirm(_) => return,
    };
    select(state, |_, _, _| snapshot);
    match line.is_empty() {
        true => {}
        false => run_regex(state, op, &line, PromptEvent::Update),
    }
}

pub fn submit_regex(
    state: &mut State,
    op: RegexOp,
    register: Register,
    snapshot: Selection,
    line: String,
    out: &mut Vec<Effect>,
) {
    select(state, |_, _, _| snapshot);
    let Some(pattern) = prompt::remember(state, register, line, out) else {
        return;
    };
    match op {
        RegexOp::Search(_) => state.registers.last_search = register,
        RegexOp::Select | RegexOp::Split | RegexOp::Keep | RegexOp::Remove => {}
    }
    run_regex(state, op, &pattern, PromptEvent::Validate);
}

fn run_regex(state: &mut State, op: RegexOp, pattern: &str, event: PromptEvent) {
    let movement = movement_of(state);
    try_select(state, |doc, selection, _| {
        let text = doc.text.slice(..);
        let outcome = build_regex(pattern, doc.line_ending).and_then(|regex| match op {
            RegexOp::Search(direction) => {
                Ok(search(text, selection, &regex, movement, direction, Warnings::Hide).0)
            }
            RegexOp::Select => helix_core::selection::select_on_matches(text, selection, &regex)
                .ok_or(Notice::NothingSelected),
            RegexOp::Split => Ok(helix_core::selection::split_on_matches(
                text, selection, &regex,
            )),
            RegexOp::Keep => {
                helix_core::selection::keep_or_remove_matches(text, selection, &regex, false)
                    .ok_or(Notice::NoSelectionsRemaining)
            }
            RegexOp::Remove => {
                helix_core::selection::keep_or_remove_matches(text, selection, &regex, true)
                    .ok_or(Notice::NoSelectionsRemaining)
            }
        });
        match (outcome, event) {
            (Ok(selected), _) => Ok(selected),
            (Err(_), PromptEvent::Update) => Ok(selection.clone()),
            (Err(notice), PromptEvent::Validate) => Err(notice),
        }
    });
}
