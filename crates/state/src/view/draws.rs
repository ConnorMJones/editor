use crate::State;
use crate::git::conflict::{self, Conflicts};
use crate::git::diff::{Bases, Tracked};
use crate::git::diffview::{self, DiffSides, LEFT, Left, RIGHT, Right, SideRows};
use crate::text::buffer::TextDoc;
use crate::view::display::{Gutter, Shade};
use crate::view::layout::{Pane, PaneId, Scroll, TextView};
use crate::view::scroll::Padding;
use crate::view::tab::Showing;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Part {
    Text,
    /// A diff's read-only HEAD side.
    Head,
    /// A diff's buffer side, which takes the keys.
    Buffer,
}

#[derive(Clone, Copy)]
pub struct Drawing<'a> {
    pub draws: Draws<'a>,
    /// Both sides of a diff share one gutter width so their text wraps alike.
    pub gutter: Gutter,
    pub lines: usize,
}

#[derive(Clone, Copy)]
pub enum Draws<'a> {
    /// Takes the keys: a source, a split's source side or a diff's buffer side.
    Editable {
        view: &'a TextView,
        doc: &'a TextDoc,
        rows: Rows<'a>,
        tracked: Option<&'a Tracked>,
    },
    /// A diff's buffer side for a file deleted from the worktree: blank, but still measured.
    Deleted {
        view: &'a TextView,
    },
    Head {
        doc: &'a TextDoc,
        top: Scroll,
        side: &'a SideRows,
    },
    NoHead,
    /// The pane shows no such part; the host only asks for parts it laid out this frame.
    Absent,
}

#[derive(Clone, Copy)]
pub enum Rows<'a> {
    /// Conflict regions, with label rows above their markers.
    Conflicts(&'a Conflicts),
    Diff(&'a SideRows),
}

impl<'a> Rows<'a> {
    pub fn padding(self) -> Padding<'a> {
        match self {
            Rows::Conflicts(conflicts) => conflicts.padding(),
            Rows::Diff(side) => side.padding(),
        }
    }

    pub fn shades(self) -> &'a [Shade] {
        match self {
            Rows::Conflicts(conflicts) => &conflicts.shades,
            Rows::Diff(side) => &side.shades,
        }
    }
}

pub fn drawing(state: &State, pane: PaneId, part: Part) -> Drawing<'_> {
    let Some(tab) = state.layout.panes().get(pane).and_then(Pane::active) else {
        return Drawing {
            draws: Draws::Absent,
            gutter: Gutter::new(1),
            lines: 1,
        };
    };
    let buffer = &state.buffers[tab.buffer()];
    let doc = &buffer.doc;
    let lines = doc.text.len_lines();
    let tracked = match &buffer.diff.bases {
        Bases::Tracked(tracked) => Some(tracked),
        Bases::NoRepo | Bases::Loading { .. } | Bases::Untracked => None,
    };
    let showing = tab.showing();
    let gutter_lines = match showing {
        Showing::Diff(
            _,
            DiffSides {
                left: Left::Head { doc, .. },
                ..
            },
        ) => lines.max(doc.text.len_lines()),
        Showing::Diff(..) | Showing::Source(_) | Showing::Document(..) | Showing::Pages(..) => {
            lines
        }
    };
    let draws = match (part, showing) {
        (Part::Text, showing) => match showing.source() {
            Some(view) => Draws::Editable {
                view,
                doc,
                rows: Rows::Conflicts(conflict::current(doc)),
                tracked,
            },
            None => Draws::Absent,
        },
        (Part::Buffer, Showing::Diff(view, sides)) => match diffview::right(buffer) {
            Right::Live => Draws::Editable {
                view,
                doc,
                rows: Rows::Diff(&sides.align.sides[RIGHT]),
                tracked,
            },
            Right::Deleted => Draws::Deleted { view },
        },
        (Part::Head, Showing::Diff(_, sides)) => match &sides.left {
            Left::Head { doc, scroll, .. } => Draws::Head {
                doc,
                top: *scroll,
                side: &sides.align.sides[LEFT],
            },
            Left::Pending | Left::Absent | Left::Parsing { .. } => Draws::NoHead,
        },
        (
            Part::Buffer | Part::Head,
            Showing::Source(_) | Showing::Document(..) | Showing::Pages(..),
        ) => Draws::Absent,
    };
    Drawing {
        draws,
        gutter: Gutter::new(gutter_lines),
        lines,
    }
}
