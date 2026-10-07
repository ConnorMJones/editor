use helix_core::Selection;
use slotmap::{SlotMap, new_key_type};

use crate::buffer::BufferId;

new_key_type! {
    pub struct PaneId;
}

pub struct Layout {
    pub panes: SlotMap<PaneId, Pane>,
    pub focus: PaneId,
}

impl Layout {
    pub fn new() -> Layout {
        let mut panes = SlotMap::with_key();
        let focus = panes.insert(Pane { view: View::Empty });
        Layout { panes, focus }
    }
}

impl Default for Layout {
    fn default() -> Self {
        Layout::new()
    }
}

pub struct Pane {
    pub view: View,
}

pub enum View {
    Empty,
    Text(TextView),
}

pub struct TextView {
    pub buffer: BufferId,
    pub selection: Selection,
    pub scroll: Scroll,
}

/// First visible position; refined by the text element.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Scroll {
    pub anchor: usize,
}
