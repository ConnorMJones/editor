use ink::brush::EPSILON;
use ink::geom::Local;
use ink::input::{InvalidWindow, Model, Modeled, Modeler, Pressure, Sample, Source, WindowParams};

use crate::{sample, wave, window};

#[test]
fn passthrough_keeps_every_sample_as_a_final_point() {
    let samples = wave::<Local>(20);
    let modeled = Modeler::model(Model::Passthrough, EPSILON, &samples);
    assert_eq!(modeled.stable().len(), 20);
    assert_eq!(modeled.real().len(), 20);
    for (point, sample) in modeled.all().iter().zip(&samples) {
        assert_eq!(
            (point.at, point.time, point.pressure),
            (sample.at, sample.time, sample.pressure)
        );
    }
}

/// Feeds `samples` in chunks of the given sizes, checking the stable, real and predicted split
/// after each update.
fn incrementally(model: Model, samples: &[Sample<Local>], chunks: &[usize]) -> Modeled {
    let mut modeler = Modeler::new(model, EPSILON);
    let mut out = Modeled::default();
    let mut at = 0;
    for size in chunks.iter().cycle() {
        match at >= samples.len() {
            true => break,
            false => {}
        }
        let end = (at + size).min(samples.len());
        let stable_before = out.stable().to_vec();
        modeler.extend(&samples[at..end], &[], &mut out);
        assert!(out.stable().len() <= out.real().len() && out.real().len() <= out.all().len());
        assert_eq!(
            &out.stable()[..stable_before.len()],
            stable_before.as_slice(),
            "final points moved"
        );
        at = end;
    }
    out
}

#[test]
fn a_stroke_modeled_as_it_is_drawn_matches_it_modeled_whole() {
    let samples = wave::<Local>(400);
    let whole = Modeler::model(window(), EPSILON, &samples);
    for chunks in [&[1][..], &[3], &[1, 7, 2], &[50]] {
        let live = incrementally(window(), &samples, chunks);
        assert_eq!(live.real(), whole.real(), "chunks {chunks:?}");
    }
}

#[test]
fn the_last_final_point_comes_from_a_sample() {
    let samples = wave::<Local>(100);
    let live = incrementally(window(), &samples, &[1]);
    assert_eq!(live.stable().last().unwrap().source, Source::Sample);
}

#[test]
fn sparse_samples_are_upsampled() {
    let samples: Vec<Sample<Local>> = (0..5)
        .map(|i| sample(i as f32 * 20.0, 0.0, i as f32 * 0.05, 0.5))
        .collect();
    let modeled = Modeler::model(window(), EPSILON, &samples);
    let upsample = WindowParams::DEFAULT.upsample.get();
    for pair in modeled.real().windows(2) {
        assert!(
            pair[1].time.get() - pair[0].time.get() <= upsample + 1e-5,
            "{pair:?}"
        );
    }
}

#[test]
fn jitter_is_smoothed() {
    let samples: Vec<Sample<Local>> = (0..200)
        .map(|i| {
            sample(
                i as f32,
                match i % 2 {
                    0 => 0.4,
                    _ => -0.4,
                },
                i as f32 / 200.0,
                0.5,
            )
        })
        .collect();
    let modeled = Modeler::model(window(), EPSILON, &samples);
    let middle = &modeled.real()[20..modeled.real().len() - 20];
    assert!(
        middle.iter().all(|point| point.at.y.abs() < 0.2),
        "{middle:?}"
    );
}

#[test]
fn modeled_points_are_spaced_by_more_than_epsilon() {
    let samples: Vec<Sample<Local>> = (0..100)
        .map(|i| sample(0.0, 0.0, i as f32 / 200.0, 0.5))
        .collect();
    let modeled = Modeler::model(window(), EPSILON, &samples);
    assert_eq!(modeled.real().len(), 1);
}

#[test]
fn averaged_pressure_stays_in_range() {
    let samples: Vec<Sample<Local>> = (0..50)
        .map(|i| sample(i as f32, 0.0, i as f32 / 200.0, 1.0))
        .collect();
    let modeled = Modeler::model(window(), EPSILON, &samples);
    assert!(
        modeled
            .real()
            .iter()
            .all(|point| point.pressure == Some(Pressure::new(1.0).unwrap()))
    );
}

#[test]
fn window_params_must_be_positive_durations() {
    assert_eq!(
        WindowParams::new(0.0, 0.01),
        Err(InvalidWindow::Window(0.0))
    );
    assert!(matches!(
        WindowParams::new(0.02, f32::NAN),
        Err(InvalidWindow::Upsample(_))
    ));
}
