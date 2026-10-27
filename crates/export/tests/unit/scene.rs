use export::scene::{self, Region, wound};
use ink::doc::{
    Background, Board, Drawing, LayerIndex, PageIndex, PageLayer, PageSize, Paged, Pattern, Spacing,
};
use model::color::Rgb;
use model::sheet::GAP;
use smallvec::smallvec;

use crate::rectangle;

const WHITE: Background = Background::Plain(Rgb::hex(0xffffff));

#[test]
fn a_page_is_its_size_with_its_marks() {
    let mut paged = Paged::new(PageSize::A4, WHITE);
    let target = PageLayer {
        page: PageIndex(0),
        layer: LayerIndex(0),
    };
    paged
        .add(target, rectangle((10.0, 10.0), (50.0, 30.0)))
        .unwrap();
    let scene = scene::page(&paged, PageIndex(0));
    assert_eq!(scene.area.max().x, PageSize::A4.width.get());
    assert_eq!(scene.area.max().y, PageSize::A4.height.get());
    assert_eq!(scene.marks.len(), 1);
}

#[test]
fn a_board_region_is_its_marks_and_a_margin() {
    let mut board = Board::new(WHITE);
    assert!(
        scene::region(&board, &Region::Everything).is_none(),
        "nothing to show"
    );
    let first = board
        .add(LayerIndex(0), rectangle((0.0, 0.0), (100.0, 50.0)))
        .unwrap();
    board
        .add(LayerIndex(0), rectangle((300.0, 300.0), (320.0, 320.0)))
        .unwrap();
    let everything = scene::region(&board, &Region::Everything).unwrap();
    assert_eq!(everything.marks.len(), 2);
    assert!(everything.area.max().x >= 320.0 + GAP);
    let picked = Region::Picked {
        layer: LayerIndex(0),
        ids: smallvec![first],
    };
    let picked = scene::region(&board, &picked).unwrap();
    assert_eq!(picked.marks.len(), 1, "only the picked mark");
    assert!(picked.area.max().x < 300.0, "{:?}", picked.area);
}

#[test]
fn ruled_paper_follows_the_surface() {
    let lines = Background::Pattern {
        paper: Rgb::hex(0xffffff),
        ink: Rgb::hex(0x8899aa),
        pattern: Pattern::Lines,
        spacing: Spacing::new(20.0).unwrap(),
    };
    let mut board = Board::new(lines);
    board
        .add(LayerIndex(0), rectangle((5.0, 5.0), (95.0, 95.0)))
        .unwrap();
    let scene = scene::region(&board, &Region::Everything).unwrap();
    let paper = scene.paper();
    assert_eq!(paper.len(), 2, "the sheet and its rules");
    for rule in &paper[1].rects {
        let middle = (rule.min().y + rule.max().y) * 0.5;
        assert!(
            (middle / 20.0 - (middle / 20.0).round()).abs() < 1e-4,
            "{middle}"
        );
    }
}

#[test]
fn every_triangle_is_wound_the_same_way() {
    let mark = rectangle((0.0, 0.0), (40.0, 40.0));
    let mesh = &mark.built().mesh;
    assert!(wound(mesh).count() > 0);
    for [a, b, c] in wound(mesh) {
        assert!((b - a).cross(c - a) >= 0.0);
    }
}
