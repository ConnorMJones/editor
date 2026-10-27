use ink::brush::Size;
use ink::doc::{Background, Board, Drawing, Ink, LayerIndex, MarkChange, Placement, ShapeKind};
use ink::geom::{Affine, Point, Rect, Surface};
use ink::history::Edited;
use ink::input::Sample;
use ink::tool::{Gather, Grip, Handle, Outcome, PenInput, Selecting, ToolKind, Tools, Using};
use model::color::Rgb;

use crate::{brush, paint, sample, wave, window};

struct Hand {
    tools: Tools,
    using: Using<LayerIndex>,
}

impl Hand {
    fn input(
        &mut self,
        kind: ToolKind,
        input: PenInput<LayerIndex>,
        board: &Board,
        reach: Size,
    ) -> Outcome<LayerIndex> {
        self.tools.input(kind, input, &mut self.using, board, reach)
    }
}

fn tools() -> Hand {
    Hand {
        tools: Tools::new(brush(window()), paint(), Size::new(6.0).unwrap()),
        using: Using::Idle,
    }
}

fn reach() -> Size {
    Size::new(4.0).unwrap()
}

fn board() -> Edited<Board> {
    Edited::new(Board::new(Background::Plain(Rgb::hex(0xffffff))))
}

/// Samples along straight lines through `points`, 5 ms apart.
fn path(points: &[(f32, f32)]) -> Vec<Sample<Surface>> {
    let mut samples = Vec::new();
    for pair in points.windows(2) {
        let ((ax, ay), (bx, by)) = (pair[0], pair[1]);
        for step in 0..10 {
            let t = step as f32 / 10.0;
            let time = samples.len() as f32 / 200.0;
            samples.push(sample(ax + (bx - ax) * t, ay + (by - ay) * t, time, 0.5));
        }
    }
    let &(x, y) = points.last().unwrap();
    samples.push(sample(x, y, samples.len() as f32 / 200.0, 0.5));
    samples
}

/// Works `kind` through `samples`, returning the outcome of lifting the pen.
fn gesture(
    tools: &mut Hand,
    kind: ToolKind,
    board: &Board,
    samples: &[Sample<Surface>],
) -> Outcome<LayerIndex> {
    let (first, rest) = samples.split_first().unwrap();
    let (last, middle) = rest.split_last().unwrap();
    let down = PenInput::Down {
        target: LayerIndex(0),
        sample: *first,
    };
    assert!(matches!(
        tools.input(kind, down, board, reach()),
        Outcome::Drawn
    ));
    for sample in middle {
        tools.input(kind, PenInput::Move(*sample), board, reach());
    }
    tools.input(kind, PenInput::Up(*last), board, reach())
}

/// Commits what `kind` proposes over `points`.
fn commit(edited: &mut Edited<Board>, tools: &mut Hand, kind: ToolKind, points: &[(f32, f32)]) {
    match gesture(tools, kind, edited.drawing(), &path(points)) {
        Outcome::Commit(change) => {
            edited.commit(change.into()).unwrap();
        }
        other => panic!("{other:?}"),
    }
}

/// Two short strokes, one near the origin and one at x 100.
fn two_strokes(tools: &mut Hand) -> Edited<Board> {
    let mut edited = board();
    commit(
        &mut edited,
        tools,
        ToolKind::Brush,
        &[(0.0, 0.0), (20.0, 0.0)],
    );
    commit(
        &mut edited,
        tools,
        ToolKind::Brush,
        &[(100.0, 0.0), (120.0, 0.0)],
    );
    edited
}

fn bounds(edited: &Edited<Board>) -> Vec<Rect<Surface>> {
    (edited.drawing().marks(LayerIndex(0)).unwrap().iter())
        .map(|(_, mark)| mark.bounds())
        .collect()
}

fn picked(tools: &Hand) -> usize {
    match &tools.using {
        Using::Selecting(Selecting::Holding { picked, .. }) => picked.ids().len(),
        _ => 0,
    }
}

