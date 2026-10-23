use std::fs;

use model::command::{ReadCommand, ReadMove, Shown};
use model::notice::Notice;
use model::units::Px;
use state::input::keymap::Context;
use state::project::links::{self, Origin};
use state::project::path::RelPath;
use state::project::session;
use state::render;
use state::render::document::{
    self, BlockClick, DocumentAt, DocumentMotion, DocumentScroll, DocumentState,
};
use state::render::markdown::{LinkTarget, WikiLink};

use state::view::layout::PaneId;
use state::view::mode;
use state::view::tab::{Beside, Share, Showing, TextTabs};
use state::{Confirm, input};

use crate::harness::Harness;

/// Ten top-level blocks; block `n` starts on line `2n`, headings at 0, 3 and 6.
const NOTE: &str = "\
# One
\n- [ ] task
\npara two
\n## Two
\npara three
\npara four
\n## Three
\npara five
\npara six
\npara seven
";

fn harness() -> (tempfile::TempDir, Harness) {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("note.md"), NOTE).unwrap();
    fs::write(dir.path().join("other.md"), "# Other\n").unwrap();
    fs::write(dir.path().join("plain.txt"), "text\n").unwrap();
    let mut h = Harness::new(dir.path());
    h.keys(":o note.md<ret>");
    (dir, h)
}

fn rendered_in(h: &Harness, pane: PaneId) -> &DocumentState {
    match h.state.layout.panes()[pane]
        .active_text()
        .map(TextTabs::showing)
    {
        Some(Showing::Document(_, view, _)) => view,
        Some(Showing::Source(_) | Showing::Diff(..) | Showing::Pages(..)) | None => {
            panic!("pane shows no rendered document")
        }
    }
}

fn shown(h: &Harness) -> Shown {
    h.state
        .layout
        .focused()
        .active_text()
        .map(TextTabs::shown)
        .unwrap()
}

fn cursor_line(h: &Harness) -> usize {
    let text = h.doc().text.slice(..);
    h.view().selection.primary().cursor_line(text)
}

fn top_line(h: &Harness, pane: PaneId) -> usize {
    let view = h.state.layout.panes()[pane].active_text().unwrap().text();
    let doc = &h.state.buffers[view.buffer()].doc;
    doc.text.char_to_line(view.scroll.anchor)
}

#[test]
fn space_tab_cycles_through_the_modes_the_kind_of_file_has() {
    let (_dir, mut h) = harness();
    h.keys("<space><tab>");
    assert_eq!(shown(&h), Shown::Split);
    h.keys("<space><tab>");
    assert_eq!(shown(&h), Shown::Render);
    h.keys("<space><tab>");
    assert_eq!(shown(&h), Shown::Diff);
    h.keys("<space><tab>");
    assert_eq!(shown(&h), Shown::Source);
    h.keys("<space><S-tab>");
    assert_eq!(shown(&h), Shown::Diff);
    assert_eq!(h.focused_tabs(), ["±note.md"], "one tab, in its mode");
    // A plain text file has no rendering to cycle through.
    h.keys(":o plain.txt<ret><space><tab>");
    assert_eq!(shown(&h), Shown::Diff);
    h.keys("<space><tab>");
    assert_eq!(shown(&h), Shown::Source);
}

#[test]
fn rendering_is_refused_for_files_without_one() {
    let (_dir, mut h) = harness();
    h.keys(":o plain.txt<ret><space>V");
    assert_eq!(shown(&h), Shown::Source);
    assert!(matches!(h.state.notice, Some(Notice::NotRenderable { .. })));
}

#[test]
fn space_capital_v_toggles_in_place_keeping_the_position() {
    let (_dir, mut h) = harness();
    h.keys("11gg<space>V");
    assert_eq!(h.focused_tabs(), ["◉note.md"]);
    let focus = h.state.layout.focus();
    assert_eq!(input::context(&h.state), Context::Document);
    let buffer = h.view().buffer();
    match h.state.documents.get(buffer) {
        Some(parsed) => assert_eq!(parsed.version, h.doc().version, "parsed for the view"),
        None => panic!("no parse while shown"),
    }
    // `]]` and `[[` move between top-level headings; `gg` and `ge` go to the ends.
    assert_eq!(rendered_in(&h, focus).scroll, DocumentScroll::item(5));
    h.keys("]]");
    assert_eq!(rendered_in(&h, focus).scroll, DocumentScroll::item(6));
    h.keys("[[[[");
    assert_eq!(rendered_in(&h, focus).scroll, DocumentScroll::item(0));
    h.keys("ge");
    assert_eq!(rendered_in(&h, focus).scroll, DocumentScroll::item(9));
    h.keys("]]");
    assert!(matches!(h.state.notice, Some(Notice::NoHeading)));
    h.keys("[[");
    assert_eq!(rendered_in(&h, focus).scroll, DocumentScroll::item(6));

    // Back to the source at the heading the rendering showed, cursor moved there since it was
    // out of view.
    h.keys("<space>V");
    assert_eq!(h.focused_tabs(), ["note.md"]);
    assert_eq!(top_line(&h, focus), 12);
    assert_eq!(cursor_line(&h), 12);
    assert!(
        h.state.documents.get(buffer).is_none(),
        "the parse goes with the rendering"
    );

    // `esc` in a rendering goes back to the source too.
    h.keys("<space>V<esc>");
    assert_eq!(shown(&h), Shown::Source);
}

