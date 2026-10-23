//! Raw pen samples, and the models that smooth them into the points a stroke is built from.

mod passthrough;
mod window;

use model::units::Positive;

use crate::geom::{Local, Point};

pub use passthrough::Passthrough;
pub use window::SlidingWindow;

/// Seconds since the stroke's first sample.
#[derive(Clone, Copy, Debug, PartialEq, PartialOrd)]
pub struct Elapsed(f32); // private: checked by its constructor

#[derive(Debug, thiserror::Error, PartialEq)]
#[error("{0} is not a time since the stroke started")]
pub struct InvalidElapsed(pub f32);

impl Elapsed {
    pub const ZERO: Elapsed = Elapsed(0.0);

    pub fn of(duration: std::time::Duration) -> Elapsed {
        Elapsed(duration.as_secs_f32())
    }

    pub fn seconds(seconds: f32) -> Result<Elapsed, InvalidElapsed> {
        match seconds.is_finite() && seconds >= 0.0 {
            true => Ok(Elapsed(seconds)),
            false => Err(InvalidElapsed(seconds)),
        }
    }

    /// The time `t` of the way from `self` to `to`, for `t` in `0..=1`.
    pub fn lerp(self, to: Elapsed, t: f32) -> Elapsed {
        Elapsed((self.0 + (to.0 - self.0) * t).max(0.0))
    }

    pub fn get(self) -> f32 {
        self.0
    }
}

/// Tip pressure in `0..=1`.
#[derive(Clone, Copy, Debug, PartialEq, PartialOrd)]
pub struct Pressure(f32); // private: checked by its constructor

#[derive(Debug, thiserror::Error, PartialEq)]
#[error("pressure {0} is not between 0 and 1")]
pub struct InvalidPressure(pub f32);

impl Pressure {
    pub fn new(pressure: f32) -> Result<Pressure, InvalidPressure> {
        match (0.0..=1.0).contains(&pressure) {
            true => Ok(Pressure(pressure)),
            false => Err(InvalidPressure(pressure)),
        }
    }

    /// The pressure `t` of the way from `self` to `to`, for `t` in `0..=1`.
    pub fn lerp(self, to: Pressure, t: f32) -> Pressure {
        Pressure((self.0 + (to.0 - self.0) * t).clamp(0.0, 1.0))
    }

    /// The mean of pressures summed with `weight` in all, each scaled by its share of it.
    pub fn average(weighted: f32, weight: f32) -> Pressure {
        Pressure((weighted / weight).clamp(0.0, 1.0))
    }

    pub fn get(self) -> f32 {
        self.0
    }
}

/// Stylus tilt from upright, towards +x and towards +y.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tilt {
    pub x: Angle,
    pub y: Angle,
}

/// Degrees from upright, within a quarter turn either way.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Angle(f32); // private: checked by `new`

impl Angle {
    pub fn new(degrees: f32) -> Option<Angle> {
        (-90.0..=90.0).contains(&degrees).then_some(Angle(degrees))
    }

    pub fn get(self) -> f32 {
        self.0
    }
}

#[derive(Debug, thiserror::Error, PartialEq)]
#[error("tilt ({0}, {1}) is not within 90 degrees of upright")]
pub struct InvalidTilt(pub f32, pub f32);

impl Tilt {
    pub fn new(x: f32, y: f32) -> Result<Tilt, InvalidTilt> {
        match (Angle::new(x), Angle::new(y)) {
            (Some(x), Some(y)) => Ok(Tilt { x, y }),
            (None, _) | (_, None) => Err(InvalidTilt(x, y)),
        }
    }
}

/// One reading from the pen; `pressure` and `tilt` are absent for devices without the sensor.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sample<S> {
    pub at: Point<S>,
    pub time: Elapsed,
    pub pressure: Option<Pressure>,
    pub tilt: Option<Tilt>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Source {
    Sample,
    /// Inserted between samples that were too far apart in time.
    Upsampled,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ModeledPoint {
    pub at: Point<Local>,
    pub time: Elapsed,
    pub pressure: Option<Pressure>,
    pub source: Source,
}

/// A stroke's modeled points: final ones, then a tail that later samples may still move, then
/// points modeled from predicted samples.
#[derive(Clone, Debug, Default)]
pub struct Modeled {
    points: Vec<ModeledPoint>, // private: stable <= real <= len, kept by the methods below
    stable: usize,             // private: stable <= real <= len, kept by the methods below
    real: usize,               // private: stable <= real <= len, kept by the methods below
}

/// Whether a modeled point comes from samples the pen gave, or from a prediction.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Real,
    Predicted,
}

