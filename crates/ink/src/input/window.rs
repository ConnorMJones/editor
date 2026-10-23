//! Google ink's sliding-window model: each point is the time-weighted average of the samples
//! within half a window either side of it.

use crate::geom::{Local, Point};
use crate::input::{Elapsed, Kind, Modeled, ModeledPoint, Pressure, Sample, Source, WindowParams};

/// No gap between samples is split into more modeled points than this.
const MAX_DIVISIONS: f32 = 100.0;

#[derive(Clone, Debug)]
pub struct SlidingWindow {
    half: f32,     // private: derived from WindowParams by `new`, plus the samples in flight
    upsample: f32, // private: derived from WindowParams by `new`, plus the samples in flight
    epsilon: f32,  // private: derived from WindowParams by `new`, plus the samples in flight
    /// Real samples still inside some unstable point's window, then the predicted ones.
    raw: Vec<Sample<Local>>, // private: derived from WindowParams by `new`, plus the samples in flight
}

/// Indices into the raw samples bounding the last window averaged; they only move forward.
#[derive(Clone, Copy, Debug, Default)]
struct Bounds {
    first: usize,
    last: usize,
}

impl SlidingWindow {
    pub fn new(params: WindowParams, epsilon: f32) -> SlidingWindow {
        SlidingWindow {
            half: params.window.get() * 0.5,
            upsample: params.upsample.get(),
            epsilon,
            raw: Vec::new(),
        }
    }

    pub fn extend(
        &mut self,
        real: &[Sample<Local>],
        predicted: &[Sample<Local>],
        out: &mut Modeled,
    ) {
        out.reopen();
        self.raw.extend_from_slice(real);
        let real_raw = self.raw.len();
        self.raw.extend_from_slice(predicted);
        self.model_unstable(out, real_raw);
        self.mark_stable(out);
        self.trim(out, real_raw);
    }

    fn model_unstable(&self, out: &mut Modeled, real_raw: usize) {
        // Points up to the last real sample are real; later ones come from the prediction.
        let cutoff = real_raw.checked_sub(1).map(|last| self.raw[last].time);
        let kind = |time: Elapsed| match cutoff {
            Some(cutoff) if time <= cutoff => Kind::Real,
            Some(_) | None => Kind::Predicted,
        };
        let mut previous = out.all().last().map(|point| point.time);
        let mut bounds = Bounds::default();
        for sample in &self.raw {
            let time = sample.time;
            match previous {
                Some(previous) if time <= previous => continue,
                Some(previous) => {
                    let gap = time.get() - previous.get();
                    let divisions = (gap / self.upsample).ceil().min(MAX_DIVISIONS) as u32;
                    for division in 1..divisions {
                        let at = previous.lerp(time, division as f32 / divisions as f32);
                        self.model_at(out, at, Source::Upsampled, kind(at), &mut bounds);
                    }
                }
                None => {}
            }
            self.model_at(out, time, Source::Sample, kind(time), &mut bounds);
            previous = Some(time);
        }
    }

    fn model_at(
        &self,
        out: &mut Modeled,
        time: Elapsed,
        source: Source,
        kind: Kind,
        bounds: &mut Bounds,
    ) {
        let raw = &self.raw;
        let (first, last, at) = (raw[0].time.get(), raw[raw.len() - 1].time.get(), time.get());
        let half = self.half.min(at - first).min(last - at);
        let (from, to) = ((at - half).max(first), (at + half).min(last));
        while bounds.first + 1 < raw.len() && raw[bounds.first + 1].time.get() <= from {
            bounds.first += 1;
        }
        while bounds.last + 1 < raw.len() && raw[bounds.last].time.get() <= to {
            bounds.last += 1;
        }
        let span = to - from;
        let (point, pressure) = match span > 0.0 {
            // A window with no width is the sample it sits on.
            false => {
                let sample = raw[bounds.first];
                (sample.at, sample.pressure)
            }
            true => {
                let (mut x, mut y, mut pressed) = (0.0, 0.0, 0.0);
                for index in bounds.first..bounds.last {
                    let (mut a, mut b) = (raw[index], raw[index + 1]);
                    match a.time.get() < from {
                        true => a = interpolate(a, b, from),
                        false => {}
                    }
                    match b.time.get() > to {
                        true => b = interpolate(a, b, to),
                        false => {}
                    }
                    let dt = b.time.get() - a.time.get();
                    x += dt * 0.5 * (a.at.x + b.at.x);
                    y += dt * 0.5 * (a.at.y + b.at.y);
                    match (a.pressure, b.pressure) {
                        (Some(pa), Some(pb)) => pressed += dt * 0.5 * (pa.get() + pb.get()),
                        (None, _) | (_, None) => {}
                    }
                }
                let pressure = raw[0].pressure.map(|_| Pressure::average(pressed, span));
                (Point::new(x / span, y / span), pressure)
            }
        };
        match out.all().last() {
            Some(last) if last.at.distance(point) <= self.epsilon => return,
            Some(_) | None => {}
        }
        let modeled = ModeledPoint {
            at: point,
            time,
            pressure,
            source,
        };
        out.push(modeled, kind);
    }

    /// Points whose window ends before the last real sample are final, backed off to one from a
    /// sample so the next update resumes exactly as a whole-stroke model would.
    fn mark_stable(&self, out: &mut Modeled) {
        let real = out.real();
        let Some(last) = real.last().map(|point| point.time.get()) else {
            return;
        };
        let mut stable = out.stable().len();
        while stable < real.len() && real[stable].time.get() + self.half < last {
            stable += 1;
        }
        while stable > 0 && real[stable - 1].source == Source::Upsampled {
            stable -= 1;
        }
        out.stabilize(stable);
    }

    /// Drops the prediction, and the samples no unstable point's window reaches back to.
    fn trim(&mut self, out: &Modeled, real_raw: usize) {
        self.raw.truncate(real_raw);
        let Some(stable) = out.stable().last().map(|point| point.time.get()) else {
            return;
        };
        let cutoff = stable - self.half;
        let mut next = 1;
        while next < real_raw && self.raw[next].time.get() <= cutoff {
            next += 1;
        }
        self.raw.drain(..next - 1);
    }
}

/// The sample at `time` on the segment from `a` to `b`.
fn interpolate(a: Sample<Local>, b: Sample<Local>, time: f32) -> Sample<Local> {
    let span = b.time.get() - a.time.get();
    // Two samples at one time are one place.
    let t = (span > 0.0)
        .then(|| (time - a.time.get()) / span)
        .unwrap_or(0.0);
    Sample {
        at: a.at.lerp(b.at, t),
        time: a.time.lerp(b.time, t),
        pressure: match (a.pressure, b.pressure) {
            (Some(pa), Some(pb)) => Some(pa.lerp(pb, t)),
            (None, _) | (_, None) => None,
        },
        tilt: a.tilt,
    }
}
