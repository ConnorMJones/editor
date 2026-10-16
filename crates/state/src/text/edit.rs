use std::borrow::Cow;
use std::sync::LazyLock;

use helix_core::auto_pairs::{self, AutoPairs};
use helix_core::comment;
use helix_core::graphemes;
use helix_core::indent::{self, IndentStyle};
use helix_core::line_ending::{get_line_ending_of_str, line_end_char_index};
use helix_core::movement::skip_while;
use helix_core::syntax::Loader;
use helix_core::syntax::config::{BlockCommentToken, IndentationHeuristic};
use helix_core::unicode::width::UnicodeWidthChar;
use helix_core::{
    Change, Deletion, Range, RopeSlice, Selection, SmallVec, Tendril, Transaction, match_brackets,
    surround,
};
use helix_stdx::rope::RopeSliceExt;
use model::command::{
    CharTarget, Direction, Join, Paste, SortCase, SortOrder, SurroundPair, Yanking,
};
use model::mode::Mode;
use model::notice::{Notice, SurroundError};
use model::register::Register;

use crate::apply::{commit_focused, count_of, edit, exit_select_mode, focused_text, try_edit};
use crate::text::buffer::TextDoc;
use crate::{Effect, State};

/// Pairs used when the buffer's language configures none.
static DEFAULT_PAIRS: LazyLock<AutoPairs> = LazyLock::new(AutoPairs::default);

static LINE_ENDINGS: LazyLock<helix_core::regex::Regex> =
    LazyLock::new(|| helix_core::regex::Regex::new(r"\r\n|\r|\n").expect("valid regex"));

const HEURISTIC: IndentationHeuristic = IndentationHeuristic::Hybrid;

fn auto_pairs<'a>(doc: &TextDoc, selection: &Selection, loader: &'a Loader) -> &'a AutoPairs {
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

pub fn delete_char_backward(state: &mut State) {
    let count = count_of(state);
    edit(state, |doc, selection, loader| {
        let text = doc.text.slice(..);
        let pairs = auto_pairs(doc, selection, loader);
        Transaction::delete_by_and_with_selection(&doc.text, selection, |range| {
            let pos = range.cursor(text);
            let line_start = text.line_to_char(range.cursor_line(text));
            let indent = Cow::from(text.slice(line_start..pos));
            let only_indent =
                !indent.is_empty() && indent.chars().all(|ch| ch == ' ' || ch == '\t');
            // Only indentation before the cursor deletes back to the previous indent stop.
            let dedent = match (only_indent, text.get_char(pos.saturating_sub(1))) {
                (false, _) => None,
                (true, Some('\t')) => Some((pos - 1, pos)),
                (true, _) => {
                    let width: usize = indent
                        .chars()
                        .map(|ch| match ch {
                            '\t' => doc.tab_width,
                            ch => ch.width().unwrap_or(1),
                        })
                        .sum();
                    let drop = match width % doc.indent_width() {
                        0 => doc.indent_width(),
                        drop => drop,
                    };
                    let spaces = indent.chars().rev().take(drop).take_while(|ch| *ch == ' ');
                    Some((pos - spaces.count(), pos))
                }
            };
            match pos {
                0 => ((pos, pos), None),
                _ => match dedent {
                    Some(deletion) => (deletion, None),
                    None => match auto_pairs::hook_delete(&doc.text, range, pairs) {
                        Some((deletion, range)) => (deletion, Some(range)),
                        None => (
                            (graphemes::nth_prev_grapheme_boundary(text, pos, count), pos),
                            None,
                        ),
                    },
                },
            }
        })
    });
}

