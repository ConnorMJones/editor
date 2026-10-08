use helix_core::doc_formatter::TextFormat;
use helix_core::graphemes::{self, next_grapheme_boundary};
use helix_core::line_ending::line_end_char_index;
use helix_core::movement::{self, Movement};
use helix_core::text_annotations::TextAnnotations;
use helix_core::textobject::{self, TextObject};
use helix_core::{
    Range, RopeSlice, char_idx_at_visual_offset, match_brackets, search, visual_offset_from_anchor,
};
use helix_stdx::rope::RopeSliceExt;
use model::command::{
    Align, CharTarget, Direction, Find, Inclusion, Motion, Object, ObjectScope, ScrollBy,
    SyntaxObject,
};
use model::mode::Mode;
use model::notice::Notice;

use crate::State;
use crate::apply::{count_of, focused, movement_of, reselect, select, try_select};
use crate::display::{Indent, text_format};
use crate::layout::Viewport;
use crate::scroll::{self, SCROLLOFF};

pub(crate) fn direction(direction: Direction) -> movement::Direction {
    match direction {
        Direction::Forward => movement::Direction::Forward,
        Direction::Backward => movement::Direction::Backward,
    }
}

type WordMotion = fn(RopeSlice, Range, usize) -> Range;
type MoveFn = fn(
    RopeSlice,
    Range,
    movement::Direction,
    usize,
    Movement,
    &TextFormat,
    &mut TextAnnotations,
) -> Range;

