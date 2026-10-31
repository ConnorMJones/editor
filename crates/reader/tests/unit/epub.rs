use std::io::{Seek, Write};

use reader::epub::{Book, Contents, EpubError, Spot};

use crate::writer::{Packing, zip};

const CONTAINER: &str = r#"<container xmlns="urn:oasis:names:tc:opendocument:xmlns:container">
<rootfiles><rootfile full-path="book/package.opf"/></rootfiles></container>"#;

const CHAPTER: &str =
    r#"<html xmlns="http://www.w3.org/1999/xhtml"><body><p>Text&nbsp;here.</p></body></html>"#;

fn open(entries: &[(&str, Packing, &[u8])]) -> Result<Book, EpubError> {
    let mut file = tempfile::tempfile().unwrap();
    file.write_all(&zip(entries)).unwrap();
    file.rewind().unwrap();
    Book::open(file)
}

fn package(manifest: &str, spine: &str) -> String {
    format!(
        r#"<package xmlns="http://www.idpf.org/2007/opf" xmlns:dc="http://purl.org/dc/elements/1.1/">
<metadata><dc:title> The Title </dc:title><dc:creator>One</dc:creator><dc:creator>Two</dc:creator></metadata>
<manifest>{manifest}</manifest><spine {spine}><itemref idref="a"/><itemref idref="b"/></spine></package>"#
    )
}

#[test]
fn a_package_gives_its_chapters_styles_and_contents_from_its_nav() {
    let opf = package(
        r#"<item id="a" href="text/a.xhtml"/><item id="b" href="text/b%20two.xhtml"/>
<item id="css" href="style.css" media-type="text/css"/>
<item id="nav" href="nav.xhtml" properties="nav"/>"#,
        "",
    );
    let nav = r#"<html xmlns="http://www.w3.org/1999/xhtml"><body><nav type="toc"><ol>
<li><a href="text/a.xhtml">  First   part </a><ol><li><a href="text/b%20two.xhtml#end">Second</a></li></ol></li>
<li><a href="elsewhere.xhtml">Not in the spine</a></li>
</ol></nav></body></html>"#;
    let book = open(&[
        (
            "META-INF/container.xml",
            Packing::Stored,
            CONTAINER.as_bytes(),
        ),
        ("book/package.opf", Packing::Deflated, opf.as_bytes()),
        ("book/nav.xhtml", Packing::Deflated, nav.as_bytes()),
        ("book/style.css", Packing::Stored, b"p { color: red }"),
        ("book/text/a.xhtml", Packing::Deflated, CHAPTER.as_bytes()),
        (
            "book/text/b two.xhtml",
            Packing::Deflated,
            CHAPTER.as_bytes(),
        ),
    ])
    .unwrap();
    assert_eq!(book.title, "The Title");
    assert_eq!(book.authors, ["One", "Two"]);
    let paths: Vec<&str> = book.chapters.iter().map(|chapter| &*chapter.path).collect();
    assert_eq!(paths, ["book/text/a.xhtml", "book/text/b two.xhtml"]);
    assert_eq!(book.styles["book/style.css"], "p { color: red }");
    assert_eq!(
        book.contents,
        [
            Contents {
                title: "First part".to_owned(),
                depth: 0,
                at: Spot {
                    chapter: 0,
                    anchor: None
                },
            },
            Contents {
                title: "Second".to_owned(),
                depth: 1,
                at: Spot {
                    chapter: 1,
                    anchor: Some("end".into())
                },
            },
        ]
    );
}

#[test]
fn an_ncx_gives_the_contents_of_a_book_without_a_nav() {
    let opf = package(
        r#"<item id="a" href="a.xhtml"/><item id="b" href="b.xhtml"/><item id="ncx" href="toc.ncx"/>"#,
        r#"toc="ncx""#,
    );
    let ncx = r#"<ncx xmlns="http://www.daisy.org/z3986/2005/ncx/"><navMap>
<navPoint><navLabel><text>Opening</text></navLabel><content src="a.xhtml"/>
<navPoint><navLabel><text>Inside</text></navLabel><content src="b.xhtml#mid"/></navPoint>
</navPoint></navMap></ncx>"#;
    let book = open(&[
        (
            "META-INF/container.xml",
            Packing::Stored,
            CONTAINER.as_bytes(),
        ),
        ("book/package.opf", Packing::Stored, opf.as_bytes()),
        ("book/toc.ncx", Packing::Stored, ncx.as_bytes()),
        ("book/a.xhtml", Packing::Stored, CHAPTER.as_bytes()),
        ("book/b.xhtml", Packing::Stored, CHAPTER.as_bytes()),
    ])
    .unwrap();
    let titles: Vec<(&str, usize, &Spot)> = (book.contents.iter())
        .map(|entry| (entry.title.as_str(), entry.depth, &entry.at))
        .collect();
    assert_eq!(
        titles,
        [
            (
                "Opening",
                0,
                &Spot {
                    chapter: 0,
                    anchor: None
                }
            ),
            (
                "Inside",
                1,
                &Spot {
                    chapter: 1,
                    anchor: Some("mid".into())
                }
            ),
        ]
    );
}

#[test]
fn a_book_missing_its_package_or_its_chapters_is_refused() {
    let no_package = r#"<container><rootfiles/></container>"#;
    assert!(matches!(
        open(&[(
            "META-INF/container.xml",
            Packing::Stored,
            no_package.as_bytes()
        )]),
        Err(EpubError::NoPackage)
    ));
    let opf = package(
        r#"<item id="a" href="a.xhtml"/><item id="b" href="b.xhtml"/>"#,
        "",
    );
    assert!(matches!(
        open(&[
            (
                "META-INF/container.xml",
                Packing::Stored,
                CONTAINER.as_bytes()
            ),
            ("book/package.opf", Packing::Stored, opf.as_bytes()),
            ("book/a.xhtml", Packing::Stored, CHAPTER.as_bytes()),
        ]),
        Err(EpubError::Zip(_))
    ));
}
