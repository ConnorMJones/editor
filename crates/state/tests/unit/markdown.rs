use std::path::Path;

use helix_core::Rope;
use model::markdown::CalloutKind;
use model::typeset::Display;
use state::project::links::{Index, Resolution};
use state::project::path::RelPath;
use state::render::markdown::{
    self, Base, BlockKind, CalloutFold, Column, EmbedKind, Emphasis, FigureKind, Format, ImageFile,
    Level, LinkTarget, ListKind, Markdown, RowKind, SpanKind, Task, WikiLink,
};

fn parse(text: &str) -> Markdown {
    let vault = Index::new(1, Default::default());
    let base = Base {
        dir: Path::new("/nonexistent/notes"),
        root: Path::new("/nonexistent"),
        folder: "notes",
        vault: &vault,
    };
    markdown::parse(Rope::from(text).slice(..), base, None)
}

/// Kinds of the top-level blocks, by a short name.
fn roots(markdown: &Markdown) -> Vec<&'static str> {
    markdown
        .roots
        .iter()
        .map(|&root| name(&markdown.blocks[root].kind))
        .collect()
}

fn name(kind: &BlockKind) -> &'static str {
    match kind {
        BlockKind::Heading { .. } => "heading",
        BlockKind::Paragraph(_) => "paragraph",
        BlockKind::List(_) => "list",
        BlockKind::Item(_) => "item",
        BlockKind::Quote => "quote",
        BlockKind::Callout(_) => "callout",
        BlockKind::Code(_) => "code",
        BlockKind::Math(_) => "math",
        BlockKind::Mermaid(_) => "mermaid",
        BlockKind::Table(_) => "table",
        BlockKind::Row(_) => "row",
        BlockKind::Cell(_) => "cell",
        BlockKind::Image(_) => "image",
        BlockKind::Embed(_) => "embed",
        BlockKind::Rule => "rule",
        BlockKind::Html(_) => "html",
        BlockKind::Footnote { .. } => "footnote",
        BlockKind::Frontmatter(_) => "frontmatter",
    }
}

fn text(kind: &BlockKind) -> &str {
    match kind {
        BlockKind::Heading { text, .. } | BlockKind::Paragraph(text) | BlockKind::Cell(text) => {
            &text.text
        }
        other => panic!("{} has no text", name(other)),
    }
}

const NOTE: &str = "\
---
tags: [a]
---
# Title

Some *italic*, **bold**, ~~gone~~ and `code`.

- [ ] open task
- [x] done task
- plain

1. first
2. second

> a quote

> [!warning]- Careful now
> folded body

```rust
fn main() {}
```

```mermaid
graph TD; A-->B
```

$$
x^2
$$

| a | b |
|:--|--:|
| 1 | 2 |

![alt text](img.png)

![[pic.png]]

---

<div>raw</div>

Note[^1].

[^1]: The footnote.
";

#[test]
fn parses_every_block_kind() {
    let parsed = parse(NOTE);
    assert_eq!(
        roots(&parsed),
        [
            "frontmatter",
            "heading",
            "paragraph",
            "list",
            "list",
            "quote",
            "callout",
            "code",
            "mermaid",
            "math",
            "table",
            "image",
            "embed",
            "rule",
            "html",
            "paragraph",
            "footnote",
        ]
    );
}

#[test]
fn blocks_keep_source_lines() {
    let parsed = parse(NOTE);
    let lines: Vec<(&str, std::ops::Range<usize>)> = parsed
        .roots
        .iter()
        .map(|root| {
            let block = &parsed.blocks[*root];
            (name(&block.kind), block.lines.clone())
        })
        .collect();
    assert_eq!(lines[0], ("frontmatter", 0..3));
    assert_eq!(lines[1], ("heading", 3..4));
    assert_eq!(lines[2], ("paragraph", 5..6));
    assert_eq!(lines[3], ("list", 7..10));
    assert_eq!(lines[6], ("callout", 16..18));
    assert_eq!(lines[7], ("code", 19..22));
    assert_eq!(lines[9], ("math", 27..30));
    // Mapping back: every line of a block finds it.
    assert_eq!(parsed.item_at_line(20), 7);
    assert_eq!(
        parsed.item_at_line(4),
        1,
        "a blank line belongs to the block before"
    );
    assert_eq!(parsed.item_line(7), Some(19));
    assert_eq!(parsed.item_at_line(0), 0);
}

