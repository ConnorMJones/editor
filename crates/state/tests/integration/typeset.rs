use std::fs;

use model::notice::Notice;
use model::typeset::{Origin, SourceFile};
use state::render::document;
use state::render::markdown::Figure;
use state::render::pages::{self, PagesScroll};
use state::render::typeset::{self, Compile, Outcome, Render, SPARE, Typst};
use state::text::buffer::Content;
use state::view::layout::{PaneId, View};

use crate::harness::{Harness, Typesetting};

const NOTE: &str = "# Math

Inline $a^2 + b^2$ here.

$$
\\begin{pmatrix} 1 & 0 \\\\ 0 & 1 \\end{pmatrix}
$$

```mermaid
graph TD
  A --> B
```

Bad $\\frac$ one.
";

const DOC: &str = "#set page(width: 10cm, height: 6cm)
= One
#pagebreak()
= Two
";

fn open(name: &str, text: &str) -> (tempfile::TempDir, Harness) {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join(name), text).unwrap();
    let mut h = Harness::new(dir.path());
    h.keys(&format!(":o {name}<ret>"));
    (dir, h)
}

/// The figures of the focused buffer's parsed markdown, with their sources.
fn figures(h: &Harness) -> Vec<(String, Figure)> {
    let Content::Text { doc, .. } = &h.state.buffers[h.view().buffer()].content else {
        panic!("loading");
    };
    let parsed = document::markdown(&h.state.documents, h.view().buffer(), doc).expect("parsed");
    let mut found = Vec::new();
    typeset::each_figure(&parsed.blocks, |figure, source| {
        found.push((source.to_owned(), figure))
    });
    found
}

fn render<'a>(h: &'a Harness, source: &str) -> &'a Render {
    let figure = figures(h)
        .into_iter()
        .find(|(text, _)| text.contains(source))
        .map(|(_, figure)| figure)
        .unwrap_or_else(|| panic!("no figure {source:?}"));
    h.state.figures.get(figure).expect("cached")
}

fn typst(h: &Harness) -> (u64, &Typst) {
    let buffer = h.view().buffer();
    let doc = h.state.buffers[buffer].doc().expect("loaded");
    match h.state.compiles.get(buffer) {
        Some(typst) => (doc.version, typst),
        None => panic!("no typst preview"),
    }
}

fn preview(h: &Harness) -> PaneId {
    *h.state
        .layout
        .leaves()
        .iter()
        .find(|pane| {
            matches!(
                h.state.layout.panes()[**pane].active(),
                Some(View::Pages(_))
            )
        })
        .expect("a preview pane")
}

#[test]
fn figures_go_from_pending_to_ready_and_bad_tex_fails() {
    let (_dir, mut h) = open("note.md", NOTE);
    h.typesetting = Typesetting::Hold;
    h.keys("<space>v");
    assert_eq!(h.held.len(), 4, "one request per figure");
    for (source, _) in figures(&h) {
        assert!(
            matches!(render(&h, &source), Render::Pending),
            "{source} pending"
        );
    }
    while !h.held.is_empty() {
        h.release(0);
    }
    match render(&h, "a^2") {
        Render::Ready(svg) => {
            let baseline = svg.baseline.0;
            assert!(baseline > 0.0 && baseline < svg.height.0, "{svg:?}");
        }
        other => panic!("inline math: {other:?}"),
    }
    match render(&h, "pmatrix") {
        Render::Ready(svg) => assert!(svg.baseline == svg.height && svg.height.0 > 20.0),
        other => panic!("block math: {other:?}"),
    }
    match render(&h, "A --> B") {
        Render::Ready(svg) => assert!(svg.bytes.starts_with(b"<svg")),
        other => panic!("mermaid: {other:?}"),
    }
    match render(&h, "\\frac") {
        Render::Failed(error) => assert!(!error.is_empty()),
        other => panic!("bad tex: {other:?}"),
    }
}

#[test]
fn figures_are_reused_across_parses_and_bounded() {
    let (_dir, mut h) = open("note.md", NOTE);
    h.keys("<space>v");
    let cached = h.state.figures.entries.len();
    assert_eq!(cached, 4);
    let generation = h.state.figures.generation;
    // An edit away from the formulas reparses without typesetting anything again.
    h.typesetting = Typesetting::Hold;
    h.keys("ggA more<esc>");
    assert!(h.held.is_empty());
    assert_eq!(h.state.figures.entries.len(), cached);
    assert_eq!(h.state.figures.generation, generation);
    // Each round adds a formula and deletes it; the dead ones are evicted past SPARE.
    h.typesetting = Typesetting::Run;
    for round in 0..SPARE + 40 {
        h.keys(&format!("ggO$y_{{{round}}}$<esc>xd"));
    }
    let live = figures(&h).len();
    assert_eq!(live, 4);
    assert_eq!(h.state.figures.entries.len(), live + SPARE);
    for (source, _) in figures(&h) {
        assert!(
            !matches!(render(&h, &source), Render::Pending),
            "{source} is still cached"
        );
    }
    // Closing the preview leaves every figure unreferenced, still bounded.
    h.keys("<space>v");
    assert!(h.state.figures.entries.len() <= SPARE);
}

