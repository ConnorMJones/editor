use std::borrow::Cow;
use std::sync::LazyLock;

use helix_core::line_ending::get_line_ending_of_str;
use helix_core::{Range, Selection, SmallVec, Tendril, Transaction};
use model::command::Paste;
use model::mode::Mode;
use model::notice::Notice;
use model::register::Register;

use crate::apply::{commit_focused, count_of, edit, exit_select_mode, focused_text};
use crate::text::registers::{self, ClipboardUse, Values};
use crate::{Effect, State};

static LINE_ENDINGS: LazyLock<helix_core::regex::Regex> =
    LazyLock::new(|| helix_core::regex::Regex::new(r"\r\n|\r|\n").expect("valid regex"));

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Yank {
    All,
    Primary,
}

pub fn yank(state: &mut State, register: Register, yank: Yank, out: &mut Vec<Effect>) {
    let Some((view, doc)) = focused_text(&mut state.layout, &mut state.buffers, &mut state.notice)
    else {
        return;
    };
    let text = doc.text.slice(..);
    let values: Vec<String> = match yank {
        Yank::All => view
            .selection
            .fragments(text)
            .map(Cow::into_owned)
            .collect(),
        Yank::Primary => vec![view.selection.primary().fragment(text).into_owned()],
    };
    let count = values.len();
    state.notice = Some(match state.registers.write(register, values, out) {
        Ok(()) => Notice::Yanked {
            count,
            register: register.char(),
        },
        Err(notice) => notice,
    });
    exit_select_mode(state);
}

fn map_value(value: &str, line_ending: &str, count: usize) -> Tendril {
    let value = LINE_ENDINGS.replace_all(value, line_ending);
    let mut out = Tendril::from(value.as_ref());
    for _ in 1..count {
        out.push_str(&value);
    }
    out
}

/// One value per selection, repeating the last; values ending in a newline paste linewise.
pub fn paste(state: &mut State, values: &[String], at: Paste, count: usize) {
    let Some(last) = values.last() else {
        return;
    };
    let mode = state.input.mode;
    if mode == Mode::Insert {
        commit_focused(state);
    }
    let linewise = values
        .iter()
        .any(|value| get_line_ending_of_str(value).is_some());
    edit(state, |doc, selection, _| {
        let text = &doc.text;
        let line_ending = doc.line_ending.as_str();
        let repeat = std::iter::repeat(map_value(last, line_ending, count));
        let mut values = values
            .iter()
            .map(|value| map_value(value, line_ending, count))
            .chain(repeat);
        let mut offset = 0;
        let mut ranges = SmallVec::with_capacity(selection.len());
        let tx = Transaction::change_by_selection(text, selection, |range| {
            let pos = match (at, linewise) {
                (Paste::Before, true) => text.line_to_char(text.char_to_line(range.from())),
                (Paste::After, true) => {
                    let line = range.line_range(text.slice(..)).1;
                    text.line_to_char((line + 1).min(text.len_lines()))
                }
                (Paste::Before, false) => range.from(),
                (Paste::After, false) => range.to(),
                (Paste::Cursor, _) => range.cursor(text.slice(..)),
            };
            let value = values.next();
            let len = value.as_ref().map_or(0, |value| value.chars().count());
            let anchor = offset + pos;
            ranges.push(Range::new(anchor, anchor + len).with_direction(range.direction()));
            offset += len;
            (pos, pos, value)
        });
        match mode {
            Mode::Normal => tx.with_selection(Selection::new(ranges, selection.primary_index())),
            Mode::Insert | Mode::Select => tx,
        }
    });
    commit_focused(state);
}

/// `R`: replaces each non-empty selection with a register value.
pub fn replace_with(state: &mut State, values: &[String], count: usize) {
    let Some(last) = values.last() else {
        return;
    };
    edit(state, |doc, selection, _| {
        let line_ending = doc.line_ending.as_str();
        let repeat = std::iter::repeat(map_value(last, line_ending, count));
        let mut values = values
            .iter()
            .map(|value| map_value(value, line_ending, count))
            .chain(repeat);
        Transaction::change_by_selection(&doc.text, selection, |range| match range.is_empty() {
            false => (range.from(), range.to(), values.next()),
            true => (range.from(), range.to(), None),
        })
    });
    commit_focused(state);
}

pub fn paste_from(state: &mut State, register: Register, at: Paste, out: &mut Vec<Effect>) {
    let count = count_of(state);
    match registers::read(state, register) {
        Values::Ready(values) => {
            paste(state, &values, at, count);
            exit_select_mode(state);
        }
        Values::Clipboard(clipboard) => out.push(Effect::ReadClipboard {
            clipboard,
            then: ClipboardUse::Paste { at, count },
        }),
    }
}

pub fn replace_from(state: &mut State, register: Register, out: &mut Vec<Effect>) {
    let count = count_of(state);
    match registers::read(state, register) {
        Values::Ready(values) => {
            replace_with(state, &values, count);
            exit_select_mode(state);
        }
        Values::Clipboard(clipboard) => out.push(Effect::ReadClipboard {
            clipboard,
            then: ClipboardUse::Replace { count },
        }),
    }
}