pub fn motion(state: &mut State, motion: Motion) {
    let count = count_of(state);
    let behaviour = movement_of(state);
    let extend = behaviour == Movement::Extend;
    let viewport = match focused(&mut state.layout, &mut state.buffers) {
        Some((view, _)) => view.scroll.viewport,
        None => Viewport::default(),
    };
    // Same format as drawn, so visual lines follow soft wrap.
    let vertical = |state: &mut State, f: MoveFn, dir: Direction| {
        select(state, |doc, selection, loader| {
            let format = text_format(viewport, Indent::of(doc, loader));
            let mut annotations = TextAnnotations::default();
            let text = doc.text.slice(..);
            selection.clone().transform(|range| {
                f(
                    text,
                    range,
                    direction(dir),
                    count,
                    behaviour,
                    &format,
                    &mut annotations,
                )
            })
        });
    };
    let word = |state: &mut State, f: WordMotion| {
        reselect(state, |text, range| match behaviour {
            Movement::Move => f(text, range, count),
            Movement::Extend => {
                let word = f(text, range, count);
                range.put_cursor(text, word.cursor(text), true)
            }
        })
    };
    let to_line = |state: &mut State, line: Line| {
        reselect(state, |text, range| {
            let last = match text.line(text.len_lines() - 1).len_chars() {
                0 => text.len_lines().saturating_sub(2),
                _ => text.len_lines() - 1,
            };
            let line = match line {
                Line::Last => last,
                Line::Nth(n) => (n - 1).min(last),
            };
            range.put_cursor(text, text.line_to_char(line), extend)
        })
    };
    match motion {
        Motion::CharLeft => vertical(state, movement::move_horizontally, Direction::Backward),
        Motion::CharRight => vertical(state, movement::move_horizontally, Direction::Forward),
        Motion::VisualLineUp => {
            vertical(state, movement::move_vertically_visual, Direction::Backward)
        }
        Motion::VisualLineDown => {
            vertical(state, movement::move_vertically_visual, Direction::Forward)
        }
        Motion::LineUp => vertical(state, movement::move_vertically, Direction::Backward),
        Motion::LineDown => vertical(state, movement::move_vertically, Direction::Forward),
        Motion::NextWordStart => word(state, movement::move_next_word_start),
        Motion::PrevWordStart => word(state, movement::move_prev_word_start),
        Motion::NextWordEnd => word(state, movement::move_next_word_end),
        Motion::NextLongWordStart => word(state, movement::move_next_long_word_start),
        Motion::PrevLongWordStart => word(state, movement::move_prev_long_word_start),
        Motion::NextLongWordEnd => word(state, movement::move_next_long_word_end),
        Motion::LineStart => reselect(state, |text, range| {
            let line = range.cursor_line(text);
            range.put_cursor(text, text.line_to_char(line), extend)
        }),
        Motion::LineEnd => reselect(state, |text, range| {
            let line = range.cursor_line(text);
            let pos = graphemes::prev_grapheme_boundary(text, line_end_char_index(&text, line))
                .max(text.line_to_char(line));
            range.put_cursor(text, pos, extend)
        }),
        Motion::LineEndNewline => reselect(state, |text, range| {
            let line = range.cursor_line(text);
            range.put_cursor(text, line_end_char_index(&text, line), extend)
        }),
        Motion::FirstNonWhitespace => reselect(state, |text, range| {
            let line = range.cursor_line(text);
            match text.line(line).first_non_whitespace_char() {
                Some(pos) => range.put_cursor(text, pos + text.line_to_char(line), extend),
                None => range,
            }
        }),
        Motion::FileStart => match state.input.count {
            Some(n) => to_line(state, Line::Nth(n.get())),
            None => reselect(state, |text, range| range.put_cursor(text, 0, extend)),
        },
        Motion::LastLine => to_line(state, Line::Last),
        Motion::Line => match state.input.count {
            Some(n) => to_line(state, Line::Nth(n.get())),
            None => {}
        },
        Motion::Column => reselect(state, |text, range| {
            let line = range.cursor_line(text);
            let line_start = text.line_to_char(line);
            let pos = graphemes::nth_next_grapheme_boundary(text, line_start, count - 1)
                .min(line_end_char_index(&text, line));
            range.put_cursor(text, pos, extend)
        }),
        Motion::NextParagraph => reselect(state, |text, range| {
            movement::move_next_paragraph(text, range, count, behaviour)
        }),
        Motion::PrevParagraph => reselect(state, |text, range| {
            movement::move_prev_paragraph(text, range, count, behaviour)
        }),
        Motion::MatchingBracket => select(state, |doc, selection, _| {
            let text = doc.text.slice(..);
            selection.clone().transform(|range| {
                let pos = range.cursor(text);
                let matched = match &doc.syntax {
                    Some(syntax) => match_brackets::find_matching_bracket_fuzzy(syntax, text, pos),
                    None => match_brackets::find_matching_bracket_plaintext(text, pos),
                };
                match matched {
                    Some(pos) => range.put_cursor(text, pos, extend),
                    None => range,
                }
            })
        }),
        Motion::ParentNodeEnd | Motion::ParentNodeStart => {
            let dir = match motion {
                Motion::ParentNodeStart => movement::Direction::Backward,
                _ => movement::Direction::Forward,
            };
            select(state, |doc, selection, _| match &doc.syntax {
                Some(syntax) => movement::move_parent_node_end(
                    syntax,
                    doc.text.slice(..),
                    selection.clone(),
                    dir,
                    behaviour,
                ),
                None => selection.clone(),
            })
        }
        Motion::Window(align) => {
            let State {
                layout,
                buffers,
                languages,
                ..
            } = state;
            let Some((view, doc)) = focused(layout, buffers) else {
                return;
            };
            let text = doc.text.slice(..);
            let format = text_format(viewport, Indent::of(doc, languages));
            let annotations = TextAnnotations::default();
            let anchor = view.scroll.anchor.min(text.len_chars());
            let height = viewport.rows as usize;
            let scrolloff = SCROLLOFF.min(height.saturating_sub(1) / 2);
            // Last row that shows text; shorter documents end above the bottom edge.
            let last = match visual_offset_from_anchor(
                text,
                anchor,
                text.len_chars(),
                &format,
                &annotations,
                height,
            ) {
                Ok((pos, _)) => pos.row,
                Err(_) => height.saturating_sub(1),
            };
            let row = match align {
                Align::Top => scrolloff + count - 1,
                Align::Center => last / 2,
                Align::Bottom => last.saturating_sub(scrolloff + count - 1),
            };
            let row = row.max(scrolloff).min(last.saturating_sub(scrolloff));
            let pos =
                char_idx_at_visual_offset(text, anchor, row as isize, 0, &format, &annotations).0;
            view.selection = view
                .selection
                .clone()
                .transform(|range| range.put_cursor(text, pos, extend))
                .ensure_invariants(text);
        }
        Motion::LastModification => {
            let select_mode = state.input.mode == Mode::Select;
            select(state, |doc, selection, _| {
                let text = doc.text.slice(..);
                match doc.history.last_edit_pos() {
                    Some(pos) => selection
                        .clone()
                        .transform(|range| range.put_cursor(text, pos, select_mode)),
                    None => selection.clone(),
                }
            })
        }
    }
}

