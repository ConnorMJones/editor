/// Logical pixels as the host measured them.
#[derive(Clone, Copy, Debug, Default, PartialEq, PartialOrd)]
pub struct Px(pub f32);

/// Direction panes are laid out along inside a split.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
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
    /// Notes linking to the focused note.
    Backlinks,
}
