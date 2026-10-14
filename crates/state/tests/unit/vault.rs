use std::sync::Arc;

use state::backlinks::{self, OutLink};
use state::markdown::{LinkTarget, WikiLink};
use state::path::RelPath;
use state::vault::{self, Dest, Index, Resolution};

fn p(path: &str) -> RelPath {
    RelPath::parse(path).unwrap()
}

fn vault(files: &[&str]) -> Index {
    let mut files: Vec<RelPath> = files.iter().map(|file| p(file)).collect();
    files.sort_unstable();
    Index::new(1, Arc::new(files))
}

fn wiki(index: &Index, dir: &str, target: &str) -> Resolution {
    vault::resolve(index, dir, Dest::Wiki(target))
}

fn file(path: &str) -> Resolution {
    Resolution::File(p(path))
}

#[test]
fn names_resolve_anywhere_as_notes_first() {
    let index = vault(&["a/Note.md", "b/c/photo.png", "plan", "plan.md"]);
    assert_eq!(wiki(&index, "", "Note"), file("a/Note.md"));
    assert_eq!(wiki(&index, "x", "Note.md"), file("a/Note.md"));
    assert_eq!(wiki(&index, "", "photo.png"), file("b/c/photo.png"));
    // A note wins over an extensionless file of the same name.
    assert_eq!(wiki(&index, "", "plan"), file("plan.md"));
}

#[test]
fn case_is_ignored_but_an_exact_match_wins() {
    let index = vault(&["notes/Topic.md", "Archive/deep/topic.md"]);
    assert_eq!(wiki(&index, "", "TOPIC"), file("notes/Topic.md"));
    assert_eq!(wiki(&index, "", "topic"), file("Archive/deep/topic.md"));
}

#[test]
fn duplicates_prefer_the_shallowest_then_the_same_folder() {
    let index = vault(&["x/idea.md", "y/idea.md", "y/z/deeper/idea.md"]);
    assert_eq!(wiki(&index, "y", "idea"), file("y/idea.md"));
    assert_eq!(wiki(&index, "x", "idea"), file("x/idea.md"));
    // Neither is beside the note: the first by path.
    assert_eq!(wiki(&index, "q", "idea"), file("x/idea.md"));
    let index = vault(&["idea.md", "y/idea.md"]);
    assert_eq!(wiki(&index, "y", "idea"), file("idea.md"));
}

#[test]
fn folders_match_path_suffixes() {
    let index = vault(&["a/project/plan.md", "b/plan.md", "c/myproject/plan.md"]);
    assert_eq!(wiki(&index, "", "project/plan"), file("a/project/plan.md"));
    assert_eq!(
        wiki(&index, "", "a/project/plan"),
        file("a/project/plan.md")
    );
    assert_eq!(wiki(&index, "", "/b/plan"), file("b/plan.md"));
    assert_eq!(
        wiki(&index, "", "other/plan"),
        Resolution::Missing(p("other/plan.md"))
    );
}

#[test]
fn missing_notes_go_beside_the_linking_note() {
    let index = vault(&["notes/a.md"]);
    assert_eq!(
        wiki(&index, "notes", "New idea"),
        Resolution::Missing(p("notes/New idea.md"))
    );
    assert_eq!(
        wiki(&index, "", "draft.md"),
        Resolution::Missing(p("draft.md"))
    );
    assert_eq!(wiki(&index, "", "../escape"), Resolution::Invalid);
    assert_eq!(wiki(&index, "", "  "), Resolution::Invalid);
}

#[test]
fn markdown_paths_resolve_relative_then_root_then_by_name() {
    let index = vault(&["notes/sub/b.md", "notes/a.md", "top.md", "assets/img.png"]);
    let path = |dir: &str, target: &str| vault::resolve(&index, dir, Dest::Path(target));
    assert_eq!(path("notes", "sub/b.md"), file("notes/sub/b.md"));
    assert_eq!(path("notes/sub", "../a.md"), file("notes/a.md"));
    assert_eq!(path("notes/sub", "../a"), file("notes/a.md"));
    assert_eq!(path("notes", "/top.md"), file("top.md"));
    assert_eq!(path("notes", "top.md"), file("top.md"));
    assert_eq!(path("notes", "img.png"), file("assets/img.png"));
    assert_eq!(path("notes", "new"), Resolution::Missing(p("notes/new.md")));
    assert_eq!(path("", "../out.md"), Resolution::Invalid);
}

