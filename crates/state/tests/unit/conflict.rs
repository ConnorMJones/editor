use std::path::Path;

use helix_core::{Rope, Selection};
use model::theme::Background;
use state::git::conflict::{self, Conflict, Conflicts, Label, Side};
use state::text::buffer::TextDoc;
use state::view::display::{self, Focus, Frame, RowKind, Shade};
use state::view::layout::{Scroll, Viewport};

fn parse(text: &str) -> Conflicts {
    let mut conflicts = Conflicts::EMPTY;
    conflict::parse(Rope::from(text).slice(..), 7, &mut conflicts);
    assert_eq!(conflicts.version, 7);
    conflicts
}

fn two_way(open: usize, separator: usize, close: usize) -> Conflict {
    Conflict {
        open,
        base: None,
        separator,
        close,
    }
}

const TWO_WAY: &str = "\
keep
<<<<<<< HEAD
ours 1
ours 2
=======
theirs
>>>>>>> feature
after
";

#[test]
fn parses_a_two_way_conflict_with_sections_and_labels() {
    let conflicts = parse(TWO_WAY);
    assert_eq!(conflicts.list, [two_way(1, 4, 6)]);
    let conflict = conflicts.list[0];
    assert_eq!(conflict.ours(), 2..4);
    assert_eq!(conflict.base_lines(), None);
    assert_eq!(conflict.theirs(), 5..6);
    assert_eq!(conflict.lines(), 1..7);
    assert_eq!(
        conflicts.labels,
        [
            Label {
                line: 0,
                side: Side::Ours,
                marker: 1
            },
            Label {
                line: 6,
                side: Side::Theirs,
                marker: 6
            },
        ]
    );
    assert_eq!(conflicts.rows, [(0, 1), (6, 1)]);
    let shades: Vec<(std::ops::Range<usize>, Background)> = conflicts
        .shades
        .iter()
        .map(|shade| (shade.lines.clone(), shade.role))
        .collect();
    assert_eq!(
        shades,
        [
            (1..2, Background::OursMarker),
            (2..4, Background::Ours),
            (4..5, Background::TheirsMarker),
            (5..6, Background::Theirs),
            (6..7, Background::TheirsMarker),
        ]
    );
}

#[test]
fn parses_a_diff3_conflict() {
    let conflicts = parse(
        "<<<<<<< ours\na\n||||||| merged common ancestors\nbase\n=======\nb\n>>>>>>> theirs\n",
    );
    let conflict = conflicts.list[0];
    assert_eq!(
        conflict,
        Conflict {
            open: 0,
            base: Some(2),
            separator: 4,
            close: 6
        }
    );
    assert_eq!(conflict.ours(), 1..2);
    assert_eq!(conflict.base_lines(), Some(3..4));
    assert_eq!(conflict.theirs(), 5..6);
    assert!(conflicts.shades.contains(&Shade {
        lines: 2..3,
        role: Background::BaseMarker
    }));
    assert!(conflicts.shades.contains(&Shade {
        lines: 3..4,
        role: Background::Base
    }));
}

#[test]
fn parses_crlf_text() {
    let conflicts = parse("x\r\n<<<<<<< HEAD\r\na\r\n=======\r\nb\r\n>>>>>>> topic\r\n");
    assert_eq!(conflicts.list, [two_way(1, 3, 5)]);
}

#[test]
fn markers_split_across_rope_chunks_still_parse() {
    // Rope chunks hold about a kilobyte, so some prefix length puts each marker on a boundary.
    for prefix in 900..1100 {
        let text = format!("{}\n{TWO_WAY}", "x".repeat(prefix));
        assert_eq!(
            parse(&text).list,
            [two_way(2, 5, 7)],
            "prefix of {prefix} bytes"
        );
    }
}

#[test]
fn markers_need_seven_characters_then_whitespace() {
    for text in [
        "<<<<<<<< HEAD\na\n=======\nb\n>>>>>>> x\n",
        "<<<<<<HEAD\na\n=======\nb\n>>>>>>> x\n",
        "<<<<<<<HEAD\na\n=======\nb\n>>>>>>> x\n",
        "<<<<<<< HEAD\na\n========\nb\n>>>>>>> x\n",
        "<<<<<<< HEAD\na\n=======\nb\n>>>>>>>x\n",
        " <<<<<<< HEAD\na\n=======\nb\n>>>>>>> x\n",
    ] {
        assert_eq!(parse(text).list, [], "{text:?}");
    }
    assert_eq!(
        parse("<<<<<<<\na\n=======\nb\n>>>>>>>").list,
        [two_way(0, 2, 4)],
        "labels are optional"
    );
}

#[test]
fn malformed_or_unterminated_conflicts_are_ignored() {
    for text in [
        "<<<<<<< HEAD\na\n=======\nb\n",
        "<<<<<<< HEAD\na\n>>>>>>> x\n",
        "=======\nb\n>>>>>>> x\n",
        "a\n||||||| base\n=======\n>>>>>>> x\n",
        ">>>>>>> x\n<<<<<<< HEAD\n",
    ] {
        let conflicts = parse(text);
        assert_eq!(conflicts.list, [], "{text:?}");
        assert!(conflicts.shades.is_empty() && conflicts.labels.is_empty());
    }
}

