use ink::brush::{Brush, Size, WidthFrom};
use ink::doc::{
    Background, Board, BoardChange, Drawing, Geometry, Ink, Layer, LayerChange, LayerIndex,
    LayerName, Mark, MarkChange, PageIndex, PageLayer, PageSize, Paged, PagedChange, Pattern,
    Placement, Shape, ShapeInk, Spacing, Visibility,
};
use ink::format::{FormatError, read_board, read_paged, write_board, write_paged};
use ink::geom::{Affine, Local, Point};
use ink::input::{Elapsed, Model, Sample};
use model::color::Rgb;

use crate::{brush, sample, wave, window};

fn stroke(samples: Vec<Sample<Local>>, brush: Brush, placement: Placement) -> Mark {
    crate::mark(Geometry::new(samples.into(), brush).unwrap(), placement)
}

/// Two pages on two layers, the second hidden, holding strokes with and without pressure and tilt.
fn document() -> Paged {
    let lined = Background::Pattern {
        paper: Rgb::hex(0xfdfdf8),
        ink: Rgb::hex(0x9090c0),
        pattern: Pattern::Lines,
        spacing: Spacing::new(18.0).unwrap(),
    };
    let mut paged = Paged::new(PageSize::A4, lined);
    paged
        .apply(PagedChange::InsertPage {
            at: PageIndex(1),
            size: PageSize::new(400.0, 300.0).unwrap(),
            background: Background::Plain(Rgb::hex(0xffffff)),
        })
        .unwrap();
    let hidden = Layer {
        name: LayerName::new("Answers ✓".to_owned()).unwrap(),
        visibility: Visibility::Hidden,
    };
    paged
        .apply(PagedChange::Layers(LayerChange::Add(hidden)))
        .unwrap();
    let bare: Vec<Sample<Local>> = (0..20)
        .map(|i| Sample {
            at: Point::new(i as f32, 2.0 * i as f32),
            time: Elapsed::seconds(i as f32 / 120.0).unwrap(),
            pressure: None,
            tilt: None,
        })
        .collect();
    let constant = Brush {
        size: Size::new(2.5).unwrap(),
        width: WidthFrom::Constant,
        model: Model::Passthrough,
    };
    let turned = Placement::new(Affine::new([0.0, 1.5, -1.5, 0.0, 30.0, 40.0])).unwrap();
    let marks = [
        (
            PageIndex(0),
            LayerIndex(0),
            stroke(wave(60), brush(window()), Placement::ALIGNED),
        ),
        (PageIndex(0), LayerIndex(1), stroke(bare, constant, turned)),
        (
            PageIndex(1),
            LayerIndex(0),
            stroke(
                vec![sample(5.0, 5.0, 0.0, 0.2)],
                brush(window()),
                Placement::ALIGNED,
            ),
        ),
    ];
    for (page, layer, mark) in marks {
        paged
            .apply(MarkChange::Add(PageLayer { page, layer }, mark).into())
            .unwrap();
    }
    paged
}

fn bytes(paged: &Paged) -> Vec<u8> {
    let mut out = Vec::new();
    write_paged(paged, &mut out);
    out
}

#[test]
fn a_document_reads_back_as_written() {
    let written = bytes(&document());
    let read = read_paged(&written).unwrap();
    assert_eq!(bytes(&read), written);
    assert_eq!(read.layers(), document().layers());
    let first = read
        .marks(PageLayer {
            page: PageIndex(0),
            layer: LayerIndex(0),
        })
        .unwrap();
    let (_, mark) = first.iter().next().unwrap();
    let Ink::Stroke(stroke) = &mark.ink else {
        panic!("the first mark is a stroke");
    };
    let original = Geometry::new(wave::<Local>(60).into(), brush(window())).unwrap();
    assert_eq!(
        stroke.built.mesh, original.built.mesh,
        "meshes are rebuilt the same"
    );
}

#[test]
fn a_board_reads_back_as_written() {
    let mut board = Board::new(Background::Plain(Rgb::hex(0x102030)));
    board
        .apply(
            MarkChange::Add(
                LayerIndex(0),
                stroke(wave(40), brush(window()), Placement::ALIGNED),
            )
            .into(),
        )
        .unwrap();
    board
        .apply(BoardChange::Layers(LayerChange::Add(Layer {
            name: LayerName::new("Top".to_owned()).unwrap(),
            visibility: Visibility::Shown,
        })))
        .unwrap();
    let mut written = Vec::new();
    write_board(&board, &mut written);
    let mut again = Vec::new();
    write_board(&read_board(&written).unwrap(), &mut again);
    assert_eq!(again, written);
}

#[test]
fn every_cut_short_file_is_refused() {
    let written = bytes(&document());
    for length in 0..written.len() {
        assert!(read_paged(&written[..length]).is_err(), "length {length}");
    }
}

