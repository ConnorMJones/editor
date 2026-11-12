use std::path::{Path, PathBuf};
use std::sync::Arc;

use library::{Notebook, Part};
use model::notice::Notice;
use model::picture::{Bitmap, Encoding};
use reader::epub::Spot;
use state::book::{BookTab, Pressed};
use state::project::session;
use state::reading::{Found, Notes, Press, Reading, Taps};
use state::render::book::{Align, BookTarget, InChapter};
use state::render::document::{DocumentMotion, Placed, TopItem};
use state::render::flow::{Block, BlockKind, Emphasis, ImagePlace, Inline, LocalImage};
use state::render::typeset::Probe;
use state::view::tab::Tab;

use crate::harness::Harness;
use crate::zip_writer::{Packing, zip};

const CONTAINER: &str = r#"<?xml version="1.0"?>
<container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container">
  <rootfiles><rootfile full-path="OEBPS/content.opf" media-type="application/oebps-package+xml"/></rootfiles>
</container>"#;

const PACKAGE: &str = r#"<?xml version="1.0"?>
<package xmlns="http://www.idpf.org/2007/opf" version="3.0">
  <metadata xmlns:dc="http://purl.org/dc/elements/1.1/">
    <dc:title>A Novel</dc:title>
    <dc:creator>Someone</dc:creator>
  </metadata>
  <manifest>
    <item id="nav" href="nav.xhtml" media-type="application/xhtml+xml" properties="nav"/>
    <item id="style" href="style/book.css" media-type="text/css"/>
    <item id="one" href="text/one.xhtml" media-type="application/xhtml+xml"/>
    <item id="two" href="text/two.xhtml" media-type="application/xhtml+xml"/>
    <item id="cover" href="images/cover.png" media-type="image/png"/>
  </manifest>
  <spine><itemref idref="one"/><itemref idref="two"/></spine>
</package>"#;

const NAV: &str = r#"<html xmlns="http://www.w3.org/1999/xhtml" xmlns:epub="http://www.idpf.org/2007/ops"><body>
<nav epub:type="toc"><ol>
  <li><a href="text/one.xhtml">One</a></li>
  <li><a href="text/two.xhtml">Two</a><ol><li><a href="text/two.xhtml#later">Later</a></li></ol></li>
</ol></nav>
</body></html>"#;

const STYLE: &str = ".centred { text-align: center }
p.aside { margin-left: 2em; font-size: 80% }
h1 { text-align: right }";

const ONE: &str = r#"<html xmlns="http://www.w3.org/1999/xhtml">
<head><title>One</title><link rel="stylesheet" href="../style/book.css"/></head>
<body>
<h1>Chapter   One</h1>
<p class="centred">It   began
  with <em>rain</em> &amp; <b>wind</b>.</p>
<p class="aside" style="text-indent: 1.5em">An aside.</p>
<p><img src="../images/cover.png" alt="cover"/></p>
<p>See <a href="two.xhtml#later">later</a> or <a href="https://example.com/">the web</a>&nbsp;now.</p>
</body></html>"#;

const TWO: &str = r#"<html xmlns="http://www.w3.org/1999/xhtml">
<head><style>.quiet { font-style: italic }</style></head>
<body>
<h2>Chapter Two</h2>
<p>First.</p>
<p class="quiet">Second.</p>
<h3 id="later">Later on</h3>
<p>Third.</p>
</body></html>"#;

fn png(width: u32, height: u32) -> Vec<u8> {
    let mut bytes = std::io::Cursor::new(Vec::new());
    image::RgbImage::new(width, height)
        .write_to(&mut bytes, image::ImageFormat::Png)
        .unwrap();
    bytes.into_inner()
}

fn write_book(dir: &Path, chapters: [&str; 2]) -> PathBuf {
    let cover = png(12, 8);
    let bytes = zip(&[
        ("mimetype", Packing::Stored, b"application/epub+zip"),
        (
            "META-INF/container.xml",
            Packing::Deflated,
            CONTAINER.as_bytes(),
        ),
        ("OEBPS/content.opf", Packing::Deflated, PACKAGE.as_bytes()),
        ("OEBPS/nav.xhtml", Packing::Deflated, NAV.as_bytes()),
        ("OEBPS/style/book.css", Packing::Deflated, STYLE.as_bytes()),
        (
            "OEBPS/text/one.xhtml",
            Packing::Deflated,
            chapters[0].as_bytes(),
        ),
        (
            "OEBPS/text/two.xhtml",
            Packing::Deflated,
            chapters[1].as_bytes(),
        ),
        ("OEBPS/images/cover.png", Packing::Stored, &cover),
    ]);
    let path = dir.canonicalize().unwrap().join("novel.epub");
    std::fs::write(&path, bytes).unwrap();
    path
}

