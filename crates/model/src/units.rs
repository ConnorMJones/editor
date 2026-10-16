use std::ops::{Mul, Sub};

const PX_PER_PT: f32 = 96.0 / 72.0;

/// Logical pixels; the host applies the screen's scale factor.
#[derive(
    Clone, Copy, Debug, Default, PartialEq, PartialOrd, serde::Serialize, serde::Deserialize,
)]
#[serde(transparent)]
pub struct Px(pub f32);

#[derive(Clone, Copy, Debug, Default, PartialEq, PartialOrd)]
pub struct Pt(pub f32);

impl From<Pt> for Px {
    fn from(pt: Pt) -> Px {
        Px(pt.0 * PX_PER_PT)
    }
}

impl From<Px> for Pt {
    fn from(px: Px) -> Pt {
        Pt(px.0 / PX_PER_PT)
    }
}

impl Sub for Pt {
    type Output = Pt;

    fn sub(self, other: Pt) -> Pt {
        Pt(self.0 - other.0)
    }
}

impl Mul<f32> for Pt {
    type Output = Pt;

    fn mul(self, scale: f32) -> Pt {
        Pt(self.0 * scale)
    }
}