#[test]
fn lifting_the_brush_proposes_the_stroke() {
    let board = board();
    let board = board.drawing();
    let mut tools = tools();
    let samples = wave::<Surface>(50);
    let (first, rest) = samples.split_first().unwrap();
    let down = PenInput::Down {
        target: LayerIndex(0),
        sample: *first,
    };
    tools.input(ToolKind::Brush, down, board, reach());
    for sample in &rest[..10] {
        tools.input(ToolKind::Brush, PenInput::Move(*sample), board, reach());
    }
    assert!(matches!(tools.using, Using::Brushing { .. }));
    assert!(!tools.tools.stroke.mesh.mesh().triangles.is_empty());
    tools.input(ToolKind::Brush, PenInput::Cancel, board, reach());
    assert!(matches!(tools.using, Using::Idle));
    match gesture(&mut tools, ToolKind::Brush, board, &samples) {
        Outcome::Commit(MarkChange::Add(LayerIndex(0), mark)) => match &mark.ink {
            Ink::Stroke(geometry) => assert_eq!(geometry.samples.len(), 50),
            Ink::Shape(_) | Ink::Text(_) | Ink::Image(_) => panic!("the brush draws strokes"),
        },
        other => panic!("{other:?}"),
    }
    assert!(matches!(tools.using, Using::Idle));
}

#[test]
fn input_without_a_stroke_is_ignored() {
    let board = board();
    let mut tools = tools();
    let at = sample::<Surface>(1.0, 1.0, 0.0, 0.5);
    let kinds = [
        ToolKind::Brush,
        ToolKind::Eraser,
        ToolKind::Select(Gather::Lasso),
        ToolKind::Shape(ShapeKind::Line),
    ];
    for input in [PenInput::Move(at), PenInput::Up(at), PenInput::Cancel] {
        for kind in kinds {
            assert!(
                matches!(
                    tools.input(kind, input, board.drawing(), reach()),
                    Outcome::Ignored
                ),
                "{kind:?}"
            );
        }
    }
}

#[test]
fn erasing_removes_the_strokes_crossed_in_one_step() {
    let mut tools = tools();
    let mut edited = board();
    commit(
        &mut edited,
        &mut tools,
        ToolKind::Brush,
        &[(0.0, 0.0), (100.0, 0.0)],
    );
    commit(
        &mut edited,
        &mut tools,
        ToolKind::Brush,
        &[(0.0, 100.0), (100.0, 100.0)],
    );
    let across = path(&[(50.0, -20.0), (50.0, 20.0)]);
    let (first, rest) = across.split_first().unwrap();
    let down = PenInput::Down {
        target: LayerIndex(0),
        sample: *first,
    };
    tools.input(ToolKind::Eraser, down, edited.drawing(), reach());
    for sample in &rest[..rest.len() - 1] {
        tools.input(
            ToolKind::Eraser,
            PenInput::Move(*sample),
            edited.drawing(),
            reach(),
        );
    }
    match &tools.using {
        Using::Erasing { erased, .. } => {
            assert_eq!(erased.len(), 1, "erased marks are shown hidden")
        }
        other => panic!("still erasing: {other:?}"),
    }
    let Outcome::Commit(change) = tools.input(
        ToolKind::Eraser,
        PenInput::Up(rest[rest.len() - 1]),
        edited.drawing(),
        reach(),
    ) else {
        panic!("the eraser proposes a removal");
    };
    edited.commit(change.into()).unwrap();
    assert_eq!(edited.drawing().marks(LayerIndex(0)).unwrap().len(), 1);
    edited.undo().unwrap();
    assert_eq!(edited.drawing().marks(LayerIndex(0)).unwrap().len(), 2);
}