#[test]
fn opening_a_file_from_a_picker_keeps_its_mode() {
    let (_dir, mut h) = harness();
    h.keys("<space>V:o other.md<ret><space>bnote<ret>");
    assert_eq!(h.focused_tabs(), ["◉note.md", "other.md"]);
    assert_eq!(shown(&h), Shown::Render);
}

#[test]
fn clicking_a_block_puts_the_text_cursor_on_it() {
    let (_dir, mut h) = harness();
    h.keys("<space>V");
    let pane = h.state.layout.focus();
    // As the host draws it: the block's first line at the parsed version.
    let click = |h: &Harness| {
        let buffer = h.view().buffer();
        let doc = &h.state.buffers[buffer].doc;
        let markdown = document::markdown(&h.state.documents, buffer, doc).unwrap();
        BlockClick {
            line: markdown.blocks[markdown.roots[6]].lines.start,
            version: doc.version,
        }
    };
    let drawn = click(&h);
    h.run(|state, _| document::click(state, pane, drawn));
    assert_eq!(cursor_line(&h), 12);

    // A click drawn before an edit is ignored.
    h.keys("<space>VggOx<esc><space>V");
    h.run(|state, _| document::click(state, pane, drawn));
    assert_eq!(cursor_line(&h), 0);
}

#[test]
fn rendered_keys_scroll_by_motions_for_the_host_to_measure() {
    let (_dir, mut h) = harness();
    h.keys("<space>V");
    let pane = h.state.layout.focus();
    let from = DocumentAt {
        item: 0,
        offset: Px(0.0),
    };
    h.keys("jj");
    assert_eq!(
        rendered_in(&h, pane).scroll,
        DocumentScroll::Moved {
            from,
            by: DocumentMotion {
                lines: 2,
                half_pages: 0,
            }
        }
    );
    h.keys("k<C-d>");
    assert_eq!(
        rendered_in(&h, pane).scroll,
        DocumentScroll::Moved {
            from,
            by: DocumentMotion {
                lines: 1,
                half_pages: 1,
            }
        }
    );
    // The host reports where the move landed.
    h.run(|state, _| document::scrolled(state, pane, from));
    assert_eq!(rendered_in(&h, pane).scroll, DocumentScroll::At(from));
    // A report from a frame drawn before the tab left the rendering is ignored.
    h.keys("<esc>");
    assert_eq!(shown(&h), Shown::Source);
    h.run(|state, _| document::scrolled(state, pane, from));
    h.run(|state, _| render::read(state, ReadCommand::Move(ReadMove::Top)));
    assert_eq!(shown(&h), Shown::Source);
}

#[test]
fn checkbox_toggles_the_source_as_one_undo_step() {
    let (_dir, mut h) = harness();
    let buffer = h.view().buffer();
    let version = h.doc().version;
    let marker = NOTE.find("[ ]").unwrap();
    h.run(|state, _| document::toggle_task(state, buffer, version, marker));
    assert!(h.doc().text.to_string().contains("- [x] task"));
    // A stale version is ignored.
    h.run(|state, _| document::toggle_task(state, buffer, version, marker));
    assert!(h.doc().text.to_string().contains("- [x] task"));
    let version = h.doc().version;
    h.run(|state, _| document::toggle_task(state, buffer, version, marker));
    assert!(h.doc().text.to_string().contains("- [ ] task"));
    let version = h.doc().version;
    h.run(|state, _| document::toggle_task(state, buffer, version, marker));
    // Undo in the source steps back one toggle at a time.
    h.keys("u");
    assert!(h.doc().text.to_string().contains("- [ ] task"));
    h.keys("u");
    assert!(h.doc().text.to_string().contains("- [x] task"));
    h.keys("u");
    assert_eq!(h.doc().text, NOTE);
}

