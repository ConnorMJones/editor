//! Renderings of markdown and typst, and the typesetting they ask for.

pub mod document;
pub mod markdown;
pub mod pages;
pub mod typeset;

use model::command::{Direction, ReadCommand, ScrollBy, Shown};

use crate::view::mode;
use crate::view::tab::{ShowingMut, Tab};
use crate::{Effect, State};

/// Where a rendering opens: on a source line, placed once the rendering is ready, or at a block
/// or page it showed before.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Opening {
    Line(usize),
    Item(usize),
}

/// A rendering's place and settings, kept while its tab shows it.
pub trait Rendering: Clone {
    /// Where a split's source and rendering last agreed; a change on one side moves the other.
    type Seen: Clone + Copy;

    fn open(at: Opening) -> Self;
}

/// Brings every open document and pages view up to date with its text.
pub fn sync(state: &mut State, out: &mut Vec<Effect>) {
    document::sync(state, out);
    pages::sync(state, out);
}

/// Scrolling keys for the rendering focused, and leaving it for the source.
pub fn read(state: &mut State, command: ReadCommand) {
    let motion = match command {
        ReadCommand::Leave => return mode::show(state, Shown::Source),
        ReadCommand::Move(motion) => motion,
    };
    let State {
        layout,
        documents,
        compiles,
        ..
    } = state;
    match layout.focused_mut().active_mut().map(Tab::showing_mut) {
        Some(ShowingMut::Document(text, view, _)) => {
            document::moved(view, documents.get(text.buffer()), motion)
        }
        Some(ShowingMut::Pages(text, view, _)) => {
            pages::moved(view, compiles.get(text.buffer()), motion)
        }
        Some(ShowingMut::Source(_) | ShowingMut::Diff(..)) | None => {}
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