#[test]
fn inline_styles_are_typed_spans() {
    let parsed = parse("Some *italic*, **bold**, ~~gone~~ and `code`.\n");
    let BlockKind::Paragraph(inline) = &parsed.blocks[0].kind else {
        panic!("not a paragraph");
    };
    let spans: Vec<(&str, Emphasis, SpanKind)> = inline
        .spans
        .iter()
        .map(|span| (&inline.text[span.range.clone()], span.style, span.kind))
        .collect();
    assert_eq!(
        spans,
        [
            ("Some ", Emphasis::NONE, SpanKind::Text),
            ("italic", Emphasis::ITALIC, SpanKind::Text),
            (", ", Emphasis::NONE, SpanKind::Text),
            ("bold", Emphasis::STRONG, SpanKind::Text),
            (", ", Emphasis::NONE, SpanKind::Text),
            ("gone", Emphasis::STRIKE, SpanKind::Text),
            (" and ", Emphasis::NONE, SpanKind::Text),
            ("code", Emphasis::NONE, SpanKind::Code),
            (".", Emphasis::NONE, SpanKind::Text),
        ]
    );
}

#[test]
fn headings_have_levels() {
    let parsed = parse("# One\n\n###### Six {#id}\n");
    let levels: Vec<(Level, &str)> = parsed
        .blocks
        .iter()
        .map(|block| match &block.kind {
            BlockKind::Heading { level, text } => (*level, &*text.text),
            other => panic!("unexpected {}", name(other)),
        })
        .collect();
    assert_eq!(levels, [(Level::H1, "One"), (Level::H6, "Six")]);
}

#[test]
fn tasks_record_their_marker() {
    let source = "- [ ] open\n- [x] done\n- plain\n";
    let parsed = parse(source);
    let tasks: Vec<Task> = parsed
        .blocks
        .iter()
        .filter_map(|block| match block.kind {
            BlockKind::Item(task) => Some(task),
            _ => None,
        })
        .collect();
    assert_eq!(
        tasks,
        [
            Task::Open { marker: 2 },
            Task::Done { marker: 13 },
            Task::None
        ]
    );
    assert_eq!(&source[2..5], "[ ]");
    assert_eq!(&source[13..16], "[x]");
    // Tight items hold their text as a paragraph.
    let item = parsed
        .blocks
        .iter()
        .position(|block| matches!(block.kind, BlockKind::Item(_)))
        .unwrap();
    let children: Vec<&str> = parsed
        .children(item)
        .map(|child| text(&parsed.blocks[child].kind))
        .collect();
    assert_eq!(children, ["open"]);
}

#[test]
fn ordered_lists_keep_their_start() {
    let parsed = parse("3. three\n4. four\n");
    assert_eq!(
        parsed.blocks[0].kind,
        BlockKind::List(ListKind::Ordered { start: 3 })
    );
    assert_eq!(parsed.children(0).count(), 2);
}

#[test]
fn nested_lists_nest() {
    let parsed = parse("- a\n  - b\n- c\n");
    let outer: Vec<&str> = parsed
        .children(0)
        .map(|item| name(&parsed.blocks[item].kind))
        .collect();
    assert_eq!(outer, ["item", "item"]);
    let first: Vec<&str> = parsed
        .children(1)
        .map(|child| name(&parsed.blocks[child].kind))
        .collect();
    assert_eq!(first, ["paragraph", "list"]);
}

fn callout(markdown: &Markdown) -> (&markdown::Callout, Vec<&str>) {
    let index = markdown
        .blocks
        .iter()
        .position(|block| matches!(block.kind, BlockKind::Callout(_)))
        .expect("a callout");
    let BlockKind::Callout(callout) = &markdown.blocks[index].kind else {
        unreachable!()
    };
    let body = markdown
        .children(index)
        .map(|child| text(&markdown.blocks[child].kind))
        .collect();
    (callout, body)
}

