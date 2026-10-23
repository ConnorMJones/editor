use std::hint::black_box;
use std::sync::Arc;

use criterion::{Criterion, criterion_group, criterion_main};
use ink::brush::{Brush, Curve, EPSILON, Fraction, Paint, Size, WidthFrom};
use ink::doc::{
    Background, Drawing, Geometry, Ink, LayerIndex, Mark, MarkChange, PageIndex, PageLayer,
    PageSize, Paged, Placement,
};
use ink::format::{read_paged, write_paged};
use ink::geom::{Local, Point};
use ink::input::{Elapsed, Model, Modeled, Modeler, Pressure, Sample, WindowParams};
use ink::mesh::MeshBuilder;
use model::color::Rgb;
use model::theme::Color;

/// A two second stroke sampled at 200 Hz, wavy and with varying pressure.
fn samples() -> Vec<Sample<Local>> {
    (0..400)
        .map(|index| Sample {
            at: Point::new(index as f32 * 1.5, (index as f32 * 0.2).sin() * 12.0),
            time: Elapsed::seconds(index as f32 / 200.0).unwrap(),
            pressure: Some(Pressure::new(0.5 + 0.4 * (index as f32 * 0.05).sin()).unwrap()),
            tilt: None,
        })
        .collect()
}

fn brush() -> Brush {
    Brush {
        size: Size::new(4.0).unwrap(),
        width: WidthFrom::Pressure {
            curve: Curve::Sqrt,
            thinnest: Fraction::new(0.3).unwrap(),
        },
        model: Model::SlidingWindow(WindowParams::DEFAULT),
    }
}

/// A page holding 1,000 two second strokes.
fn page() -> Paged {
    let mut paged = Paged::new(PageSize::A4, Background::Plain(Rgb::hex(0xffffff)));
    let geometry = Arc::new(Geometry::new(samples().into(), brush()).unwrap());
    let target = PageLayer {
        page: PageIndex(0),
        layer: LayerIndex(0),
    };
    for _ in 0..1000 {
        let mark = Mark {
            placement: Placement::ALIGNED,
            paint: Paint {
                color: Color::opaque(Rgb::hex(0x203040)),
            },
            ink: Ink::Stroke(geometry.clone()),
        };
        paged.apply(MarkChange::Add(target, mark).into()).unwrap();
    }
    paged
}

fn drawing(c: &mut Criterion) {
    let samples = samples();
    let brush = brush();
    c.bench_function("model and mesh a 400 sample stroke live", |b| {
        b.iter(|| {
            let mut modeler = Modeler::new(brush.model, EPSILON);
            let mut modeled = Modeled::default();
            let mut mesh = MeshBuilder::default();
            for sample in &samples {
                modeler.extend(std::slice::from_ref(sample), &[], &mut modeled);
                mesh.update(&brush, &modeled);
            }
            black_box(mesh.mesh().triangles.len())
        })
    });
    c.bench_function("build a 400 sample stroke whole", |b| {
        b.iter(|| black_box(Geometry::new(samples.as_slice().into(), brush).unwrap()))
    });
}

fn files(c: &mut Criterion) {
    let paged = page();
    let mut written = Vec::new();
    write_paged(&paged, &mut written);
    c.bench_function("write a page of 1000 strokes", |b| {
        b.iter(|| {
            let mut out = Vec::with_capacity(written.len());
            write_paged(&paged, &mut out);
            black_box(out)
        })
    });
    let mut group = c.benchmark_group("read");
    group.sample_size(10);
    group.bench_function("read a page of 1000 strokes", |b| {
        b.iter(|| black_box(read_paged(&written).unwrap()))
    });
    group.finish();
}

criterion_group!(benches, drawing, files);
criterion_main!(benches);