#[test]
fn wikilink_parts_split_heading_and_alias() {
    let link = |line: &str, at: usize| vault::link_at(line, at);
    assert_eq!(
        link("see [[Note#Part|shown]] here", 8),
        Some(LinkTarget::Wiki(WikiLink {
            target: "Note".into(),
            heading: Some("Part".into()),
            alias: Some("shown".into()),
        }))
    );
    assert_eq!(
        link("![[pic.png]]", 0),
        Some(LinkTarget::Wiki(WikiLink {
            target: "pic.png".into(),
            heading: None,
            alias: None,
        }))
    );
    assert_eq!(
        link("a [text](dir/b.md#Some%20Head) z", 4),
        Some(LinkTarget::Path {
            path: "dir/b.md".into(),
            heading: Some("Some Head".into()),
        })
    );
    assert_eq!(
        link("go to https://example.org/x. now", 12),
        Some(LinkTarget::Url("https://example.org/x".into()))
    );
    assert_eq!(
        link("<https://a.org>", 3),
        Some(LinkTarget::Url("https://a.org".into()))
    );
    assert_eq!(link("[[a]] plain words", 9), None);
}

#[test]
fn headings_are_found_by_scan_outside_fences() {
    let text = "# Top\n```\n## Inner\n```\n## Inner\ntext\n### Last One ##\n";
    let doc = state::buffer::TextDoc::new(
        helix_core::Rope::from(text),
        std::path::Path::new("n.md"),
        &helix_core::config::default_lang_loader(),
    );
    assert_eq!(vault::heading_line(&doc, None, "inner"), Some(4));
    assert_eq!(vault::heading_line(&doc, None, "last-one"), Some(6));
    assert_eq!(vault::heading_line(&doc, None, "Top#Last One"), Some(6));
    assert_eq!(vault::heading_line(&doc, None, "nowhere"), None);
}

#[test]
fn extraction_finds_links_with_their_lines() {
    let mut links: Vec<OutLink> = Vec::new();
    backlinks::extract(
        "---\nlink: \"[[not a link]]\"\n---\n# Title\n\nSee [[Other|o]] and [x](sub/y.md#h).\n\n```\n[[code]]\n```\n\n- ![[pic.png]] and <https://u.rl> and [h](#here)\n",
        &mut links,
    );
    let found: Vec<(Dest<&str>, u32, u32)> = links
        .iter()
        .map(|link| (link.dest.as_str(), link.line, link.column))
        .collect();
    assert_eq!(
        found,
        [
            (Dest::Wiki("Other"), 5, 4),
            (Dest::Path("sub/y.md"), 5, 20),
            (Dest::Wiki("pic.png"), 11, 2),
        ]
    );
    assert_eq!(links[0].text, "See [[Other|o]] and [x](sub/y.md#h).");
    assert_eq!(links[1].key, vault::link_key("y.md"));
    assert_eq!(links[0].key, vault::link_key("Other.md"));
}

#[test]
fn unresolved_links_parse_as_missing() {
    use state::markdown::{self, Base, BlockKind, EmbedKind, Emphasis, ImageFile};
    let index = vault(&["notes/here.md", "assets/pic.png"]);
    let base = Base {
        dir: std::path::Path::new("/nonexistent/notes"),
        root: std::path::Path::new("/nonexistent"),
        folder: "notes",
        vault: &index,
    };
    let source =
        helix_core::Rope::from("[[here]][[gone]] [x](here.md)\n\n![[pic.png]]\n\n![[here]]\n");
    let parsed = markdown::parse(source.slice(..), base, None);
    let BlockKind::Paragraph(text) = &parsed.blocks[0].kind else {
        panic!("not a paragraph");
    };
    let missing: Vec<(&str, bool)> = text
        .spans
        .iter()
        .map(|span| {
            (
                &text.text[span.range.clone()],
                span.style.contains(Emphasis::MISSING),
            )
        })
        .collect();
    assert_eq!(
        missing,
        [("here", false), ("gone", true), (" ", false), ("x", false)]
    );
    let BlockKind::Embed(image) = &parsed.blocks[1].kind else {
        panic!("not an embed");
    };
    assert_eq!(
        image.kind,
        EmbedKind::Image(ImageFile::Local("/nonexistent/assets/pic.png".into()))
    );
    let BlockKind::Embed(note) = &parsed.blocks[2].kind else {
        panic!("not an embed");
    };
    assert_eq!(note.kind, EmbedKind::Note(file("notes/here.md")));
}
