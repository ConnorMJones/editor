use std::path::Path;

use helix_core::doc_formatter::TextFormat;
use helix_core::syntax::Loader;
use helix_core::{Rope, Selection};
use model::theme::Background;
use state::buffer::TextDoc;
use state::diff::{Hunk, Side};
use state::diffview::{self, Align, LEFT, RIGHT};
use state::display::{self, Focus, Frame, Indent, RowKind, Shade, text_format};
use state::layout::{Scroll, Viewport};
use state::scroll::{self, Padding};

const INDENT: Indent = Indent {
    tab_width: 4,
    unit: 4,
};

const COLS: u16 = 20;

fn hunk(before: std::ops::Range<u32>, after: std::ops::Range<u32>) -> Hunk {
    Hunk { before, after }
}

fn format() -> TextFormat {
    text_format(
        Viewport {
            rows: 100,
            cols: COLS,
        },
        INDENT,
    )
}

fn align(left: &str, right: &str, hunks: &[Hunk]) -> Align {
    let (left, right) = (Rope::from(left), Rope::from(right));
    let mut align = Align::default();
    diffview::realign(
        &mut align,
        left.slice(..),
        right.slice(..),
        hunks,
        &format(),
    );
    align
}

#[test]
fn rows_count_soft_wrapped_lines() {
    let text = Rope::from("short\nthis line is long enough to wrap twice\nend\n");
    let format = format();
    assert_eq!(diffview::rows(text.slice(..), 0..1, &format), 1);
    assert_eq!(diffview::rows(text.slice(..), 1..2, &format), 3);
    assert_eq!(diffview::rows(text.slice(..), 0..3, &format), 5);
    assert_eq!(diffview::rows(text.slice(..), 2..2, &format), 0);
    // The empty line after the final newline still takes a row.
    assert_eq!(diffview::rows(text.slice(..), 3..4, &format), 1);
}

#[test]
fn shorter_side_of_a_hunk_gets_fillers() {
    // Two lines become three; the left pads one row after its last hunk line.
    let align = align("a\nb\nc\nd\n", "a\nX\nY\nZ\nd\n", &[hunk(1..3, 1..4)]);
    assert_eq!(align.sides[LEFT].fillers, [(2, 1)]);
    assert!(align.sides[RIGHT].fillers.is_empty());
    assert_eq!(align.hunks, [[2, 3]]);
    assert_eq!(
        align.sides[LEFT].shades,
        [Shade {
            lines: 1..3,
            role: Background::Modified,
        }]
    );
    assert_eq!(
        align.sides[RIGHT].shades,
        [Shade {
            lines: 1..4,
            role: Background::Modified,
        }]
    );
}

#[test]
fn fillers_count_visual_rows_under_soft_wrap() {
    // One line each, but the new one wraps into three rows.
    let long = "this line is long enough to wrap twice";
    let align = align("a\nb\nc\n", &format!("a\n{long}\nc\n"), &[hunk(1..2, 1..2)]);
    assert_eq!(align.hunks, [[1, 3]]);
    assert_eq!(align.sides[LEFT].fillers, [(1, 2)]);
    assert!(align.sides[RIGHT].fillers.is_empty());
}

#[test]
fn hunks_at_the_top_pad_above_the_first_line() {
    let long = "this line is long enough to wrap twice";
    let added = align("a\n", &format!("{long}\na\n"), &[hunk(0..0, 0..1)]);
    assert_eq!(added.sides[LEFT].lead, 3);
    assert!(added.sides[LEFT].fillers.is_empty());
    assert_eq!(
        added.sides[RIGHT].shades,
        [Shade {
            lines: 0..1,
            role: Background::Added,
        }]
    );

    let removed = align("x\ny\na\n", "a\n", &[hunk(0..2, 0..0)]);
    assert_eq!(removed.sides[RIGHT].lead, 2);
    assert_eq!(
        removed.sides[LEFT].shades,
        [Shade {
            lines: 0..2,
            role: Background::Removed,
        }]
    );
}

#[test]
fn removals_pad_after_the_preceding_line() {
    let align = align("a\nb\nc\nd\n", "a\nd\n", &[hunk(1..3, 1..1)]);
    assert_eq!(align.sides[RIGHT].fillers, [(0, 2)]);
    assert!(align.sides[LEFT].fillers.is_empty());
}

#[test]
fn facing_lines_map_through_hunks() {
    let hunks = [hunk(0..0, 0..1), hunk(2..3, 3..4), hunk(4..5, 5..5)];
    // Equal lines shift by the hunks before them.
    assert_eq!(diffview::facing(&hunks, Side::Before, 0), 1);
    assert_eq!(diffview::facing(&hunks, Side::After, 1), 0);
    assert_eq!(diffview::facing(&hunks, Side::Before, 3), 4);
    assert_eq!(diffview::facing(&hunks, Side::After, 6), 6);
    // Inside a hunk, the matching line of the other side.
    assert_eq!(diffview::facing(&hunks, Side::Before, 2), 3);
    // A removal faces the line where it was.
    assert_eq!(diffview::facing(&hunks, Side::Before, 4), 5);
    // An insertion at the top faces the first line.
    assert_eq!(diffview::facing(&hunks, Side::After, 0), 0);
}

/// One row of a laid-out side: its text, or `None` for a filler row.
fn rows_of(
    doc: &TextDoc,
    scroll: Scroll,
    padding: Padding,
    loader: &Loader,
    frame: &mut Frame,
) -> Vec<Option<String>> {
    let annotations = scroll::annotations(doc.text.slice(..), padding);
    display::layout(
        frame,
        doc,
        &Selection::point(0),
        scroll,
        loader,
        Focus::ReadOnly,
        &annotations,
        &[],
    );
    frame
        .rows
        .iter()
        .map(|row| match row.kind {
            RowKind::Virtual(_) => None,
            RowKind::First | RowKind::Wrapped => Some(frame.text[row.text.clone()].to_owned()),
        })
        .collect()
}