#[test]
fn callout_folds_are_view_state() {
    let (_dir, mut h) = harness();
    h.keys("<space>V");
    let pane = h.state.layout.focus();
    h.run(|state, _| document::toggle_fold(state, pane, 2));
    assert_eq!(rendered_in(&h, pane).toggled, [2]);
    h.run(|state, _| document::toggle_fold(state, pane, 2));
    assert!(rendered_in(&h, pane).toggled.is_empty());
    assert_eq!(h.doc().text, NOTE);
}

#[test]
fn links_follow_urls_headings_and_notes() {
    let (_dir, mut h) = harness();
    h.keys("<space>V");
    let buffer = h.view().buffer();
    let origin = Origin::Document(h.state.layout.focus());
    h.run(|state, out| {
        links::follow(
            state,
            buffer,
            &LinkTarget::Url("https://a.org".into()),
            origin,
            out,
        )
    });
    assert_eq!(h.urls, ["https://a.org"]);
    let heading = LinkTarget::Heading("three".into());
    h.run(|state, out| links::follow(state, buffer, &heading, origin, out));
    let focus = h.state.layout.focus();
    assert_eq!(rendered_in(&h, focus).scroll, DocumentScroll::item(6));
    let wiki = LinkTarget::Wiki(WikiLink {
        target: "other".into(),
        heading: None,
        alias: None,
    });
    h.run(|state, out| links::follow(state, buffer, &wiki, origin, out));
    assert_eq!(h.focused_tabs(), ["◉note.md", "◉other.md"]);
    let missing = LinkTarget::Path {
        path: "nope.md".into(),
        heading: None,
    };
    h.run(|state, out| links::follow(state, buffer, &missing, origin, out));
    assert_eq!(
        h.confirm(),
        Some(&Confirm::Create(RelPath::parse("nope.md").unwrap()))
    );
    assert_eq!(input::context(&h.state), Context::Confirm);
    h.keys("n");
    assert_eq!(h.confirm(), None);
    assert!(matches!(h.state.notice, Some(Notice::CreateNotConfirmed)));
}

#[test]
fn sessions_keep_rendered_tabs() {
    let (_dir, mut h) = harness();
    h.keys("<space>V");
    let pane = h.state.layout.focus();
    h.run(|state, _| {
        document::scrolled(
            state,
            pane,
            DocumentAt {
                item: 4,
                offset: Px(0.0),
            },
        )
    });
    let saved = session::snapshot(&h.state);
    let text = toml::to_string(&saved).unwrap();
    let saved: session::Session = toml::from_str(&text).unwrap();
    let mut after = Harness::new(h.state.project.root.path());
    after
        .run(|state, out| session::restore(state, saved, out))
        .expect("valid session");
    assert_eq!(after.focused_tabs(), ["◉note.md"]);
    let focus = after.state.layout.focus();
    assert_eq!(rendered_in(&after, focus).scroll, DocumentScroll::item(4));
}

#[test]
fn sessions_rendering_a_file_without_one_are_rejected() {
    let (_dir, mut h) = harness();
    h.keys("<space>V");
    let text = toml::to_string(&session::snapshot(&h.state))
        .unwrap()
        .replace("note.md", "plain.txt");
    let saved: session::Session = toml::from_str(&text).unwrap();
    let mut after = Harness::new(h.state.project.root.path());
    assert!(matches!(
        after.run(|state, out| session::restore(state, saved, out)),
        Err(session::SessionError::NoRendering { .. })
    ));
}

#[test]
fn space_v_splits_the_tab_into_source_and_rendering() {
    let (_dir, mut h) = harness();
    h.keys("<space>v");
    assert_eq!(h.focused_tabs(), ["◧note.md"]);
    assert_eq!(shown(&h), Shown::Split);
    assert_eq!(
        input::context(&h.state),
        Context::Normal,
        "the source takes the keys"
    );
    let buffer = h.view().buffer();
    assert!(
        h.state.documents.get(buffer).is_some(),
        "parsed for the rendering"
    );
    h.keys("<space>v");
    assert_eq!(h.focused_tabs(), ["note.md"]);
    assert!(h.state.documents.get(buffer).is_none());
}

#[test]
fn the_split_rendering_follows_the_source_cursor_and_scroll() {
    let (_dir, mut h) = harness();
    h.keys("<space>v");
    let pane = h.state.layout.focus();
    assert_eq!(rendered_in(&h, pane).scroll, DocumentScroll::item(0));
    // Line 10 is "para four", block 5.
    h.keys("11gg");
    assert_eq!(rendered_in(&h, pane).scroll, DocumentScroll::item(5));
    // Within the same block nothing moves.
    h.keys("l");
    assert_eq!(rendered_in(&h, pane).scroll, DocumentScroll::item(5));
    // A scroll of the source with the cursor in place follows the top line.
    h.run(|state, _| state::view::scroll::scroll_to_line(state, pane, 6));
    assert_eq!(rendered_in(&h, pane).scroll, DocumentScroll::item(3));
}

