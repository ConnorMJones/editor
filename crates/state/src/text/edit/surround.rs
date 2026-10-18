use helix_core::{Range, Selection, SmallVec, Tendril, Transaction, match_brackets, surround};
use model::command::{CharTarget, SurroundPair};
use model::notice::{Notice, SurroundError};

use crate::State;
use crate::apply::{count_of, edit, try_edit};
use crate::text::buffer::TextDoc;

pub fn surround_add(state: &mut State, target: CharTarget) {
    edit(state, |doc, selection, _| {
        let (open, close, len): (Tendril, Tendril, usize) = match target {
            CharTarget::Char(ch) => {
                let (open, close) = match_brackets::get_pair(ch);
                (Tendril::from_iter([open]), Tendril::from_iter([close]), 2)
            }
            CharTarget::LineEnding => (
                doc.line_ending.as_str().into(),
                doc.line_ending.as_str().into(),
                2 * doc.line_ending.len_chars(),
            ),
        };
        let mut changes = Vec::with_capacity(selection.len() * 2);
        let mut ranges = SmallVec::with_capacity(selection.len());
        let mut offs = 0;
        for range in selection.iter() {
            changes.push((range.from(), range.from(), Some(open.clone())));
            changes.push((range.to(), range.to(), Some(close.clone())));
            ranges.push(
                Range::new(offs + range.from(), offs + range.to() + len)
                    .with_direction(range.direction()),
            );
            offs += len;
        }
        Transaction::change(&doc.text, changes.into_iter())
            .with_selection(Selection::new(ranges, selection.primary_index()))
    });
}

fn surround_positions(
    doc: &TextDoc,
    selection: &Selection,
    pair: SurroundPair,
    count: usize,
) -> Result<Vec<usize>, Notice> {
    let ch = match pair {
        SurroundPair::Closest => None,
        SurroundPair::Char(ch) => Some(ch),
    };
    surround::get_surround_pos(
        doc.highlight.syntax(),
        doc.text.slice(..),
        selection,
        ch,
        count,
    )
    .map_err(|error| {
        Notice::Surround(match error {
            surround::Error::PairNotFound => SurroundError::PairNotFound,
            surround::Error::CursorOverlap => SurroundError::CursorOverlap,
            surround::Error::RangeExceedsText => SurroundError::RangeExceedsText,
            surround::Error::CursorOnAmbiguousPair => SurroundError::CursorOnAmbiguousPair,
        })
    })
}

pub fn surround_replace(state: &mut State, from: SurroundPair, to: char) {
    let count = count_of(state);
    try_edit(state, |doc, selection, _| {
        let positions = surround_positions(doc, selection, from, count)?;
        let (open, close) = match_brackets::get_pair(to);
        let mut sorted: Vec<(usize, char)> = positions
            .chunks(2)
            .flat_map(|pair| [(pair[0], open), (pair[1], close)])
            .collect();
        sorted.sort_unstable();
        Ok(Transaction::change(
            &doc.text,
            sorted
                .into_iter()
                .map(|(pos, ch)| (pos, pos + 1, Some(Tendril::from_iter([ch])))),
        ))
    });
}

pub fn surround_delete(state: &mut State, pair: SurroundPair) {
    let count = count_of(state);
    try_edit(state, |doc, selection, _| {
        let mut positions = surround_positions(doc, selection, pair, count)?;
        positions.sort_unstable();
        Ok(Transaction::change(
            &doc.text,
            positions.into_iter().map(|pos| (pos, pos + 1, None)),
        ))
    });
}
