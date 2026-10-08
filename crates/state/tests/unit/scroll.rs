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

mod mouse {
    use helix_core::{Range, Rope};
    use state::layout::{View, Viewport};
    use state::scroll::{self, Gesture, Press, Unit};

    use crate::harness::Harness;

    fn harness(marked: &str) -> Harness {
        let mut h = Harness::with("m.txt", marked);
        let focus = h.state.layout.focus;
        scroll::set_viewport(&mut h.state, focus, Viewport { rows: 10, cols: 40 });
        h
    }

    fn press(h: &mut Harness, row: u16, col: u16, press: Press) -> Option<Gesture> {
        let focus = h.state.layout.focus;
        h.run(|state, _| scroll::press(state, focus, row, col, press))
    }

    #[test]
    fn units_around_a_position() {
        let text = Rope::from("foo bar_baz\nnext\n");
        let text = text.slice(..);
        assert_eq!(Unit::Char.around(text, 5), Range::point(5));
        assert_eq!(Unit::Word.around(text, 5), Range::new(4, 11));
        // Whitespace has no word; the click stays a cursor.
        assert_eq!(Unit::Word.around(text, 3), Range::point(3));
        assert_eq!(Unit::Line.around(text, 5), Range::new(0, 12));
        assert_eq!(Unit::Line.around(text, 13), Range::new(12, 17));
        assert_eq!(Unit::from_clicks(1), Unit::Char);
        assert_eq!(Unit::from_clicks(2), Unit::Word);
        assert_eq!(Unit::from_clicks(3), Unit::Line);
        assert_eq!(Unit::from_clicks(4), Unit::Line);
    }

    #[test]
    fn drags_extend_by_whole_units() {
        let text = Rope::from("one two three\nfour\n");
        let text = text.slice(..);
        let word = Gesture {
            unit: Unit::Word,
            origin: Range::new(4, 7),
        };
        assert_eq!(scroll::extend(text, word, 10), Range::new(4, 13));
        assert_eq!(scroll::extend(text, word, 1), Range::new(7, 0));
        assert_eq!(scroll::extend(text, word, 5), Range::new(4, 7));
        let line = Gesture {
            unit: Unit::Line,
            origin: Range::new(14, 19),
        };
        assert_eq!(scroll::extend(text, line, 2), Range::new(19, 0));
    }

    #[test]
    fn double_and_triple_click_select_word_and_line() {
        let mut h = harness("#[f|]#oo bar\nbaz\n");
        press(&mut h, 0, 5, Press::Select(Unit::Word));
        assert_eq!(h.plain(), "foo #[bar|]#\nbaz\n");
        let gesture = press(&mut h, 0, 1, Press::Select(Unit::Line)).expect("text pane");
        assert_eq!(h.plain(), "#[foo bar\n|]#baz\n");
        let focus = h.state.layout.focus;
        h.run(|state, _| scroll::drag(state, focus, 1, 0, gesture));
        assert_eq!(h.plain(), "#[foo bar\nbaz\n|]#");
    }

    #[test]
    fn alt_click_adds_a_cursor() {
        let mut h = harness("#[f|]#oo bar\n");
        let gesture = press(&mut h, 0, 4, Press::AddCursor);
        assert_eq!(gesture, None);
        assert_eq!(h.plain(), "#(f|)#oo #[b|]#ar\n");
    }

    #[test]
    fn release_copies_wide_selections_to_primary() {
        let mut h = harness("#[f|]#oo bar\n");
        let focus = h.state.layout.focus;
        press(&mut h, 0, 1, Press::Select(Unit::Char));
        h.run(|state, out| scroll::release(state, focus, out));
        assert_eq!(h.clipboard[1], None);
        press(&mut h, 0, 5, Press::Select(Unit::Word));
        h.run(|state, out| scroll::release(state, focus, out));
        assert_eq!(h.clipboard[1].as_deref(), Some("bar"));
    }

    #[test]
    fn middle_click_pastes_primary_at_the_pointer() {
        let mut h = harness("#[f|]#oo bar\nbaz\n");
        h.clipboard[1] = Some("XY".to_owned());
        let focus = h.state.layout.focus;
        h.run(|state, out| scroll::middle_click(state, focus, 1, 1, out));
        assert_eq!(h.plain(), "foo bar\nb#[XY|]#az\n");
    }

    #[test]
    fn scroll_to_line_drags_the_cursor_along() {
        let text: String = (0..100).map(|i| format!("{i}\n")).collect();
        let mut h = harness(&format!("#[0|]#{}", &text[1..]));
        let focus = h.state.layout.focus;
        h.run(|state, _| scroll::scroll_to_line(state, focus, 50));
        let (line, top) = match &h.state.layout.panes[focus].view {
            View::Text(view) => {
                let text = h.doc().text.slice(..);
                (
                    text.char_to_line(view.selection.primary().cursor(text)),
                    text.char_to_line(view.scroll.anchor),
                )
            }
            View::Empty => panic!("no view"),
        };
        // A 10-row view narrows the scrolloff band to 4 rows.
        assert_eq!((line, top), (54, 50));
    }
}

mod scrollbar {
    use state::scroll::{Scrollbar, Thumb, line_at_thumb, scrollbar};

    #[test]
    fn hidden_when_everything_fits() {
        assert_eq!(scrollbar(10, 0..10, 100., 10.), Scrollbar::Hidden);
        assert_eq!(scrollbar(10, 0..12, 100., 10.), Scrollbar::Hidden);
    }

    #[test]
    fn thumb_spans_the_visible_lines() {
        assert_eq!(
            scrollbar(100, 0..25, 200., 10.),
            Scrollbar::Shown(Thumb {
                top: 0.,
                height: 50.
            })
        );
        assert_eq!(
            scrollbar(100, 75..100, 200., 10.),
            Scrollbar::Shown(Thumb {
                top: 150.,
                height: 50.
            })
        );
        // Scrolled to the last line, the bar stays shown and pinned to the bottom.
        assert_eq!(
            scrollbar(100, 99..100, 200., 10.),
            Scrollbar::Shown(Thumb {
                top: 190.,
                height: 10.
            })
        );
    }

    #[test]
    fn small_thumbs_are_clamped_and_still_reach_both_ends() {
        let Scrollbar::Shown(top) = scrollbar(100_000, 0..50, 200., 16.) else {
            panic!("shown");
        };
        assert_eq!(top.height, 16.);
        let Scrollbar::Shown(bottom) = scrollbar(100_000, 99_950..100_000, 200., 16.) else {
            panic!("shown");
        };
        assert_eq!(bottom.top + bottom.height, 200.);
    }

    #[test]
    fn dragging_inverts_the_geometry() {
        let (len, shown, track) = (1_000, 40, 400.);
        for first in [0, 1, 123, 480, 960] {
            let Scrollbar::Shown(thumb) = scrollbar(len, first..first + shown, track, 16.) else {
                panic!("shown");
            };
            assert_eq!(
                line_at_thumb(len, shown, thumb.height, track, thumb.top),
                first
            );
        }
        assert_eq!(line_at_thumb(len, shown, 16., track, -50.), 0);
        assert_eq!(line_at_thumb(len, shown, 16., track, 1e6), len - shown);
    }
}
