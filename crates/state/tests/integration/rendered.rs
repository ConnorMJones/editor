use std::fs;

use model::command::RenderedCommand;
use model::notice::Notice;
use model::window::Px;
use state::buffer::Content;
use state::keymap::Context;
use state::layout::{PaneId, View};
use state::markdown::{LinkTarget, WikiLink};
use state::path::RelPath;
use state::rendered::{self, BlockScroll, Link, Motion, Preview, RenderedView, Scroll};
use state::session;
use state::vault::{self, Origin};
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

fn rendered_in(h: &Harness, pane: PaneId) -> &RenderedView {
    match h.state.layout.panes[pane].active() {
        Some(View::Rendered(view)) => view,
        Some(View::Text(_) | View::Diff(_)) | None => panic!("pane shows no rendered view"),
    }
}

/// The other pane of a two-pane layout.
fn other(h: &Harness) -> PaneId {
    let focus = h.state.layout.focus;
    *h.state
        .layout
        .leaves()
        .iter()
        .find(|pane| **pane != focus)
        .expect("a second pane")
}

fn cursor_line(h: &Harness) -> usize {
    let text = h.doc().text.slice(..);
    h.view().selection.primary().cursor_line(text)
}

fn top_line(h: &Harness, pane: PaneId) -> usize {
    let view = h.state.layout.panes[pane].active().unwrap().text();
    let doc = match &h.state.buffers[view.buffer()].content {
        Content::Text { doc, .. } => doc,
        Content::Loading { .. } => panic!("loading"),
    };
    doc.text.char_to_line(view.scroll.anchor)
}

#[test]
fn space_v_opens_and_closes_a_following_split() {
    let (_dir, mut h) = harness();
    let source = h.state.layout.focus;
    h.keys("<space>v");
    assert_eq!(h.state.layout.leaves().len(), 2);
    assert_eq!(h.state.layout.focus, source, "focus stays in the text");
    let preview = other(&h);
    assert_eq!(
        h.state.layout.leaves(),
        [source, preview],
        "opens to the right"
    );
    assert_eq!(h.tabs(preview), ["◉note.md"]);
    assert!(matches!(
        rendered_in(&h, preview).link,
        Link::Follows { pane, .. } if pane == source
    ));
    let buffer = h.view().buffer();
    match h.state.previews.get(buffer) {
        Some(Preview::Markdown(parsed)) => {
            assert_eq!(parsed.version, h.doc().version, "parsed for the view")
        }
        Some(Preview::Typst(_)) | None => panic!("no parse while shown"),
    }

    // Again from the text closes it, and the buffer stays.
    h.keys("<space>v");
    assert_eq!(h.state.layout.leaves(), [source]);
    assert_eq!(h.focused_tabs(), ["note.md"]);
    assert!(
        h.state.previews.get(buffer).is_none(),
        "the parse goes with the view"
    );

    // From the rendered side it closes too, returning focus to the text.
    h.keys("<space>v<C-w>l");
    assert_eq!(input::context(&h.state), Context::Rendered);
    h.keys("<space>v");
    assert_eq!(h.state.layout.leaves(), [source]);
    assert_eq!(h.state.layout.focus, source);
}

#[test]
fn space_v_refuses_files_without_a_rendered_view() {
    let (_dir, mut h) = harness();
    h.keys(":o plain.txt<ret><space>v");
    assert_eq!(h.state.layout.leaves().len(), 1);
    assert!(matches!(h.state.notice, Some(Notice::NotRenderable { .. })));
}

#[test]
fn space_capital_v_toggles_in_place_keeping_the_position() {
    let (_dir, mut h) = harness();
    h.keys("11gg<space>V");
    assert_eq!(h.focused_tabs(), ["note.md", "◉note.md"]);
    let focus = h.state.layout.focus;
    assert_eq!(rendered_in(&h, focus).link, Link::Standalone);
    assert_eq!(input::context(&h.state), Context::Rendered);
    // `]]` and `[[` move between top-level headings; `gg` and `ge` go to the ends.
    assert_eq!(rendered_in(&h, focus).scroll, Scroll::item(5));
    h.keys("]]");
    assert_eq!(rendered_in(&h, focus).scroll, Scroll::item(6));
    h.keys("[[[[");
    assert_eq!(rendered_in(&h, focus).scroll, Scroll::item(0));
    h.keys("ge");
    assert_eq!(rendered_in(&h, focus).scroll, Scroll::item(9));
    h.keys("]]");
    assert!(matches!(h.state.notice, Some(Notice::NoHeading)));
    h.keys("[[");
    assert_eq!(rendered_in(&h, focus).scroll, Scroll::item(6));

    // Back to text at the heading the view showed, cursor moved there since it was out of view.
    h.keys("<space>V");
    assert_eq!(h.focused_tabs(), ["note.md", "◉note.md"]);
    assert!(matches!(
        h.state.layout.panes[focus].active(),
        Some(View::Text(_))
    ));
    assert_eq!(top_line(&h, focus), 12);
    assert_eq!(cursor_line(&h), 12);

    // `esc` in a standalone view toggles back too.
    h.keys("<space>V<esc>");
    assert!(matches!(
        h.state.layout.panes[focus].active(),
        Some(View::Text(_))
    ));
}

