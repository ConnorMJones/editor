use std::sync::Arc;

use krilla::Document as Writer;
use krilla::geom::PathBuilder;
use krilla::page::PageSettings;
use reader::pdf::{Bgra, Document, Grid, Level, PdfError, TILE, TileBlock};

/// A 200 by 100 point page with its left half black, then a blank 100 by 300 page.
fn sample() -> Arc<[u8]> {
    let mut writer = Writer::new();
    let mut page = writer.start_page_with(PageSettings::from_wh(200.0, 100.0).unwrap());
    let mut surface = page.surface();
    let mut path = PathBuilder::new();
    path.move_to(0.0, 0.0);
    path.line_to(100.0, 0.0);
    path.line_to(100.0, 100.0);
    path.line_to(0.0, 100.0);
    path.close();
    surface.draw_path(&path.finish().unwrap());
    surface.finish();
    page.finish();
    writer.start_page_with(PageSettings::from_wh(100.0, 300.0).unwrap());
    writer.finish().unwrap().into()
}

#[test]
fn a_pdf_opens_with_its_page_sizes() {
    let document = Document::open(sample()).unwrap();
    let sizes = document.pages.sizes();
    assert_eq!(sizes.len(), 2);
    assert_eq!(
        (sizes[0].width.get(), sizes[0].height.get()),
        (200.0, 100.0)
    );
    assert_eq!(
        (sizes[1].width.get(), sizes[1].height.get()),
        (100.0, 300.0)
    );
}

#[test]
fn a_page_renders_in_tiles_at_a_level_on_white() {
    let document = Document::open(sample()).unwrap();
    let level = Level(8);
    assert_eq!(level.pixels_per_point(), 4.0);
    let grid = Grid::of(document.pages.sizes()[0], level);
    assert_eq!(
        (grid.width, grid.height, grid.columns, grid.rows),
        (800, 400, 2, 1)
    );
    let block = |columns: std::ops::Range<u16>| TileBlock {
        page: 0,
        level,
        columns,
        rows: 0..1,
    };
    let tiles = document.render_tiles(block(0..2)).unwrap();
    let [(first, left), (second, right)] = &tiles[..] else {
        panic!("two tiles");
    };
    assert_eq!((first.column, second.column), (0, 1));
    let alone = document.render_tiles(block(1..2)).unwrap();
    assert_eq!(
        alone[0].1.pixels, right.pixels,
        "a tile renders the same alone or in a block"
    );
    assert_eq!((left.width, left.height), (512, 400));
    assert_eq!(
        (right.width, right.height),
        (288, 400),
        "the last column is cut to the page"
    );
    let pixel = |image: &Bgra, x: usize, y: usize| {
        image.pixels[(y * usize::from(image.width) + x) * 4..][..4].to_vec()
    };
    assert_eq!(
        pixel(left, 300, 200),
        [0, 0, 0, 255],
        "x 75 pt is in the black half"
    );
    assert_eq!(
        pixel(left, 500, 200),
        [255, 255, 255, 255],
        "x 125 pt is white paper"
    );
    assert_eq!(pixel(right, 10, 200), [255, 255, 255, 255]);
    assert!(matches!(
        document.render_tiles(block(2..3)),
        Err(PdfError::NoBlock(_))
    ));
    assert!(matches!(
        document.render_tiles(TileBlock {
            page: 2,
            ..block(0..1)
        }),
        Err(PdfError::NoPage(2))
    ));
}

#[test]
fn levels_cover_a_scale_and_the_whole_page_fits_one_tile() {
    assert_eq!(Level::covering(1.0), Level(0));
    assert_eq!(Level::covering(1.01), Level(1));
    assert_eq!(Level::covering(2.0), Level(4));
    let document = Document::open(sample()).unwrap();
    for size in document.pages.sizes() {
        let grid = Grid::of(*size, Level::whole_page(*size));
        assert_eq!((grid.columns, grid.rows), (1, 1));
        assert!(grid.width.max(grid.height) > u32::from(TILE) / 2);
    }
}

