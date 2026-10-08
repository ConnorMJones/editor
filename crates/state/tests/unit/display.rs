use std::path::Path;

use helix_core::syntax::Loader;
use helix_core::{Range, Rope, Selection};
use model::color::Base16;
use model::mode::Mode;
use model::theme::{self, Modifiers, Style};
use state::buffer::{BufferId, TextDoc};
use state::display::{self, CursorShape, Focus, Frame, Gutter, LineMark, Numbering, Rank, RowKind};
use state::layout::{Scroll, TextView, Viewport};

fn loader() -> Loader {
    let loader = helix_core::config::default_lang_loader();
    loader.set_scopes(
        theme::SCOPES
            .iter()
            .map(|(scope, _)| (*scope).to_owned())
            .collect(),
    );
    loader
}

fn view(selection: Selection, rows: u16, cols: u16) -> TextView {
    TextView {
        buffer: BufferId::default(),
        selection,
        scroll: Scroll {
            anchor: 0,
            viewport: Viewport { rows, cols },
        },
    }
}

fn rows(frame: &Frame) -> Vec<&str> {
    frame
        .rows
        .iter()
        .map(|r| &frame.text[r.text.clone()])
        .collect()
}

/// Style of the run covering byte `at` of row `row`.
fn style_at(frame: &Frame, row: usize, at: usize) -> Style {
    let mut start = 0;
    for run in &frame.runs[frame.rows[row].runs.clone()] {
        match start + run.len > at {
            true => return run.style,
            false => start += run.len,
        }
    }
    panic!("byte {at} outside row {row}");
}

#[test]
fn relative_numbers_keep_cursor_line_absolute() {
    assert_eq!(display::line_label(9, 9, Numbering::Relative), 10);
    assert_eq!(display::line_label(4, 9, Numbering::Relative), 5);
    assert_eq!(display::line_label(12, 9, Numbering::Relative), 3);
    assert_eq!(display::line_label(4, 9, Numbering::Absolute), 5);
}

#[test]
fn gutter_fits_at_least_three_digits() {
    assert_eq!(Gutter::new(1), Gutter { digits: 3, cols: 7 });
    assert_eq!(Gutter::new(999), Gutter { digits: 3, cols: 7 });
    assert_eq!(Gutter::new(1000), Gutter { digits: 4, cols: 8 });
}

#[test]
fn style_patch_layers_like_helix() {
    let heading = Style::fg(Base16::Base0D).with(Modifiers::BOLD);
    let italic = Style {
        fg: None,
        modifiers: Modifiers::ITALIC,
    };
    assert_eq!(
        heading.patch(italic),
        Style::fg(Base16::Base0D).with(Modifiers::BOLD | Modifiers::ITALIC)
    );
    assert_eq!(
        heading.patch(Style::fg(Base16::Base08)).fg,
        Some(Base16::Base08)
    );
}

#[test]
fn whitespace_and_indent_guides() {
    let loader = loader();
    let doc = TextDoc::new(
        Rope::from("fn a() {\n        x\n\n\tb\n"),
        Path::new("a.txt"),
        &loader,
    );
    let mut frame = Frame::default();
    display::layout(
        &mut frame,
        &doc,
        &view(Selection::point(0), 10, 40),
        &loader,
        Focus::Focused(Mode::Normal),
    );
    assert_eq!(
        rows(&frame),
        ["fn·a()·{", "│···│···x", "│   │", "│   b", "│"]
    );
    assert_eq!(style_at(&frame, 1, 0), theme::INDENT_GUIDE);
    assert_eq!(style_at(&frame, 0, 2), theme::TEXT.patch(theme::WHITESPACE));
}

#[test]
fn soft_wrap_marks_continuation_rows() {
    let loader = loader();
    let line = "word ".repeat(10);
    let doc = TextDoc::new(
        Rope::from(format!("{line}\nend")),
        Path::new("a.txt"),
        &loader,
    );
    let mut frame = Frame::default();
    display::layout(
        &mut frame,
        &doc,
        &view(Selection::point(0), 10, 20),
        &loader,
        Focus::Focused(Mode::Normal),
    );
    let kinds: Vec<RowKind> = frame.rows.iter().map(|r| r.kind).collect();
    assert_eq!(
        kinds,
        [
            RowKind::First,
            RowKind::Wrapped,
            RowKind::Wrapped,
            RowKind::First
        ]
    );
    assert!(rows(&frame)[1].starts_with("↪ "));
    assert_eq!(frame.rows[3].line, 1);
}

#[test]
fn selections_and_cursors_land_on_cells() {
    let loader = loader();
    let doc = TextDoc::new(Rope::from("abcdef\nghij\n"), Path::new("a.txt"), &loader);
    let selection = Selection::new(
        [Range::new(1, 3), Range::new(7, 8)].into_iter().collect(),
        0,
    );
    let mut frame = Frame::default();
    display::layout(
        &mut frame,
        &doc,
        &view(selection, 10, 40),
        &loader,
        Focus::Focused(Mode::Insert),
    );
    let spans: Vec<(u16, std::ops::Range<u16>, Rank)> = frame
        .selections
        .iter()
        .map(|s| (s.row, s.cols.clone(), s.rank))
        .collect();
    assert_eq!(
        spans,
        [(0, 1..3, Rank::Primary), (1, 0..1, Rank::Secondary)]
    );
    let cursors: Vec<(u16, u16, Rank, CursorShape)> = frame
        .cursors
        .iter()
        .map(|c| (c.row, c.col, c.rank, c.shape))
        .collect();
    assert_eq!(
        cursors,
        [
            (0, 2, Rank::Primary, CursorShape::Bar),
            (1, 0, Rank::Secondary, CursorShape::Block),
        ]
    );
    assert_eq!(frame.primary_line, 0);
    assert_eq!(frame.rows[1].mark, LineMark::Cursor);
    assert_eq!(frame.rows[2].mark, LineMark::Plain);
    assert_eq!(style_at(&frame, 1, 0).fg, Some(theme::CURSOR_SECONDARY.fg));
}

#[test]
fn rust_captures_fall_back_to_theme_prefixes() {
    let loader = loader();
    let doc = TextDoc::new(
        Rope::from("fn main() { if true { let x = 42; } }\n"),
        Path::new("main.rs"),
        &loader,
    );
    assert!(
        doc.syntax.is_some(),
        "rust grammar missing from HELIX_RUNTIME"
    );
    let mut frame = Frame::default();
    display::layout(
        &mut frame,
        &doc,
        &view(Selection::point(0), 4, 80),
        &loader,
        Focus::Unfocused,
    );
    let text = rows(&frame)[0];
    let at = |needle: &str| style_at(&frame, 0, text.find(needle).expect("needle in row"));
    assert_eq!(at("main").fg, Some(Base16::Base0C));
    assert_eq!(at("if").fg, Some(Base16::Base0E));
    assert_eq!(at("42").fg, Some(Base16::Base09));
    assert_eq!(at("true").fg, Some(Base16::Base09));
}
