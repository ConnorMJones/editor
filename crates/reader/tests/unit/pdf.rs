use std::sync::Arc;

use krilla::Document as Writer;
use krilla::geom::PathBuilder;
use krilla::page::PageSettings;
use reader::pdf::{Document, PdfError};

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
fn a_page_renders_at_the_size_asked_on_white() {
    let document = Document::open(sample()).unwrap();
    let page = document.render(0, 400, 200).unwrap();
    assert_eq!((page.width, page.height), (400, 200));
    let pixel = |x: usize, y: usize| &page.pixels[(y * 400 + x) * 4..][..4];
    assert_eq!(pixel(50, 100), [0, 0, 0, 255], "the black half");
    assert_eq!(pixel(350, 100), [255, 255, 255, 255], "white paper");
    assert!(matches!(
        document.render(2, 10, 10),
        Err(PdfError::NoPage(2))
    ));
}

#[test]
fn bytes_that_are_not_a_pdf_are_refused() {
    let bytes: Arc<[u8]> = Arc::from(&b"not a pdf at all"[..]);
    assert!(matches!(Document::open(bytes), Err(PdfError::Load(_))));
}
