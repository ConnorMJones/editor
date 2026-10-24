use std::sync::LazyLock;

use helix_core::auto_pairs::{self, AutoPairs};
use helix_core::chars::char_is_whitespace;
use helix_core::comment;
use helix_core::graphemes;
use helix_core::indent::{self, IndentStyle};
use helix_core::line_ending::{line_end_char_index, str_is_line_ending};
use helix_core::syntax::Loader;
use helix_core::syntax::config::IndentationHeuristic;
use helix_core::unicode::segmentation::UnicodeSegmentation;
use helix_core::{Range, RopeSlice, Selection, SmallVec, Tendril, Transaction};
use helix_stdx::rope::RopeSliceExt;
use model::command::{Direction, Motion};
use model::mode::Mode;

use crate::apply::{Focused, FocusedText, commit_focused, count_of, edit, focused, reselect};
use crate::text::buffer::{TextDoc, Uncommitted};
use crate::text::motion;
use crate::{InsertExit, State};

/// Pairs used when the buffer's language configures none.
static DEFAULT_PAIRS: LazyLock<AutoPairs> = LazyLock::new(AutoPairs::default);

const HEURISTIC: IndentationHeuristic = IndentationHeuristic::Hybrid;

pub fn auto_pairs<'a>(doc: &TextDoc, selection: &Selection, loader: &'a Loader) -> &'a AutoPairs {
    let (start, _) = selection.primary().into_byte_range(doc.text.slice(..));
    doc.language_config_at(loader, start)
        .and_then(|config| config.auto_pairs.as_ref())
        .unwrap_or(&DEFAULT_PAIRS)
}

/// Comment leader to continue on a new line, from the layer owning the line's first character.
fn continued_comment<'a>(
    doc: &TextDoc,
    loader: &'a Loader,
    text: RopeSlice,
    line: usize,
) -> Option<&'a str> {
    let first = text.line(line).first_non_whitespace_char()?;
    let byte = text.char_to_byte(text.line_to_char(line) + first);
    match doc.highlight.syntax() {
        Some(syntax) => syntax
            .layers_for_byte_range(byte as u32, byte as u32)
            .filter_map(|layer| {
                let config = loader.language(syntax.layer(layer).language).config();
                config
                    .comment_tokens
                    .as_ref()
                    .and_then(|tokens| comment::get_comment_token(text, tokens.as_slice(), line))
            })
            .last(),
        None => (doc.highlight.language())
            .map(|language| &**loader.language(language).config())
            .and_then(|config| config.comment_tokens.as_ref())
            .and_then(|tokens| comment::get_comment_token(text, tokens.as_slice(), line)),
    }
}

pub fn insert_char(state: &mut State, c: char) {
    edit(state, |doc, selection, loader| {
        let text = &doc.text;
        let pairs = auto_pairs(doc, selection, loader);
        Transaction::change_by_and_with_selection(text, selection, |range| {
            match auto_pairs::hook_insert(text, range, c, pairs) {
                Some((change, range)) => (change, Some(range)),
                None => {
                    let cursor = range.cursor(text.slice(..));
                    ((cursor, cursor, Some(Tendril::from_iter([c]))), None)
                }
            }
        })
    });
}

pub fn insert_tab(state: &mut State) {
    edit(state, |doc, selection, _| {
        let text = doc.text.slice(..);
        Transaction::change(
            &doc.text,
            selection.ranges().iter().map(|range| {
                let cursor = range.cursor(text);
                let indent = match doc.indent_style {
                    IndentStyle::Spaces(width) => {
                        let line_start = text.line_to_char(range.cursor_line(text));
                        let offset = (cursor - line_start) % width as usize;
                        Tendril::from(&doc.indent_style.as_str()[offset..])
                    }
                    IndentStyle::Tabs => Tendril::from(doc.indent_style.as_str()),
                };
                (cursor, cursor, Some(indent))
            }),
        )
    });
}