#[test]
fn source_cursor_and_scroll_move_the_rendered_view() {
    let (_dir, mut h) = harness();
    let source = h.state.layout.focus;
    h.keys("<space>v");
    let preview = other(&h);
    assert_eq!(rendered_in(&h, preview).scroll, Scroll::item(0));
    // Line 10 is "para four", block 5.
    h.keys("11gg");
    assert_eq!(rendered_in(&h, preview).scroll, Scroll::item(5));
    // Within the same block nothing moves.
    h.keys("l");
    assert_eq!(rendered_in(&h, preview).scroll, Scroll::item(5));
    // A scroll of the source with the cursor in place follows the top line.
    h.run(|state, _| state::scroll::scroll_to_line(state, source, 6));
    assert_eq!(rendered_in(&h, preview).scroll, Scroll::item(3));
}

#[test]
fn scrolling_the_rendered_view_scrolls_the_source_not_its_cursor() {
    let (_dir, mut h) = harness();
    let source = h.state.layout.focus;
    h.keys("<space>v");
    let preview = other(&h);
    // The host reports the list now shows item 8 ("para six", line 16).
    h.run(|state, _| {
        rendered::scrolled(
            state,
            preview,
            BlockScroll {
                item: 8,
                offset: Px(3.0),
                left: Px(0.0),
            },
        )
    });
    assert_eq!(top_line(&h, source), 16);
    assert_eq!(cursor_line(&h), 0, "the cursor stays");
    // The sync that follows does not bounce the view back to the cursor.
    assert_eq!(
        rendered_in(&h, preview).scroll,
        Scroll::At(BlockScroll {
            item: 8,
            offset: Px(3.0),
            left: Px(0.0),
        })
    );
    // Within the same item the source is left alone.
    h.run(|state, _| state::scroll::scroll_to_line(state, source, 0));
    h.run(|state, _| {
        rendered::scrolled(
            state,
            preview,
            BlockScroll {
                item: 0,
                offset: Px(0.0),
                left: Px(0.0),
            },
        )
    });
    h.run(|state, _| {
        rendered::scrolled(
            state,
            preview,
            BlockScroll {
                item: 0,
                offset: Px(20.0),
                left: Px(0.0),
            },
        )
    });
    assert_eq!(top_line(&h, source), 0);
}

#[test]
fn clicking_a_block_moves_the_source_cursor_and_focus() {
    let (_dir, mut h) = harness();
    let source = h.state.layout.focus;
    h.keys("<space>v<C-w>l");
    let preview = h.state.layout.focus;
    let block = {
        let buffer = rendered_in(&h, preview).text.buffer();
        let doc = state::buffer::doc(&h.state.buffers, buffer).expect("loaded");
        rendered::markdown(&h.state.previews, buffer, doc)
            .unwrap()
            .roots[6]
    };
    h.run(|state, _| rendered::click(state, preview, block));
    assert_eq!(h.state.layout.focus, source);
    assert_eq!(cursor_line(&h), 12);
}

#[test]
fn rendered_keys_scroll_by_motions_for_the_host_to_measure() {
    let (_dir, mut h) = harness();
    h.keys("<space>v<C-w>l");
    let preview = h.state.layout.focus;
    let from = BlockScroll {
        item: 0,
        offset: Px(0.0),
        left: Px(0.0),
    };
    h.keys("jj");
    assert_eq!(
        rendered_in(&h, preview).scroll,
        Scroll::Moved {
            from,
            by: Motion {
                lines: 2,
                half_pages: 0,
                columns: 0
            }
        }
    );
    h.keys("k<C-d>");
    assert_eq!(
        rendered_in(&h, preview).scroll,
        Scroll::Moved {
            from,
            by: Motion {
                lines: 1,
                half_pages: 1,
                columns: 0
            }
        }
    );
    // The host reports where the move landed.
    h.run(|state, _| rendered::scrolled(state, preview, from));
    assert_eq!(rendered_in(&h, preview).scroll, Scroll::At(from));
    // `esc` returns to the source pane.
    h.keys("<esc>");
    assert_ne!(h.state.layout.focus, preview);
    h.run(|state, _| rendered::command(state, RenderedCommand::Top));
}

