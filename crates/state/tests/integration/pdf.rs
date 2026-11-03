use std::fs;
use std::path::Path;

use ink::brush::{Paint, Size};
use ink::doc::{Background, Drawing, PageIndex, PageLayer, PageSize, Paged, PagedChange};
use ink::doc::{Ink, Mark, Placement};
use ink::text::{FontFamily, Fonts, Inked, TextStyle, Wrap};
use library::Place;
use library::{Highlight, Notebook};
use model::color::Rgb;
use model::geom::Point;
use model::mode::Mode;
use model::notice::Notice;
use model::sheet::PageIndex as Page;
use model::theme::Color;
use state::Overlay;
use state::drawing::pen::Stylus;
use state::drawing::{InkTabs, PdfMode, PdfTab};
use state::input::picker::Items;
use state::input::{self, keymap::Context};
use state::project::session;
use state::reading::{Found, Notes, Press, ReaderKind, Reading, Taps, Unlisted};
use state::view::tab::Tab;

use crate::drawing::{draw, line};
use crate::harness::Harness;

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
    let style = TextStyle {
        family: FontFamily::new(model::fonts::SANS).unwrap(),
        size: Size::new(12.0).unwrap(),
    };
    let Inked::Ink(text) = fonts.lay_out("Hello pages", &style, Wrap::Lines).inked else {
        panic!("text leaves ink");
    };
    let mark = Mark {
        placement: Placement::at(Point::new(100.0, 200.0)),
        paint: Paint {
            color: Color::opaque(Rgb::hex(0)),
        },
        ink: Ink::Text(text),
    };
    let first = PageLayer {
        page: PageIndex(0),
        layer: ink::doc::LayerIndex(0),
    };
    paged.add(first, mark).unwrap();
    let Inked::Ink(text) = fonts.lay_out("Second leaf", &style, Wrap::Lines).inked else {
        panic!("text leaves ink");
    };
    let mark = Mark {
        placement: Placement::at(Point::new(100.0, 200.0)),
        paint: Paint {
            color: Color::opaque(Rgb::hex(0)),
        },
        ink: Ink::Text(text),
    };
    let second = PageLayer {
        page: PageIndex(1),
        layer: ink::doc::LayerIndex(0),
    };
    paged.add(second, mark).unwrap();
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
    let paged = h.state.pdfs[tab(h).file()].ink.edited.drawing();
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
    assert_eq!(tab(&h).mode, PdfMode::Read(Reading::Idle));
    assert_eq!(input::context(&h.state), Context::PdfRead);
    let paged = h.state.pdfs[tab(&h).file()].ink.edited.drawing();
    assert_eq!(paged.stack().sizes().len(), 3);
    h.keys("pnpn");
    assert_eq!(tab(&h).view.centre.page, Page(2));
    h.keys("gg");
    assert_eq!(tab(&h).view.centre.page, Page(0));
    draw(&mut h, Stylus::Tip, &line((100.0, 100.0), (300.0, 120.0)));
    assert_eq!(marks(&h), 0, "reading draws nothing");
}

#[test]
fn tool_keys_wait_for_markup_and_esc_reads_again() {
    let dir = tempfile::tempdir().unwrap();
    write_pdf(&dir.path().join("paper.pdf"));
    let mut h = Harness::new(dir.path());
    h.keys(":o paper.pdf<ret>b");
    assert_eq!(
        tab(&h).mode,
        PdfMode::Read(Reading::Idle),
        "tool keys pick tools only in markup"
    );
    h.keys("<space>db");
    assert_eq!(tab(&h).mode, PdfMode::Markup);
    assert_eq!(input::context(&h.state), Context::PdfMarkup);
    draw(&mut h, Stylus::Tip, &line((100.0, 100.0), (300.0, 120.0)));
    assert_eq!(marks(&h), 1);
    h.keys("po");
    let pages = h.state.pdfs[tab(&h).file()]
        .ink
        .edited
        .drawing()
        .stack()
        .sizes()
        .len();
    assert_eq!(pages, 3, "a PDF's pages are its own");
    h.keys("<esc>");
    assert_eq!(tab(&h).mode, PdfMode::Read(Reading::Idle));
    assert_eq!(h.state.input.mode, Mode::Normal);
    h.keys(":export-pdf<ret>");
    assert!(
        dir.path().join("paper-marked.pdf").is_file(),
        "{:?}",
        h.state.notice
    );
}

