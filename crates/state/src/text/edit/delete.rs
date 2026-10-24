use std::borrow::Cow;

use helix_core::auto_pairs::{self};
use helix_core::graphemes;
use helix_core::line_ending::line_end_char_index;
use helix_core::movement::{self};
use helix_core::unicode::width::UnicodeWidthChar;
use helix_core::{Deletion, Range, RopeSlice, Selection, SmallVec, Transaction};
use helix_stdx::rope::RopeSliceExt;
use model::command::{Direction, Yanking};
use model::mode::Mode;
use model::register::Register;

use super::insert::{Comments, auto_pairs, open_lines};
use crate::apply::{FocusedText, count_of, edit, exit_select_mode, focused_text};
use crate::{Effect, State};

pub fn delete_char_backward(state: &mut State) {
    let count = count_of(state);
    edit(state, |doc, selection, loader| {
        let text = doc.text.slice(..);
        let pairs = auto_pairs(doc, selection, loader);
        Transaction::delete_by_and_with_selection(&doc.text, selection, |range| {
            let pos = range.cursor(text);
            let line_start = text.line_to_char(range.cursor_line(text));
            let indent = Cow::from(text.slice(line_start..pos));
            let only_indent =
                !indent.is_empty() && indent.chars().all(|ch| ch == ' ' || ch == '\t');
            // Only indentation before the cursor deletes back to the previous indent stop.
            let dedent = match (only_indent, text.get_char(pos.saturating_sub(1))) {
                (false, _) => None,
                (true, Some('\t')) => Some((pos - 1, pos)),
                (true, _) => {
                    let width: usize = indent
                        .chars()
                        .map(|ch| match ch {
                            '\t' => doc.tab_width,
                            ch => ch.width().unwrap_or(1),
                        })
                        .sum();
                    let drop = match width % doc.indent_width() {
                        0 => doc.indent_width(),
                        drop => drop,
                    };
                    let spaces = indent.chars().rev().take(drop).take_while(|ch| *ch == ' ');
                    Some((pos - spaces.count(), pos))
                }
            };
            match pos {
                0 => ((pos, pos), None),
                _ => match dedent {
                    Some(deletion) => (deletion, None),
                    None => match auto_pairs::hook_delete(&doc.text, range, pairs) {
                        Some((deletion, range)) => (deletion, Some(range)),
                        None => (
                            (graphemes::nth_prev_grapheme_boundary(text, pos, count), pos),
                            None,
                        ),
                    },
                },
            }
        })
    });
}

/// Insert-mode deletion; forward deletes keep appended selections from shrinking.
pub fn delete_insert_mode(
    state: &mut State,
    direction: Direction,
    mut f: impl FnMut(RopeSlice, &Range) -> Deletion,
) {
    edit(state, |doc, selection, _| {
        let text = doc.text.slice(..);
        let len = text.len_chars();
        let mut adjusted = SmallVec::<[Range; 1]>::new();
        let mut insert_newline = false;
        let tx = Transaction::delete_by_selection(&doc.text, selection, |range| {
            let (start, end) = f(text, range);
            if direction == Direction::Forward {
                let mut range = *range;
                if range.head > range.anchor {
                    insert_newline |= end == len;
                    range.head += 1;
                }
                adjusted.push(range);
            }
            (start, end)
        });
        // Deleting the last character would leave the cursor on EOF, so re-add a line ending.
        let tx = match insert_newline {
            true => tx.insert_at_eof(doc.line_ending.as_str().into()),
            false => tx,
        };
        match direction {
            Direction::Forward => {
                // Invariants first: the bumped head may sit past the end of the old text.
                let mapped = Selection::new(adjusted, selection.primary_index())
                    .ensure_invariants(text)
                    .map(tx.changes());
                tx.with_selection(mapped)
            }
            Direction::Backward => tx,
        }
    });
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Operation {
    Delete,
    Change,
}

pub fn delete_selection(
    state: &mut State,
    operation: Operation,
    yanking: Yanking,
    out: &mut Vec<Effect>,
) {
    let register = state.input.register.unwrap_or(Register::DEFAULT);
    let Some(FocusedText { view, doc }) =
        focused_text(&mut state.layout, &mut state.buffers, &mut state.notice)
    else {
        return;
    };
    let text = doc.text.slice(..);
    let linewise = view.selection.ranges().iter().all(|range| {
        let (start_line, end_line) = range.line_range(text);
        range.slice(text).len_lines() >= 2
            && text.line_to_char(start_line) == range.from()
            && text.line_to_char((end_line + 1).min(text.len_lines())) == range.to()
    });
    match (yanking, register) {
        (Yanking::Yank, Register::BlackHole) | (Yanking::NoYank, _) => {}
        (Yanking::Yank, register) => {
            let values: Vec<String> = view
                .selection
                .fragments(text)
                .map(Cow::into_owned)
                .collect();
            if let Err(notice) = state.registers.write(register, values, out) {
                state.notice = Some(notice);
                return;
            }
        }
    }
    edit(state, |doc, selection, _| {
        Transaction::delete_by_selection(&doc.text, selection, |range| (range.from(), range.to()))
    });
    match (operation, linewise) {
        (Operation::Delete, _) => exit_select_mode(state),
        (Operation::Change, true) => open_lines(state, Direction::Backward, Comments::Skip),
        (Operation::Change, false) => state.input.mode = Mode::Insert,
    }
}

pub fn delete_char_forward(state: &mut State) {
    let count = count_of(state);
    delete_insert_mode(state, Direction::Forward, |text, range| {
        let pos = range.cursor(text);
        (pos, graphemes::nth_next_grapheme_boundary(text, pos, count))
    })
}

pub fn delete_word_backward(state: &mut State) {
    let count = count_of(state);
    delete_insert_mode(state, Direction::Backward, |text, range| {
        let anchor = movement::move_prev_word_start(text, *range, count).from();
        let next = Range::new(anchor, range.cursor(text));
        // Exclude the cursor itself unless it sits at the end of the text.
        let next = match next.to() == range.to() && text.len_chars() != range.to() {
            true => Range::new(
                next.from(),
                graphemes::prev_grapheme_boundary(text, range.to()),
            ),
            false => next,
        };
        (next.from(), next.to())
    })
}

pub fn delete_word_forward(state: &mut State) {
    let count = count_of(state);
    delete_insert_mode(state, Direction::Forward, |text, range| {
        let head = movement::move_next_word_end(text, *range, count).to();
        (range.cursor(text), head)
    })
}

pub fn kill_to_line_start(state: &mut State) {
    delete_insert_mode(state, Direction::Backward, |text, range| {
        let line = range.cursor_line(text);
        let first_char = text.line_to_char(line);
        let anchor = range.cursor(text);
        let head = match (anchor == first_char && line != 0, text.line(line)) {
            (true, _) => line_end_char_index(&text, line - 1),
            (false, line_text) => match line_text.first_non_whitespace_char() {
                Some(pos) if first_char + pos < anchor => first_char + pos,
                Some(_) | None => first_char,
            },
        };
        (head, anchor)
    })
}