#[test]
fn callouts_read_kind_fold_and_title() {
    let parsed = parse("> [!warning]- Careful now\n> folded body\n");
    let (header, body) = callout(&parsed);
    assert_eq!(header.kind, CalloutKind::Warning);
    assert_eq!(header.fold, CalloutFold::Closed);
    assert_eq!(&*header.title, "Careful now");
    assert_eq!(body, ["folded body"]);

    let parsed = parse("> [!TIP]+\n> open body\n>\n> second\n");
    let (header, body) = callout(&parsed);
    assert_eq!(header.kind, CalloutKind::Tip);
    assert_eq!(header.fold, CalloutFold::Open);
    assert_eq!(&*header.title, "Tip");
    assert_eq!(body, ["open body", "second"]);

    let parsed = parse("> [!note]\n> plain\n");
    let (header, _) = callout(&parsed);
    assert_eq!(
        (header.kind, header.fold, &*header.title),
        (CalloutKind::Note, CalloutFold::Static, "Note")
    );

    let parsed = parse("> [!custom] Mine\n> body\n");
    assert_eq!(callout(&parsed).0.kind, CalloutKind::Note);

    let parsed = parse("> [!faq]\n> body\n");
    assert_eq!(callout(&parsed).0.kind, CalloutKind::Question);
}

#[test]
fn quotes_without_a_marker_stay_quotes() {
    let parsed = parse("> just a quote\n\n> > [!tip] inner\n> > body\n");
    let kinds: Vec<&str> = parsed
        .blocks
        .iter()
        .map(|block| name(&block.kind))
        .collect();
    assert_eq!(
        kinds,
        ["quote", "paragraph", "quote", "callout", "paragraph"]
    );
}

#[test]
fn callouts_are_numbered_in_order() {
    let parsed = parse("> [!a]\n> x\n\n> [!b]\n> y\n");
    let indices: Vec<u32> = parsed
        .blocks
        .iter()
        .filter_map(|block| match &block.kind {
            BlockKind::Callout(callout) => Some(callout.index),
            _ => None,
        })
        .collect();
    assert_eq!(indices, [0, 1]);
}

fn links(markdown: &Markdown) -> Vec<(String, LinkTarget)> {
    markdown
        .blocks
        .iter()
        .filter_map(|block| block.inline())
        .flat_map(|inline| {
            inline.links.iter().map(|link| {
                (
                    inline.text[link.range.clone()].to_owned(),
                    link.target.clone(),
                )
            })
        })
        .collect()
}

#[test]
fn wikilinks_parse_target_heading_and_alias() {
    let parsed = parse("[[a]] [[a|b]] [[a#h]] [[a#h|c]] [[#only]]\n");
    let wiki = |target: &str, heading: Option<&str>, alias: Option<&str>| {
        LinkTarget::Wiki(WikiLink {
            target: target.to_owned(),
            heading: heading.map(str::to_owned),
            alias: alias.map(str::to_owned),
        })
    };
    assert_eq!(
        links(&parsed),
        [
            ("a".to_owned(), wiki("a", None, None)),
            ("b".to_owned(), wiki("a", None, Some("b"))),
            ("a#h".to_owned(), wiki("a", Some("h"), Some("a#h"))),
            ("c".to_owned(), wiki("a", Some("h"), Some("c"))),
            ("#only".to_owned(), wiki("", Some("only"), Some("#only"))),
        ]
    );
}

#[test]
fn markdown_links_are_typed() {
    let parsed = parse(
        "[web](https://example.com) [mail](mailto:a@b.c) [file](sub/My%20Note.md#Part) \
         [here](#Intro) <https://auto.link>\n",
    );
    assert_eq!(
        links(&parsed),
        [
            (
                "web".to_owned(),
                LinkTarget::Url("https://example.com".to_owned())
            ),
            (
                "mail".to_owned(),
                LinkTarget::Url("mailto:a@b.c".to_owned())
            ),
            (
                "file".to_owned(),
                LinkTarget::Path {
                    path: "sub/My Note.md".to_owned(),
                    heading: Some("Part".to_owned())
                }
            ),
            ("here".to_owned(), LinkTarget::Heading("Intro".to_owned())),
            (
                "https://auto.link".to_owned(),
                LinkTarget::Url("https://auto.link".to_owned())
            ),
        ]
    );
    let BlockKind::Paragraph(inline) = &parsed.blocks[0].kind else {
        panic!("not a paragraph");
    };
    assert!(inline.spans[0].style.contains(Emphasis::LINK));
}