#[test]
fn a_new_open_marker_abandons_the_unfinished_conflict() {
    let text = "\
<<<<<<< outer
a
<<<<<<< inner
b
=======
c
>>>>>>> inner
d
=======
e
>>>>>>> outer
";
    assert_eq!(parse(text).list, [two_way(2, 4, 6)]);
}

#[test]
fn several_conflicts_and_adjacent_label_rows() {
    let text = "\
<<<<<<< HEAD
a
=======
b
>>>>>>> x
<<<<<<< HEAD
c
=======
d
>>>>>>> x
middle
<<<<<<< HEAD
=======
e
>>>>>>> x";
    let conflicts = parse(text);
    assert_eq!(
        conflicts.list,
        [two_way(0, 2, 4), two_way(5, 7, 9), two_way(11, 12, 14)]
    );
    // Line 0 has no row above it, and line 4 ends one conflict right before the next.
    assert_eq!(conflicts.rows, [(0, 1), (4, 2), (9, 1), (10, 1), (14, 1)]);
    let sides: Vec<(usize, Side)> = conflicts
        .labels
        .iter()
        .map(|label| (label.line, label.side))
        .collect();
    assert_eq!(
        sides,
        [
            (0, Side::Ours),
            (4, Side::Theirs),
            (4, Side::Ours),
            (9, Side::Theirs),
            (10, Side::Ours),
            (14, Side::Theirs),
        ]
    );
    assert!(
        conflicts
            .shades
            .windows(2)
            .all(|pair| pair[0].lines.end <= pair[1].lines.start),
        "shades are sorted and disjoint; the empty ours section of the last is skipped"
    );
}

#[test]
fn documents_parse_on_creation_and_refresh_after_edits() {
    let loader = helix_core::config::default_lang_loader();
    let mut doc = TextDoc::new(Rope::from(TWO_WAY), Path::new("a.txt"), &loader);
    assert_eq!(conflict::current(&doc).list.len(), 1);
    doc.text = Rope::from("plain\n");
    doc.version += 1;
    assert!(
        conflict::current(&doc).list.is_empty(),
        "stale conflicts are hidden"
    );
    assert_eq!(doc.conflicts.list.len(), 1);
    conflict::refresh(&mut doc);
    assert_eq!(doc.conflicts.version, doc.version);
    assert!(doc.conflicts.list.is_empty());
}

fn rows(text: &str, rows: u16) -> Vec<(usize, RowKind, Option<Background>, String)> {
    let loader = helix_core::config::default_lang_loader();
    let doc = TextDoc::new(Rope::from(text), Path::new("a.txt"), &loader);
    let conflicts = conflict::current(&doc);
    let mut frame = Frame::default();
    display::layout(
        &mut frame,
        &doc,
        &Selection::point(0),
        Scroll {
            anchor: 0,
            rows: 0,
            viewport: Viewport { rows, cols: 60 },
        },
        &loader,
        Focus::Unfocused,
        &conflict::annotations(&doc),
        &conflicts.shades,
    );
    conflict::label(&mut frame, doc.text.slice(..), conflicts, rows);
    frame
        .rows
        .iter()
        .map(|row| {
            (
                row.line,
                row.kind,
                row.background,
                frame.text[row.text.clone()].to_owned(),
            )
        })
        .collect()
}

#[test]
fn label_rows_name_each_side_and_its_keys() {
    let rows = rows(TWO_WAY, 12);
    let labels: Vec<(usize, RowKind, Option<Background>, &str)> = rows
        .iter()
        .filter(|row| matches!(row.1, RowKind::Virtual(_)))
        .map(|row| (row.0, row.1, row.2, row.3.as_str()))
        .collect();
    assert_eq!(
        labels,
        [
            (
                0,
                RowKind::Virtual(0),
                Some(Background::OursMarker),
                "ours (HEAD) — space m o"
            ),
            (
                6,
                RowKind::Virtual(0),
                Some(Background::TheirsMarker),
                "theirs (feature) — space m t"
            ),
        ]
    );
    let backgrounds: Vec<Option<Background>> = rows.iter().map(|row| row.2).collect();
    assert_eq!(
        backgrounds,
        [
            None,
            Some(Background::OursMarker),
            Some(Background::OursMarker),
            Some(Background::Ours),
            Some(Background::Ours),
            Some(Background::TheirsMarker),
            Some(Background::Theirs),
            Some(Background::TheirsMarker),
            Some(Background::TheirsMarker),
            None,
            None,
        ]
    );
}

#[test]
fn a_conflict_at_line_zero_and_end_of_file_keeps_its_labels() {
    let rows = rows("<<<<<<<\na\n=======\nb\n>>>>>>> end", 10);
    let shown: Vec<(usize, RowKind, &str)> = rows
        .iter()
        .map(|row| (row.0, row.1, row.3.as_str()))
        .collect();
    assert_eq!(
        shown,
        [
            (0, RowKind::First, "<<<<<<<"),
            (0, RowKind::Virtual(0), "ours — space m o"),
            (1, RowKind::First, "a"),
            (2, RowKind::First, "======="),
            (3, RowKind::First, "b"),
            (4, RowKind::First, ">>>>>>>·end"),
            (4, RowKind::Virtual(0), "theirs (end) — space m t"),
        ]
    );
}
