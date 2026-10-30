use std::sync::Arc;

use krilla::Document as Writer;
use krilla::page::PageSettings;
use library::{
    Annotation, AnnotationId, Bookmark, Change, Comment, FastHash, Highlight, Identity, Library,
    SearchQuery, TextSpan,
};
use model::sheet::PageIndex;
use reader::pdf::Document;

fn pdf(pages: &[&[&str]]) -> Arc<Document> {
    let font = krilla::text::Font::new(model::fonts::BUNDLED[4].to_vec().into(), 0).unwrap();
    let mut writer = Writer::new();
    for lines in pages {
        let mut page = writer.start_page_with(PageSettings::from_wh(300.0, 200.0).unwrap());
        let mut surface = page.surface();
        for (line, text) in lines.iter().enumerate() {
            surface.draw_text(
                krilla::geom::Point::from_xy(20.0, 40.0 + 20.0 * line as f32),
                font.clone(),
                12.0,
                text,
                false,
                krilla::text::TextDirection::Auto,
            );
        }
        surface.finish();
        page.finish();
    }
    Arc::new(Document::open(writer.finish().unwrap().into()).unwrap())
}

const FIRST: &[&[&str]] = &[&["Hello world", "the quick brown fox"], &["jumps over"]];

fn highlight(document: &Arc<Document>, page: usize, word: &str) -> Highlight {
    let text = document.read_text(page).unwrap();
    let start = text.text.find(word).unwrap();
    let span = TextSpan {
        page: PageIndex(page),
        chars: text.chars_in(start..start + word.len()),
    };
    Highlight {
        comment: Comment::new("a note".to_owned()),
        ..Highlight::of(document, span).unwrap()
    }
}

fn put(highlight: &Highlight) -> Change {
    Change::Put(Annotation::Highlight(highlight.clone()))
}

#[test]
fn the_hash_follows_the_bytes() {
    let bytes = vec![7u8; 3 * 1024 * 1024];
    let mut changed = bytes.clone();
    changed[1024 * 1024 + 10] = 8;
    assert_eq!(FastHash::of(&bytes), FastHash::of(&bytes.clone()));
    assert_ne!(
        FastHash::of(&bytes),
        FastHash::of(&changed),
        "a sampled chunk"
    );
    assert_ne!(
        FastHash::of(&bytes),
        FastHash::of(&bytes[1..]),
        "the length"
    );
}

#[test]
fn a_highlight_quotes_its_text_with_context() {
    let document = pdf(FIRST);
    let quick = highlight(&document, 0, "quick");
    assert_eq!(quick.quote.exact(), "quick");
    assert_eq!(quick.quote.prefix, "Hello world\nthe ");
    assert_eq!(quick.quote.suffix, " brown fox");
    assert!(Comment::new("  \n".to_owned()).is_none());
    assert!(SearchQuery::new("   ").is_none());
}

#[test]
fn a_document_is_known_again_by_path_and_by_bytes() {
    let folder = tempfile::tempdir().unwrap();
    let library = Library::at(folder.path().to_owned());
    let document = pdf(FIRST);
    let path = library.place(&folder.path().join("books/a.pdf")).unwrap();
    assert_eq!(path.as_str(), "books/a.pdf");
    assert!(
        library
            .place(std::path::Path::new("/elsewhere/a.pdf"))
            .is_none()
    );

    let listed = library.identify(&path, &document).unwrap();
    assert_eq!(listed.identity, Identity::New);
    assert!(folder.path().join(".edt/library.db").is_file());
    assert_eq!(
        std::fs::read_to_string(folder.path().join(".edt/.gitignore")).unwrap(),
        "*\n"
    );
    let fox = highlight(&document, 0, "fox");
    library.write(listed.id, &put(&fox)).unwrap();
    library.keep_place(listed.id, PageIndex(1)).unwrap();

    let again = library.identify(&path, &document).unwrap();
    assert_eq!(again.identity, Identity::Known);
    assert_eq!(again.notebook.highlights, [fox.clone()]);
    assert_eq!(again.position, PageIndex(1));

    let moved = library.place(&folder.path().join("b.pdf")).unwrap();
    let listed = library.identify(&moved, &document).unwrap();
    assert_eq!(listed.identity, Identity::Moved { from: path.clone() });
    assert_eq!(listed.notebook.highlights, [fox]);

    std::fs::write(folder.path().join("b.pdf"), b"here").unwrap();
    let copy = library.place(&folder.path().join("c.pdf")).unwrap();
    assert_eq!(
        library.identify(&copy, &document).unwrap().identity,
        Identity::New,
        "a copy beside the original"
    );
}