pub fn insert_newline(state: &mut State) {
    edit(state, |doc, selection, loader| {
        let contents = &doc.text;
        let text = contents.slice(..);
        let line_ending = doc.line_ending.as_str();
        let pairs = auto_pairs(doc, selection, loader);
        let mut ranges = SmallVec::with_capacity(selection.len());
        let mut global_offs: isize = 0;
        let mut new_text = String::new();
        let mut last_pos = 0;
        let tx = Transaction::change_by_selection(contents, selection, |range| {
            let pos = range.cursor(text);
            let prev = match pos {
                0 => ' ',
                _ => contents.char(pos - 1),
            };
            let curr = contents.get_char(pos).unwrap_or(' ');
            let current_line = text.char_to_line(pos);
            let line_start = text.line_to_char(current_line);
            let comment = continued_comment(doc, loader, text, current_line);
            let (from, to, local_offs, chars_deleted) =
                match text.slice(line_start..pos).last_non_whitespace_char() {
                    Some(idx) => {
                        let first_trailing_whitespace = (line_start + idx + 1).clamp(last_pos, pos);
                        last_pos = pos;
                        let line = text.line(current_line);
                        let indent = match (line.first_non_whitespace_char(), comment) {
                            (Some(first), Some(_)) => line.slice(..first).to_string(),
                            (Some(_) | None, _) => indent::indent_for_newline(
                                loader,
                                doc.highlight.syntax(),
                                &HEURISTIC,
                                &doc.indent_style,
                                doc.tab_width,
                                text,
                                current_line,
                                pos,
                                current_line,
                            ),
                        };
                        let on_auto_pair = pairs
                            .get(prev)
                            .is_some_and(|pair| pair.open == prev && pair.close == curr);
                        let local_offs = match (comment, on_auto_pair) {
                            (Some(token), _) => {
                                new_text.push_str(line_ending);
                                new_text.push_str(&indent);
                                new_text.push_str(token);
                                new_text.push(' ');
                                new_text.chars().count()
                            }
                            (None, true) => {
                                new_text.push_str(line_ending);
                                new_text.push_str(&indent);
                                new_text.push_str(doc.indent_style.as_str());
                                let local_offs = new_text.chars().count();
                                new_text.push_str(line_ending);
                                new_text.push_str(&indent);
                                local_offs
                            }
                            (None, false) => {
                                new_text.push_str(line_ending);
                                new_text.push_str(&indent);
                                new_text.chars().count()
                            }
                        };
                        let chars_deleted = pos - first_trailing_whitespace;
                        (
                            first_trailing_whitespace,
                            pos,
                            local_offs as isize - chars_deleted as isize,
                            chars_deleted,
                        )
                    }
                    None => {
                        // A blank line keeps its indentation on the new line and empties this one.
                        new_text.push_str(line_ending);
                        (line_start, line_start, new_text.chars().count() as isize, 0)
                    }
                };
            let new_range = match range.cursor(text) > range.anchor {
                true => Range::new(
                    (range.anchor as isize + global_offs) as usize,
                    (range.head as isize + local_offs + global_offs) as usize,
                ),
                false => Range::new(
                    (range.anchor as isize + local_offs + global_offs) as usize,
                    (range.head as isize + local_offs + global_offs) as usize,
                ),
            };
            ranges.push(new_range);
            global_offs += new_text.chars().count() as isize - chars_deleted as isize;
            let tendril = Tendril::from(new_text.as_str());
            new_text.clear();
            (from, to, Some(tendril))
        });
        tx.with_selection(Selection::new(ranges, selection.primary_index()))
    });
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Fallback {
    LineStart,
    LineEnd,
}

/// `I`/`A`: auto-indents empty lines, otherwise moves to the line's start or end.
pub fn insert_with_indent(state: &mut State, fallback: Fallback) {
    let was_select = state.input.mode == Mode::Select;
    state.input.mode = Mode::Insert;
    edit(state, |doc, selection, loader| {
        let contents = &doc.text;
        let text = contents.slice(..);
        let mut ranges = SmallVec::with_capacity(selection.len());
        let mut offs = 0;
        let tx = Transaction::change_by_selection(contents, selection, |range| {
            let line = range.cursor_line(text);
            let line_start = text.line_to_char(line);
            match line_end_char_index(&text, line) == line_start {
                true => {
                    let indent = indent::indent_for_newline(
                        loader,
                        doc.highlight.syntax(),
                        &HEURISTIC,
                        &doc.indent_style,
                        doc.tab_width,
                        text,
                        line,
                        line_start,
                        line,
                    );
                    let width = indent.chars().count();
                    ranges.push(Range::point(offs + line_start + width));
                    offs += width;
                    (line_start, line_start, Some(indent.into()))
                }
                false => {
                    let pos = match fallback {
                        Fallback::LineStart => text
                            .line(line)
                            .first_non_whitespace_char()
                            .map_or(line_start, |offset| offset + line_start),
                        Fallback::LineEnd => line_end_char_index(&text, line),
                    };
                    ranges.push(range.put_cursor(text, pos + offs, was_select));
                    (line_start, line_start, None)
                }
            }
        });
        tx.with_selection(Selection::new(ranges, selection.primary_index()))
    });
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Comments {
    Continue,
    Skip,
}

/// `o`/`O`: Forward opens below, Backward above.
pub fn open(state: &mut State, direction: Direction) {
    open_lines(state, direction, Comments::Continue)
}

pub fn open_lines(state: &mut State, direction: Direction, comments: Comments) {
    let count = count_of(state);
    state.input.mode = Mode::Insert;
    edit(state, |doc, selection, loader| {
        let contents = &doc.text;
        let text = contents.slice(..);
        let line_ending = doc.line_ending;
        let mut offs = 0;
        let mut ranges = SmallVec::with_capacity(selection.len());
        let tx = Transaction::change_by_selection(contents, selection, |range| {
            let current = text.char_to_line(match direction {
                Direction::Forward => graphemes::prev_grapheme_boundary(text, range.to()),
                Direction::Backward => range.from(),
            });
            let next = match direction {
                Direction::Forward => current + 1,
                Direction::Backward => current,
            };
            let above_next = next.saturating_sub(1);
            let comment = match comments {
                Comments::Continue => continued_comment(doc, loader, text, current),
                Comments::Skip => None,
            };
            let (insert_at, line_ending_width) = match next {
                0 => (0, 0),
                _ => (
                    line_end_char_index(&text, above_next),
                    line_ending.len_chars(),
                ),
            };
            let line = text.line(current);
            let indent = match (line.first_non_whitespace_char(), comment) {
                (Some(first), Some(_)) => line.slice(..first).to_string(),
                (Some(_) | None, _) => indent::indent_for_newline(
                    loader,
                    doc.highlight.syntax(),
                    &HEURISTIC,
                    &doc.indent_style,
                    doc.tab_width,
                    text,
                    above_next,
                    insert_at,
                    current,
                ),
            };
            let indent_len = indent.len();
            let mut inserted = String::with_capacity(1 + indent_len);
            match (direction, next) {
                (Direction::Backward, 0) => {
                    inserted.push_str(&indent);
                    if let Some(token) = comment {
                        inserted.push_str(token);
                        inserted.push(' ');
                    }
                    inserted.push_str(line_ending.as_str());
                }
                _ => {
                    inserted.push_str(line_ending.as_str());
                    inserted.push_str(&indent);
                    if let Some(token) = comment {
                        inserted.push_str(token);
                        inserted.push(' ');
                    }
                }
            }
            let inserted = inserted.repeat(count);
            let pos = offs + insert_at + line_ending_width;
            let comment_len = comment.map_or(0, |token| token.len() + 1);
            for i in 0..count {
                ranges.push(Range::point(
                    pos + i * (line_ending.len_chars() + indent_len + comment_len)
                        + indent_len
                        + comment_len,
                ));
            }
            offs += inserted.chars().count();
            (insert_at, insert_at, Some(inserted.into()))
        });
        tx.with_selection(Selection::new(ranges, selection.primary_index()))
    });
}

pub fn exit_insert(state: &mut State) {
    let blank_line = match focused(&mut state.layout, &mut state.buffers) {
        Focused::Text(FocusedText { view, doc }) => {
            let text = doc.text.slice(..);
            let range = view.selection.primary();
            let pos = range.cursor(text);
            let line = range.cursor_line(text);
            let inserted_blank = match &doc.uncommitted {
                Uncommitted::Pending { changes, .. } => match changes.changes().changes() {
                    [
                        helix_core::Operation::Retain(at),
                        helix_core::Operation::Insert(inserted),
                        helix_core::Operation::Retain(_),
                    ] => {
                        let mut graphemes = inserted.graphemes(true);
                        at + inserted.len() == pos
                            && graphemes.next().is_some_and(str_is_line_ending)
                            && graphemes.all(|g| g.chars().all(char_is_whitespace))
                            && pos == line_end_char_index(&text, line)
                    }
                    _ => false,
                },
                Uncommitted::Clean => false,
            };
            inserted_blank.then(|| (text.line_to_char(line), pos))
        }
        Focused::Empty | Focused::NotText | Focused::ReadOnly => None,
    };
    if let Some((start, end)) = blank_line {
        edit(state, |doc, _, _| {
            Transaction::change(&doc.text, [(start, end, None)].into_iter())
        });
    }
    match std::mem::replace(&mut state.input.insert_exit, InsertExit::Keep) {
        InsertExit::RestoreCursor => reselect(state, |text, range| {
            let head = match range.head > range.anchor {
                true => graphemes::prev_grapheme_boundary(text, range.to()),
                false => range.to(),
            };
            Range::new(range.from(), head)
        }),
        InsertExit::Keep => {}
    }
    state.input.mode = Mode::Normal;
    commit_focused(state);
}

pub fn append_mode(state: &mut State) {
    state.input.mode = Mode::Insert;
    state.input.insert_exit = InsertExit::RestoreCursor;
    edit(state, |doc, selection, _| {
        let end = doc.text.len_chars();
        let last = selection
            .ranges()
            .last()
            .copied()
            .unwrap_or(Range::point(0));
        let changes = match !last.is_empty() && last.to() == end {
            true => Some((end, end, Some(doc.line_ending.as_str().into()))),
            false => None,
        };
        Transaction::change(&doc.text, changes.into_iter())
    });
    reselect(state, |text, range| {
        Range::new(
            range.from(),
            graphemes::next_grapheme_boundary(text, range.to()),
        )
    });
}

pub fn smart_tab(state: &mut State) {
    let after_text = match focused(&mut state.layout, &mut state.buffers) {
        Focused::Text(FocusedText { view, doc }) => view.selection.ranges().iter().any(|range| {
            let text = doc.text.slice(..);
            let cursor = range.cursor(text);
            let line_start = text.line_to_char(text.char_to_line(cursor));
            !text
                .slice(line_start..cursor)
                .chars()
                .all(char::is_whitespace)
        }),
        Focused::Empty | Focused::NotText | Focused::ReadOnly => false,
    };
    match after_text {
        true => motion::motion(state, Motion::ParentNodeEnd),
        false => insert_tab(state),
    }
}
