use std::path::{Path, PathBuf};

use model::color::{Palette, Rgb};
use typeset::{Display, Pt};

use crate::{assert_valid, typesetter};

const WHITE: Rgb = Rgb {
    r: 0xff,
    g: 0xff,
    b: 0xff,
};

fn examples() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples")
}

fn markdown_files(dir: &Path, found: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).expect("examples dir") {
        let path = entry.expect("dir entry").path();
        match (path.is_dir(), path.extension().and_then(|ext| ext.to_str())) {
            (true, _) => markdown_files(&path, found),
            (false, Some("md")) => found.push(path),
            (false, _) => {}
        }
    }
}

/// Every markdown file under examples, with its text.
fn notes() -> Vec<(PathBuf, String)> {
    let mut found = Vec::new();
    markdown_files(&examples(), &mut found);
    assert!(!found.is_empty(), "no markdown examples");
    found
        .into_iter()
        .map(|path| {
            let text = std::fs::read_to_string(&path).expect("read note");
            (path, text)
        })
        .collect()
}

/// Text outside fenced code blocks, and the bodies of the mermaid fences.
fn split(text: &str) -> (String, Vec<String>) {
    enum Fence {
        Outside,
        Code,
        Mermaid(String),
    }
    let mut prose = String::new();
    let mut mermaid = Vec::new();
    let mut fence = Fence::Outside;
    for line in text.lines() {
        let marker = line.trim_start().starts_with("```");
        fence = match (fence, marker) {
            (Fence::Outside, true) => match line.trim_start().trim_start_matches('`') {
                "mermaid" => Fence::Mermaid(String::new()),
                _ => Fence::Code,
            },
            (Fence::Outside, false) => {
                prose.push_str(line);
                prose.push('\n');
                Fence::Outside
            }
            (Fence::Code, true) => Fence::Outside,
            (Fence::Code, false) => Fence::Code,
            (Fence::Mermaid(body), true) => {
                mermaid.push(body);
                Fence::Outside
            }
            (Fence::Mermaid(mut body), false) => {
                body.push_str(line);
                body.push('\n');
                Fence::Mermaid(body)
            }
        };
    }
    (prose, mermaid)
}

/// `$$...$$` and `$...$` snippets in prose, ignoring inline code spans.
fn formulas(prose: &str) -> Vec<(String, Display)> {
    let mut found = Vec::new();
    let mut rest = prose;
    while let Some(start) = rest.find(['$', '`']) {
        let after = &rest[start..];
        let (delimiter, display) = match after.as_bytes() {
            [b'`', ..] => ("`", None),
            [b'$', b'$', ..] => ("$$", Some(Display::Block)),
            _ => ("$", Some(Display::Inline)),
        };
        let body = &after[delimiter.len()..];
        let end = body
            .find(delimiter)
            .unwrap_or_else(|| panic!("unclosed {delimiter} in {after}"));
        match display {
            Some(display) => found.push((body[..end].trim().to_owned(), display)),
            None => {}
        }
        rest = &body[end + delimiter.len()..];
    }
    found
}

#[test]
fn typst_example_compiles_to_several_pages() {
    let mut typesetter = typesetter();
    let root = examples().join("typst");
    let main = root.join("main.typ");
    let source = std::fs::read_to_string(&main).expect("read main.typ");
    let pages = typesetter
        .typst_pages(&root, &main, &source)
        .result
        .unwrap_or_else(|error| panic!("{error}: {error:?}"));
    assert!(pages.pages.len() > 1, "{} pages", pages.pages.len());
    assert!(pages.warnings.is_empty(), "{:?}", pages.warnings);
    for page in &pages.pages {
        assert_valid(page);
    }
}

#[test]
fn markdown_math_renders() {
    let mut typesetter = typesetter();
    let mut count = 0;
    for (path, text) in notes() {
        for (tex, display) in formulas(&split(&text).0) {
            let svg = typesetter
                .math(&tex, display, WHITE, Pt(16.0))
                .unwrap_or_else(|error| panic!("{}: {tex}: {error}", path.display()));
            assert_valid(&svg);
            count += 1;
        }
    }
    assert!(count > 10, "only {count} formulas found");
}

#[test]
fn markdown_mermaid_renders() {
    let palette: Palette =
        serde_json::from_str(include_str!("../../../model/tests/fixtures/palette.json"))
            .expect("valid palette");
    let mut typesetter = typesetter();
    let mut count = 0;
    for (path, text) in notes() {
        for diagram in split(&text).1 {
            let svg = typesetter
                .mermaid(&diagram, &palette)
                .unwrap_or_else(|error| panic!("{}: {diagram}: {error}", path.display()));
            assert_valid(&svg);
            count += 1;
        }
    }
    assert_eq!(count, 2, "flowchart and sequence diagram");
}