#[test]
fn embeds_and_images_lift_to_blocks() {
    let parsed =
        parse("![[pic.png]]\n\n![[Other note#Part|shown]]\n\n![alt](https://x.org/a.png)\n");
    let kinds: Vec<&BlockKind> = parsed.blocks.iter().map(|block| &block.kind).collect();
    match kinds.as_slice() {
        [
            BlockKind::Embed(image),
            BlockKind::Embed(note),
            BlockKind::Image(remote),
        ] => {
            assert_eq!(image.link.target, "pic.png");
            assert_eq!(image.kind, EmbedKind::Image(ImageFile::Missing));
            assert_eq!(note.link.target, "Other note");
            assert_eq!(note.link.heading.as_deref(), Some("Part"));
            assert_eq!(
                note.kind,
                EmbedKind::Note(Resolution::Missing(
                    RelPath::parse("notes/Other note.md").unwrap()
                ))
            );
            assert_eq!(&*remote.alt, "alt");
            assert_eq!(remote.file, ImageFile::Remote("https://x.org/a.png".into()));
        }
        other => panic!("unexpected blocks {other:?}"),
    }
}

#[test]
fn images_resolve_against_the_note_then_the_root() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir(dir.path().join("notes")).unwrap();
    std::fs::write(dir.path().join("notes/near.png"), b"png").unwrap();
    std::fs::write(dir.path().join("far.png"), b"png").unwrap();
    let vault = Index::new(1, Default::default());
    let base = Base {
        dir: &dir.path().join("notes"),
        root: dir.path(),
        folder: "notes",
        vault: &vault,
    };
    let source = Rope::from("![](near.png)\n\n![](far.png)\n\n![](none.png)\n");
    let parsed = markdown::parse(source.slice(..), base, None);
    let files: Vec<&ImageFile> = parsed
        .blocks
        .iter()
        .map(|block| match &block.kind {
            BlockKind::Image(image) => &image.file,
            other => panic!("unexpected {}", name(other)),
        })
        .collect();
    assert_eq!(
        files,
        [
            &ImageFile::Local(dir.path().join("notes/near.png")),
            &ImageFile::Local(dir.path().join("far.png")),
            &ImageFile::Missing,
        ]
    );
}

#[test]
fn images_inside_text_stay_inline() {
    let parsed = parse("see ![icon](i.png) here\n");
    let BlockKind::Paragraph(inline) = &parsed.blocks[0].kind else {
        panic!("not a paragraph");
    };
    assert!(inline.spans.iter().any(|span| span.kind == SpanKind::Image));
    assert_eq!(&*inline.text, "see icon here");
}

#[test]
fn math_inline_and_display() {
    let parsed = parse("Inline $a+b$ here.\n\n$$\n\\int x\n$$\n\n```math\ny\n```\n");
    let BlockKind::Paragraph(inline) = &parsed.blocks[0].kind else {
        panic!("not a paragraph");
    };
    let math: Vec<(&str, SpanKind)> = inline
        .spans
        .iter()
        .filter(|span| matches!(span.kind, SpanKind::Math(_)))
        .map(|span| (&inline.text[span.range.clone()], span.kind))
        .collect();
    assert!(matches!(
        math.as_slice(),
        [("a+b", SpanKind::Math(figure))] if figure.kind == FigureKind::Math(Display::Inline)
    ));
    let blocks: Vec<(&str, Display)> = parsed.blocks[1..]
        .iter()
        .map(|block| match &block.kind {
            BlockKind::Math(math) => (math.tex.trim(), math.display),
            other => panic!("unexpected {}", name(other)),
        })
        .collect();
    assert_eq!(blocks, [("\\int x", Display::Block), ("y", Display::Block)]);
}

