use std::path::Path;

use git2::Oid;
use helix_core::text_annotations::TextAnnotations;
use helix_core::{Rope, Selection};
use model::mode::Mode;
use model::theme::Background;
use state::buffer::TextDoc;
use state::diff::{
    Base, Hunk, Lines, Outcome, Removal, Side, Stage, Stamp, hunk_at, intersecting, next_hunk,
    prev_hunk, splice, stage,
};
use state::display::{self, Fillers, Focus, Frame, RowKind, Shade};
use state::layout::{Scroll, Viewport};

fn blob(byte: u8, text: &str) -> Base {
    Base::Blob {
        oid: Oid::from_bytes(&[byte; 20]).unwrap(),
        text: Rope::from(text),
    }
}

fn hunk(before: std::ops::Range<u32>, after: std::ops::Range<u32>) -> Hunk {
    Hunk { before, after }
}

/// HEAD-to-text hunks, with the index equal to HEAD.
fn diff(base: &str, text: &str) -> Vec<Hunk> {
    let head = blob(1, base);
    let mut lines = Lines::new();
    let stamp = Stamp {
        version: 0,
        bases: [head.oid(), head.oid()],
    };
    match lines.compute(stamp, &head, &head, Rope::from(text)) {
        Outcome::Diffed { .. } => lines.hunks.clone(),
        Outcome::TooLarge => panic!("small input"),
    }
}

// Ported from helix-vcs/src/diff/worker/test.rs.
#[test]
fn append_line() {
    assert_eq!(diff("foo\n", "foo\nbar\n"), [hunk(1..1, 1..2)]);
}

#[test]
fn prepend_line() {
    assert_eq!(diff("foo\n", "bar\nfoo\n"), [hunk(0..0, 0..1)]);
}

#[test]
fn modify() {
    assert_eq!(diff("foo\nbar\n", "foo bar\nbar\n"), [hunk(0..1, 0..1)]);
}

#[test]
fn delete_line() {
    assert_eq!(
        diff("foo\nfoo bar\nbar\n", "foo\nbar\n"),
        [hunk(1..2, 1..1)]
    );
}

#[test]
fn delete_line_and_modify() {
    assert_eq!(
        diff("foo\nbar\ntest\nfoo", "foo\ntest\nfoo bar"),
        [hunk(1..2, 1..1), hunk(3..4, 2..3)]
    );
}

#[test]
fn add_use() {
    assert_eq!(
        diff(
            "use ropey::Rope;\nuse tokio::task::JoinHandle;\n",
            "use ropey::Rope;\nuse ropey::RopeSlice;\nuse tokio::task::JoinHandle;\n",
        ),
        [hunk(1..1, 1..2)]
    );
}

#[test]
fn update_document_and_base_reuse_the_cache() {
    let mut lines = Lines::new();
    let head = blob(1, "foo\nbar\ntest\nfoo");
    let index = blob(2, "foo\nbar\ntest\nfoo bar");
    let bases = [head.oid(), index.oid()];
    let first = lines.compute(
        Stamp { version: 0, bases },
        &head,
        &index,
        Rope::from("foo\nbar\ntest\nfoo"),
    );
    assert!(matches!(first, Outcome::Diffed { staged: Some(_) }));
    assert!(lines.hunks.is_empty());
    assert_eq!(lines.unstaged, [hunk(3..4, 3..4)]);

    // Same bases: only the text is re-interned and the staged hunks are not recomputed.
    let second = lines.compute(
        Stamp { version: 1, bases },
        &head,
        &index,
        Rope::from("foo\ntest\nfoo bar"),
    );
    assert!(matches!(second, Outcome::Diffed { staged: None }));
    assert_eq!(lines.hunks, [hunk(1..2, 1..1), hunk(3..4, 2..3)]);
    assert_eq!(lines.unstaged, [hunk(1..2, 1..1)]);

    let moved = blob(3, "foo\ntest\nfoo bar");
    let third = lines.compute(
        Stamp {
            version: 1,
            bases: [moved.oid(), moved.oid()],
        },
        &moved,
        &moved,
        Rope::from("foo\ntest\nfoo bar"),
    );
    match third {
        Outcome::Diffed {
            staged: Some(staged),
        } => assert!(staged.is_empty()),
        Outcome::Diffed { staged: None } | Outcome::TooLarge => panic!("bases moved"),
    }
    assert!(lines.hunks.is_empty());
}

#[test]
fn absent_base_diffs_as_empty() {
    let mut lines = Lines::new();
    let index = blob(2, "a\nb\n");
    let stamp = Stamp {
        version: 0,
        bases: [None, index.oid()],
    };
    lines.compute(stamp, &Base::Absent, &index, Rope::from("a\nb\n"));
    assert_eq!(lines.hunks, [hunk(0..0, 0..2)]);
    assert!(lines.unstaged.is_empty());
}

fn sample() -> Vec<Hunk> {
    // An insertion, a removal shown on line 6 and a modification.
    vec![hunk(1..1, 1..3), hunk(4..6, 6..6), hunk(8..9, 8..10)]
}

#[test]
fn next_and_previous_hunks_by_line() {
    let hunks = sample();
    assert_eq!(next_hunk(&hunks, Side::After, 0), Some(0));
    assert_eq!(next_hunk(&hunks, Side::After, 1), Some(1));
    assert_eq!(next_hunk(&hunks, Side::After, 6), Some(2));
    assert_eq!(next_hunk(&hunks, Side::After, 8), None);
    assert_eq!(prev_hunk(&hunks, Side::After, 9), Some(1));
    assert_eq!(prev_hunk(&hunks, Side::After, 10), Some(2));
    // On a removal's line the cursor is inside it, so the previous hunk is the one before.
    assert_eq!(prev_hunk(&hunks, Side::After, 6), Some(0));
    assert_eq!(prev_hunk(&hunks, Side::After, 3), Some(0));
    assert_eq!(prev_hunk(&hunks, Side::After, 1), None);
    assert_eq!(next_hunk(&hunks, Side::Before, 4), Some(2));
}

