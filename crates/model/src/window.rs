/// Direction panes are laid out along inside a split.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Axis {
    /// Side by side (helix `vsplit`).
    Row,
    /// Stacked (helix `hsplit`).
    Column,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    Left,
    Right,
    Up,
    Down,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PickerKind {
    Files,
    Buffers,
    Grep,
    Commands,
}
