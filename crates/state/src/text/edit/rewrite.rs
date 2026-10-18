use std::num::NonZeroUsize;

use helix_core::{Range, Selection, SmallVec, Transaction};
use helix_stdx::rope::RopeSliceExt;
use model::command::{Case, CharTarget, Direction};
use model::register::Register;

use crate::State;
use crate::apply::{count_of, edit, exit_select_mode, select};

/// `C-a`/`C-x`; with register `#` each later selection steps one further.
pub fn increment(state: &mut State, sign: i64) {
    let amount = sign * count_of(state) as i64;
    let step = match state.input.register {
        Some(Register::SelectionIndices) => sign,
        Some(_) | None => 0,
    };
    let mut changed = false;
    edit(state, |doc, selection, _| {
        let text = doc.text.slice(..);
        let mut amount = amount;
        let mut ranges = SmallVec::<[Range; 1]>::new();
        let mut shift: i128 = 0;
        let mut changes = Vec::new();
        for range in selection {
            let selected = range.fragment(text);
            let from = (range.from() as i128 + shift) as usize;
            let incremented = helix_core::increment::integer(&selected, amount)
                .or_else(|| helix_core::increment::date_time(&selected, amount));
            amount += step;
            match incremented {
                None => ranges.push(Range::new(from, (range.to() as i128 + shift) as usize)),
                Some(new) => {
                    ranges.push(Range::new(from, from + new.len()));
                    shift += new.len() as i128 - selected.len() as i128;
                    changes.push((range.from(), range.to(), Some(new.into())));
                }
            }
        }
        changed = !changes.is_empty();
        let tx = Transaction::change(&doc.text, changes.into_iter());
        match changed {
            true => tx.with_selection(Selection::new(ranges, selection.primary_index())),
            false => tx,
        }
    });
    if changed {
        exit_select_mode(state);
    }
}

pub fn rotate_contents(state: &mut State, direction: Direction) {
    let rotate = state.input.count.map_or(1, NonZeroUsize::get);
    select(state, |_, selection, _| {
        let len = selection.len();
        let rotate = rotate.min(len);
        let mut selection = selection.clone();
        selection.set_primary_index(match direction {
            Direction::Forward => (selection.primary_index() + len + rotate) % len,
            Direction::Backward => (selection.primary_index() + len - rotate) % len,
        });
        selection
    });
    edit(state, |doc, selection, _| {
        let text = doc.text.slice(..);
        let mut fragments: Vec<helix_core::Tendril> = selection
            .slices(text)
            .map(|fragment| fragment.chunks().collect())
            .collect();
        let rotate = rotate.min(fragments.len());
        match direction {
            Direction::Forward => fragments.rotate_right(rotate),
            Direction::Backward => fragments.rotate_left(rotate),
        }
        Transaction::change(
            &doc.text,
            selection
                .ranges()
                .iter()
                .zip(fragments)
                .map(|(range, fragment)| (range.from(), range.to(), Some(fragment))),
        )
    });
}

pub fn replace(state: &mut State, target: CharTarget) {
    edit(state, |doc, selection, _| {
        let mut buf = [0u8; 4];
        let ch: &str = match target {
            model::command::CharTarget::Char(c) => c.encode_utf8(&mut buf),
            model::command::CharTarget::LineEnding => doc.line_ending.as_str(),
        };
        Transaction::change_by_selection(&doc.text, selection, |range| match range.is_empty() {
            false => {
                let text: helix_core::Tendril = doc
                    .text
                    .slice(range.from()..range.to())
                    .graphemes()
                    .map(|_| ch)
                    .collect();
                (range.from(), range.to(), Some(text))
            }
            true => (range.from(), range.to(), None),
        })
    });
    exit_select_mode(state);
}

pub fn change_case(state: &mut State, case: Case) {
    edit(state, |doc, selection, _| {
        let text = doc.text.slice(..);
        Transaction::change_by_selection(&doc.text, selection, |range| {
            let fragment = range.slice(text);
            let changed: helix_core::Tendril = match case {
                model::command::Case::Switch => fragment
                    .chars()
                    .flat_map(|c| {
                        let (upper, lower) = (c.is_uppercase(), c.is_lowercase());
                        match (lower, upper) {
                            (true, _) => CaseChars::Upper(c.to_uppercase()),
                            (false, true) => CaseChars::Lower(c.to_lowercase()),
                            (false, false) => CaseChars::Keep(Some(c)),
                        }
                    })
                    .collect(),
                model::command::Case::Lower => fragment
                    .chunks()
                    .map(|chunk| chunk.to_lowercase())
                    .collect(),
                model::command::Case::Upper => fragment
                    .chunks()
                    .map(|chunk| chunk.to_uppercase())
                    .collect(),
            };
            (range.from(), range.to(), Some(changed))
        })
    });
    exit_select_mode(state);
}

enum CaseChars {
    Upper(std::char::ToUppercase),
    Lower(std::char::ToLowercase),
    Keep(Option<char>),
}

impl Iterator for CaseChars {
    type Item = char;

    fn next(&mut self) -> Option<char> {
        match self {
            CaseChars::Upper(upper) => upper.next(),
            CaseChars::Lower(lower) => lower.next(),
            CaseChars::Keep(c) => c.take(),
        }
    }
}