#[test]
fn hunk_at_line() {
    let hunks = sample();
    assert_eq!(hunk_at(&hunks, Side::After, 2, Removal::Exclude), Some(0));
    assert_eq!(hunk_at(&hunks, Side::After, 3, Removal::Exclude), None);
    assert_eq!(hunk_at(&hunks, Side::After, 6, Removal::Include), Some(1));
    assert_eq!(hunk_at(&hunks, Side::After, 7, Removal::Include), None);
    assert_eq!(hunk_at(&hunks, Side::After, 9, Removal::Exclude), Some(2));
}

#[test]
fn hunks_intersecting_line_ranges_once_each() {
    let hunks = sample();
    let found: Vec<&Hunk> =
        intersecting(&hunks, Side::After, [(0, 1), (2, 2), (6, 9)].into_iter()).collect();
    assert_eq!(found, [&hunks[0], &hunks[1], &hunks[2]]);
    let none: Vec<&Hunk> = intersecting(&hunks, Side::After, [(4, 5)].into_iter()).collect();
    assert!(none.is_empty());
}

#[test]
fn stage_follows_the_unstaged_hunks() {
    let shown = hunk(2..4, 2..5);
    assert_eq!(stage(&shown, &[]), Stage::Staged);
    assert_eq!(stage(&shown, &[hunk(2..4, 2..5)]), Stage::Unstaged);
    assert_eq!(stage(&shown, &[hunk(3..4, 3..4)]), Stage::Partial);
    assert_eq!(stage(&shown, &[hunk(9..9, 9..10)]), Stage::Staged);
    let removal = hunk(4..6, 6..6);
    assert_eq!(stage(&removal, &[hunk(4..6, 6..6)]), Stage::Unstaged);
}

#[test]
fn splice_replaces_hunk_lines_from_the_other_side() {
    let index = Rope::from("a\nb\nc\nd\n");
    let text = Rope::from("a\nB\nc\nx\nd\n");
    let hunks = [hunk(1..2, 1..2), hunk(3..3, 3..4)];
    assert_eq!(
        splice(&index, &text, hunks.iter(), Side::Before),
        "a\nB\nc\nx\nd\n"
    );
    assert_eq!(
        splice(&index, &text, hunks[1..].iter(), Side::Before),
        "a\nb\nc\nx\nd\n"
    );
    assert_eq!(
        splice(&text, &index, hunks[..1].iter(), Side::After),
        "a\nb\nc\nx\nd\n"
    );
}

#[test]
fn fillers_insert_virtual_rows_and_shades_colour_lines() {
    let loader = helix_core::config::default_lang_loader();
    let doc = TextDoc::new(Rope::from("a\nb\nc\nd\n"), Path::new("a.txt"), &loader);
    let after = [(0, 2), (2, 1)];
    let mut annotations = TextAnnotations::default();
    annotations.add_line_annotation(Box::new(Fillers::new(doc.text.slice(..), &after)));
    let shades = [Shade {
        lines: 1..3,
        role: Background::Added,
    }];
    let mut frame = Frame::default();
    display::layout(
        &mut frame,
        &doc,
        &Selection::point(0),
        Scroll {
            anchor: 0,
            viewport: Viewport { rows: 7, cols: 40 },
        },
        &loader,
        Focus::Focused(Mode::Normal),
        &annotations,
        &shades,
    );
    let rows: Vec<(usize, RowKind, Option<Background>)> = frame
        .rows
        .iter()
        .map(|row| (row.line, row.kind, row.background))
        .collect();
    assert_eq!(
        rows,
        [
            (0, RowKind::First, None),
            (0, RowKind::Virtual(0), Some(Background::Filler)),
            (0, RowKind::Virtual(1), Some(Background::Filler)),
            (1, RowKind::First, Some(Background::Added)),
            (2, RowKind::First, Some(Background::Added)),
            (2, RowKind::Virtual(0), Some(Background::Filler)),
            (3, RowKind::First, None),
        ]
    );
    assert!(
        frame
            .rows
            .iter()
            .filter(|row| matches!(row.kind, RowKind::Virtual(_)))
            .all(|row| row.text.is_empty())
    );
}

#[test]
fn virtual_rows_fill_the_viewport_end() {
    let loader = helix_core::config::default_lang_loader();
    let doc = TextDoc::new(Rope::from("a\nb\nc\n"), Path::new("a.txt"), &loader);
    let after = [(1, 5)];
    let mut annotations = TextAnnotations::default();
    annotations.add_line_annotation(Box::new(Fillers::new(doc.text.slice(..), &after)));
    let mut frame = Frame::default();
    display::layout(
        &mut frame,
        &doc,
        &Selection::point(0),
        Scroll {
            anchor: 0,
            viewport: Viewport { rows: 4, cols: 40 },
        },
        &loader,
        Focus::Unfocused,
        &annotations,
        &[],
    );
    let kinds: Vec<RowKind> = frame.rows.iter().map(|row| row.kind).collect();
    assert_eq!(
        kinds,
        [
            RowKind::First,
            RowKind::First,
            RowKind::Virtual(0),
            RowKind::Virtual(1)
        ]
    );
}