#[test]
fn mermaid_and_code_fences() {
    let parsed =
        parse("```mermaid\ngraph TD; A-->B\n```\n\n```rust\nfn a() {}\n```\n\n    indented\n");
    match &parsed.blocks[0].kind {
        BlockKind::Mermaid(mermaid) => {
            assert_eq!(&*mermaid.src, "graph TD; A-->B");
            assert_eq!(mermaid.figure.kind, FigureKind::Mermaid);
        }
        other => panic!("unexpected {}", name(other)),
    }
    match (&parsed.blocks[1].kind, &parsed.blocks[2].kind) {
        (BlockKind::Code(fenced), BlockKind::Code(indented)) => {
            assert_eq!(fenced.lang.as_deref(), Some("rust"));
            assert_eq!(&*fenced.text, "fn a() {}");
            assert_eq!(indented.lang, None);
            assert_eq!(&*indented.text, "indented");
        }
        (first, second) => panic!("unexpected {} {}", name(first), name(second)),
    }
}

#[test]
fn tables_keep_alignment_rows_and_cells() {
    let parsed = parse("| a | b | c |\n|:--|:-:|--:|\n| 1 | 2 | 3 |\n| 4 | 5 | 6 |\n");
    assert_eq!(
        parsed.blocks[0].kind,
        BlockKind::Table([Column::Left, Column::Center, Column::Right].into())
    );
    let rows: Vec<(RowKind, Vec<&str>)> = parsed
        .children(0)
        .map(|row| {
            let BlockKind::Row(kind) = parsed.blocks[row].kind else {
                panic!("not a row");
            };
            let cells = parsed
                .children(row)
                .map(|cell| text(&parsed.blocks[cell].kind))
                .collect();
            (kind, cells)
        })
        .collect();
    assert_eq!(
        rows,
        [
            (RowKind::Head, vec!["a", "b", "c"]),
            (RowKind::Body, vec!["1", "2", "3"]),
            (RowKind::Body, vec!["4", "5", "6"]),
        ]
    );
    assert_eq!(parsed.blocks[0].lines, 0..4);
}

#[test]
fn reparse_keeps_highlights_of_unchanged_code() {
    let loader = helix_core::config::default_lang_loader();
    loader.set_scopes(
        model::theme::SCOPES
            .iter()
            .map(|(scope, _)| (*scope).to_owned())
            .collect(),
    );
    let vault = Index::new(1, Default::default());
    let base = Base {
        dir: Path::new("/"),
        root: Path::new("/"),
        folder: "",
        vault: &vault,
    };
    let first = Rope::from("```rust\nfn a() {}\n```\n");
    let mut parsed = markdown::parse(first.slice(..), base, None);
    markdown::highlight(&mut parsed, &loader);
    let BlockKind::Code(code) = &parsed.blocks[0].kind else {
        panic!("not code");
    };
    let hash = code.hash;
    let runs = parsed.highlights[&hash].clone();
    assert!(runs.len() > 1, "rust is highlighted: {runs:?}");
    assert_eq!(runs.last().unwrap().end, "fn a() {}".len());

    let second = Rope::from("Intro\n\n```rust\nfn a() {}\n```\n\n```nosuchlang\nx\n```\n");
    let mut parsed = markdown::parse(second.slice(..), base, Some(parsed));
    assert_eq!(parsed.highlights.get(&hash), Some(&runs), "carried over");
    markdown::highlight(&mut parsed, &loader);
    assert_eq!(parsed.highlights.len(), 2);

    let third = Rope::from("no code\n");
    let parsed = markdown::parse(third.slice(..), base, Some(parsed));
    assert!(parsed.highlights.is_empty(), "dropped with their blocks");
}

#[test]
fn root_hashes_follow_content() {
    let a = parse("# A\n\npara\n");
    let b = parse("# A\n\npara changed\n");
    assert_eq!(a.hashes[0], b.hashes[0]);
    assert_ne!(a.hashes[1], b.hashes[1]);
}

#[test]
fn formats_by_extension() {
    assert_eq!(Format::of(Path::new("a.md")), Some(Format::Markdown));
    assert_eq!(Format::of(Path::new("a.markdown")), Some(Format::Markdown));
    assert_eq!(Format::of(Path::new("a.typ")), Some(Format::Typst));
    assert_eq!(Format::of(Path::new("a.rs")), None);
    assert_eq!(Format::of(Path::new("README")), None);
}