#[test]
fn bytes_that_are_not_a_pdf_are_refused() {
    let bytes: Arc<[u8]> = Arc::from(&b"not a pdf at all"[..]);
    assert!(matches!(Document::open(bytes), Err(PdfError::Load(_))));
}

/// "Hello world" on one line from (20, 50), then "again" on the line below, at 12 points.
fn with_text() -> Arc<[u8]> {
    let font = krilla::text::Font::new(model::fonts::BUNDLED[4].to_vec().into(), 0).unwrap();
    let mut writer = Writer::new();
    let mut page = writer.start_page_with(PageSettings::from_wh(300.0, 200.0).unwrap());
    let mut surface = page.surface();
    for (text, y) in [("Hello world", 50.0), ("again", 70.0)] {
        surface.draw_text(
            krilla::geom::Point::from_xy(20.0, y),
            font.clone(),
            12.0,
            text,
            false,
            krilla::text::TextDirection::Auto,
        );
    }
    surface.finish();
    page.finish();
    writer.finish().unwrap().into()
}

#[test]
fn a_page_gives_its_text_with_a_box_per_character() {
    let document = Document::open(with_text()).unwrap();
    let text = Arc::new(document).claim_text(0).unwrap().read().unwrap();
    assert_eq!(text.text, "Hello world\nagain");
    assert_eq!(
        text.chars.len(),
        "Hello worldagain".len(),
        "the space is drawn, the line break is not"
    );
    let [bottom_left, bottom_right, _, top_left] = text.chars[0].quad;
    assert!((bottom_left.x - 20.0).abs() < 0.5, "{bottom_left:?}");
    assert!(bottom_right.x > bottom_left.x, "the box runs to the right");
    assert!(
        top_left.y < 50.0 && bottom_left.y > 50.0,
        "around the baseline at 50"
    );
    let again = text.chars.last().unwrap();
    assert_eq!(&text.text[again.at..], "n");
}

#[test]
fn a_point_finds_the_nearest_character_on_its_line() {
    let document = Arc::new(Document::open(with_text()).unwrap());
    let text = document.claim_text(0).unwrap().read().unwrap();
    let at = |x: f32, y: f32| text.nearest(model::geom::Point::new(x, y)).unwrap();
    assert_eq!(&text.text[text.chars[at(21.0, 46.0)].at..][..1], "H");
    assert_eq!(
        &text.text[text.chars[at(500.0, 47.0)].at..][..1],
        "d",
        "past the line's end"
    );
    assert_eq!(
        &text.text[text.chars[at(21.0, 67.0)].at..][..1],
        "a",
        "the line below"
    );
    assert_eq!(text.slice(0..5), "Hello");
    assert!(document.claim_text(0).is_none(), "one claim per page");
}

#[test]
fn the_outline_names_its_pages_in_order_and_depth() {
    use krilla::destination::XyzDestination;
    use krilla::outline::{Outline, OutlineNode};
    let mut writer = Writer::new();
    for _ in 0..3 {
        writer.start_page_with(PageSettings::from_wh(100.0, 100.0).unwrap());
    }
    let to = |page| XyzDestination::new(page, krilla::geom::Point::from_xy(0.0, 0.0));
    let mut chapter = OutlineNode::new("Chapter".to_owned(), to(1));
    chapter.push_child(OutlineNode::new("Section".to_owned(), to(2)));
    let mut outline = Outline::new();
    outline.push_child(OutlineNode::new("Preface".to_owned(), to(0)));
    outline.push_child(chapter);
    writer.set_outline(outline);
    let document = Document::open(writer.finish().unwrap().into()).unwrap();
    let read: Vec<(String, usize, Option<usize>)> = (document.outline().into_iter())
        .map(|heading| (heading.title, heading.depth, heading.page))
        .collect();
    assert_eq!(
        read,
        [
            ("Preface".to_owned(), 0, Some(0)),
            ("Chapter".to_owned(), 0, Some(1)),
            ("Section".to_owned(), 1, Some(2)),
        ]
    );
}
