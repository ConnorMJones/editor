use ink::brush::{EPSILON, Size, WidthFrom};
use ink::doc::Geometry;
use ink::geom::{Local, Point, triangle_distance};
use ink::input::{Model, Modeled, Modeler, Sample};
use ink::mesh::{Mesh, MeshBuilder};

use crate::{brush, sample, wave, window};

fn covers(mesh: &Mesh, at: Point<Local>) -> bool {
    mesh.triangles.iter().any(|triangle| {
        triangle_distance(at, triangle.map(|index| mesh.vertices[index as usize])) < 1e-4
    })
}

fn constant(size: f32) -> ink::brush::Brush {
    ink::brush::Brush {
        size: Size::new(size).unwrap(),
        width: WidthFrom::Constant,
        model: Model::Passthrough,
    }
}

#[test]
fn a_single_sample_is_a_dot_of_the_brush_size() {
    let geometry = Geometry::new(
        vec![sample::<Local>(5.0, 5.0, 0.0, 1.0)].into(),
        constant(4.0),
    )
    .unwrap();
    let bounds = geometry.built.bounds;
    assert!(
        (bounds.max().x - bounds.min().x - 4.0).abs() < 2.0 * EPSILON,
        "{bounds:?}"
    );
    assert!(covers(&geometry.built.mesh, Point::new(5.0, 5.0)));
    assert!(covers(&geometry.built.mesh, Point::new(6.9, 5.0)));
    assert!(!covers(&geometry.built.mesh, Point::new(7.1, 5.0)));
}

#[test]
fn a_line_covers_its_width_and_no_more() {
    let samples: Vec<Sample<Local>> = (0..=10)
        .map(|i| sample(i as f32 * 10.0, 0.0, i as f32 / 100.0, 1.0))
        .collect();
    let geometry = Geometry::new(samples.into(), constant(6.0)).unwrap();
    let mesh = &geometry.built.mesh;
    for x in [0.0, 33.0, 50.0, 99.0] {
        assert!(covers(mesh, Point::new(x, 2.9)), "x {x}");
        assert!(!covers(mesh, Point::new(x, 3.1)), "x {x}");
    }
    assert!(covers(mesh, Point::new(102.9, 0.0)));
    assert!(!covers(mesh, Point::new(103.1, 0.0)));
}

#[test]
fn a_sharp_turn_leaves_no_gap_on_its_outside() {
    let samples: Vec<Sample<Local>> = [(0.0, 0.0), (20.0, 0.0), (20.0, 20.0)]
        .iter()
        .enumerate()
        .map(|(i, (x, y))| sample(*x, *y, i as f32 / 100.0, 1.0))
        .collect();
    let mesh = Geometry::new(samples.into(), constant(6.0))
        .unwrap()
        .built
        .mesh;
    let half = std::f32::consts::FRAC_1_SQRT_2;
    let corner = Point::new(20.0 + 2.9 * half, -2.9 * half);
    assert!(covers(&mesh, corner));
}

#[test]
fn the_mesh_built_while_drawing_matches_the_one_rebuilt_from_the_samples() {
    let samples = wave::<Local>(300);
    let brush = brush(window());
    let mut modeler = Modeler::new(brush.model, EPSILON);
    let mut modeled = Modeled::default();
    let mut live = MeshBuilder::default();
    for sample in &samples {
        modeler.extend(std::slice::from_ref(sample), &[], &mut modeled);
        live.update(&brush, &modeled);
    }
    let rebuilt = Geometry::new(samples.into(), brush).unwrap();
    assert_eq!(live.mesh(), &rebuilt.built.mesh);
}

#[test]
fn a_two_second_stroke_stays_within_the_triangle_estimate() {
    let geometry = Geometry::new(wave::<Local>(400).into(), brush(window())).unwrap();
    let triangles = geometry.built.mesh.triangles.len();
    assert!(triangles < 400 * 8, "{triangles} triangles");
}

#[test]
fn a_tip_swelling_in_place_is_covered_all_round() {
    let swell = (0..20).map(|i| {
        sample(
            i as f32 * 0.1,
            0.0,
            i as f32 / 200.0,
            0.05 + i as f32 * 0.05,
        )
    });
    let away = (1..10).map(|i| sample(1.9 + i as f32 * 3.0, 0.0, (19 + i) as f32 / 200.0, 1.0));
    let samples: Vec<Sample<Local>> = swell.chain(away).collect();
    let brush = pressure_brush();
    let mesh = Geometry::new(samples.into(), brush).unwrap().built.mesh;
    for angle in [0.5f32, 1.5, 2.5, std::f32::consts::PI, 4.0] {
        let rim = Point::new(1.9 + 4.8 * angle.cos(), 4.8 * angle.sin());
        assert!(covers(&mesh, rim), "angle {angle}");
    }
}

fn pressure_brush() -> ink::brush::Brush {
    ink::brush::Brush {
        size: Size::new(10.0).unwrap(),
        width: WidthFrom::Pressure {
            curve: ink::brush::Curve::Linear,
            thinnest: ink::brush::Fraction::new(0.0).unwrap(),
        },
        model: Model::Passthrough,
    }
}
