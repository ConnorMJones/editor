use crate::geom::Local;
use crate::input::{Kind, Modeled, ModeledPoint, Sample, Source};

/// Every sample becomes a final point as it arrives.
#[derive(Clone, Copy, Debug)]
pub struct Passthrough;

impl Passthrough {
    pub fn extend(
        &mut self,
        real: &[Sample<Local>],
        predicted: &[Sample<Local>],
        out: &mut Modeled,
    ) {
        let point = |sample: &Sample<Local>| ModeledPoint {
            at: sample.at,
            time: sample.time,
            pressure: sample.pressure,
            source: Source::Sample,
        };
        for sample in real {
            out.push(point(sample), Kind::Real);
        }
        out.stabilize(out.real().len());
        for sample in predicted {
            out.push(point(sample), Kind::Predicted);
        }
    }
}
