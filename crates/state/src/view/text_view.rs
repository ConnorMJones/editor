use helix_core::Selection;

use crate::text::buffer::BufferId;

#[derive(Clone)]
pub struct TextView {
    buffer: BufferId, // private: tabs are found by their buffer; a changed buffer can show one file twice in a pane
    pub selection: Selection,
    pub scroll: Scroll,
}

impl TextView {
    pub fn new(buffer: BufferId) -> TextView {
        TextView {
            buffer,
            selection: Selection::point(0),
            scroll: Scroll::default(),
        }
    }

    pub fn buffer(&self) -> BufferId {
        self.buffer
    }

    pub fn editing(&mut self) -> Editing<'_> {
        Editing {
            selection: &mut self.selection,
            edited: Edited::Buffer {
                buffer: self.buffer,
                scroll: &mut self.scroll,
            },
        }
    }
}

pub struct Editing<'a> {
    pub selection: &'a mut Selection,
    pub edited: Edited<'a>,
}

pub enum Edited<'a> {
    Buffer {
        buffer: BufferId,
        scroll: &'a mut Scroll,
    },
    Mark,
    Draft {
        scroll: &'a mut Scroll,
    },
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Scroll {
    pub anchor: usize,
    pub rows: isize,
    pub viewport: Viewport,
    pub sideways: Sideways,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Sideways {
    #[default]
    Wrapped,
    Unwrapped {
        left: u16,
    },
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Viewport {
    pub rows: u16,
    pub cols: u16,
}
