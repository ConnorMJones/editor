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

impl Side {
    /// The axis panes on this side of each other lie along.
    pub fn axis(self) -> Axis {
        match self {
            Side::Left | Side::Right => Axis::Row,
            Side::Up | Side::Down => Axis::Column,
        }
    }
}