impl Modeled {
    pub fn all(&self) -> &[ModeledPoint] {
        &self.points
    }

    pub fn stable(&self) -> &[ModeledPoint] {
        &self.points[..self.stable]
    }

    /// The points modeled from real samples, final or not.
    pub fn real(&self) -> &[ModeledPoint] {
        &self.points[..self.real]
    }

    pub fn clear(&mut self) {
        self.points.clear();
        self.stable = 0;
        self.real = 0;
    }

    pub fn drop_predicted(&mut self) {
        self.points.truncate(self.real);
    }

    /// Keeps only the final points, for a model to work out the rest again.
    pub fn reopen(&mut self) {
        self.points.truncate(self.stable);
        self.real = self.stable;
    }

    /// Real points come before every predicted one, so a real point after a prediction drops it.
    pub fn push(&mut self, point: ModeledPoint, kind: Kind) {
        match kind {
            Kind::Real => {
                self.points.truncate(self.real);
                self.points.push(point);
                self.real = self.points.len();
            }
            Kind::Predicted => self.points.push(point),
        }
    }

    /// The first `count` real points become final; a count past the real points stops at them.
    pub fn stabilize(&mut self, count: usize) {
        self.stable = count.min(self.real);
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Model {
    /// Samples pass through unchanged; the baseline the others are judged against.
    Passthrough,
    SlidingWindow(WindowParams),
}

/// In seconds: `window` is the span samples are averaged over; `upsample` the longest gap left
/// between modeled points.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WindowParams {
    pub window: Positive,
    pub upsample: Positive,
}

#[derive(Debug, thiserror::Error, PartialEq)]
pub enum InvalidWindow {
    #[error("the window of {0} s is not a positive duration")]
    Window(f32),
    #[error("the upsampling period of {0} s is not a positive duration")]
    Upsample(f32),
}

impl WindowParams {
    pub const DEFAULT: WindowParams = WindowParams {
        window: Positive::new(0.02).unwrap(),
        upsample: Positive::new(1.0 / 180.0).unwrap(),
    };

    pub fn new(window: f32, upsample: f32) -> Result<WindowParams, InvalidWindow> {
        Ok(WindowParams {
            window: Positive::new(window).ok_or(InvalidWindow::Window(window))?,
            upsample: Positive::new(upsample).ok_or(InvalidWindow::Upsample(upsample))?,
        })
    }
}

/// A running input model for one stroke at a time.
#[derive(Clone, Debug)]
pub enum Modeler {
    Passthrough(Passthrough),
    SlidingWindow(SlidingWindow),
}

impl Modeler {
    /// Points closer than `epsilon` to the previous one are dropped.
    pub fn new(model: Model, epsilon: f32) -> Modeler {
        match model {
            Model::Passthrough => Modeler::Passthrough(Passthrough),
            Model::SlidingWindow(params) => {
                Modeler::SlidingWindow(SlidingWindow::new(params, epsilon))
            }
        }
    }

    /// Models `real` samples after the ones already given, and `predicted` ones after them,
    /// replacing the previous prediction.
    pub fn extend(
        &mut self,
        real: &[Sample<Local>],
        predicted: &[Sample<Local>],
        out: &mut Modeled,
    ) {
        out.drop_predicted();
        match self {
            Modeler::Passthrough(model) => model.extend(real, predicted, out),
            Modeler::SlidingWindow(model) => model.extend(real, predicted, out),
        }
    }

    /// Models `samples` as a whole stroke.
    pub fn model(model: Model, epsilon: f32, samples: &[Sample<Local>]) -> Modeled {
        let mut out = Modeled::default();
        Modeler::new(model, epsilon).extend(samples, &[], &mut out);
        out
    }
}