#[test]
fn a_session_keeps_the_page_and_whether_the_pdf_was_marked_up() {
    let dir = tempfile::tempdir().unwrap();
    write_pdf(&dir.path().join("paper.pdf"));
    let mut before = Harness::new(dir.path());
    before.keys(":o paper.pdf<ret>pn<space>d");
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

#[test]
fn dragging_across_text_selects_it_and_y_copies_it() {
    let dir = tempfile::tempdir().unwrap();
    write_pdf(&dir.path().join("paper.pdf"));
    let mut h = Harness::new(dir.path());
    h.keys(":o paper.pdf<ret>");
    draw(&mut h, Stylus::Tip, &line((101.0, 208.0), (128.0, 208.0)));
    match &tab(&h).mode {
        PdfMode::Read(Reading::Selected(span)) => assert_eq!(span.start().part, library::Part(0)),
        other => panic!("{other:?}"),
    }
    h.keys("y");
    assert_eq!(h.clipboard[0].as_deref(), Some("Hello"));
    h.keys("<esc>");
    assert_eq!(tab(&h).mode, PdfMode::Read(Reading::Idle));
    draw(&mut h, Stylus::Tip, &line((101.0, 208.0), (101.2, 208.0)));
    assert_eq!(
        tab(&h).mode,
        PdfMode::Read(Reading::Idle),
        "a tap selects nothing"
    );
}

#[test]
fn slash_finds_text_and_n_steps_through_the_hits() {
    let dir = tempfile::tempdir().unwrap();
    write_pdf(&dir.path().join("paper.pdf"));
    let mut h = Harness::new(dir.path());
    h.keys(":o paper.pdf<ret>pn/PAGES<ret>");
    match &tab(&h).found {
        Found::Hits { hits, at, query } => {
            assert_eq!((query.as_str(), hits.len(), *at), ("PAGES", 1, 0));
            assert_eq!(
                hits[0].start().part,
                library::Part(0),
                "found on another page, wrapping"
            );
        }
        other => panic!("{other:?}"),
    }
    assert_eq!(tab(&h).view.centre.page, Page(0), "the hit is shown");
    h.keys("n");
    assert!(matches!(tab(&h).found, Found::Hits { at: 0, .. }));
    h.keys("/zebra<ret>");
    assert!(matches!(h.state.notice, Some(Notice::NoHits { .. })));
    assert_eq!(tab(&h).found, Found::Nothing);
    h.keys("n");
    assert!(matches!(h.state.notice, Some(Notice::NoSearch)));
}

#[test]
fn a_pdf_without_a_table_of_contents_says_so() {
    let dir = tempfile::tempdir().unwrap();
    write_pdf(&dir.path().join("paper.pdf"));
    let mut h = Harness::new(dir.path());
    h.keys(":o paper.pdf<ret><space>t");
    assert!(matches!(h.state.notice, Some(Notice::NoOutline)));
}

fn notebook(h: &Harness) -> Notebook {
    match &h.state.pdfs[tab(h).file()].notes {
        Notes::Listed(listed) => listed.notebook.clone(),
        Notes::Unlisted(unlisted) => panic!("{unlisted:?}"),
    }
}

fn select_hello(h: &mut Harness) {
    draw(h, Stylus::Tip, &line((101.0, 208.0), (128.0, 208.0)));
}

fn tap_hello(h: &mut Harness) {
    draw(h, Stylus::Tip, &line((105.0, 208.0), (105.2, 208.0)));
}

fn picked(h: &Harness) -> Highlight {
    match &tab(h).mode {
        PdfMode::Read(Reading::OnHighlight(highlight)) => highlight.clone(),
        other => panic!("a highlight is picked, not {other:?}"),
    }
}

#[test]
fn a_highlight_takes_a_comment_and_is_kept_in_the_library() {
    let dir = tempfile::tempdir().unwrap();
    write_pdf(&dir.path().join("paper.pdf"));
    let mut h = Harness::new(dir.path());
    h.keys(":o paper.pdf<ret>");
    select_hello(&mut h);
    h.keys("v");
    let [highlight] = &notebook(&h).highlights[..] else {
        panic!("{:?}", notebook(&h));
    };
    assert_eq!(highlight.quote.exact(), "Hello");
    assert_eq!(highlight.comment, None);
    assert_eq!(&picked(&h), highlight, "the selection became the highlight");
    assert!(dir.path().join(".edt/library.db").is_file());
    assert_eq!(
        fs::read_to_string(dir.path().join(".edt/.gitignore")).unwrap(),
        "*\n"
    );

    h.keys("<esc>");
    tap_hello(&mut h);
    assert_eq!(picked(&h).id, highlight.id);
    h.keys("ifirst word<ret>");
    let commented = picked(&h)
        .comment
        .map(|comment| comment.as_str().to_owned());
    assert_eq!(commented.as_deref(), Some("first word"));
    assert_eq!(notebook(&h).highlights, [picked(&h)]);

    let mut again = Harness::new(dir.path());
    again.keys(":o paper.pdf<ret>");
    assert_eq!(notebook(&again), notebook(&h), "read back from the library");

    h.keys("d");
    assert!(notebook(&h).highlights.is_empty());
    assert_eq!(tab(&h).mode, PdfMode::Read(Reading::Idle));
    h.keys("d");
    assert!(matches!(h.state.notice, Some(Notice::NoHighlightPicked)));
}

#[test]
fn a_comment_on_a_selection_makes_a_highlight() {
    let dir = tempfile::tempdir().unwrap();
    write_pdf(&dir.path().join("paper.pdf"));
    let mut h = Harness::new(dir.path());
    h.keys(":o paper.pdf<ret>");
    select_hello(&mut h);
    h.keys("isaid hello<ret>");
    let [highlight] = &notebook(&h).highlights[..] else {
        panic!("{:?}", notebook(&h));
    };
    assert_eq!(
        highlight.comment.as_ref().map(|comment| comment.as_str()),
        Some("said hello")
    );
    assert_eq!(&picked(&h), highlight);
}

#[test]
fn m_bookmarks_the_page_and_the_notebook_goes_back_to_it() {
    let dir = tempfile::tempdir().unwrap();
    write_pdf(&dir.path().join("paper.pdf"));
    let mut h = Harness::new(dir.path());
    h.keys(":o paper.pdf<ret><space>a");
    assert!(matches!(h.state.notice, Some(Notice::NoAnnotations)));
    h.keys("pnmgg");
    let pages: Vec<_> = (notebook(&h).bookmarks.iter())
        .map(|bookmark| bookmark.at.part)
        .collect();
    assert_eq!(pages, [library::Part(1)]);
    h.keys("<space>a<ret>");
    assert_eq!(tab(&h).view.centre.page, Page(1));
    h.keys("m");
    assert!(
        notebook(&h).bookmarks.is_empty(),
        "m again takes the bookmark off"
    );
}

#[test]
fn the_page_read_last_is_where_the_pdf_opens_next() {
    let dir = tempfile::tempdir().unwrap();
    write_pdf(&dir.path().join("paper.pdf"));
    let mut h = Harness::new(dir.path());
    h.keys(":o paper.pdf<ret>pnpn");
    let mut again = Harness::new(dir.path());
    again.keys(":o paper.pdf<ret>");
    assert_eq!(tab(&again).view.centre.page, Page(2));
}

#[test]
fn space_shift_a_searches_every_annotation_and_opens_the_one_picked() {
    let dir = tempfile::tempdir().unwrap();
    write_pdf(&dir.path().join("paper.pdf"));
    let mut h = Harness::new(dir.path());
    h.keys(":o paper.pdf<ret>");
    select_hello(&mut h);
    h.keys("izebra crossing<ret><esc>pn<space>Azeb");
    match &h.state.overlay {
        Overlay::Picker(picker) => match &picker.items {
            Items::Annotations(found) => assert_eq!(found.hits.len(), 1),
            _ => panic!("the annotation search"),
        },
        _ => panic!("a picker"),
    }
    h.keys("<ret>");
    assert_eq!(tab(&h).view.centre.page, Page(0));
    assert_eq!(picked(&h).quote.exact(), "Hello");
}

#[test]
fn a_pdf_outside_the_library_is_read_without_annotations() {
    let dir = tempfile::tempdir().unwrap();
    let elsewhere = tempfile::tempdir().unwrap();
    let path = elsewhere.path().join("paper.pdf");
    write_pdf(&path);
    let mut h = Harness::new(dir.path());
    h.keys(&format!(":o {}<ret>", path.display()));
    assert!(matches!(
        h.state.pdfs[tab(&h).file()].notes,
        Notes::Unlisted(Unlisted::Outside)
    ));
    h.keys("m");
    assert!(matches!(h.state.notice, Some(Notice::NotInLibrary)));
    assert!(!dir.path().join(".edt").exists());
}

#[test]
fn space_tab_and_space_d_switch_between_reading_and_markup() {
    let dir = tempfile::tempdir().unwrap();
    write_pdf(&dir.path().join("paper.pdf"));
    let mut h = Harness::new(dir.path());
    h.keys(":o paper.pdf<ret><space><tab>");
    assert_eq!(tab(&h).mode, PdfMode::Markup);
    h.keys("e");
    assert_eq!(input::context(&h.state), Context::PdfMarkup);
    h.keys("<space><tab>");
    assert_eq!(tab(&h).mode, PdfMode::Read(Reading::Idle));
    h.keys("<space>d");
    assert_eq!(tab(&h).mode, PdfMode::Markup);
    h.keys("<space>d");
    assert_eq!(tab(&h).mode, PdfMode::Read(Reading::Idle));
}

fn on_disk_blob(h: &Harness) -> Option<Vec<u8>> {
    h.state.pdfs[tab(h).file()].on_disk().blob()
}

#[test]
fn markup_written_into_the_pdf_comes_back_when_it_opens_again() {
    let dir = tempfile::tempdir().unwrap();
    write_pdf(&dir.path().join("paper.pdf"));
    let mut h = Harness::new(dir.path());
    h.keys(":o paper.pdf<ret><space>d");
    draw(&mut h, Stylus::Tip, &line((100.0, 100.0), (300.0, 120.0)));
    assert_eq!(marks(&h), 1);
    h.keys(":w<ret>");
    assert!(
        matches!(h.state.notice, Some(Notice::Written { .. })),
        "{:?}",
        h.state.notice
    );
    assert!(
        !h.state
            .is_modified(state::view::tab::Opened::Pdf(tab(&h).file()))
    );
    assert!(on_disk_blob(&h).is_some());
    let paper = state::project::path::RelPath::parse("paper.pdf").unwrap();
    assert!(
        h.state.tree.marked_up.contains(&paper),
        "unflattened markup shows in the tree"
    );

    let mut again = Harness::new(dir.path());
    assert!(
        again.state.tree.marked_up.contains(&paper),
        "the library remembers it"
    );
    again.keys(":o paper.pdf<ret>");
    assert_eq!(marks(&again), 1, "the stroke reads back from the PDF");
    assert_eq!(
        again.state.pdfs[tab(&again).file()]
            .document()
            .read_text(0)
            .unwrap()
            .text,
        h.state.pdfs[tab(&h).file()]
            .document()
            .read_text(0)
            .unwrap()
            .text,
    );
}

#[test]
fn flatten_draws_the_markup_into_the_pages_and_leaves_nothing_to_edit() {
    let dir = tempfile::tempdir().unwrap();
    write_pdf(&dir.path().join("paper.pdf"));
    let mut h = Harness::new(dir.path());
    h.keys(":o paper.pdf<ret><space>d");
    draw(&mut h, Stylus::Tip, &line((100.0, 100.0), (300.0, 120.0)));
    h.keys(":w<ret>:flatten<ret>");
    assert!(
        matches!(h.state.notice, Some(Notice::Flattened { .. })),
        "{:?}",
        h.state.notice
    );
    assert_eq!(marks(&h), 0);
    assert!(on_disk_blob(&h).is_none());
    let paper = state::project::path::RelPath::parse("paper.pdf").unwrap();
    assert!(!h.state.tree.marked_up.contains(&paper));
    let mut again = Harness::new(dir.path());
    again.keys(":o paper.pdf<ret>");
    assert_eq!(marks(&again), 0);
}

#[test]
fn a_selection_crosses_pages_and_taps_pick_a_word_or_a_line() {
    let dir = tempfile::tempdir().unwrap();
    write_pdf(&dir.path().join("paper.pdf"));
    let mut h = Harness::new(dir.path());
    h.keys(":o paper.pdf<ret>");
    let file = &h.state.pdfs[tab(&h).file()];
    let (document, notes) = (file.document().clone(), &file.notes);
    let at = |page: usize, char: usize| {
        Some(Place {
            part: library::Part(page),
            char,
        })
    };
    let drag = |kind: ReaderKind| {
        Reading::Idle
            .step(Press::Down(Taps::One), at(0, 6), kind, &document, notes)
            .step(Press::Move, at(1, 5), kind, &document, notes)
            .step(Press::Up, at(1, 5), kind, &document, notes)
    };
    let text = |reading: Reading| match reading {
        Reading::Selected(span) => span.text(&document).unwrap(),
        other => panic!("{other:?}"),
    };
    assert_eq!(text(drag(ReaderKind::Pdf)), "pages\nSecond");
    assert_eq!(
        drag(ReaderKind::Book),
        Reading::Idle,
        "books stay in a chapter"
    );
    let tap = |taps: Taps| {
        Reading::Idle.step(
            Press::Down(taps),
            at(0, 1),
            ReaderKind::Pdf,
            &document,
            notes,
        )
    };
    assert_eq!(text(tap(Taps::Two)), "Hello");
    assert_eq!(text(tap(Taps::Three)), "Hello pages");

    let Reading::Selected(across) = drag(ReaderKind::Pdf) else {
        panic!("a selection");
    };
    match h.state.layout.focused_mut().active_mut() {
        Some(Tab::Ink(InkTabs::Pdf(tab))) => tab.mode = PdfMode::Read(Reading::Selected(across)),
        _ => panic!("a PDF"),
    }
    h.keys("v");
    let kept = &notebook(&h).highlights[0];
    assert_eq!(kept.quote.exact(), "pages\nSecond");
    let mut again = Harness::new(dir.path());
    again.keys(":o paper.pdf<ret>");
    assert_eq!(notebook(&again).highlights[0].span, kept.span);
}

#[test]
fn export_notes_writes_the_annotations_beside_the_pdf_and_keeps_what_was_written_around_them() {
    let dir = tempfile::tempdir().unwrap();
    write_pdf(&dir.path().join("paper.pdf"));
    let note = dir.path().join("paper.md");
    let mut h = Harness::new(dir.path());
    h.keys(":o paper.pdf<ret>:export-notes<ret>");
    assert!(matches!(h.state.notice, Some(Notice::NothingToExport)));
    select_hello(&mut h);
    h.keys("isaid hello<ret><esc>pnm:export-notes<ret>");
    let written = fs::read_to_string(&note).unwrap();
    let notebook = notebook(&h);
    let (said, page) = (&notebook.highlights[0], &notebook.bookmarks[0]);
    let entry = format!(
        "[qt] **pg 1**\n[[paper.pdf#page=1&note={}|pg 1]]\n<!-- quote -->\nHello\n<!-- /quote -->\n<!-- comment -->\nsaid hello\n<!-- /comment -->\n",
        said.id.0
    );
    assert!(written.contains(&entry), "{written}");
    let entry = format!(
        "[bm] **pg 2**\n[[paper.pdf#page=2&note={}|pg 2]]\n",
        page.id.0
    );
    assert!(written.contains(&entry), "{written}");

    fs::write(&note, format!("my own\n\n{written}after\n")).unwrap();
    h.keys("pbm:export-notes<ret>");
    let again = fs::read_to_string(&note).unwrap();
    assert!(again.starts_with("my own\n\n"), "{again}");
    assert!(again.ends_with("after\n"), "{again}");
    assert!(again.contains("[bm] **pg 1**"), "{again}");

    fs::write(&note, "no markers\n").unwrap();
    h.keys(":export-notes<ret>");
    assert!(matches!(h.state.notice, Some(Notice::ExportFailed { .. })));
    assert_eq!(fs::read_to_string(&note).unwrap(), "no markers\n");
}

#[test]
fn export_notes_edits_an_open_note_and_its_page_links_open_the_pdf_there() {
    let dir = tempfile::tempdir().unwrap();
    write_pdf(&dir.path().join("paper.pdf"));
    let note = dir.path().join("paper.md");
    fs::write(
        &note,
        format!(
            "top\n\n{}\n{}\n",
            state::literature::START,
            state::literature::END
        ),
    )
    .unwrap();
    let mut h = Harness::new(dir.path());
    h.keys(":o paper.md<ret>ggityped <esc>:o paper.pdf<ret>pnm:export-notes<ret>");
    h.keys(":o paper.md<ret>");
    let text = h.doc().text.to_string();
    assert!(text.starts_with("typed top\n"), "{text}");
    assert!(text.contains("[[paper.pdf#page=2&note="), "{text}");
    assert!(h.doc().is_modified(), "left for the writer to save");

    h.keys("gg/page=2<ret>gf");
    assert_eq!(tab(&h).view.centre.page, Page(1));
}

#[test]
fn a_note_link_picks_its_highlight_and_falls_back_to_the_page_once_it_is_gone() {
    let dir = tempfile::tempdir().unwrap();
    write_pdf(&dir.path().join("paper.pdf"));
    let mut h = Harness::new(dir.path());
    h.keys(":o paper.pdf<ret>");
    select_hello(&mut h);
    h.keys("v");
    let said = picked(&h);
    let link = format!("[[paper.pdf#page=2&note={}|here]]\n", said.id.0);
    fs::write(dir.path().join("links.md"), &link).unwrap();
    h.keys("<esc>pn:o links.md<ret>gf");
    assert_eq!(
        tab(&h).view.centre.page,
        Page(0),
        "at the highlight, not the page"
    );
    assert_eq!(picked(&h), said);

    h.keys("d:o links.md<ret>gf");
    assert_eq!(
        tab(&h).view.centre.page,
        Page(1),
        "the page once the highlight is gone"
    );
}

fn writing(h: &Harness) -> &state::comment::Writing {
    match &tab(h).mode {
        PdfMode::Read(Reading::Writing(writing)) => writing,
        other => panic!("a comment is being written, not {other:?}"),
    }
}

#[test]
fn shift_i_writes_a_comment_beside_the_pdf_and_esc_saves_it() {
    let dir = tempfile::tempdir().unwrap();
    write_pdf(&dir.path().join("paper.pdf"));
    let mut h = Harness::new(dir.path());
    h.keys(":o paper.pdf<ret>");
    select_hello(&mut h);
    h.keys("I");
    assert_eq!(input::context(&h.state), Context::DraftInsert);
    h.keys("first line<ret>second line<C-s>");
    let saved = &notebook(&h).highlights[0];
    assert_eq!(
        saved.comment.as_ref().map(|comment| comment.as_str()),
        Some("first line\nsecond line")
    );
    assert_eq!(
        saved
            .comment
            .as_ref()
            .map(|comment| comment.line().into_owned()),
        Some("first line\u{2026}".to_owned())
    );
    h.keys("<esc>");
    assert_eq!(input::context(&h.state), Context::Draft);
    h.keys("<space>v");
    assert_eq!(writing(&h).beside.spread, state::draft::Spread::Full);
    h.keys("<space>v<C-w><");
    assert_eq!(writing(&h).beside.spread, state::draft::Spread::Split);
    assert!(writing(&h).beside.share.get() < 0.5);
    h.keys("ggA, edited<esc>:w<ret>");
    assert_eq!(
        notebook(&h).highlights[0]
            .comment
            .as_ref()
            .map(|comment| comment.as_str()),
        Some("first line, edited\nsecond line")
    );
    h.keys("<esc>");
    assert_eq!(picked(&h), notebook(&h).highlights[0]);
    assert_eq!(input::context(&h.state), Context::PdfRead);

    // `i` on a comment of several lines writes it out in full too.
    h.keys("i");
    assert_eq!(
        writing(&h).draft.doc.text,
        "first line, edited\nsecond line"
    );
}

#[test]
fn leaving_a_comment_unsaved_asks_and_keeps_the_library_as_it_was() {
    let dir = tempfile::tempdir().unwrap();
    write_pdf(&dir.path().join("paper.pdf"));
    let mut h = Harness::new(dir.path());
    h.keys(":o paper.pdf<ret>");
    select_hello(&mut h);
    h.keys("isaid hello<ret>Ito myself<esc>:q<ret>");
    assert!(matches!(
        h.state.notice,
        Some(Notice::UnsavedBuffers { count: 1 })
    ));
    h.keys("<space>qy");
    let kept = &notebook(&h).highlights[0];
    assert_eq!(
        kept.comment.as_ref().map(|comment| comment.as_str()),
        Some("said hello")
    );
    assert_eq!(picked(&h), *kept);
}

#[test]
fn ctrl_v_carries_the_one_line_comment_into_the_full_view() {
    let dir = tempfile::tempdir().unwrap();
    write_pdf(&dir.path().join("paper.pdf"));
    let mut h = Harness::new(dir.path());
    h.keys(":o paper.pdf<ret>");
    select_hello(&mut h);
    h.keys("ishort<C-v>");
    assert_eq!(writing(&h).draft.doc.text, "short");
    assert!(notebook(&h).highlights.is_empty(), "nothing saved yet");
    h.keys(" and long<esc><esc>");
    assert_eq!(
        notebook(&h).highlights[0]
            .comment
            .as_ref()
            .map(|comment| comment.as_str()),
        Some("short and long")
    );

    // A new highlight left blank is let go without being made.
    h.keys("<esc>");
    tap_hello(&mut h);
    h.keys("d");
    select_hello(&mut h);
    h.keys("I<esc><esc>");
    assert!(notebook(&h).highlights.is_empty());
    assert_eq!(tab(&h).mode, PdfMode::Read(Reading::Idle));
}