fn opened() -> (tempfile::TempDir, PathBuf, Harness) {
    let dir = tempfile::tempdir().unwrap();
    let path = write_book(dir.path(), [ONE, TWO]);
    let mut h = Harness::new(dir.path());
    h.keys(":o novel.epub<ret>");
    (dir, path, h)
}

fn tab(h: &Harness) -> &BookTab {
    match h.state.layout.focused().active() {
        Some(Tab::Book(tab)) => tab,
        Some(Tab::Text(_) | Tab::Ink(_) | Tab::Image(_) | Tab::JsonCanvas(_)) | None => {
            panic!("a book is focused")
        }
    }
}

fn root(h: &Harness, item: usize) -> &Block<InChapter> {
    let flow = &h.state.books[tab(h).file()].flow.flow;
    &flow.blocks[flow.roots[item]]
}

fn text(block: &Block<InChapter>) -> &Inline<BookTarget> {
    block.inline().expect("a block with text")
}

fn at(item: usize) -> Placed {
    Placed::item(item)
}

#[test]
fn a_book_opens_as_one_flow_of_its_chapters() {
    let (_dir, _, h) = opened();
    assert_eq!(h.focused_tabs(), ["❧novel.epub"]);
    let file = &h.state.books[tab(&h).file()];
    assert_eq!(file.title, "A Novel");
    assert_eq!(file.flow.flow.roots.len(), 10);
    assert_eq!(&*file.flow.starts, [0, 5]);
    assert_eq!(&*text(root(&h, 0)).text, "Chapter One");
    assert_eq!(&*text(root(&h, 5)).text, "Chapter Two");
    assert_eq!(root(&h, 5).origin.chapter, 1);
    assert_eq!(tab(&h).scroll, at(0));
}

#[test]
fn the_books_css_and_markup_shape_its_blocks() {
    let (_dir, _, h) = opened();
    assert_eq!(root(&h, 0).origin.layout.align, Align::End);
    let centred = root(&h, 1);
    assert_eq!(centred.origin.layout.align, Align::Center);
    let words = text(centred);
    assert_eq!(&*words.text, "It began with rain & wind.");
    let styles: Vec<(&str, Emphasis)> = (words.spans.iter())
        .map(|span| (&words.text[span.range.clone()], span.style))
        .collect();
    assert_eq!(
        styles,
        [
            ("It began with ", Emphasis::NONE),
            ("rain", Emphasis::ITALIC),
            (" & ", Emphasis::NONE),
            ("wind", Emphasis::STRONG),
            (".", Emphasis::NONE),
        ]
    );
    let aside = root(&h, 2).origin.layout;
    assert_eq!((aside.margin, aside.scale, aside.indent), (2.0, 0.8, 1.5));
    let quiet = text(root(&h, 7));
    assert!(quiet.spans[0].style.contains(Emphasis::ITALIC));
    assert!(matches!(root(&h, 3).kind, BlockKind::Image(_)));
}

#[test]
fn links_in_a_book_scroll_to_their_place_or_open_the_web() {
    let (_dir, _, mut h) = opened();
    let links = text(root(&h, 4)).links.clone();
    assert_eq!(&*text(root(&h, 4)).text, "See later or the web\u{a0}now.");
    assert_eq!(
        links[0].target,
        BookTarget::Spot(Spot {
            chapter: 1,
            anchor: Some("later".into()),
        })
    );
    assert_eq!(
        links[1].target,
        BookTarget::Url("https://example.com/".to_owned())
    );
    let (pane, book) = (h.state.layout.focus(), tab(&h).file());
    h.run(|state, out| state::book::follow(state, pane, book, &links[0].target, out));
    assert_eq!(tab(&h).scroll, at(8));
    h.run(|state, out| state::book::follow(state, pane, book, &links[1].target, out));
    assert_eq!(h.urls, ["https://example.com/"]);
}

#[test]
fn the_contents_take_the_reader_to_a_chapter_or_an_anchor() {
    let (_dir, _, mut h) = opened();
    h.keys("<space>olater<ret>");
    assert_eq!(tab(&h).scroll, at(8));
    h.keys("<space>otwo<ret>");
    assert_eq!(tab(&h).scroll, at(5));
}

#[test]
fn reading_keys_scroll_and_jump_by_heading() {
    let (_dir, _, mut h) = opened();
    h.keys("j");
    assert_eq!(
        tab(&h).scroll,
        Placed::Moved {
            from: TopItem::default(),
            by: DocumentMotion {
                lines: 1,
                half_pages: 0,
            },
        }
    );
    h.keys("ge");
    assert_eq!(tab(&h).scroll, at(9));
    h.keys("gg]]");
    assert_eq!(tab(&h).scroll, at(5));
    h.keys("]]");
    assert_eq!(tab(&h).scroll, at(8));
}

