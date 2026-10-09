use state::layout::Viewport;
use state::{Sticky, scroll};

use crate::harness::Harness;

/// A 100-line buffer `0\n1\n...` drawn in a `rows`-tall, 80-column view.
fn numbered(rows: u16) -> Harness {
    let text: String = (0..100).map(|i| format!("{i}\n")).collect();
    let mut h = Harness::with("n.txt", &format!("#[0|]#{}", &text[1..]));
    let focus = h.state.layout.focus;
    scroll::set_viewport(&mut h.state, focus, Viewport { rows, cols: 80 });
    h
}

/// Line of the primary cursor and of the first visible row.
fn lines(h: &Harness) -> (usize, usize) {
    let view = h.view();
    let text = h.doc().text.slice(..);
    (
        text.char_to_line(view.selection.primary().cursor(text)),
        text.char_to_line(view.scroll.anchor),
    )
}

#[test]
fn page_down_and_up_drag_the_cursor() {
    let mut h = numbered(10);
    h.keys("<C-f>");
    assert_eq!(lines(&h), (14, 10));
    h.keys("<C-b>");
    // Dragged to the scroll band edge, then followed by the slightly wider follow band.
    assert_eq!(lines(&h), (5, 1));
    h.keys("<pagedown>");
    assert_eq!(lines(&h), (15, 11));
}

#[test]
fn half_page_moves_cursor_with_view() {
    let mut h = numbered(10);
    h.keys("<C-d>");
    assert_eq!(lines(&h).0, 5);
    h.keys("<C-d>");
    assert_eq!(lines(&h).0, 10);
    h.keys("<C-u>");
    assert_eq!(lines(&h).0, 5);
}

#[test]
fn align_view_keeps_scrolloff() {
    let mut h = numbered(30);
    h.keys("51Gzt");
    assert_eq!(lines(&h), (50, 45));
    h.keys("zb");
    assert_eq!(lines(&h), (50, 26));
    h.keys("zz");
    assert_eq!(lines(&h), (50, 36));
}

#[test]
fn goto_window_rows() {
    let mut h = numbered(30);
    h.keys("gt");
    assert_eq!(lines(&h).0, 5);
    h.keys("gb");
    assert_eq!(lines(&h).0, 24);
    h.keys("gc");
    assert_eq!(lines(&h).0, 14);
    h.keys("2gt");
    assert_eq!(lines(&h).0, 6);
}

#[test]
fn scroll_lines_and_sticky_view_mode() {
    let mut h = numbered(10);
    h.keys("50G");
    let (_, top) = lines(&h);
    h.keys("zj");
    assert_eq!(lines(&h).1, top + 1);
    h.keys("Zjjj");
    assert_eq!(h.state.input.sticky, Sticky::View);
    assert_eq!(lines(&h).1, top + 4);
    h.keys("<esc>");
    assert_eq!(h.state.input.sticky, Sticky::Off);
    let (cursor, _) = lines(&h);
    h.keys("j");
    assert_eq!(lines(&h).0, cursor + 1);
}

#[test]
fn visual_line_motion_follows_soft_wrap() {
    let line = "a".repeat(30);
    let mut h = Harness::with("w.txt", &format!("#[a|]#{}\nb\n", &line[1..]));
    let focus = h.state.layout.focus;
    scroll::set_viewport(&mut h.state, focus, Viewport { rows: 10, cols: 12 });
    h.keys("j");
    let (cursor_line, _) = lines(&h);
    assert_eq!(cursor_line, 0);
    h.keys("gj");
    assert_eq!(lines(&h).0, 1);
}

#[test]
fn edits_elsewhere_shift_other_views_anchor() {
    let mut h = numbered(10);
    h.keys("51Gzt");
    let before = h.view().scroll.anchor;
    let original = h.state.layout.focus;
    h.keys("<C-w>vggxd");
    match h.state.layout.panes[original].active() {
        Some(view) => assert_eq!(view.text().scroll.anchor, before - 2),
        None => panic!("no view"),
    }
}
