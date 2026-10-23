use ink::doc::{
    Background, Board, ChangeError, Drawing, Geometry, Layer, LayerChange, LayerIndex, LayerName,
    Mark, MarkChange, Marks, PageIndex, PageLayer, PageSize, Paged, PagedChange, Placement,
    Visibility,
};
use ink::geom::{Local, Point};
use model::color::Rgb;
use smallvec::smallvec;

use crate::{brush, mark, sample, wave, window};

fn stroke() -> Mark {
    mark(
        Geometry::new(wave::<Local>(30).into(), brush(window())).unwrap(),
        Placement::ALIGNED,
    )
}

fn plain() -> Background {
    Background::Plain(Rgb::hex(0xffffff))
}

const FIRST: PageLayer = PageLayer {
    page: PageIndex(0),
    layer: LayerIndex(0),
};

#[test]
fn marks_keep_drawing_order_through_removals() {
    let mut marks = Marks::default();
    let ids: Vec<_> = (0..4).map(|_| marks.insert(stroke())).collect();
    marks.remove(ids[1]).unwrap();
    let order: Vec<_> = marks.iter().map(|(id, _)| id).collect();
    assert_eq!(order, vec![ids[0], ids[2], ids[3]]);
    assert!(marks.remove(ids[1]).is_none());
}

#[test]
fn a_new_document_has_one_page_on_one_layer() {
    let paged = Paged::new(PageSize::A4, plain());
    assert_eq!(paged.pages().len(), 1);
    assert_eq!(paged.layers().len(), 1);
    assert!(paged.marks(FIRST).unwrap().is_empty());
}

#[test]
fn a_new_layer_reaches_every_page() {
    let mut paged = Paged::new(PageSize::A4, plain());
    let add_page = PagedChange::InsertPage {
        at: PageIndex(1),
        size: PageSize::A4,
        background: plain(),
    };
    paged.apply(add_page).unwrap();
    let layer = Layer {
        name: LayerName::new("Notes".to_owned()).unwrap(),
        visibility: Visibility::Shown,
    };
    paged
        .apply(PagedChange::Layers(LayerChange::Add(layer)))
        .unwrap();
    for page in [PageIndex(0), PageIndex(1)] {
        assert!(
            paged
                .marks(PageLayer {
                    page,
                    layer: LayerIndex(1)
                })
                .is_some()
        );
    }
}

#[test]
fn pages_come_and_go_with_their_marks() {
    let mut paged = Paged::new(PageSize::new(100.0, 200.0).unwrap(), plain());
    paged
        .apply(MarkChange::Add(FIRST, stroke()).into())
        .unwrap();
    let before = PagedChange::InsertPage {
        at: PageIndex(0),
        size: PageSize::new(50.0, 100.0).unwrap(),
        background: plain(),
    };
    paged.apply(before).unwrap();
    let second = PageLayer {
        page: PageIndex(1),
        layer: LayerIndex(0),
    };
    assert!(
        paged.marks(FIRST).unwrap().is_empty(),
        "the new page is blank"
    );
    assert_eq!(
        paged.marks(second).unwrap().len(),
        1,
        "the old page moved down"
    );
    assert_eq!(
        paged.stack().sizes()[0],
        PageSize::new(50.0, 100.0).unwrap()
    );
    paged.apply(PagedChange::RemovePage(PageIndex(1))).unwrap();
    assert!(paged.marks(second).is_none());
    assert_eq!(
        paged.apply(PagedChange::RemovePage(PageIndex(0))),
        Err(ChangeError::LastPage)
    );
    assert_eq!(
        paged.apply(PagedChange::RemovePage(PageIndex(4))),
        Err(ChangeError::NoPage(4))
    );
}

#[test]
fn changes_naming_missing_parts_are_refused_unchanged() {
    let mut paged = Paged::new(PageSize::A4, plain());
    let missing_page = PageLayer {
        page: PageIndex(3),
        layer: LayerIndex(0),
    };
    assert_eq!(
        paged.apply(MarkChange::Add(missing_page, stroke()).into()),
        Err(ChangeError::NoPage(3))
    );
    let missing_layer = PageLayer {
        page: PageIndex(0),
        layer: LayerIndex(2),
    };
    assert_eq!(
        paged.apply(MarkChange::Add(missing_layer, stroke()).into()),
        Err(ChangeError::NoLayer(2))
    );
    paged
        .apply(MarkChange::Add(FIRST, stroke()).into())
        .unwrap();
    paged
        .apply(MarkChange::Add(FIRST, stroke()).into())
        .unwrap();
    let (gone, kept) = {
        let mut ids = paged.marks(FIRST).unwrap().iter().map(|(id, _)| id);
        (ids.next().unwrap(), ids.next().unwrap())
    };
    paged
        .apply(MarkChange::Remove(FIRST, smallvec![gone]).into())
        .unwrap();
    let stale = MarkChange::Remove(FIRST, smallvec![kept, gone]);
    assert_eq!(paged.apply(stale.into()), Err(ChangeError::NoMark));
    assert_eq!(paged.marks(FIRST).unwrap().len(), 1, "the kept mark stays");
}

#[test]
fn hidden_layers_are_left_out_of_what_is_shown() {
    let mut board = Board::new(plain());
    let hidden = Layer {
        name: LayerName::new("Scratch".to_owned()).unwrap(),
        visibility: Visibility::Hidden,
    };
    board
        .apply(ink::doc::BoardChange::Layers(ink::doc::LayerChange::Add(
            hidden,
        )))
        .unwrap();
    board
        .apply(MarkChange::Add(LayerIndex(1), stroke()).into())
        .unwrap();
    let shown: Vec<_> = board.layer_marks().shown(board.layers()).collect();
    assert_eq!(shown.len(), 1);
    assert!(shown[0].is_empty());
}

#[test]
fn a_stroke_touches_what_lies_within_reach_of_its_mesh() {
    let samples = vec![
        sample::<Local>(0.0, 0.0, 0.0, 1.0),
        sample(40.0, 0.0, 0.1, 1.0),
    ];
    let mark = mark(
        Geometry::new(samples.into(), brush(window())).unwrap(),
        Placement::new(ink::geom::Affine::translation(100.0, 0.0)).unwrap(),
    );
    assert!(mark.touches(Point::new(120.0, 3.0), 2.0));
    assert!(
        !mark.touches(Point::new(20.0, 0.0), 2.0),
        "the stroke moved away"
    );
    assert!(!mark.touches(Point::new(120.0, 10.0), 2.0));
}