#[test]
fn scrolling_the_split_rendering_scrolls_the_source_not_its_cursor() {
    let (_dir, mut h) = harness();
    h.keys("<space>v");
    let pane = h.state.layout.focus();
    let at = |item: usize, offset: f32| DocumentAt {
        item,
        offset: Px(offset),
    };
    // The host reports the list now shows item 8 ("para six", line 16).
    h.run(|state, _| document::scrolled(state, pane, at(8, 3.0)));
    assert_eq!(top_line(&h, pane), 16);
    assert_eq!(cursor_line(&h), 0, "the cursor stays");
    // The sync that follows does not bounce the rendering back to the cursor.
    assert_eq!(rendered_in(&h, pane).scroll, DocumentScroll::At(at(8, 3.0)));
    // Within the same item the source is left alone.
    h.run(|state, _| state::view::scroll::scroll_to_line(state, pane, 0));
    h.run(|state, _| document::scrolled(state, pane, at(0, 0.0)));
    h.run(|state, _| document::scrolled(state, pane, at(0, 20.0)));
    assert_eq!(top_line(&h, pane), 0);
}

#[test]
fn clicking_a_split_rendering_moves_the_cursor_and_leaves_the_rendering() {
    let (_dir, mut h) = harness();
    h.keys("<space>v");
    let pane = h.state.layout.focus();
    let click = {
        let buffer = h.view().buffer();
        let doc = &h.state.buffers[buffer].doc;
        let markdown = document::markdown(&h.state.documents, buffer, doc).unwrap();
        BlockClick {
            line: markdown.blocks[markdown.roots[6]].lines.start,
            version: doc.version,
        }
    };
    h.run(|state, _| document::click(state, pane, click));
    assert_eq!(cursor_line(&h), 12);
    assert_eq!(rendered_in(&h, pane).scroll, DocumentScroll::item(0));
}

#[test]
fn sessions_keep_split_tabs() {
    let (_dir, mut h) = harness();
    h.keys("<space>v");
    let pane = h.state.layout.focus();
    h.run(|state, _| {
        document::scrolled(
            state,
            pane,
            DocumentAt {
                item: 4,
                offset: Px(0.0),
            },
        )
    });
    let text = toml::to_string(&session::snapshot(&h.state)).unwrap();
    let saved: session::Session = toml::from_str(&text).unwrap();
    let mut after = Harness::new(h.state.project.root.path());
    after
        .run(|state, out| session::restore(state, saved, out))
        .expect("valid session");
    assert_eq!(after.focused_tabs(), ["◧note.md"]);
    let focus = after.state.layout.focus();
    assert_eq!(
        rendered_in(&after, focus).scroll,
        DocumentScroll::item(4),
        "the restored source cursor does not override it"
    );
}

#[test]
fn split_keys_and_drags_resize_the_source_side() {
    let (_dir, mut h) = harness();
    h.keys("<C-w>>");
    assert!(matches!(h.state.notice, Some(Notice::NotSplit)));
    h.keys("<space>v");
    let pane = h.state.layout.focus();
    let share = |h: &Harness| match h
        .state
        .layout
        .focused()
        .active_text()
        .map(TextTabs::showing)
    {
        Some(Showing::Document(_, _, Beside::Source(_, share))) => share.get(),
        Some(
            Showing::Source(_) | Showing::Diff(..) | Showing::Document(..) | Showing::Pages(..),
        )
        | None => panic!("not split"),
    };
    assert_eq!(share(&h), 0.5);
    h.keys("<C-w><");
    assert!((share(&h) - 0.45).abs() < 1e-6);
    for _ in 0..30 {
        h.keys("<C-w>>");
    }
    assert!(
        (share(&h) - 0.95).abs() < 1e-6,
        "stops a step short of the edge"
    );
    h.keys("<C-w>=");
    assert_eq!(share(&h), 0.5);
    h.run(|state, _| mode::drag(state, pane, Share::new(0.3).unwrap()));
    assert_eq!(share(&h), 0.3);
    // A drag reported after the tab left the split is dropped.
    h.keys("<space>v");
    h.run(|state, _| mode::drag(state, pane, Share::new(0.7).unwrap()));
    assert_eq!(shown(&h), Shown::Source);
    assert!(Share::new(1.0).is_err());
    assert!(Share::new(f32::NAN).is_err());
}