#[test]
fn checkbox_toggles_the_source_as_one_undo_step() {
    let (_dir, mut h) = harness();
    h.keys("<space>v");
    let buffer = h.view().buffer();
    let version = h.doc().version;
    let marker = NOTE.find("[ ]").unwrap();
    h.run(|state, _| rendered::toggle_task(state, buffer, version, marker));
    assert!(h.doc().text.to_string().contains("- [x] task"));
    // A stale version is ignored.
    h.run(|state, _| rendered::toggle_task(state, buffer, version, marker));
    assert!(h.doc().text.to_string().contains("- [x] task"));
    let version = h.doc().version;
    h.run(|state, _| rendered::toggle_task(state, buffer, version, marker));
    assert!(h.doc().text.to_string().contains("- [ ] task"));
    let version = h.doc().version;
    h.run(|state, _| rendered::toggle_task(state, buffer, version, marker));
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
    h.keys("<space>v");
    let preview = other(&h);
    h.run(|state, _| rendered::toggle_fold(state, preview, 2));
    assert_eq!(rendered_in(&h, preview).toggled, [2]);
    h.run(|state, _| rendered::toggle_fold(state, preview, 2));
    assert!(rendered_in(&h, preview).toggled.is_empty());
    assert_eq!(h.doc().text, NOTE);
}

#[test]
fn links_follow_urls_headings_and_notes() {
    let (_dir, mut h) = harness();
    h.keys("<space>V");
    let buffer = h.view().buffer();
    let origin = Origin::Rendered(h.state.layout.focus);
    h.run(|state, out| {
        vault::follow(
            state,
            buffer,
            &LinkTarget::Url("https://a.org".into()),
            origin,
            out,
        )
    });
    assert_eq!(h.urls, ["https://a.org"]);
    let heading = LinkTarget::Heading("three".into());
    h.run(|state, out| vault::follow(state, buffer, &heading, origin, out));
    let focus = h.state.layout.focus;
    assert_eq!(rendered_in(&h, focus).scroll, Scroll::item(6));
    let wiki = LinkTarget::Wiki(WikiLink {
        target: "other".into(),
        heading: None,
        alias: None,
    });
    h.run(|state, out| vault::follow(state, buffer, &wiki, origin, out));
    assert_eq!(h.focused_tabs(), ["note.md", "◉note.md", "◉other.md"]);
    let missing = LinkTarget::Path {
        path: "nope.md".into(),
        heading: None,
    };
    h.run(|state, out| vault::follow(state, buffer, &missing, origin, out));
    assert_eq!(
        h.state.confirm,
        Some(Confirm::Create(RelPath::parse("nope.md").unwrap()))
    );
    assert_eq!(input::context(&h.state), Context::Confirm);
    h.keys("n");
    assert_eq!(h.state.confirm, None);
    assert!(matches!(h.state.notice, Some(Notice::CreateNotConfirmed)));
}

#[test]
fn closing_the_source_pane_leaves_a_standalone_view() {
    let (_dir, mut h) = harness();
    h.keys("<space>v<C-w>q");
    let pane = h.state.layout.focus;
    assert_eq!(rendered_in(&h, pane).link, Link::Standalone);
}

#[test]
fn sessions_keep_rendered_tabs_and_their_link() {
    let (_dir, mut h) = harness();
    h.keys("<space>v");
    let preview = other(&h);
    h.run(|state, _| {
        rendered::scrolled(
            state,
            preview,
            BlockScroll {
                item: 4,
                offset: Px(0.0),
                left: Px(0.0),
            },
        )
    });
    let saved = session::snapshot(&h.state);
    let text = toml::to_string(&saved).unwrap();
    let saved: session::Session = toml::from_str(&text).unwrap();
    let mut after = Harness::new(h.state.root.path());
    after
        .run(|state, out| session::restore(state, saved, out))
        .expect("valid session");
    let leaves = after.state.layout.leaves();
    assert_eq!(leaves.len(), 2);
    assert_eq!(after.tabs(leaves[1]), ["◉note.md"]);
    let view = rendered_in(&after, leaves[1]);
    assert!(matches!(view.link, Link::Follows { pane, .. } if pane == leaves[0]));
    assert_eq!(
        view.scroll,
        Scroll::item(4),
        "the restored source cursor does not override it"
    );
}

#[test]
fn toggling_a_split_preview_restores_the_pane_widths() {
    let (_dir, mut h) = harness();
    h.keys("<C-w>v");
    let before = h.state.layout.rects();
    for _ in 0..3 {
        h.keys("<space>v");
        assert_eq!(h.state.layout.leaves().len(), 3);
        h.keys("<space>v");
        assert_eq!(h.state.layout.rects(), before);
    }
}
