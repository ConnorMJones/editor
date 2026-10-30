use std::sync::Arc;

use export::markup::{flatten, save, unmarked};
use ink::doc::{
    Background, Drawing, LayerIndex, PageIndex, PageLayer, PageSize, Paged, PagedChange,
};
use model::color::Rgb;
use reader::pdf::Document;
use reader::pdf::revise::{Owner, Update};

use crate::{fonts, rectangle, text};

fn first_layer(page: usize) -> PageLayer {
    PageLayer {
        page: PageIndex(page),
        layer: LayerIndex(0),
    }
}

fn base() -> Arc<Document> {
    let fonts = fonts();
    let white = Background::Plain(Rgb::hex(0xffffff));
    let mut paged = Paged::new(PageSize::A4, white);
    paged
        .apply(PagedChange::InsertPage {
            at: PageIndex(1),
            size: PageSize::A4,
            background: white,
        })
        .unwrap();
    paged
        .add(first_layer(0), text(&fonts, "Base page", (100.0, 100.0)))
        .unwrap();
    let scenes = (0..2).map(|page| export::scene::page(&paged, PageIndex(page)));
    let bytes = export::pdf::pdf(scenes, &fonts).unwrap();
    Arc::new(Document::open(bytes.into()).unwrap())
}

fn applied(document: &Document, update: Update) -> Arc<Document> {
    let mut bytes = document.bytes.to_vec();
    bytes.extend(update.0);
    Arc::new(Document::open(bytes.into()).unwrap())
}

fn ours_on(document: &Document, page: usize) -> usize {
    let revision = document.revise().unwrap();
    let (_, dict) = revision.page(page).unwrap();
    (revision.annotations(&dict).into_iter())
        .filter(|(_, owner)| *owner == Owner::Ours)
        .count()
}

fn marked(document: &Arc<Document>) -> Paged {
    let fonts = fonts();
    let mut paged = Paged::over_pdf(document.clone());
    paged
        .add(first_layer(0), rectangle((50.0, 50.0), (150.0, 90.0)))
        .unwrap();
    paged
        .add(first_layer(1), text(&fonts, "Signed here", (80.0, 300.0)))
        .unwrap();
    paged
}

#[test]
fn markup_saved_into_a_pdf_reads_back_and_shows_as_annotations() {
    let fonts = fonts();
    let document = base();
    let paged = marked(&document);
    let saved = applied(&document, save(&document, &paged, &fonts).unwrap());
    assert_eq!(saved.pages.sizes().len(), 2, "the PDF's own pages are kept");
    assert_eq!((ours_on(&saved, 0), ours_on(&saved, 1)), (1, 1));
    let blob = saved.blob().expect("the markup is in the catalog");
    let read = ink::format::read_markup(&blob, saved.clone(), &fonts).unwrap();
    let count = |paged: &Paged, page: usize| {
        paged
            .page(PageIndex(page))
            .marks()
            .iter()
            .map(|marks| marks.len())
            .sum::<usize>()
    };
    assert_eq!((count(&read, 0), count(&read, 1)), (1, 1));

    let mut fewer = read;
    let first = fewer
        .page(PageIndex(1))
        .marks()
        .iter()
        .next()
        .unwrap()
        .iter()
        .next()
        .unwrap()
        .0;
    fewer
        .apply(PagedChange::Marks(ink::doc::MarkChange::Remove(
            first_layer(1),
            [first].into_iter().collect(),
        )))
        .unwrap();
    let again = applied(&saved, save(&saved, &fewer, &fonts).unwrap());
    assert_eq!(
        (ours_on(&again, 0), ours_on(&again, 1)),
        (1, 0),
        "a second save replaces ours"
    );
    let read = ink::format::read_markup(&again.blob().unwrap(), again.clone(), &fonts).unwrap();
    assert_eq!((count(&read, 0), count(&read, 1)), (1, 0));
}

#[test]
fn the_unmarked_view_leaves_our_annotations_out() {
    let fonts = fonts();
    let document = base();
    assert!(
        unmarked(&document).unwrap().is_none(),
        "nothing of ours to leave out"
    );
    let saved = applied(
        &document,
        save(&document, &marked(&document), &fonts).unwrap(),
    );
    let view = applied(&saved, unmarked(&saved).unwrap().unwrap());
    assert_eq!((ours_on(&view, 0), ours_on(&view, 1)), (0, 0));
    assert_eq!(
        view.read_text(0).unwrap().text,
        saved.read_text(0).unwrap().text
    );
}

#[test]
fn flattening_draws_the_markup_into_the_pages() {
    let fonts = fonts();
    let document = base();
    let paged = marked(&document);
    let saved = applied(&document, save(&document, &paged, &fonts).unwrap());
    let flat = applied(&saved, flatten(&saved, &paged, &fonts).unwrap());
    assert!(flat.blob().is_none(), "no markup is left to edit");
    assert_eq!((ours_on(&flat, 0), ours_on(&flat, 1)), (0, 0));
    assert!(
        flat.read_text(1).unwrap().text.contains("Signed here"),
        "{:?}",
        flat.read_text(1).unwrap().text
    );
    assert!(flat.read_text(0).unwrap().text.contains("Base page"));
    let page = flat.render_tiles(reader::pdf::TileBlock {
        page: 0,
        level: reader::pdf::Level(0),
        columns: 0..1,
        rows: 0..1,
    });
    assert!(page.is_ok());
}