#[test]
fn a_books_images_are_sized_when_it_opens() {
    let (_dir, path, h) = opened();
    let cover = LocalImage {
        place: ImagePlace::Zipped {
            archive: Arc::from(path.as_path()),
            name: Arc::from("OEBPS/images/cover.png"),
        },
        encoding: Encoding::Bitmap(Bitmap::Png),
    };
    match h.state.figures.image(&cover) {
        Some(Probe::Read(probed)) => {
            assert_eq!(
                (probed.size.width.get(), probed.size.height.get()),
                (12.0, 8.0)
            )
        }
        Some(Probe::Pending | Probe::Failed(_)) | None => panic!("the cover was sized"),
    }
}

#[test]
fn a_session_keeps_the_place_and_zoom_in_a_book() {
    let (dir, _, mut before) = opened();
    before.keys("<space>olater<ret>+");
    let text = toml::to_string(&session::snapshot(&before.state)).unwrap();
    let mut after = Harness::new(dir.path());
    let saved: session::Session = toml::from_str(&text).unwrap();
    after
        .run(|state, out| session::restore(state, saved, out))
        .unwrap();
    assert_eq!(after.focused_tabs(), ["❧novel.epub"]);
    assert_eq!(tab(&after).scroll, at(8));
    assert_eq!(
        tab(&after).zoom,
        model::sheet::Zoom::ONE.by(model::sheet::Zoom::STEP)
    );
}

#[test]
fn a_book_with_no_text_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let blank = r#"<html xmlns="http://www.w3.org/1999/xhtml"><body> </body></html>"#;
    write_book(dir.path(), [blank, blank]);
    let mut h = Harness::new(dir.path());
    h.keys(":o novel.epub<ret>");
    assert!(matches!(h.state.notice, Some(Notice::Unreadable { .. })));
    assert!(h.focused_tabs().is_empty());
}

fn notebook(h: &Harness) -> Notebook {
    match &h.state.books[tab(h).file()].notes {
        Notes::Listed(listed) => listed.notebook.clone(),
        Notes::Unlisted(unlisted) => panic!("{unlisted:?}"),
    }
}

fn drag(h: &mut Harness, item: usize, from: usize, to: usize) {
    let (pane, book) = (h.state.layout.focus(), tab(h).file());
    let block = h.state.books[book].flow.flow.roots[item];
    let at = |byte| Some(Pressed { block, byte });
    h.run(|state, _| state::book::press(state, pane, book, at(from), Press::Down(Taps::One)));
    h.run(|state, _| state::book::press(state, pane, book, at(to), Press::Move));
    h.run(|state, _| state::book::press(state, pane, book, None, Press::Up));
}

#[test]
fn a_selection_in_a_book_is_copied_highlighted_and_commented() {
    let (dir, _, mut h) = opened();
    drag(&mut h, 1, 3, 7);
    let Reading::Selected(span) = &tab(&h).reading else {
        panic!("{:?}", tab(&h).reading);
    };
    assert_eq!(span.start().part, Part(0));
    h.keys("y");
    assert_eq!(h.clipboard[0].as_deref(), Some("began"));
    h.keys("v");
    let [highlight] = &notebook(&h).highlights[..] else {
        panic!("{:?}", notebook(&h));
    };
    assert_eq!(highlight.quote.exact(), "began");
    assert_eq!(tab(&h).reading, Reading::OnHighlight(highlight.clone()));
    h.keys("ia note<ret>");
    let commented = notebook(&h).highlights[0].comment.clone();
    assert_eq!(
        commented
            .map(|comment| comment.as_str().to_owned())
            .as_deref(),
        Some("a note")
    );

    let mut again = Harness::new(dir.path());
    again.keys(":o novel.epub<ret>");
    assert_eq!(notebook(&again), notebook(&h), "read back from the library");
    drag(&mut again, 1, 5, 5);
    assert_eq!(
        tab(&again).reading,
        Reading::OnHighlight(notebook(&h).highlights[0].clone())
    );
    again.keys("d");
    assert!(notebook(&again).highlights.is_empty());
}

