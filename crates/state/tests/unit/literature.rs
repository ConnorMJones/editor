use library::{
    Annotation, AnnotationId, Bookmark, Comment, Highlight, Lost, Notebook, Part, Place, Quote,
    TextSpan,
};
use state::literature::{END, Places, START, Source, SpliceError, section, splice};

fn quote(text: &str) -> Quote {
    Quote::new(text.to_owned(), String::new(), String::new()).unwrap()
}

fn id(n: u8) -> AnnotationId {
    AnnotationId::parse(&format!("00000000-0000-7000-8000-0000000000{n:02}")).unwrap()
}

fn highlight(n: u8, part: usize, text: &str, comment: Option<&str>) -> Annotation {
    Annotation::Highlight(Highlight {
        id: id(n),
        span: TextSpan::within(Part(part), 0..4).unwrap(),
        quote: quote(text),
        comment: comment.and_then(|comment| Comment::new(comment.to_owned())),
    })
}

fn notebook() -> Notebook {
    [
        highlight(1, 158, "a highlighted line", None),
        Annotation::Bookmark(Bookmark {
            id: id(2),
            at: Place {
                part: Part(111),
                char: 0,
            },
        }),
        highlight(
            3,
            202,
            "quoted\nacross a break",
            Some("my thoughts\non two lines"),
        ),
        Annotation::Lost(Lost {
            id: id(4),
            part: Part(3),
            quote: quote("gone"),
            comment: None,
        }),
    ]
    .into_iter()
    .collect()
}

#[test]
fn a_pdf_section_lists_each_kind_in_page_order_with_its_chapter() {
    let source = Source {
        title: "A Book",
        authors: &["One Author".to_owned(), "Two Author".to_owned()],
        file: "book.pdf",
        places: Places::Pages {
            chapters: vec![100, 150, 200],
        },
    };
    assert_eq!(
        section(&source, &notebook()),
        format!(
            "{START}
# A Book
One Author, Two Author

[bm] **chp 1** - **pg 112**
[[book.pdf#page=112&note=00000000-0000-7000-8000-000000000002|chp 1 - pg 112]]

[hl] **chp 2** - **pg 159**
[[book.pdf#page=159&note=00000000-0000-7000-8000-000000000001|chp 2 - pg 159]]
<!-- quote -->
a highlighted line
<!-- /quote -->

[qt] **chp 3** - **pg 203**
[[book.pdf#page=203&note=00000000-0000-7000-8000-000000000003|chp 3 - pg 203]]
<!-- quote -->
quoted
across a break
<!-- /quote -->
<!-- comment -->
my thoughts
on two lines
<!-- /comment -->
{END}
"
        )
    );
}

#[test]
fn a_page_before_any_chapter_and_a_book_chapter_name_only_what_they_have() {
    let pages = Source {
        title: "Plain",
        authors: &[],
        file: "plain.pdf",
        places: Places::Pages { chapters: vec![] },
    };
    let text = section(&pages, &notebook());
    assert!(
        text.contains("[bm] **pg 112**\n[[plain.pdf#page=112&note=00000000-0000-7000-8000-000000000002|pg 112]]"),
        "{text}"
    );
    assert!(!text.contains("chp"));
    let parts: Vec<usize> = (0..300).map(|part| part * 100).collect();
    let chapters = Source {
        title: "Novel",
        authors: &[],
        file: "novel.epub",
        places: Places::Chapters {
            starts: vec![11_000, 15_000],
            parts: parts.clone(),
            total: 30_000,
        },
    };
    let text = section(&chapters, &notebook());
    assert!(
        text.contains("[bm] **chp 1** - **2%**\n[[novel.epub#chapter=1&note=00000000-0000-7000-8000-000000000002|chp 1 - 2%]]"),
        "{text}"
    );
    let front = Source {
        places: Places::Chapters {
            starts: vec![20_000],
            parts,
            total: 30_000,
        },
        ..chapters
    };
    let text = section(&front, &notebook());
    assert!(
        text.contains("[bm] **55%**\n[[novel.epub#note=00000000-0000-7000-8000-000000000002|55%]]"),
        "{text}"
    );
}

#[test]
fn splicing_replaces_only_the_generated_section() {
    let new = format!("{START}\nnew\n{END}\n");
    assert_eq!(splice(None, &new).unwrap(), new);
    let existing = format!("mine above\n\n{START}\nold\nlines\n{END}\nmine below\n");
    assert_eq!(
        splice(Some(&existing), &new).unwrap(),
        format!("mine above\n\n{START}\nnew\n{END}\nmine below\n")
    );
    assert!(matches!(
        splice(Some("just my notes\n"), &new),
        Err(SpliceError::NoMarkers)
    ));
    let open = format!("{START}\nnever closed\n");
    assert!(matches!(
        splice(Some(&open), &new),
        Err(SpliceError::Unclosed)
    ));
}
