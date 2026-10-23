use model::geom::{Affine, Local, Point, Rect, Surface, triangle_distance};

#[test]
fn an_affine_followed_by_its_inverse_is_the_identity() {
    let (sin, cos) = 0.7f32.sin_cos();
    let affine: Affine<Local, Surface> =
        Affine::new([2.0 * cos, 2.0 * sin, -2.0 * sin, 2.0 * cos, 5.0, -3.0]);
    let back = affine.inverse().unwrap();
    let at = Point::new(3.0, 4.0);
    let round = back.apply(affine.apply(at));
    assert!(round.distance(at) < 1e-4, "{round:?}");
    assert!((affine.stretch() - 2.0).abs() < 1e-5);
}

#[test]
fn a_flattening_affine_has_no_inverse() {
    let flat: Affine<Local, Surface> = Affine::new([1.0, 2.0, 2.0, 4.0, 0.0, 0.0]);
    assert_eq!(flat.inverse(), None);
}

#[test]
fn composing_applies_the_first_transform_first() {
    let scale: Affine<Local, Surface> = Affine::scale(2.0);
    let shift: Affine<Surface, Surface> = Affine::translation(10.0, 0.0);
    let at = scale.then(&shift).apply(Point::new(1.0, 1.0));
    assert_eq!(at, Point::new(12.0, 2.0));
}

#[test]
fn a_mapped_box_holds_every_mapped_corner() {
    let rect: Rect<Local> = Rect::spanning(Point::new(0.0, 0.0), Point::new(4.0, 2.0));
    let (sin, cos) = 0.5f32.sin_cos();
    let turn: Affine<Local, Surface> = Affine::new([cos, sin, -sin, cos, 0.0, 0.0]);
    let mapped = rect.mapped(&turn);
    for corner in [(0.0, 0.0), (4.0, 0.0), (0.0, 2.0), (4.0, 2.0)] {
        assert!(mapped.contains(turn.apply(Point::new(corner.0, corner.1))));
    }
}

#[test]
fn triangle_distance_is_zero_inside_and_to_the_nearest_edge_outside() {
    let corners: [Point<Local>; 3] = [
        Point::new(0.0, 0.0),
        Point::new(4.0, 0.0),
        Point::new(0.0, 4.0),
    ];
    assert_eq!(triangle_distance(Point::new(1.0, 1.0), corners), 0.0);
    assert!((triangle_distance(Point::new(2.0, -3.0), corners) - 3.0).abs() < 1e-5);
    assert!((triangle_distance(Point::new(-3.0, -4.0), corners) - 5.0).abs() < 1e-5);
}