#[test]
fn a_board_is_not_a_paged_document() {
    let mut written = Vec::new();
    write_board(&Board::new(Background::Plain(Rgb::hex(0))), &mut written);
    assert_eq!(
        read_paged(&written).map(|_| ()),
        Err(FormatError::Kind("paged document"))
    );
}

#[test]
fn unknown_versions_and_extra_bytes_are_refused() {
    let mut written = bytes(&document());
    written.push(0);
    assert_eq!(
        read_paged(&written).map(|_| ()),
        Err(FormatError::Trailing(1))
    );
    written.pop();
    written[4] = 9;
    assert_eq!(
        read_paged(&written).map(|_| ()),
        Err(FormatError::Version(9))
    );
}

#[test]
fn an_out_of_range_reading_is_refused() {
    let single = vec![sample::<Local>(5.0, 5.0, 0.0, 0.5)];
    let mut paged = Paged::new(PageSize::A4, Background::Plain(Rgb::hex(0xffffff)));
    let first = PageLayer {
        page: PageIndex(0),
        layer: LayerIndex(0),
    };
    paged
        .apply(MarkChange::Add(first, stroke(single, brush(window()), Placement::ALIGNED)).into())
        .unwrap();
    let mut written = bytes(&paged);
    let pressure = 0.5f32.to_le_bytes();
    let at = written
        .windows(4)
        .rposition(|window| window == pressure)
        .unwrap();
    written[at..at + 4].copy_from_slice(&2.0f32.to_le_bytes());
    assert!(matches!(
        read_paged(&written),
        Err(FormatError::Pressure(_))
    ));
}

fn shaped(shape: Shape) -> Mark {
    Mark {
        placement: Placement::new(Affine::translation(3.0, 4.0)).unwrap(),
        paint: crate::paint(),
        ink: Ink::Shape(std::sync::Arc::new(ShapeInk::new(
            shape,
            Size::new(2.0).unwrap(),
        ))),
    }
}

/// A board holding one stroke, and the shapes given.
fn board_with(shapes: &[Shape]) -> Vec<u8> {
    let mut board = Board::new(Background::Plain(Rgb::hex(0xffffff)));
    let first = stroke(wave(20), brush(window()), Placement::ALIGNED);
    board
        .apply(MarkChange::Add(LayerIndex(0), first).into())
        .unwrap();
    for shape in shapes {
        board
            .apply(MarkChange::Add(LayerIndex(0), shaped(*shape)).into())
            .unwrap();
    }
    let mut written = Vec::new();
    write_board(&board, &mut written);
    written
}

#[test]
fn shapes_read_back_as_written() {
    let shapes = [
        Shape::Line {
            from: Point::new(0.0, 0.0),
            to: Point::new(30.0, 10.0),
        },
        Shape::Arrow {
            from: Point::new(0.0, 0.0),
            to: Point::new(-20.0, 40.0),
        },
        Shape::Rectangle(ink::geom::Rect::spanning(
            Point::new(5.0, 5.0),
            Point::new(50.0, 25.0),
        )),
        Shape::Ellipse {
            center: Point::new(10.0, 10.0),
            radii: ink::geom::Vector::new(20.0, 8.0),
        },
    ];
    let written = board_with(&shapes);
    let read = read_board(&written).unwrap();
    let mut again = Vec::new();
    write_board(&read, &mut again);
    assert_eq!(again, written);
    let found: Vec<Shape> = (read.marks(LayerIndex(0)).unwrap().iter())
        .filter_map(|(_, mark)| match &mark.ink {
            Ink::Shape(shape) => Some(shape.shape),
            Ink::Stroke(_) => None,
        })
        .collect();
    assert_eq!(found, shapes);
}

#[test]
fn version_one_reads_strokes_but_not_shapes() {
    let mut strokes = board_with(&[]);
    strokes[4..6].copy_from_slice(&1u16.to_le_bytes());
    assert_eq!(
        read_board(&strokes)
            .unwrap()
            .marks(LayerIndex(0))
            .unwrap()
            .len(),
        1
    );
    let mut shapes = board_with(&[Shape::Line {
        from: Point::new(0.0, 0.0),
        to: Point::new(1.0, 1.0),
    }]);
    shapes[4..6].copy_from_slice(&1u16.to_le_bytes());
    assert_eq!(
        read_board(&shapes).map(|_| ()),
        Err(FormatError::Tag {
            what: "mark",
            tag: 1
        })
    );
}

#[test]
fn a_coordinate_that_is_not_a_number_is_refused() {
    let mut written = board_with(&[Shape::Line {
        from: Point::new(7.5, 0.0),
        to: Point::new(1.0, 1.0),
    }]);
    let at = (written.windows(4))
        .rposition(|bytes| bytes == 7.5f32.to_le_bytes())
        .unwrap();
    written[at..at + 4].copy_from_slice(&f32::INFINITY.to_le_bytes());
    assert_eq!(
        read_board(&written).map(|_| ()),
        Err(FormatError::NotFinite(f32::INFINITY))
    );
}
