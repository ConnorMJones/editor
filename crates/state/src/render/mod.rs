//! Document and pages views of markdown and typst, and the typesetting they ask for.

pub mod document;
pub mod markdown;
pub mod pages;
pub mod source;
pub mod typeset;

use model::command::{Direction, ReadCommand, ScrollBy};

use crate::render::source::leave;
use crate::view::layout::View;
use crate::{Effect, State};

/// Brings every open document and pages view up to date with its text.
pub fn sync(state: &mut State, out: &mut Vec<Effect>) {
    document::sync(state, out);
    pages::sync(state, out);
}

/// Scrolling and leaving keys, for the kind of view focused.
pub fn read(state: &mut State, command: ReadCommand) {
    match (command, state.layout.focused().active()) {
        (ReadCommand::Leave, Some(View::Document(view))) => {
            let link = view.source.link;
            leave(state, &link)
        }
        (ReadCommand::Leave, Some(View::Pages(view))) => {
            let link = view.source.link;
            leave(state, &link)
        }
        (ReadCommand::Move(motion), Some(View::Document(_))) => document::moved(state, motion),
        (ReadCommand::Move(motion), Some(View::Pages(_))) => pages::moved(state, motion),
        (ReadCommand::Leave | ReadCommand::Move(_), Some(View::Text(_) | View::Diff(_)) | None) => {
        }
    }
}

/// Lines and half viewports a scroll key moves by.
fn step(direction: Direction, by: ScrollBy) -> (i32, i32) {
    let sign = match direction {
        Direction::Forward => 1,
        Direction::Backward => -1,
    };
    match by {
        ScrollBy::Lines => (sign, 0),
        ScrollBy::Page => (0, 2 * sign),
        ScrollBy::HalfPageCursor => (0, sign),
    }
}
