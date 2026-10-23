mod doc;
mod format;
mod history;
mod input;
mod mesh;
mod tool;

use std::sync::Arc;

use ink::brush::{Brush, Curve, Fraction, Paint, Size, WidthFrom};
use ink::doc::{Geometry, Ink, Mark, Placement};
use ink::geom::Point;
use ink::input::{Elapsed, Model, Pressure, Sample, Tilt, WindowParams};
use model::color::Rgb;
use model::theme::Color;

pub fn sample<S>(x: f32, y: f32, seconds: f32, pressure: f32) -> Sample<S> {
    Sample {
        at: Point::new(x, y),
        time: Elapsed::seconds(seconds).unwrap(),
        pressure: Some(Pressure::new(pressure).unwrap()),
        tilt: Some(Tilt::new(10.0, -5.0).unwrap()),
    }
}

/// A wavy line sampled at 200 Hz with pressure rising and falling.
pub fn wave<S>(count: usize) -> Vec<Sample<S>> {
    (0..count)
        .map(|index| {
            let t = index as f32 / 200.0;
            sample(
                index as f32 * 1.5,
                (index as f32 * 0.2).sin() * 12.0,
                t,
                0.5 + 0.4 * (index as f32 * 0.05).sin(),
            )
        })
        .collect()
}

pub fn brush(model: Model) -> Brush {
    Brush {
        size: Size::new(4.0).unwrap(),
        width: WidthFrom::Pressure {
            curve: Curve::Sqrt,
            thinnest: Fraction::new(0.3).unwrap(),
        },
        model,
    }
}

pub fn window() -> Model {
    Model::SlidingWindow(WindowParams::DEFAULT)
}

pub fn paint() -> Paint {
    Paint {
        color: Color::opaque(Rgb::hex(0x203040)),
    }
}

pub fn mark(geometry: Geometry, placement: Placement) -> Mark {
    Mark {
        placement,
        paint: paint(),
        ink: Ink::Stroke(Arc::new(geometry)),
    }
}