#[test]
fn a_drag_into_another_chapter_leaves_the_selection_where_it_was() {
    let (_dir, _, mut h) = opened();
    let (pane, book) = (h.state.layout.focus(), tab(&h).file());
    let flow = &h.state.books[book].flow.flow;
    let (first, later) = (flow.roots[1], flow.roots[6]);
    let at = |block, byte| Some(Pressed { block, byte });
    h.run(|state, _| state::book::press(state, pane, book, at(first, 0), Press::Down(Taps::One)));
    h.run(|state, _| state::book::press(state, pane, book, at(later, 3), Press::Move));
    h.run(|state, _| state::book::press(state, pane, book, None, Press::Up));
    assert_eq!(
        tab(&h).reading,
        Reading::Idle,
        "the head never left the first block"
    );
}

#[test]
fn find_in_a_book_steps_through_every_chapter() {
    let (_dir, _, mut h) = opened();
    h.keys("/later<ret>");
    let Found::Hits { hits, at, .. } = tab(&h).found.clone() else {
        panic!("{:?}", tab(&h).found);
    };
    assert_eq!((hits.len(), at), (2, 0));
    assert_eq!(tab(&h).scroll, Placed::item(4));
    h.keys("n");
    assert_eq!(tab(&h).scroll, Placed::item(8));
    h.keys("n");
    assert_eq!(tab(&h).scroll, Placed::item(4), "wrapping round");
    h.keys("/zebra<ret>");
    assert!(matches!(h.state.notice, Some(Notice::NoHits { .. })));
}

#[test]
fn a_bookmark_marks_the_place_in_view_and_the_notebook_goes_back_to_it() {
    let (_dir, _, mut h) = opened();
    h.keys("<space>olater<ret>m");
    let [bookmark] = &notebook(&h).bookmarks[..] else {
        panic!("{:?}", notebook(&h));
    };
    assert_eq!(bookmark.at.part, Part(1));
    let shown = |h: &Harness| {
        let tab = h.state.layout.focused().active().unwrap();
        state::reading::bookmark_in_view(tab, &h.state.pdfs, &h.state.books).is_some()
    };
    assert!(shown(&h), "the place in view is bookmarked");
    h.keys("gg");
    assert!(!shown(&h), "the start of the book is not");
    h.keys("<space>a<ret>");
    assert!(shown(&h));
    assert_eq!(tab(&h).scroll, Placed::item(8));
    h.keys("m");
    assert!(
        notebook(&h).bookmarks.is_empty(),
        "a second m takes it away"
    );
}

#[test]
fn a_double_tap_in_a_book_selects_the_word() {
    let (_dir, _, mut h) = opened();
    let (pane, book) = (h.state.layout.focus(), tab(&h).file());
    let block = h.state.books[book].flow.flow.roots[1];
    let at = Some(Pressed { block, byte: 4 });
    h.run(|state, _| state::book::press(state, pane, book, at, Press::Down(Taps::Two)));
    h.keys("y");
    assert_eq!(h.clipboard[0].as_deref(), Some("began"));
}

#[test]
fn notes_from_a_book_name_the_contents_chapter_and_its_links_go_there() {
    let (dir, _, mut h) = opened();
    drag(&mut h, 1, 3, 7);
    h.keys("v:export-notes<ret>");
    let written = std::fs::read_to_string(dir.path().join("novel.md")).unwrap();
    assert!(written.contains("# A Novel\nSomeone\n"), "{written}");
    assert!(written.contains("[hl] **chp 1** - **"), "{written}");
    assert!(
        written.contains("%]]\n<!-- quote -->\nbegan\n<!-- /quote -->\n"),
        "{written}"
    );
    assert!(
        written.contains(&format!(
            "[[novel.epub#chapter=1&note={}|chp 1 - ",
            notebook(&h).highlights[0].id.0
        )),
        "{written}"
    );

    std::fs::write(dir.path().join("links.md"), "[[novel.epub#chapter=2]]\n").unwrap();
    h.keys(":o links.md<ret>gf");
    let book = tab(&h).file();
    let second = h.state.books[book].flow.starts[1];
    assert_eq!(tab(&h).scroll, Placed::item(second));
}

#[test]
fn a_comment_on_a_book_highlight_is_written_beside_it() {
    let (_dir, _, mut h) = opened();
    drag(&mut h, 1, 3, 7);
    h.keys("Ia longer<ret>thought<esc>");
    assert!(matches!(tab(&h).reading, Reading::Writing(_)));
    h.keys("<esc>");
    let [highlight] = &notebook(&h).highlights[..] else {
        panic!("{:?}", notebook(&h));
    };
    assert_eq!(highlight.quote.exact(), "began");
    assert_eq!(
        highlight.comment.as_ref().map(|comment| comment.as_str()),
        Some("a longer\nthought")
    );
    assert_eq!(tab(&h).reading, Reading::OnHighlight(highlight.clone()));
}
