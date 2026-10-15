use std::fs;
use std::path::Path;

use helix_core::Selection;
use model::notice::Notice;
use state::input::keymap::Context;
use state::input::picker::{Hit, Items, Search};
use state::project::backlinks::Build;
use state::project::path::RelPath;
use state::render::markdown::{BlockKind, EmbedKind, ImageFile};
use state::render::rendered::Scroll;
use state::text::buffer::Content;
use state::view::layout::View;
use state::{Confirm, Overlay, input, render::rendered};

use crate::harness::Harness;

const A: &str = "\
# A
see [[b]] and [c](sub/c.md#Part) and https://x.org/p
then [[New note]] and [[b#Second]]
";

const C: &str = "\
# Intro

text

## Part
";

fn vault() -> (tempfile::TempDir, Harness) {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    fs::create_dir_all(root.join("notes/sub")).unwrap();
    fs::create_dir_all(root.join("assets")).unwrap();
    fs::write(root.join("notes/a.md"), A).unwrap();
    fs::write(root.join("notes/b.md"), "# B\n\nbody\n\n## Second\n").unwrap();
    fs::write(root.join("notes/sub/c.md"), C).unwrap();
    fs::write(root.join("assets/pic.png"), b"png").unwrap();
    let h = Harness::new(root);
    (dir, h)
}

/// Opens `name` with the cursor on the first `needle` in it.
fn open_at(h: &mut Harness, name: &str, needle: &str) {
    h.keys(&format!(":o {name}<ret>"));
    let text = h.doc().text.to_string();
    let at = text
        .find(needle)
        .unwrap_or_else(|| panic!("{needle:?} not in {name}"));
    let char = h.doc().text.byte_to_char(at);
    h.view_mut().selection = Selection::point(char);
}

fn focused(h: &Harness) -> String {
    let path = h.focused_path().unwrap();
    path.strip_prefix(h.state.root.path())
        .unwrap()
        .to_string_lossy()
        .into_owned()
}

fn cursor_line(h: &Harness) -> usize {
    let text = h.doc().text.slice(..);
    h.view().selection.primary().cursor_line(text)
}

fn backlinks(h: &Harness) -> Vec<(String, u32)> {
    match &h.state.overlay {
        Overlay::Picker(picker) => match &picker.items {
            Items::Backlinks(referrers) => {
                assert_eq!(referrers.search, Search::Done);
                picker
                    .matches
                    .iter()
                    .map(|&index| {
                        let Hit { path, line, .. } = &referrers.hits[index as usize];
                        (path.as_str().to_owned(), *line)
                    })
                    .collect()
            }
            Items::Files(_) | Items::Buffers(_) | Items::Grep(_) | Items::Commands => {
                panic!("wrong picker")
            }
        },
        Overlay::None | Overlay::Prompt(_) => panic!("picker not open"),
    }
}

#[test]
fn gf_follows_wikilinks_headings_and_urls() {
    let (_dir, mut h) = vault();
    open_at(&mut h, "notes/a.md", "[[b]]");
    h.keys("gf");
    assert_eq!(focused(&h), "notes/b.md");
    // A markdown link's heading puts the cursor on it once the note loads.
    open_at(&mut h, "notes/a.md", "c.md#");
    h.keys("gf");
    assert_eq!(focused(&h), "notes/sub/c.md");
    assert_eq!(cursor_line(&h), 4);
    // A wikilink heading in an open note moves the cursor at once.
    open_at(&mut h, "notes/a.md", "b#Second");
    h.keys("gf");
    assert_eq!(focused(&h), "notes/b.md");
    assert_eq!(cursor_line(&h), 4);
    open_at(&mut h, "notes/a.md", "x.org");
    h.keys("gf");
    assert_eq!(h.urls, ["https://x.org/p"]);
    assert_eq!(focused(&h), "notes/a.md");
}

#[test]
fn gf_opens_paths_in_other_files() {
    let (dir, mut h) = vault();
    fs::create_dir_all(dir.path().join("code/cfg")).unwrap();
    fs::write(dir.path().join("code/cfg/settings.toml"), "x = 1\n").unwrap();
    fs::write(dir.path().join("README.txt"), "readme\n").unwrap();
    fs::write(
        dir.path().join("code/main.rs"),
        "load(\"cfg/settings.toml\");\n// see README.txt and nowhere.txt\n",
    )
    .unwrap();
    open_at(&mut h, "code/main.rs", "settings");
    h.keys("gf");
    assert_eq!(focused(&h), "code/cfg/settings.toml");
    open_at(&mut h, "code/main.rs", "README");
    h.keys("gf");
    assert_eq!(focused(&h), "README.txt");
    open_at(&mut h, "code/main.rs", "nowhere");
    h.keys("gf");
    assert!(matches!(h.state.notice, Some(Notice::LinkNotFound { .. })));
    assert_eq!(focused(&h), "code/main.rs");
}

#[test]
fn unresolved_links_create_notes_on_yes() {
    let (dir, mut h) = vault();
    open_at(&mut h, "notes/a.md", "New note");
    h.keys("gf");
    let path = RelPath::parse("notes/New note.md").unwrap();
    assert_eq!(h.state.confirm, Some(Confirm::Create(path.clone())));
    assert_eq!(input::context(&h.state), Context::Confirm);
    h.keys("n");
    assert_eq!(h.state.confirm, None);
    assert!(matches!(h.state.notice, Some(Notice::CreateNotConfirmed)));
    assert!(!dir.path().join("notes/New note.md").exists());
    h.keys("gf");
    assert_eq!(h.state.confirm, Some(Confirm::Create(path)));
    h.keys("y");
    assert_eq!(h.state.confirm, None);
    assert_eq!(
        fs::read_to_string(dir.path().join("notes/New note.md")).unwrap(),
        ""
    );
    assert_eq!(focused(&h), "notes/New note.md");
}

#[test]
fn rendered_views_follow_into_rendered_notes_at_headings() {
    let (_dir, mut h) = vault();
    open_at(&mut h, "notes/a.md", "[[b]]");
    h.keys("<space>V");
    let buffer = h.view().buffer();
    let pane = h.state.layout.focus;
    let target = state::render::markdown::LinkTarget::Path {
        path: "sub/c.md".into(),
        heading: Some("Part".into()),
    };
    h.run(|state, out| {
        state::project::links::follow(
            state,
            buffer,
            &target,
            state::project::links::Origin::Rendered(pane),
            out,
        )
    });
    assert_eq!(focused(&h), "notes/sub/c.md");
    match h.state.layout.panes[pane].active() {
        Some(View::Rendered(view)) => {
            assert_eq!(
                view.scroll,
                Scroll::item(2),
                "the Part heading is the third block"
            );
            assert_eq!(cursor_line(&h), 4);
        }
        Some(View::Text(_) | View::Diff(_)) | None => panic!("not rendered"),
    }
}

#[test]
fn embedded_images_resolve_from_other_folders() {
    let (dir, mut h) = vault();
    fs::write(
        dir.path().join("notes/gallery.md"),
        "![[pic.png]]\n\n![[sub/c]]\n",
    )
    .unwrap();
    h.changed(vec![dir.path().join("notes/gallery.md")]);
    h.keys(":o notes/gallery.md<ret><space>V");
    let Content::Text { doc, .. } = &h.state.buffers[h.view().buffer()].content else {
        panic!("loading");
    };
    let parsed = rendered::markdown(&h.state.previews, h.view().buffer(), doc).expect("parsed");
    let kinds: Vec<&BlockKind> = parsed.blocks.iter().map(|block| &block.kind).collect();
    match kinds.as_slice() {
        [BlockKind::Embed(image), BlockKind::Embed(note)] => {
            assert_eq!(
                image.kind,
                EmbedKind::Image(ImageFile::Local(h.state.root.path().join("assets/pic.png")))
            );
            assert_eq!(
                note.kind,
                EmbedKind::Note(state::project::links::Resolution::File(
                    RelPath::parse("notes/sub/c.md").unwrap()
                ))
            );
        }
        other => panic!("unexpected blocks {other:?}"),
    }
}

#[test]
fn backlinks_list_linking_notes_and_follow_edits() {
    let (dir, mut h) = vault();
    fs::write(
        dir.path().join("notes/sub/d.md"),
        "intro\n\nalso [[B]] here\n",
    )
    .unwrap();
    h.changed(vec![dir.path().join("notes/sub/d.md")]);
    h.keys(":o notes/b.md<ret><space>l");
    assert_eq!(
        backlinks(&h),
        [
            ("notes/a.md".to_owned(), 1),
            ("notes/a.md".to_owned(), 2),
            ("notes/sub/d.md".to_owned(), 2),
        ]
    );
    // Enter opens the note at the link's line.
    h.keys("<down><down><ret>");
    assert_eq!(focused(&h), "notes/sub/d.md");
    assert_eq!(cursor_line(&h), 2);
    // Unsaved edits count: a new link above the old one moves it down a line.
    h.keys("ggO[[b]] first<esc>");
    h.keys(":o notes/b.md<ret><space>l");
    assert_eq!(
        backlinks(&h),
        [
            ("notes/a.md".to_owned(), 1),
            ("notes/a.md".to_owned(), 2),
            ("notes/sub/d.md".to_owned(), 0),
            ("notes/sub/d.md".to_owned(), 3),
        ]
    );
    // The query narrows by note path.
    h.keys("sub/d");
    assert_eq!(backlinks(&h).len(), 2);
    h.keys("<esc>");
    // In a rendered view too.
    h.keys("<space>V<space>l");
    assert_eq!(backlinks(&h).len(), 4);
}

#[test]
fn backlinks_follow_external_changes_without_rereading_on_their_own() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    fs::create_dir_all(root.join("notes")).unwrap();
    fs::write(root.join("notes/b.md"), "# B\n").unwrap();
    fs::write(root.join("notes/a.md"), "[[b]]\n").unwrap();
    let mut h = Harness::watched(root);
    h.keys(":o notes/b.md<ret><space>l");
    assert!(matches!(h.state.backlinks.build, Build::Ready));
    assert_eq!(backlinks(&h), [("notes/a.md".to_owned(), 0)]);
    h.keys("<esc>");
    // Reading every note is not a change the watcher reports.
    assert!(!h.events_arrive(), "the index read woke the watcher");
    let canonical = h.state.root.path().to_owned();
    fs::write(canonical.join("notes/e.md"), "x\ny [[b]]\n").unwrap();
    fs::write(canonical.join("notes/a.md"), "no links now\n").unwrap();
    let key = RelPath::parse("notes/e.md").unwrap();
    let a = RelPath::parse("notes/a.md").unwrap();
    assert!(h.wait_for_events(|state| {
        state.backlinks.notes.contains_key(&key)
            && state
                .backlinks
                .notes
                .get(&a)
                .is_some_and(|note| note.links.is_empty())
    }));
    h.keys("<space>l");
    assert_eq!(backlinks(&h), [("notes/e.md".to_owned(), 1)]);
    h.keys("<esc>");
    fs::remove_file(canonical.join("notes/e.md")).unwrap();
    assert!(h.wait_for_events(|state| !state.backlinks.notes.contains_key(&key)));
    h.keys("<space>l");
    assert!(backlinks(&h).is_empty());
    assert!(Path::new(&canonical).exists());
}

#[test]
fn backlinks_asked_for_before_the_file_index_fill_once_it_arrives() {
    let (_dir, mut h) = vault();
    h.keys(":o notes/b.md<ret>");
    h.state.files = state::project::files::Files::Loading;
    h.keys("<space>l");
    assert!(matches!(h.state.backlinks.build, Build::Waiting));
    match &h.state.overlay {
        Overlay::Picker(picker) => match &picker.items {
            Items::Backlinks(referrers) => assert_eq!(referrers.search, Search::Running),
            Items::Files(_) | Items::Buffers(_) | Items::Grep(_) | Items::Commands => {
                panic!("wrong picker")
            }
        },
        Overlay::None | Overlay::Prompt(_) => panic!("picker not open"),
    }
    let root = h.state.root.path().to_owned();
    h.run(|_, out| out.push(state::Effect::Job(state::job::Job::Index { root })));
    assert_eq!(
        backlinks(&h),
        [("notes/a.md".to_owned(), 1), ("notes/a.md".to_owned(), 2)]
    );
}