#[test]
fn a_changed_document_finds_its_highlights_again_or_loses_them() {
    let folder = tempfile::tempdir().unwrap();
    let library = Library::at(folder.path().to_owned());
    let path = library.place(&folder.path().join("a.pdf")).unwrap();
    let document = pdf(FIRST);
    let listed = library.identify(&path, &document).unwrap();
    let (fox, jumps, world) = (
        highlight(&document, 0, "fox"),
        highlight(&document, 1, "jumps"),
        highlight(&document, 0, "world"),
    );
    for each in [&fox, &jumps, &world] {
        library.write(listed.id, &put(each)).unwrap();
    }
    let bookmark = Bookmark {
        id: AnnotationId::new(),
        page: PageIndex(1),
    };
    let marked = Change::Put(Annotation::Bookmark(bookmark.clone()));
    library.write(listed.id, &marked).unwrap();

    let changed = pdf(&[
        &["Hello there", "a new line", "the quick brown fox"],
        &["jumps over"],
    ]);
    let listed = library.identify(&path, &changed).unwrap();
    assert_eq!(listed.identity, Identity::Changed);
    let notebook = &listed.notebook;
    let found = |id| {
        notebook
            .highlights
            .iter()
            .find(|kept: &&Highlight| kept.id == id)
    };
    let text = changed.read_text(0).unwrap();
    assert_eq!(
        text.slice(found(fox.id).unwrap().span.chars.clone()),
        "fox",
        "moved down a line"
    );
    assert_eq!(found(jumps.id).unwrap().span, jumps.span, "unchanged");
    assert_eq!(
        notebook.lost.iter().map(|lost| lost.id).collect::<Vec<_>>(),
        [world.id],
        "the word is gone"
    );
    assert_eq!(notebook.bookmarks, [bookmark]);
    assert_eq!(
        library.identify(&path, &changed).unwrap().notebook,
        listed.notebook,
        "the new anchors are saved"
    );
}

#[test]
fn search_finds_quotes_and_comments_by_prefix() {
    let folder = tempfile::tempdir().unwrap();
    let library = Library::at(folder.path().to_owned());
    let path = library.place(&folder.path().join("a.pdf")).unwrap();
    let document = pdf(FIRST);
    let listed = library.identify(&path, &document).unwrap();
    let fox = highlight(&document, 0, "fox");
    let jumps = highlight(&document, 1, "jumps");
    library.write(listed.id, &put(&fox)).unwrap();
    library.write(listed.id, &put(&jumps)).unwrap();

    let ids = |query: &str| -> Vec<AnnotationId> {
        let hits = library.search(&SearchQuery::new(query).unwrap()).unwrap();
        hits.into_iter().map(|hit| hit.annotation.id()).collect()
    };
    assert_eq!(ids("fo"), [fox.id]);
    assert_eq!(ids("note").len(), 2, "comments are searched");

    let edited = Highlight {
        comment: Comment::new("leaps \"high\"".to_owned()),
        ..jumps.clone()
    };
    library.write(listed.id, &put(&edited)).unwrap();
    assert_eq!(ids("\"high"), [jumps.id]);
    assert_eq!(ids("note"), [fox.id], "the old comment is gone");
    library.write(listed.id, &Change::Remove(fox.id)).unwrap();
    assert!(ids("fox").is_empty());
}