#[test]
fn typst_preview_recompiles_drops_stale_results_and_keeps_pages_on_errors() {
    let (dir, mut h) = open("main.typ", DOC);
    h.keys("<space>v");
    let (version, preview_state) = typst(&h);
    let compiled = preview_state.pages.as_ref().expect("compiled");
    assert_eq!(compiled.pages.len(), 2);
    assert_eq!(compiled.version, version);
    assert_eq!(preview_state.outcome, Outcome::Compiled);

    h.keys("Go= Three<esc>");
    let (version, preview_state) = typst(&h);
    assert_eq!(preview_state.pages.as_ref().unwrap().version, version);

    // A result for an older version is dropped and the current one compiles next.
    h.typesetting = Typesetting::Hold;
    h.keys("Ax<esc>");
    assert_eq!(h.held.len(), 1);
    h.keys("Ay<esc>");
    assert_eq!(h.held.len(), 1, "one compile at a time");
    assert_eq!(typst(&h).1.compile, Compile::Running);
    let shown = typst(&h).1.pages.as_ref().unwrap().version;
    h.release(0);
    assert_eq!(typst(&h).1.pages.as_ref().unwrap().version, shown);
    assert_eq!(h.held.len(), 1, "the current version compiles next");
    h.release(0);
    let (version, preview_state) = typst(&h);
    assert_eq!(preview_state.pages.as_ref().unwrap().version, version);
    assert_eq!(preview_state.compile, Compile::Idle);

    // An error keeps the last good pages and reports where it is.
    h.typesetting = Typesetting::Run;
    h.keys("Go#foo(<esc>");
    let (current, preview_state) = typst(&h);
    let errors = match &preview_state.outcome {
        Outcome::Failed(errors) => errors.clone(),
        other => panic!("expected errors, got {other:?}"),
    };
    assert_eq!(preview_state.pages.as_ref().unwrap().pages.len(), 2);
    assert!(preview_state.pages.as_ref().unwrap().version < current);
    let line = match &errors[0].origin {
        Origin::Span {
            file: SourceFile::Project(path),
            start,
            ..
        } => {
            assert_eq!(path, &dir.path().canonicalize().unwrap().join("main.typ"));
            start.line
        }
        other => panic!("error without a place: {other:?}"),
    };
    let typed = h
        .doc()
        .text
        .lines()
        .position(|text| text.to_string().starts_with("#foo"));
    assert_eq!(Some(line), typed, "{errors:?}");

    // Clicking the error strip puts the source cursor on it.
    h.keys("gg");
    let pane = preview(&h);
    h.run(|state, out| pages::goto_diagnostic(state, pane, out));
    let text = h.doc().text.slice(..);
    assert_eq!(h.view().selection.primary().cursor_line(text), line);

    // Fixing it clears the errors.
    h.keys("u");
    let (version, preview_state) = typst(&h);
    assert_eq!(preview_state.outcome, Outcome::Compiled);
    assert_eq!(preview_state.pages.as_ref().unwrap().version, version);
}

#[test]
fn export_pdf_writes_beside_the_typst_file() {
    let (dir, mut h) = open("main.typ", DOC);
    h.keys(":export-pdf<ret>");
    let pdf = fs::read(dir.path().join("main.pdf")).expect("pdf written");
    assert!(pdf.starts_with(b"%PDF"));
    assert!(matches!(h.state.notice, Some(Notice::Exported { .. })));

    h.keys("Go#foo(<esc>:export-pdf<ret>");
    assert!(matches!(
        h.state.notice,
        Some(Notice::TypstErrors { count: 1.., .. })
    ));

    h.keys(":o note.md<ret>:export-pdf<ret>");
    assert!(matches!(h.state.notice, Some(Notice::MarkdownExport)));
}

#[test]
fn saving_an_imported_file_recompiles_the_preview() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("lib.typ"), "#let word = [one]\n").unwrap();
    fs::write(
        dir.path().join("main.typ"),
        "#import \"lib.typ\": word\n#word\n",
    )
    .unwrap();
    let mut h = Harness::new(dir.path());
    h.keys(":o main.typ<ret><space>v");
    let lib = dir.path().canonicalize().unwrap().join("lib.typ");
    assert_eq!(typst(&h).1.deps, [lib.clone()]);
    h.typesetting = Typesetting::Hold;
    fs::write(&lib, "#let word = [two]\n").unwrap();
    h.changed(vec![lib]);
    assert_eq!(h.held.len(), 1, "the preview compiles again");
}

#[test]
fn a_following_preview_goes_to_the_page_under_the_cursor() {
    let (_dir, mut h) = open("main.typ", DOC);
    h.keys("<space>v");
    let pane = preview(&h);
    // Line 4 is "= Two", on the second page; the cursor sits before its text.
    h.keys("4gg");
    match h.state.layout.panes()[pane].active() {
        Some(View::Pages(view)) => match view.scroll {
            PagesScroll::Point(at) => assert_eq!(at.page, 1),
            other => panic!("not located: {other:?}"),
        },
        Some(View::Text(_) | View::Diff(_) | View::Document(_)) | None => panic!("not rendered"),
    }
}