#[test]
fn a_lasso_picks_the_marks_it_encloses() {
    let mut tools = tools();
    let edited = two_strokes(&mut tools);
    let lasso = [
        (-10.0, -10.0),
        (30.0, -10.0),
        (30.0, 10.0),
        (-10.0, 10.0),
        (-10.0, -10.0),
    ];
    gesture(
        &mut tools,
        ToolKind::Select(Gather::Lasso),
        edited.drawing(),
        &path(&lasso),
    );
    assert_eq!(picked(&tools), 1);
    let around = [
        (-10.0, -10.0),
        (130.0, -10.0),
        (130.0, 10.0),
        (-10.0, 10.0),
        (-10.0, -10.0),
    ];
    gesture(
        &mut tools,
        ToolKind::Select(Gather::Lasso),
        edited.drawing(),
        &path(&around),
    );
    assert_eq!(picked(&tools), 2);
    let empty = [(-10.0, 50.0), (130.0, 50.0), (130.0, 70.0), (-10.0, 50.0)];
    gesture(
        &mut tools,
        ToolKind::Select(Gather::Lasso),
        edited.drawing(),
        &path(&empty),
    );
    assert_eq!(picked(&tools), 0);
}

#[test]
fn a_tap_picks_the_mark_under_it() {
    let mut tools = tools();
    let edited = two_strokes(&mut tools);
    gesture(
        &mut tools,
        ToolKind::Select(Gather::Lasso),
        edited.drawing(),
        &path(&[(110.0, 0.0), (111.0, 1.0)]),
    );
    match &tools.using {
        Using::Selecting(Selecting::Holding { picked, .. }) => {
            let marks = edited.drawing().marks(LayerIndex(0)).unwrap();
            let (top, _) = marks.iter().last().unwrap();
            assert_eq!(picked.ids(), [top]);
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn moving_a_selection_is_one_step_that_undoes_whole() {
    let mut tools = tools();
    let mut edited = two_strokes(&mut tools);
    let before = bounds(&edited);
    let around = [
        (-10.0, -10.0),
        (130.0, -10.0),
        (130.0, 10.0),
        (-10.0, 10.0),
        (-10.0, -10.0),
    ];
    gesture(
        &mut tools,
        ToolKind::Select(Gather::Lasso),
        edited.drawing(),
        &path(&around),
    );
    commit(
        &mut edited,
        &mut tools,
        ToolKind::Select(Gather::Lasso),
        &[(60.0, 0.0), (60.0, 50.0)],
    );
    let after = bounds(&edited);
    for (before, after) in before.iter().zip(&after) {
        assert!((after.min().y - before.min().y - 50.0).abs() < 1e-3);
        assert!((after.min().x - before.min().x).abs() < 1e-3);
    }
    edited.undo().unwrap();
    assert_eq!(bounds(&edited), before);
}

#[test]
fn the_handles_rotate_and_scale_the_selection() {
    let mut tools = tools();
    let mut edited = board();
    commit(
        &mut edited,
        &mut tools,
        ToolKind::Brush,
        &[(0.0, 0.0), (40.0, 0.0)],
    );
    let tap = path(&[(20.0, 0.0), (20.5, 0.0)]);
    gesture(
        &mut tools,
        ToolKind::Select(Gather::Lasso),
        edited.drawing(),
        &tap,
    );
    let box_of = |tools: &Hand| match &tools.using {
        Using::Selecting(Selecting::Holding {
            picked,
            grip: Grip::Still,
            ..
        }) => picked.frame(),
        other => panic!("{other:?}"),
    };
    let held = box_of(&tools);
    let surface = |frame: ink::tool::Frame| frame.rect.mapped(frame.placement.forward());

    let corner = Handle::Corner(ink::tool::Corner::BottomRight).at(&held, reach());
    let farther = (corner.x + 40.0, corner.y);
    commit(
        &mut edited,
        &mut tools,
        ToolKind::Select(Gather::Lasso),
        &[(corner.x, corner.y), farther],
    );
    let scaled = bounds(&edited)[0];
    let width = |rect: Rect<Surface>| rect.max().x - rect.min().x;
    assert!((width(scaled) - width(surface(held)) - 40.0).abs() < 0.5);
    let (held, now) = (surface(box_of(&tools)), scaled);
    assert!(
        held.min().distance(now.min()) < 1e-3 && held.max().distance(now.max()) < 1e-3,
        "the box follows the marks"
    );

    let rotate = Handle::Rotate.at(&box_of(&tools), reach());
    let center = box_of(&tools).centre();
    let beside = (center.x + (center.y - rotate.y), center.y);
    commit(
        &mut edited,
        &mut tools,
        ToolKind::Select(Gather::Lasso),
        &[(rotate.x, rotate.y), beside],
    );
    let turned = bounds(&edited)[0];
    let height = |rect: Rect<Surface>| rect.max().y - rect.min().y;
    assert!(
        (height(turned) - width(scaled)).abs() < 1.0,
        "a quarter turn stands the stroke up: {turned:?}"
    );
    assert_eq!(edited.drawing().marks(LayerIndex(0)).unwrap().len(), 1);
}

#[test]
fn deleting_removes_the_picked_marks() {
    let mut tools = tools();
    let mut edited = two_strokes(&mut tools);
    gesture(
        &mut tools,
        ToolKind::Select(Gather::Lasso),
        edited.drawing(),
        &path(&[(10.0, 0.0), (10.5, 0.0)]),
    );
    let change = tools.using.delete().unwrap();
    edited.commit(change.into()).unwrap();
    assert_eq!(edited.drawing().marks(LayerIndex(0)).unwrap().len(), 1);
    assert!(tools.using.delete().is_none(), "nothing is left picked");
}

#[test]
fn a_shape_is_dragged_out_and_a_tap_draws_none() {
    let mut tools = tools();
    let mut edited = board();
    let rectangle = ToolKind::Shape(ShapeKind::Rectangle);
    let at = sample::<Surface>(5.0, 5.0, 0.0, 0.5);
    let down = PenInput::Down {
        target: LayerIndex(0),
        sample: at,
    };
    tools.input(rectangle, down, edited.drawing(), reach());
    assert!(matches!(
        tools.input(rectangle, PenInput::Up(at), edited.drawing(), reach()),
        Outcome::Drawn
    ));
    commit(
        &mut edited,
        &mut tools,
        rectangle,
        &[(0.0, 0.0), (60.0, 40.0)],
    );
    let marks = edited.drawing().marks(LayerIndex(0)).unwrap();
    let (_, mark) = marks.iter().next().unwrap();
    assert!(
        mark.touches(Point::new(60.0, 40.0), 0.1),
        "a corner is drawn"
    );
    assert!(mark.touches(Point::new(30.0, 0.0), 0.1), "an edge is drawn");
    assert!(
        !mark.touches(Point::new(30.0, 20.0), 1.0),
        "the inside is empty"
    );
}

#[test]
fn a_box_picks_the_marks_wholly_inside_it() {
    let mut tools = tools();
    let edited = two_strokes(&mut tools);
    let select = ToolKind::Select(Gather::Box);
    gesture(
        &mut tools,
        select,
        edited.drawing(),
        &path(&[(-10.0, -10.0), (30.0, 10.0)]),
    );
    assert_eq!(picked(&tools), 1);
    gesture(
        &mut tools,
        select,
        edited.drawing(),
        &path(&[(-10.0, -10.0), (110.0, 10.0)]),
    );
    assert_eq!(picked(&tools), 1, "the second stroke is only partly inside");
    gesture(
        &mut tools,
        select,
        edited.drawing(),
        &path(&[(130.0, 10.0), (-10.0, -10.0)]),
    );
    assert_eq!(picked(&tools), 2, "a box drawn backwards picks the same");
}

/// A board holding a 100 by 50 image turned a quarter, its top left corner at (200, 200).
fn turned_image() -> Edited<Board> {
    let mut edited = board();
    let ink = ink::doc::ImageInk::new(
        std::sync::Arc::from(&b"pixels"[..]),
        ink::doc::Embedded::Png,
        ink::doc::PageSize::new(100.0, 50.0).unwrap(),
    );
    let mark = ink::doc::Mark {
        placement: Placement::new(Affine::new([0.0, 1.0, -1.0, 0.0, 200.0, 200.0])).unwrap(),
        paint: crate::paint(),
        ink: ink::doc::Ink::Image(std::sync::Arc::new(ink)),
    };
    edited
        .commit(ink::doc::MarkChange::Add(LayerIndex(0), mark).into())
        .unwrap();
    edited
}

fn placement(edited: &Edited<Board>) -> [f32; 6] {
    let marks = edited.drawing().marks(LayerIndex(0)).unwrap();
    let (_, mark) = marks.iter().next().unwrap();
    mark.placement.forward().coefficients
}

fn close(a: [f32; 6], b: [f32; 6]) -> bool {
    a.iter().zip(b).all(|(a, b)| (a - b).abs() < 1e-3)
}

#[test]
fn a_turned_mark_is_handled_on_its_own_corners_and_keeps_its_shape() {
    let mut tools = tools();
    let mut edited = turned_image();
    // Its middle, (50, 25) in its own space, is at (175, 250) on the board.
    let tap = path(&[(175.0, 250.0), (175.5, 250.0)]);
    gesture(
        &mut tools,
        ToolKind::Select(Gather::Box),
        edited.drawing(),
        &tap,
    );
    let frame = match &tools.using {
        Using::Selecting(Selecting::Holding { picked, .. }) => picked.frame(),
        other => panic!("{other:?}"),
    };
    let corner = Handle::Corner(ink::tool::Corner::BottomRight).at(&frame, reach());
    assert!(
        corner.distance(Point::new(150.0, 300.0)) < 1e-3,
        "{corner:?}"
    );
    // Dragged to twice as far from the opposite corner, it doubles, still turned.
    commit(
        &mut edited,
        &mut tools,
        ToolKind::Select(Gather::Box),
        &[(150.0, 300.0), (100.0, 400.0)],
    );
    assert!(
        close(placement(&edited), [0.0, 2.0, -2.0, 0.0, 200.0, 200.0]),
        "{:?}",
        placement(&edited)
    );
}

#[test]
fn a_side_handle_stretches_one_way_in_the_marks_own_frame() {
    let mut tools = tools();
    let mut edited = turned_image();
    let tap = path(&[(175.0, 250.0), (175.5, 250.0)]);
    gesture(
        &mut tools,
        ToolKind::Select(Gather::Box),
        edited.drawing(),
        &tap,
    );
    // The right side's middle, (100, 25) in its own space, is at (175, 300); pulled to where
    // (150, 25) would be, the image grows half again along its own width.
    commit(
        &mut edited,
        &mut tools,
        ToolKind::Select(Gather::Box),
        &[(175.0, 300.0), (175.0, 350.0)],
    );
    assert!(
        close(placement(&edited), [0.0, 1.5, -1.0, 0.0, 200.0, 200.0]),
        "{:?}",
        placement(&edited)
    );
}

#[test]
fn a_tap_on_a_held_handle_changes_nothing() {
    let mut tools = tools();
    let edited = turned_image();
    let select = ToolKind::Select(Gather::Box);
    let tap = path(&[(175.0, 250.0), (175.5, 250.0)]);
    gesture(&mut tools, select, edited.drawing(), &tap);
    let on_corner = path(&[(150.0, 300.0), (150.5, 300.0)]);
    assert!(matches!(
        gesture(&mut tools, select, edited.drawing(), &on_corner),
        Outcome::Ignored
    ));
    assert_eq!(picked(&tools), 1, "still held");
}
