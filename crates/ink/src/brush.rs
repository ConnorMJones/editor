//! What a stroke is drawn with, apart from its colour.

use model::theme::Color;

use crate::input::{Model, Pressure};

/// Closest distinct points, and the largest gap between a mesh and the outline it stands for,
/// in points.
pub const EPSILON: f32 = 0.05;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Brush {
    pub size: Size,
    pub width: WidthFrom,
    pub model: Model,
}

/// The full width of the tip, in points.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Size(f32); // private: checked by its constructor

#[derive(Debug, thiserror::Error, PartialEq)]
#[error("brush size {0} is not a positive width")]
pub struct InvalidSize(pub f32);

impl Size {
    pub fn new(size: f32) -> Result<Size, InvalidSize> {
        (size.is_finite() && size > EPSILON)
            .then_some(Size(size))
            .ok_or(InvalidSize(size))
    }

    pub fn get(self) -> f32 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum WidthFrom {
    Constant,
    /// Full pressure gives the full size and none gives `thinnest` of it.
    Pressure {
        curve: Curve,
        thinnest: Fraction,
    },
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Curve {
    Linear,
    Sqrt,
    Square,
}

/// A number in `0..=1`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Fraction(f32); // private: checked by its constructor

#[derive(Debug, thiserror::Error, PartialEq)]
#[error("{0} is not between 0 and 1")]
pub struct InvalidFraction(pub f32);

impl Fraction {
    pub fn new(fraction: f32) -> Result<Fraction, InvalidFraction> {
        ((0.0..=1.0).contains(&fraction))
            .then_some(Fraction(fraction))
            .ok_or(InvalidFraction(fraction))
    }

    pub fn get(self) -> f32 {
        self.0
    }
}

impl Brush {
    /// The tip's radius at `pressure`; a sample without pressure gets the full size.
    pub fn radius(&self, pressure: Option<Pressure>) -> f32 {
        let full = self.size.get() * 0.5;
        match (self.width, pressure) {
            (WidthFrom::Constant, _) | (WidthFrom::Pressure { .. }, None) => full,
            (WidthFrom::Pressure { curve, thinnest }, Some(pressure)) => {
                let p = pressure.get();
                let eased = match curve {
                    Curve::Linear => p,
                    Curve::Sqrt => p.sqrt(),
                    Curve::Square => p * p,
                };
                full * (thinnest.get() + (1.0 - thinnest.get()) * eased)
            }
        }
    }
}

/// How a mark is coloured.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Paint {
    pub color: Color,
}