/// Insert-mode deletion; forward deletes keep appended selections from shrinking.
pub fn delete_insert_mode(
    state: &mut State,
    direction: Direction,
    mut f: impl FnMut(RopeSlice, &Range) -> Deletion,
) {
    edit(state, |doc, selection, _| {
        let text = doc.text.slice(..);
        let len = text.len_chars();
        let mut adjusted = SmallVec::<[Range; 1]>::new();
        let mut insert_newline = false;
        let tx = Transaction::delete_by_selection(&doc.text, selection, |range| {
            let (start, end) = f(text, range);
            if direction == Direction::Forward {
                let mut range = *range;
                if range.head > range.anchor {
                    insert_newline |= end == len;
                    range.head += 1;
                }
                adjusted.push(range);
            }
            (start, end)
        });
        // Deleting the last character would leave the cursor on EOF; re-add a line ending like append mode.
        let tx = match insert_newline {
            true => tx.insert_at_eof(doc.line_ending.as_str().into()),
            false => tx,
        };
        match direction {
            Direction::Forward => {
                // Invariants first: the bumped head may sit past the end of the old text.
                let mapped = Selection::new(adjusted, selection.primary_index())
                    .ensure_invariants(text)
                    .map(tx.changes());
                tx.with_selection(mapped)
            }
            Direction::Backward => tx,
        }
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
enum Comments {
    Continue,
    Skip,
}

/// `o`/`O`: Forward opens below, Backward above.
pub fn open(state: &mut State, direction: Direction) {
    open_lines(state, direction, Comments::Continue)
}

fn open_lines(state: &mut State, direction: Direction, comments: Comments) {
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

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Operation {
    Delete,
    Change,
}

pub fn delete_selection(
    state: &mut State,
    operation: Operation,
    yanking: Yanking,
    out: &mut Vec<Effect>,
) {
    let register = state.input.register.unwrap_or(Register::DEFAULT);
    let Some((view, doc)) = focused_text(&mut state.layout, &mut state.buffers, &mut state.notice)
    else {
        return;
    };
    let text = doc.text.slice(..);
    let linewise = view.selection.ranges().iter().all(|range| {
        let (start_line, end_line) = range.line_range(text);
        range.slice(text).len_lines() >= 2
            && text.line_to_char(start_line) == range.from()
            && text.line_to_char((end_line + 1).min(text.len_lines())) == range.to()
    });
    match (yanking, register) {
        (Yanking::Yank, Register::BlackHole) | (Yanking::NoYank, _) => {}
        (Yanking::Yank, register) => {
            let values: Vec<String> = view
                .selection
                .fragments(text)
                .map(Cow::into_owned)
                .collect();
            if let Err(notice) = state.registers.write(register, values, out) {
                state.notice = Some(notice);
                return;
            }
        }
    }
    edit(state, |doc, selection, _| {
        Transaction::delete_by_selection(&doc.text, selection, |range| (range.from(), range.to()))
    });
    match (operation, linewise) {
        (Operation::Delete, _) => exit_select_mode(state),
        (Operation::Change, true) => open_lines(state, Direction::Backward, Comments::Skip),
        (Operation::Change, false) => state.input.mode = Mode::Insert,
    }
}

pub fn align_selections(state: &mut State) {
    #[allow(deprecated)]
    use helix_core::visual_coords_at_pos;

    try_edit(state, |doc, selection, _| {
        let text = doc.text.slice(..);
        let mut column_widths: Vec<usize> = Vec::new();
        let mut coordinates = Vec::with_capacity(selection.len());
        let mut previous_line = usize::MAX;
        let mut column = 0;
        let mut running_offset = 0;
        for range in selection {
            #[allow(deprecated)]
            let coords = visual_coords_at_pos(text, range.head, doc.tab_width);
            #[allow(deprecated)]
            let anchor_coords = visual_coords_at_pos(text, range.anchor, doc.tab_width);
            if coords.row != anchor_coords.row {
                return Err(Notice::AlignMultiLine);
            }
            if coords.row != previous_line {
                column = 0;
                running_offset = 0;
                previous_line = coords.row;
            }
            let width = coords.col - running_offset;
            match column_widths.get_mut(column) {
                Some(n) => *n = (*n).max(width),
                None => column_widths.push(width),
            }
            coordinates.push(coords);
            running_offset += width;
            column += 1;
        }
        let positions: Vec<usize> = column_widths
            .into_iter()
            .scan(0, |sum, n| {
                *sum += n;
                Some(*sum)
            })
            .collect();
        previous_line = usize::MAX;
        let changes = coordinates
            .into_iter()
            .zip(selection)
            .map(|(coords, range)| {
                if coords.row != previous_line {
                    column = 0;
                    running_offset = 0;
                    previous_line = coords.row;
                }
                let inserts = positions[column] - coords.col - running_offset;
                column += 1;
                running_offset += inserts;
                (range.from(), range.from(), Some(" ".repeat(inserts).into()))
            });
        Ok(Transaction::change(&doc.text, changes))
    });
}

pub fn join_selections(state: &mut State, join: Join) {
    edit(state, |doc, selection, loader| {
        let text = &doc.text;
        let slice = text.slice(..);
        let mut changes: Vec<Change> = Vec::new();
        for range in selection {
            let (start, mut end) = range.line_range(slice);
            if start == end {
                end = (end + 1).min(text.len_lines() - 1);
            }
            let byte = slice.char_to_byte(slice.line_to_char(start));
            let mut tokens: Vec<&str> = doc
                .language_config_at(loader, byte)
                .and_then(|config| config.comment_tokens.as_deref())
                .unwrap_or(&[])
                .iter()
                .map(String::as_str)
                .collect();
            tokens.sort_unstable_by_key(|token| std::cmp::Reverse(token.len()));
            let first = slice.line_to_char(start);
            let first = skip_while(slice, first, |ch| matches!(ch, ' ' | '\t')).unwrap_or(first);
            let first_line = slice.slice(first..);
            let mut current = tokens
                .iter()
                .find(|token| first_line.starts_with(token))
                .copied();
            for line in start..end {
                let from = line_end_char_index(&slice, line);
                let mut to = text.line_to_char(line + 1);
                to = skip_while(slice, to, |ch| matches!(ch, ' ' | '\t')).unwrap_or(to);
                let rest = slice.slice(to..);
                if let Some(token) = tokens.iter().find(|token| rest.starts_with(token)).copied() {
                    match Some(token) == current {
                        true => {
                            to += token.chars().count();
                            to = skip_while(slice, to, |ch| matches!(ch, ' ' | '\t')).unwrap_or(to);
                        }
                        false => current = Some(token),
                    }
                }
                let separator = match to == line_end_char_index(&slice, line + 1) {
                    true => None,
                    false => Some(Tendril::from(" ")),
                };
                changes.push((from, to, separator));
            }
        }
        changes.sort_unstable_by_key(|(from, _, _)| *from);
        changes.dedup();
        let spaces: SmallVec<[Range; 1]> = match join {
            Join::Plain => SmallVec::new(),
            Join::SelectSpace => {
                let mut offset = 0;
                changes
                    .iter()
                    .filter_map(|(from, to, separator)| match separator {
                        Some(_) => {
                            let range = Range::point(from - offset);
                            offset += to - from - 1;
                            Some(range)
                        }
                        None => {
                            offset += to - from;
                            None
                        }
                    })
                    .collect()
            }
        };
        let tx = Transaction::change(text, changes.into_iter());
        match spaces.is_empty() {
            true => tx,
            false => tx.with_selection(Selection::new(spaces, 0)),
        }
    });
}

/// Line comments when the language has only those, block comments otherwise; see helix `toggle_comments`.
pub fn toggle_comments(state: &mut State) {
    edit(state, |doc, selection, loader| {
        let text = doc.text.slice(..);
        let cursor = selection.primary().cursor(text);
        let byte = doc.text.char_to_byte(cursor) as u32;
        let (mut line_layer, mut block_layer) = (None, None);
        if let Some(syntax) = doc.highlight.syntax() {
            for layer in syntax.layers_for_byte_range(byte, byte) {
                let language = syntax.layer(layer).language;
                let config = loader.language(language).config();
                if config.comment_tokens.is_some() {
                    line_layer = Some(language);
                }
                if config.block_comment_tokens.is_some() {
                    block_layer = Some(language);
                }
            }
        }
        let config = line_layer
            .or(block_layer)
            .or(doc.highlight.language())
            .map(|language| &**loader.language(language).config());
        let cursor_line = doc.text.char_to_line(cursor);
        let line_token: Option<&str> = config
            .and_then(|config| config.comment_tokens.as_ref())
            .and_then(|tokens| {
                comment::get_comment_token(text, tokens.as_slice(), cursor_line)
                    .or_else(|| tokens.first().map(String::as_str))
            });
        let block_tokens: Option<&[BlockCommentToken]> =
            config.and_then(|config| config.block_comment_tokens.as_deref());
        if line_token.is_some() && block_tokens.is_none() {
            return comment::toggle_line_comments(&doc.text, selection, line_token);
        }
        let split_lines = comment::split_lines_of_selection(text, selection);
        let default_block = [BlockCommentToken::default()];
        let block = block_tokens.unwrap_or(&default_block);
        let (line_commented, line_changes) =
            comment::find_block_comments(block, text, &split_lines);
        if line_commented {
            return comment::create_block_comment_transaction(
                &doc.text,
                &split_lines,
                line_commented,
                line_changes,
            )
            .0;
        }
        let (block_commented, block_changes) = comment::find_block_comments(block, text, selection);
        match (block_commented, line_token, block_tokens) {
            (true, _, _) => {
                comment::create_block_comment_transaction(
                    &doc.text,
                    selection,
                    block_commented,
                    block_changes,
                )
                .0
            }
            (false, None, Some(_)) => {
                comment::create_block_comment_transaction(
                    &doc.text,
                    &split_lines,
                    line_commented,
                    line_changes,
                )
                .0
            }
            (false, _, _) => comment::toggle_line_comments(&doc.text, selection, line_token),
        }
    });
}

pub fn surround_add(state: &mut State, target: CharTarget) {
    edit(state, |doc, selection, _| {
        let (open, close, len): (Tendril, Tendril, usize) = match target {
            CharTarget::Char(ch) => {
                let (open, close) = match_brackets::get_pair(ch);
                (Tendril::from_iter([open]), Tendril::from_iter([close]), 2)
            }
            CharTarget::LineEnding => (
                doc.line_ending.as_str().into(),
                doc.line_ending.as_str().into(),
                2 * doc.line_ending.len_chars(),
            ),
        };
        let mut changes = Vec::with_capacity(selection.len() * 2);
        let mut ranges = SmallVec::with_capacity(selection.len());
        let mut offs = 0;
        for range in selection.iter() {
            changes.push((range.from(), range.from(), Some(open.clone())));
            changes.push((range.to(), range.to(), Some(close.clone())));
            ranges.push(
                Range::new(offs + range.from(), offs + range.to() + len)
                    .with_direction(range.direction()),
            );
            offs += len;
        }
        Transaction::change(&doc.text, changes.into_iter())
            .with_selection(Selection::new(ranges, selection.primary_index()))
    });
}

fn surround_positions(
    doc: &TextDoc,
    selection: &Selection,
    pair: SurroundPair,
    count: usize,
) -> Result<Vec<usize>, Notice> {
    let ch = match pair {
        SurroundPair::Closest => None,
        SurroundPair::Char(ch) => Some(ch),
    };
    surround::get_surround_pos(
        doc.highlight.syntax(),
        doc.text.slice(..),
        selection,
        ch,
        count,
    )
    .map_err(|error| {
        Notice::Surround(match error {
            surround::Error::PairNotFound => SurroundError::PairNotFound,
            surround::Error::CursorOverlap => SurroundError::CursorOverlap,
            surround::Error::RangeExceedsText => SurroundError::RangeExceedsText,
            surround::Error::CursorOnAmbiguousPair => SurroundError::CursorOnAmbiguousPair,
        })
    })
}

pub fn surround_replace(state: &mut State, from: SurroundPair, to: char) {
    let count = count_of(state);
    try_edit(state, |doc, selection, _| {
        let positions = surround_positions(doc, selection, from, count)?;
        let (open, close) = match_brackets::get_pair(to);
        let mut sorted: Vec<(usize, char)> = positions
            .chunks(2)
            .flat_map(|pair| [(pair[0], open), (pair[1], close)])
            .collect();
        sorted.sort_unstable();
        Ok(Transaction::change(
            &doc.text,
            sorted
                .into_iter()
                .map(|(pos, ch)| (pos, pos + 1, Some(Tendril::from_iter([ch])))),
        ))
    });
}

pub fn surround_delete(state: &mut State, pair: SurroundPair) {
    let count = count_of(state);
    try_edit(state, |doc, selection, _| {
        let mut positions = surround_positions(doc, selection, pair, count)?;
        positions.sort_unstable();
        Ok(Transaction::change(
            &doc.text,
            positions.into_iter().map(|pos| (pos, pos + 1, None)),
        ))
    });
}

/// `C-a`/`C-x`; with register `#` each later selection steps one further.
pub fn increment(state: &mut State, sign: i64) {
    let amount = sign * count_of(state) as i64;
    let step = match state.input.register {
        Some(Register::SelectionIndices) => sign,
        Some(_) | None => 0,
    };
    let mut changed = false;
    edit(state, |doc, selection, _| {
        let text = doc.text.slice(..);
        let mut amount = amount;
        let mut ranges = SmallVec::<[Range; 1]>::new();
        let mut shift: i128 = 0;
        let mut changes = Vec::new();
        for range in selection {
            let selected = range.fragment(text);
            let from = (range.from() as i128 + shift) as usize;
            let incremented = helix_core::increment::integer(&selected, amount)
                .or_else(|| helix_core::increment::date_time(&selected, amount));
            amount += step;
            match incremented {
                None => ranges.push(Range::new(from, (range.to() as i128 + shift) as usize)),
                Some(new) => {
                    ranges.push(Range::new(from, from + new.len()));
                    shift += new.len() as i128 - selected.len() as i128;
                    changes.push((range.from(), range.to(), Some(new.into())));
                }
            }
        }
        changed = !changes.is_empty();
        let tx = Transaction::change(&doc.text, changes.into_iter());
        match changed {
            true => tx.with_selection(Selection::new(ranges, selection.primary_index())),
            false => tx,
        }
    });
    if changed {
        exit_select_mode(state);
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Yank {
    All,
    Primary,
}

pub fn yank(state: &mut State, register: Register, yank: Yank, out: &mut Vec<Effect>) {
    let Some((view, doc)) = focused_text(&mut state.layout, &mut state.buffers, &mut state.notice)
    else {
        return;
    };
    let text = doc.text.slice(..);
    let values: Vec<String> = match yank {
        Yank::All => view
            .selection
            .fragments(text)
            .map(Cow::into_owned)
            .collect(),
        Yank::Primary => vec![view.selection.primary().fragment(text).into_owned()],
    };
    let count = values.len();
    state.notice = Some(match state.registers.write(register, values, out) {
        Ok(()) => Notice::Yanked {
            count,
            register: register.char(),
        },
        Err(notice) => notice,
    });
    exit_select_mode(state);
}

fn map_value(value: &str, line_ending: &str, count: usize) -> Tendril {
    let value = LINE_ENDINGS.replace_all(value, line_ending);
    let mut out = Tendril::from(value.as_ref());
    for _ in 1..count {
        out.push_str(&value);
    }
    out
}

/// Pastes register values, one per selection, repeating the last; values ending in a newline paste linewise.
pub fn paste(state: &mut State, values: &[String], at: Paste, count: usize) {
    let Some(last) = values.last() else {
        return;
    };
    let mode = state.input.mode;
    if mode == Mode::Insert {
        commit_focused(state);
    }
    let linewise = values
        .iter()
        .any(|value| get_line_ending_of_str(value).is_some());
    edit(state, |doc, selection, _| {
        let text = &doc.text;
        let line_ending = doc.line_ending.as_str();
        let repeat = std::iter::repeat(map_value(last, line_ending, count));
        let mut values = values
            .iter()
            .map(|value| map_value(value, line_ending, count))
            .chain(repeat);
        let mut offset = 0;
        let mut ranges = SmallVec::with_capacity(selection.len());
        let tx = Transaction::change_by_selection(text, selection, |range| {
            let pos = match (at, linewise) {
                (Paste::Before, true) => text.line_to_char(text.char_to_line(range.from())),
                (Paste::After, true) => {
                    let line = range.line_range(text.slice(..)).1;
                    text.line_to_char((line + 1).min(text.len_lines()))
                }
                (Paste::Before, false) => range.from(),
                (Paste::After, false) => range.to(),
                (Paste::Cursor, _) => range.cursor(text.slice(..)),
            };
            let value = values.next();
            let len = value.as_ref().map_or(0, |value| value.chars().count());
            let anchor = offset + pos;
            ranges.push(Range::new(anchor, anchor + len).with_direction(range.direction()));
            offset += len;
            (pos, pos, value)
        });
        match mode {
            Mode::Normal => tx.with_selection(Selection::new(ranges, selection.primary_index())),
            Mode::Insert | Mode::Select => tx,
        }
    });
    commit_focused(state);
}

/// `R`: replaces each non-empty selection with a register value.
pub fn replace_with(state: &mut State, values: &[String], count: usize) {
    let Some(last) = values.last() else {
        return;
    };
    edit(state, |doc, selection, _| {
        let line_ending = doc.line_ending.as_str();
        let repeat = std::iter::repeat(map_value(last, line_ending, count));
        let mut values = values
            .iter()
            .map(|value| map_value(value, line_ending, count))
            .chain(repeat);
        Transaction::change_by_selection(&doc.text, selection, |range| match range.is_empty() {
            false => (range.from(), range.to(), values.next()),
            true => (range.from(), range.to(), None),
        })
    });
    commit_focused(state);
}

pub fn sort(state: &mut State, case: SortCase, order: SortOrder) {
    try_edit(state, |doc, selection, _| {
        if selection.len() == 1 {
            return Err(Notice::SortNeedsSelections);
        }
        let text = doc.text.slice(..);
        let mut fragments: Vec<Tendril> = selection
            .slices(text)
            .map(|fragment| fragment.chunks().collect())
            .collect();
        fragments.sort_by(|a, b| {
            let ordering = match case {
                SortCase::Sensitive => a.cmp(b),
                SortCase::Insensitive => a.to_lowercase().cmp(&b.to_lowercase()),
            };
            match order {
                SortOrder::Ascending => ordering,
                SortOrder::Descending => ordering.reverse(),
            }
        });
        Ok(Transaction::change(
            &doc.text,
            selection
                .into_iter()
                .zip(fragments)
                .map(|(range, fragment)| (range.from(), range.to(), Some(fragment))),
        ))
    });
    commit_focused(state);
}
