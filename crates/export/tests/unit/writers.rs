use export::scene::{self, Region};
use export::{pdf, svg};
use ink::doc::{Background, Board, Drawing, LayerIndex, PageIndex, PageLayer, PageSize, Paged};
use model::color::Rgb;

use crate::{fonts, picture, rectangle, text};

const WHITE: Background = Background::Plain(Rgb::hex(0xffffff));

fn on(page: usize) -> PageLayer {
    PageLayer {
        page: PageIndex(page),
        layer: LayerIndex(0),
    }
}

#[test]
fn a_pdf_has_a_page_for_each_page_and_its_text() {
    let fonts = fonts();
    let mut paged = Paged::new(PageSize::A4, WHITE);
    paged
        .apply(ink::doc::PagedChange::InsertPage {
            at: PageIndex(1),
            size: PageSize::A4,
            background: WHITE,
        })
        .unwrap();
    paged
        .add(on(0), rectangle((10.0, 10.0), (60.0, 40.0)))
        .unwrap();
    paged
        .add(on(0), text(&fonts, "Hello pages", (20.0, 80.0)))
        .unwrap();
    paged.add(on(1), picture((30.0, 30.0))).unwrap();
    let scenes = (0..2).map(|page| scene::page(&paged, PageIndex(page)));
    let bytes = pdf::pdf(scenes, &fonts).unwrap();
    assert!(bytes.starts_with(b"%PDF-"));
    let text = String::from_utf8_lossy(&bytes);
    assert!(text.contains("/Count 2"), "two pages");
    assert!(text.contains("/FontFile"), "the font is embedded");
}

#[test]
fn an_svg_draws_glyphs_as_outlines_and_reads_back_at_its_size() {
    let fonts = fonts();
    let mut board = Board::new(WHITE);
    board
        .add(LayerIndex(0), rectangle((0.0, 0.0), (100.0, 50.0)))
        .unwrap();
    board
        .add(LayerIndex(0), text(&fonts, "noon", (10.0, 10.0)))
        .unwrap();
    board.add(LayerIndex(0), picture((50.0, 60.0))).unwrap();
    let scene = scene::region(&board, &Region::Everything).unwrap();
    let written = svg::svg(&scene, &fonts);
    assert_eq!(written.matches("<use ").count(), 4, "one use per glyph");
    assert_eq!(
        written.matches(r#"<path id="#).count(),
        2,
        "one outline per distinct glyph"
    );
    let tree = resvg::usvg::Tree::from_str(&written, &resvg::usvg::Options::default()).unwrap();
    let (min, max) = (scene.area.min(), scene.area.max());
    // Points read as pixels at 96 to the inch.
    assert!((tree.size().width() - (max.x - min.x) * 4.0 / 3.0).abs() < 0.01);
}

#[test]
fn a_png_is_the_region_at_its_scale() {
    let fonts = fonts();
    let mut board = Board::new(WHITE);
    board
        .add(LayerIndex(0), rectangle((0.0, 0.0), (100.0, 50.0)))
        .unwrap();
    let scene = scene::region(&board, &Region::Everything).unwrap();
    let png = svg::png(&scene, &fonts, 2.0).unwrap();
    let pixmap = resvg::tiny_skia::Pixmap::decode_png(&png).unwrap();
    let (min, max) = (scene.area.min(), scene.area.max());
    assert_eq!(pixmap.width(), ((max.x - min.x) * 2.0).ceil() as u32);
    // The rectangle's top left corner is inked; the paper beside it is white.
    let at = |x: f32, y: f32| {
        let (x, y) = (((x - min.x) * 2.0) as u32, ((y - min.y) * 2.0) as u32);
        pixmap.pixel(x, y).unwrap()
    };
    assert!(at(0.0, 25.0).red() < 100, "the rectangle's side");
    assert_eq!(at(50.0, 25.0).red(), 255, "paper inside the outline");
}
