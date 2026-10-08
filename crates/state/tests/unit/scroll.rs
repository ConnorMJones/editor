use helix_core::{Range, Rope, Selection};
use model::mode::Mode;
use state::display::{Indent, text_format};
use state::layout::Viewport;
use state::scroll::{self, SCROLLOFF};

const INDENT: Indent = Indent {
    tab_width: 4,
    unit: 4,
};

fn numbered(lines: usize) -> Rope {
    Rope::from((0..lines).map(|i| format!("{i}\n")).collect::<String>())
}

#[test]
fn cursor_inside_band_keeps_anchor() {
    let text = numbered(100);
    let slice = text.slice(..);
    let format = text_format(Viewport { rows: 20, cols: 80 }, INDENT);
    let cursor = text.line_to_char(10);
    assert_eq!(scroll::follow(slice, cursor, 0, 20, &format), 0);
}

#[test]
fn cursor_below_band_scrolls_to_scrolloff() {
    let text = numbered(100);
    let slice = text.slice(..);
    let format = text_format(Viewport { rows: 20, cols: 80 }, INDENT);
    let cursor = text.line_to_char(50);
    let anchor = scroll::follow(slice, cursor, 0, 20, &format);
    assert_eq!(text.char_to_line(anchor), 50 - (20 - SCROLLOFF - 1));
}

#[test]
fn cursor_above_band_scrolls_up() {
    let text = numbered(100);
    let slice = text.slice(..);
    let format = text_format(Viewport { rows: 20, cols: 80 }, INDENT);
    let anchor = text.line_to_char(60);
    let cursor = text.line_to_char(30);
    let anchor = scroll::follow(slice, cursor, anchor, 20, &format);
    assert_eq!(text.char_to_line(anchor), 30 - SCROLLOFF);
}

#[test]
fn top_of_document_never_scrolls_past_start() {
    let text = numbered(100);
    let slice = text.slice(..);
    let format = text_format(Viewport { rows: 20, cols: 80 }, INDENT);
    let anchor = text.line_to_char(10);
    assert_eq!(scroll::follow(slice, 1, anchor, 20, &format), 0);
}

#[test]
fn wrapped_rows_count_towards_scrolloff() {
    let line = "word ".repeat(10);
    let text = Rope::from(format!("{line}\n").repeat(20));
    let slice = text.slice(..);
    let format = text_format(Viewport { rows: 11, cols: 20 }, INDENT);
    let cursor = text.line_to_char(5);
    let anchor = scroll::follow(slice, cursor, 0, 11, &format);
    // Lines wrap to three rows, so five rows above the cursor is the second row of line 3.
    assert_eq!(text.char_to_line(anchor), 3);
    assert!(anchor > text.line_to_char(3));
}

#[test]
fn zero_height_viewport_is_ignored() {
    let text = numbered(10);
    let format = text_format(Viewport { rows: 0, cols: 0 }, INDENT);
    assert_eq!(scroll::follow(text.slice(..), 30, 4, 0, &format), 4);
}

#[test]
fn wheel_drags_cursor_into_band() {
    let text = numbered(100);
    let slice = text.slice(..);
    let viewport = Viewport { rows: 20, cols: 80 };
    let format = text_format(viewport, INDENT);
    let selection = Selection::point(0);
    let (anchor, moved) = scroll::scroll(slice, &selection, 0, viewport, 3, Mode::Normal, &format);
    assert_eq!(text.char_to_line(anchor), 3);
    let moved = moved.expect("cursor left the scrolloff band");
    assert_eq!(
        moved.primary(),
        Range::point(text.line_to_char(3 + SCROLLOFF))
    );

    let inside = Selection::point(text.line_to_char(10));
    let (_, moved) = scroll::scroll(slice, &inside, 0, viewport, 3, Mode::Normal, &format);
    assert_eq!(moved, None);
}

#[test]
fn click_maps_cells_to_chars() {
    let text = Rope::from("ab\n\tcd\n");
    let slice = text.slice(..);
    let format = text_format(Viewport { rows: 5, cols: 40 }, INDENT);
    assert_eq!(scroll::char_at(slice, 0, 0, 1, &format), 1);
    assert_eq!(scroll::char_at(slice, 0, 1, 2, &format), 3);
    assert_eq!(scroll::char_at(slice, 0, 1, 5, &format), 5);
    assert_eq!(scroll::char_at(slice, 0, 1, 30, &format), 6);
}