#[derive(Clone, Copy)]
enum Line {
    Last,
    /// 1-based.
    Nth(usize),
}

pub fn find_char(state: &mut State, find: Find, target: CharTarget) {
    let count = count_of(state);
    let extend = state.input.mode == Mode::Select;
    let inclusive = find.inclusion == Inclusion::Inclusive;
    reselect(state, |text, range| {
        let anchor = range.cursor(text);
        let head = next_grapheme_boundary(text, anchor);
        let pos = match (target, find.direction) {
            (CharTarget::LineEnding, Direction::Forward) => {
                let line = range.cursor_line(text);
                let line_end = line_end_char_index(&text, line);
                let on_edge = match inclusive {
                    true => line_end == anchor,
                    false => line_end == head || line_end == anchor,
                };
                let line = line + count - 1 + on_edge as usize;
                match line >= text.len_lines() - 1 {
                    true => None,
                    false => Some(line_end_char_index(&text, line) - !inclusive as usize),
                }
            }
            (CharTarget::LineEnding, Direction::Backward) => {
                let line = range.cursor_line(text) as isize;
                match inclusive {
                    true => {
                        let line = line - count as isize;
                        (line >= 0).then(|| line_end_char_index(&text, line as usize))
                    }
                    false => {
                        let on_edge = text.line_to_char(line as usize) == anchor;
                        let line = line - count as isize + 1 - on_edge as isize;
                        (line > 0).then(|| text.line_to_char(line as usize))
                    }
                }
            }
            (CharTarget::Char(ch), dir) => {
                let start = match (inclusive, dir) {
                    (true, Direction::Forward) => head,
                    (true, Direction::Backward) => anchor,
                    (false, Direction::Forward) => head + 1,
                    (false, Direction::Backward) => anchor.saturating_sub(1),
                };
                search::find_nth_char(count, text, ch, start, direction(dir)).map(|pos| {
                    match (inclusive, dir) {
                        (true, _) => pos,
                        (false, Direction::Forward) => pos - 1,
                        (false, Direction::Backward) => pos + 1,
                    }
                })
            }
        };
        match (pos, extend) {
            (None, _) => range,
            (Some(pos), true) => range.put_cursor(text, pos, true),
            (Some(pos), false) => Range::point(range.cursor(text)).put_cursor(text, pos, true),
        }
    });
}

pub fn text_object(state: &mut State, scope: ObjectScope, object: Object) {
    let count = count_of(state);
    let scope = match scope {
        ObjectScope::Inside => TextObject::Inside,
        ObjectScope::Around => TextObject::Around,
    };
    select(state, |doc, selection, loader| {
        let text = doc.text.slice(..);
        selection.clone().transform(|range| match object {
            Object::Word => textobject::textobject_word(text, range, scope, count, false),
            Object::LongWord => textobject::textobject_word(text, range, scope, count, true),
            Object::Paragraph => textobject::textobject_paragraph(text, range, scope, count),
            Object::ClosestPair => textobject::textobject_pair_surround_closest(
                doc.syntax.as_ref(),
                text,
                range,
                scope,
                count,
            ),
            Object::Pair(ch) => textobject::textobject_pair_surround(
                doc.syntax.as_ref(),
                text,
                range,
                scope,
                ch,
                count,
            ),
            Object::Syntax(object) => match &doc.syntax {
                Some(syntax) => textobject::textobject_treesitter(
                    text,
                    range,
                    scope,
                    object.capture(),
                    syntax,
                    loader,
                    count,
                ),
                None => range,
            },
        })
    });
}