#[test]
fn both_sides_scroll_row_for_row() {
    let loader = helix_core::config::default_lang_loader();
    let head = "alpha\nbravo\ncharlie charlie charlie charlie\ndelta\necho\nfoxtrot\ngolf\n";
    let text = "a new first line, long enough to wrap\nalpha\nbravo\ncharlie mod\ndelta\n\
                foxtrot\ngolf\nhotel\nindia india india india india\n";
    let hunks = [
        hunk(0..0, 0..1),
        hunk(2..3, 3..4),
        hunk(4..5, 5..5),
        hunk(7..7, 7..9),
    ];
    let left = TextDoc::new(Rope::from(head), Path::new("a.txt"), &loader);
    let right = TextDoc::new(Rope::from(text), Path::new("a.txt"), &loader);
    let format = format();
    let mut align = Align::default();
    diffview::realign(
        &mut align,
        left.text.slice(..),
        right.text.slice(..),
        &hunks,
        &format,
    );
    let leads = [align.sides[LEFT].lead, align.sides[RIGHT].lead];
    // The inserted first line wraps; the HEAD side pads all of its rows above line 0.
    let inserted = diffview::rows(right.text.slice(..), 0..1, &format);
    assert!(inserted > 1);
    assert_eq!(leads, [inserted, 0]);
    let all = Viewport {
        rows: 100,
        cols: COLS,
    };
    let top = |lead: u32| Scroll {
        anchor: 0,
        rows: -(lead as isize),
        viewport: all,
    };
    let mut frame = Frame::default();
    let left_rows = rows_of(
        &left,
        top(leads[LEFT]),
        align.sides[LEFT].padding(),
        &loader,
        &mut frame,
    );
    let right_rows = rows_of(
        &right,
        top(leads[RIGHT]),
        align.sides[RIGHT].padding(),
        &loader,
        &mut frame,
    );
    // Same height, and every row holding text on both sides outside hunks holds the same text.
    assert_eq!(left_rows.len(), right_rows.len());
    for (row, (l, r)) in left_rows.iter().zip(&right_rows).enumerate() {
        match (l, r) {
            (Some(l), Some(r)) if l.starts_with("charlie") || r.starts_with("charlie") => {}
            (Some(l), Some(r)) => assert_eq!(l, r, "row {row}"),
            (None, _) | (_, None) => {}
        }
    }

    // Scrolling the buffer side row by row keeps the HEAD side on the facing row.
    let one = Viewport {
        rows: 1,
        cols: COLS,
    };
    let right_padding = align.sides[RIGHT].padding();
    let annotations = scroll::annotations(right.text.slice(..), right_padding);
    for row in 0..right_rows.len() {
        let (anchor, rows) = scroll::offset(
            right.text.slice(..),
            0,
            0,
            row as isize,
            leads[RIGHT],
            &format,
            &annotations,
        );
        let at = Scroll {
            anchor,
            rows,
            viewport: one,
        };
        assert_eq!(
            rows_of(&right, at, right_padding, &loader, &mut frame),
            [right_rows[row].clone()],
            "buffer side, row {row}"
        );
        let spot = diffview::spot(
            right.text.slice(..),
            at,
            Side::After,
            &hunks,
            &align,
            &format,
        );
        let (anchor, rows) = diffview::place(
            left.text.slice(..),
            spot,
            Side::Before,
            &hunks,
            &align,
            &format,
        );
        let facing = Scroll {
            anchor,
            rows,
            viewport: one,
        };
        assert_eq!(
            rows_of(
                &left,
                facing,
                align.sides[LEFT].padding(),
                &loader,
                &mut frame
            ),
            [left_rows[row].clone()],
            "HEAD side, row {row}"
        );
    }
}

#[test]
fn deleted_lines_at_the_top_scroll_through_the_lead() {
    let loader = helix_core::config::default_lang_loader();
    let left = TextDoc::new(Rope::from("x\ny\na\nb\n"), Path::new("a.txt"), &loader);
    let right = TextDoc::new(Rope::from("a\nb\n"), Path::new("a.txt"), &loader);
    let hunks = [hunk(0..2, 0..0)];
    let format = format();
    let mut align = Align::default();
    diffview::realign(
        &mut align,
        left.text.slice(..),
        right.text.slice(..),
        &hunks,
        &format,
    );
    assert_eq!(align.sides[RIGHT].lead, 2);
    let annotations = scroll::annotations(right.text.slice(..), align.sides[RIGHT].padding());
    // From the top of the lead, two rows down reaches the first line, and back up again.
    let down = scroll::offset(right.text.slice(..), 0, -2, 2, 2, &format, &annotations);
    assert_eq!(down, (0, 0));
    let up = scroll::offset(right.text.slice(..), 0, 0, -5, 2, &format, &annotations);
    assert_eq!(up, (0, -2));
    let one = Viewport {
        rows: 1,
        cols: COLS,
    };
    let at = |rows: isize| Scroll {
        anchor: 0,
        rows,
        viewport: one,
    };
    // The second lead row faces HEAD's second removed line.
    let spot = diffview::spot(
        right.text.slice(..),
        at(-1),
        Side::After,
        &hunks,
        &align,
        &format,
    );
    assert_eq!(
        diffview::place(
            left.text.slice(..),
            spot,
            Side::Before,
            &hunks,
            &align,
            &format
        ),
        (left.text.line_to_char(1), 0)
    );
}
