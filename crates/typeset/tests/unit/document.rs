use typeset::{DocumentError, LineColumn, Origin, SourceFile};

use crate::{assert_valid, typesetter};

const THREE_PAGES: &str = "#set page(width: 10cm, height: 6cm)\n\
    = First\n\
    #pagebreak()\n\
    = Second $x^2$\n\
    #pagebreak()\n\
    = Third\n";

#[test]
fn multi_page_document_gives_each_page() {
    let mut typesetter = typesetter();
    let dir = tempfile::tempdir().expect("tempdir");
    let main = dir.path().join("main.typ");
    let pages = typesetter
        .typst_pages(dir.path(), &main, THREE_PAGES)
        .expect("compiles");
    assert_eq!(pages.pages.len(), 3);
    for page in &pages.pages {
        assert_valid(page);
        assert!((page.width_pt - 283.46).abs() < 0.1, "{}", page.width_pt);
        assert!((page.height_pt - 170.08).abs() < 0.1, "{}", page.height_pt);
    }
}

#[test]
fn edit_reexports_only_the_changed_page() {
    let mut typesetter = typesetter();
    let dir = tempfile::tempdir().expect("tempdir");
    let main = dir.path().join("main.typ");
    let before = typesetter
        .typst_pages(dir.path(), &main, THREE_PAGES)
        .expect("compiles");
    let edited = THREE_PAGES.replace("Third", "Thirds");
    let after = typesetter
        .typst_pages(dir.path(), &main, &edited)
        .expect("compiles");
    assert!(std::sync::Arc::ptr_eq(
        &before.pages[0].bytes,
        &after.pages[0].bytes
    ));
    assert!(std::sync::Arc::ptr_eq(
        &before.pages[1].bytes,
        &after.pages[1].bytes
    ));
    assert_ne!(before.pages[2].bytes, after.pages[2].bytes);
}

#[test]
fn imports_resolve_relative_to_root_and_track_disk_changes() {
    let mut typesetter = typesetter();
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::create_dir(dir.path().join("lib")).expect("mkdir");
    std::fs::write(
        dir.path().join("lib/util.typ"),
        "#let greet(name) = [Hello #name]\n#let pages = 2\n",
    )
    .expect("write");
    let main = dir.path().join("main.typ");
    let source = "#import \"lib/util.typ\": greet, pages\n\
        #for i in range(pages) [#greet(i) #pagebreak(weak: true)]\n";
    let pages = typesetter
        .typst_pages(dir.path(), &main, source)
        .expect("import resolves");
    assert_eq!(pages.pages.len(), 2);

    std::fs::write(
        dir.path().join("lib/util.typ"),
        "#let greet(name) = [Hello #name]\n#let pages = 4\n",
    )
    .expect("rewrite");
    let pages = typesetter
        .typst_pages(dir.path(), &main, source)
        .expect("import resolves");
    assert_eq!(pages.pages.len(), 4, "edited import is re-read");
}

#[test]
fn syntax_error_reports_its_line() {
    let mut typesetter = typesetter();
    let dir = tempfile::tempdir().expect("tempdir");
    let main = dir.path().join("main.typ");
    let source = "= Title\n\nSome text.\n#let x = (1, 2\nMore text.\n";
    let Err(DocumentError::Compile(diagnostics)) =
        typesetter.typst_pages(dir.path(), &main, source)
    else {
        panic!("expected a compile error");
    };
    let error = diagnostics.errors.first().expect("one error");
    let Origin::Span { file, start, .. } = &error.origin else {
        panic!("error has no span: {error:?}");
    };
    assert_eq!(*file, SourceFile::Project(main.clone()));
    // Zero-based: the unclosed `(` on the fourth line.
    assert_eq!(start.line, 3, "{error:?}");
}

#[test]
fn error_in_an_imported_file_points_at_that_file() {
    let mut typesetter = typesetter();
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::write(
        dir.path().join("other.typ"),
        "#let a = 1\n#let b = undefined-name\n",
    )
    .expect("write");
    let main = dir.path().join("main.typ");
    let Err(DocumentError::Compile(diagnostics)) =
        typesetter.typst_pages(dir.path(), &main, "#import \"other.typ\": a\n#a\n")
    else {
        panic!("expected a compile error");
    };
    let origins: Vec<_> = diagnostics
        .errors
        .iter()
        .map(|error| &error.origin)
        .collect();
    assert!(
        origins.iter().any(|origin| matches!(
            origin,
            Origin::Span { file: SourceFile::Project(path), start: LineColumn { line: 1, column: 9 }, .. }
                if path == &dir.path().join("other.typ")
        )),
        "{diagnostics:?}"
    );
}

#[test]
fn main_outside_root_is_rejected() {
    let mut typesetter = typesetter();
    let dir = tempfile::tempdir().expect("tempdir");
    let result = typesetter.typst_pages(
        &dir.path().join("sub"),
        &dir.path().join("main.typ"),
        "hello",
    );
    assert!(
        matches!(result, Err(DocumentError::OutsideRoot { .. })),
        "{result:?}"
    );
}

#[test]
fn missing_package_is_a_diagnostic_not_a_download() {
    let mut typesetter = typesetter();
    let dir = tempfile::tempdir().expect("tempdir");
    let result = typesetter.typst_pages(
        dir.path(),
        &dir.path().join("main.typ"),
        "#import \"@preview/cetz:0.3.0\": canvas\n",
    );
    assert!(
        matches!(result, Err(DocumentError::Compile(_))),
        "{result:?}"
    );
}

#[test]
fn pdf_export_starts_with_magic() {
    let mut typesetter = typesetter();
    let dir = tempfile::tempdir().expect("tempdir");
    let pdf = typesetter
        .typst_pdf(dir.path(), &dir.path().join("main.typ"), THREE_PAGES)
        .expect("pdf");
    assert!(pdf.starts_with(b"%PDF"), "{:?}", &pdf[..8.min(pdf.len())]);
}

#[test]
fn math_and_documents_share_one_world() {
    let mut typesetter = typesetter();
    let dir = tempfile::tempdir().expect("tempdir");
    let main = dir.path().join("main.typ");
    let pages = typesetter
        .typst_pages(dir.path(), &main, THREE_PAGES)
        .expect("document");
    typesetter
        .math(
            r"\frac{1}{2}",
            typeset::Display::Inline,
            model::color::Rgb { r: 0, g: 0, b: 0 },
            12.0,
        )
        .expect("math between documents");
    let again = typesetter
        .typst_pages(dir.path(), &main, THREE_PAGES)
        .expect("document again");
    assert_eq!(pages.pages.len(), again.pages.len());
    assert_eq!(pages.pages[0].bytes, again.pages[0].bytes);
}
