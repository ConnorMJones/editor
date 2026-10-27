use std::fs;
use std::path::Path;

use ink::doc::{Background, Drawing, PageIndex, PageLayer, PageSize, Paged, PagedChange};
use ink::text::Fonts;
use model::color::Rgb;
use model::mode::Mode;
use model::notice::Notice;
use model::sheet::PageIndex as Page;
use state::drawing::pen::Stylus;
use state::drawing::{InkTabs, PdfMode, PdfTab};
use state::input::{self, keymap::Context};
use state::project::session;
use state::view::tab::Tab;

use crate::drawing::{draw, line};
use crate::harness::Harness;

/// A three-page PDF written by our own export, the first page with a rectangle on it.
fn write_pdf(path: &Path) {
    let white = Background::Plain(Rgb::hex(0xffffff));
    let mut paged = Paged::new(PageSize::A4, white);
    for at in [1, 2] {
        paged
            .apply(PagedChange::InsertPage {
                at: PageIndex(at),
                size: PageSize::A4,
                background: white,
            })
            .unwrap();
    }
    let fonts = Fonts::new(&model::fonts::BUNDLED);
    let scenes = (0..3).map(|page| export::scene::page(&paged, PageIndex(page)));
    fs::write(path, export::pdf::pdf(scenes, &fonts).unwrap()).unwrap();
}

fn tab(h: &Harness) -> &PdfTab {
    match h.state.layout.focused().active() {
        Some(Tab::Ink(InkTabs::Pdf(tab))) => tab,
        other => panic!("a PDF is focused, not {:?}", other.map(Tab::opened)),
    }
}

fn marks(h: &Harness) -> usize {
    let paged = h.state.pdfs[tab(h).file()].edited.drawing();
    let first = PageLayer {
        page: PageIndex(0),
        layer: ink::doc::LayerIndex(0),
    };
    paged.marks(first).unwrap().len()
}

#[test]
fn a_pdf_opens_to_be_read_over_its_pages() {
    let dir = tempfile::tempdir().unwrap();
    write_pdf(&dir.path().join("paper.pdf"));
    let mut h = Harness::new(dir.path());
    h.keys(":o paper.pdf<ret>");
    assert_eq!(tab(&h).mode, PdfMode::Read);
    assert_eq!(input::context(&h.state), Context::PdfRead);
    let paged = h.state.pdfs[tab(&h).file()].edited.drawing();
    assert_eq!(paged.stack().sizes().len(), 3);
    h.keys("pnpn");
    assert_eq!(tab(&h).view.centre.page, Page(2));
    h.keys("gg");
    assert_eq!(tab(&h).view.centre.page, Page(0));
    draw(&mut h, Stylus::Tip, &line((100.0, 100.0), (300.0, 120.0)));
    assert_eq!(marks(&h), 0, "reading draws nothing");
}

#[test]
fn a_tool_key_marks_up_the_pdf_and_esc_reads_again() {
    let dir = tempfile::tempdir().unwrap();
    write_pdf(&dir.path().join("paper.pdf"));
    let mut h = Harness::new(dir.path());
    h.keys(":o paper.pdf<ret>b");
    assert_eq!(tab(&h).mode, PdfMode::Markup);
    assert_eq!(input::context(&h.state), Context::PdfMarkup);
    draw(&mut h, Stylus::Tip, &line((100.0, 100.0), (300.0, 120.0)));
    assert_eq!(marks(&h), 1);
    h.keys("po");
    let pages = h.state.pdfs[tab(&h).file()]
        .edited
        .drawing()
        .stack()
        .sizes()
        .len();
    assert_eq!(pages, 3, "a PDF's pages are its own");
    h.keys("<esc>");
    assert_eq!(tab(&h).mode, PdfMode::Read);
    assert_eq!(h.state.input.mode, Mode::Normal);
    h.keys(":w<ret>");
    assert!(
        matches!(h.state.notice, Some(Notice::ReadOnly)),
        "markup is not written yet"
    );
    h.keys(":export-pdf<ret>");
    assert!(matches!(h.state.notice, Some(Notice::NotExportable { .. })));
}

#[test]
fn a_session_keeps_the_page_and_whether_the_pdf_was_marked_up() {
    let dir = tempfile::tempdir().unwrap();
    write_pdf(&dir.path().join("paper.pdf"));
    let mut before = Harness::new(dir.path());
    before.keys(":o paper.pdf<ret>pnb");
    let text = toml::to_string(&session::snapshot(&before.state)).unwrap();
    let mut after = Harness::new(dir.path());
    let saved: session::Session = toml::from_str(&text).unwrap();
    after
        .run(|state, out| session::restore(state, saved, out))
        .unwrap();
    assert_eq!(tab(&after).view.centre.page, Page(1));
    assert_eq!(tab(&after).mode, PdfMode::Markup);
}

#[test]
fn bytes_that_are_not_a_pdf_are_refused() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("paper.pdf"), "not a pdf").unwrap();
    let mut h = Harness::new(dir.path());
    h.keys(":o paper.pdf<ret>");
    assert!(matches!(h.state.notice, Some(Notice::Unreadable { .. })));
}