/// `C-b C-f C-u C-d`, page keys and `zj zk`, as helix's `scroll` with and without cursor sync.
pub fn scroll(state: &mut State, dir: Direction, by: ScrollBy) {
    let count = count_of(state);
    let mode = state.input.mode;
    let behaviour = movement_of(state);
    let State {
        layout,
        buffers,
        languages,
        ..
    } = state;
    let Some((view, doc)) = focused(layout, buffers) else {
        return;
    };
    let text = doc.text.slice(..);
    let viewport = view.scroll.viewport;
    let format = text_format(viewport, Indent::of(doc, languages));
    let height = viewport.rows as isize;
    let sign = match dir {
        Direction::Forward => 1,
        Direction::Backward => -1,
    };
    match by {
        ScrollBy::Lines | ScrollBy::Page => {
            let rows = match by {
                ScrollBy::Page => height,
                ScrollBy::Lines | ScrollBy::HalfPageCursor => count as isize,
            };
            let (anchor, moved) = scroll::scroll(
                text,
                &view.selection,
                view.scroll.anchor,
                viewport,
                sign * rows,
                mode,
                &format,
            );
            view.scroll.anchor = anchor;
            if let Some(selection) = moved {
                view.selection = selection.ensure_invariants(text);
            }
        }
        ScrollBy::HalfPageCursor => {
            let rows = height / 2;
            let mut annotations = TextAnnotations::default();
            view.scroll.anchor = char_idx_at_visual_offset(
                text,
                view.scroll.anchor.min(text.len_chars()),
                sign * rows,
                0,
                &format,
                &annotations,
            )
            .0;
            view.selection = view
                .selection
                .clone()
                .transform(|range| {
                    movement::move_vertically_visual(
                        text,
                        range,
                        direction(dir),
                        rows as usize,
                        behaviour,
                        &format,
                        &mut annotations,
                    )
                })
                .ensure_invariants(text);
        }
    }
}

/// `zz zt zb`: puts the primary cursor's row at the center, top or bottom of the view.
pub fn align_view(state: &mut State, align: Align) {
    let State {
        layout,
        buffers,
        languages,
        ..
    } = state;
    let Some((view, doc)) = focused(layout, buffers) else {
        return;
    };
    let text = doc.text.slice(..);
    let format = text_format(view.scroll.viewport, Indent::of(doc, languages));
    let last_row = view.scroll.viewport.rows.saturating_sub(1) as isize;
    let row = match align {
        Align::Top => 0,
        Align::Center => last_row / 2,
        Align::Bottom => last_row,
    };
    let cursor = view.selection.primary().cursor(text);
    view.scroll.anchor =
        char_idx_at_visual_offset(text, cursor, -row, 0, &format, &TextAnnotations::default()).0;
}

pub fn goto_syntax_object(state: &mut State, object: SyntaxObject, dir: Direction) {
    let count = count_of(state);
    let select_mode = state.input.mode == Mode::Select;
    try_select(state, |doc, selection, loader| {
        let Some(syntax) = &doc.syntax else {
            return Err(Notice::NoSyntax);
        };
        let text = doc.text.slice(..);
        Ok(selection.clone().transform(|range| {
            let found = movement::goto_treesitter_object(
                text,
                range,
                object.capture(),
                direction(dir),
                syntax,
                loader,
                count,
            );
            match select_mode {
                true => {
                    let head = match found.head < range.anchor {
                        true => found.anchor,
                        false => found.head,
                    };
                    Range::new(range.anchor, head)
                }
                false => found.with_direction(direction(dir)),
            }
        }))
    });
}
