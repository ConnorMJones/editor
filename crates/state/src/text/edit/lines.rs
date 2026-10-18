use std::num::NonZeroUsize;

use helix_core::comment;
use helix_core::line_ending::line_end_char_index;
use helix_core::movement::skip_while;
use helix_core::syntax::config::BlockCommentToken;
use helix_core::{Change, Range, Selection, SmallVec, Tendril, Transaction};
use helix_stdx::rope::RopeSliceExt;
use model::command::{Direction, Join, SortCase, SortOrder};
use model::notice::Notice;

use crate::State;
use crate::apply::{commit_focused, count_of, edit, exit_select_mode, selected_lines, try_edit};

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

/// Line comments when the language has only those, block comments otherwise, as helix does.
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

pub fn indent(state: &mut State) {
    let count = count_of(state);
    edit(state, |doc, selection, _| {
        let indent = doc.indent_style.as_str().repeat(count);
        Transaction::change(
            &doc.text,
            selected_lines(&doc.text, selection)
                .into_iter()
                .filter_map(|line| {
                    let line_text = doc.text.line(line);
                    match line_text.chunks().all(|s| s.trim().is_empty()) {
                        true => None,
                        false => {
                            let pos = doc.text.line_to_char(line);
                            let offset = match doc.indent_style {
                                helix_core::indent::IndentStyle::Spaces(width) => {
                                    helix_stdx::rope::RopeSliceExt::first_non_whitespace_char(
                                        line_text,
                                    )
                                    .unwrap_or(0)
                                        % width as usize
                                }
                                helix_core::indent::IndentStyle::Tabs => 0,
                            };
                            Some((pos, pos, Some(indent[offset..].into())))
                        }
                    }
                }),
        )
    });
    exit_select_mode(state);
}

pub fn unindent(state: &mut State) {
    let count = count_of(state);
    edit(state, |doc, selection, _| {
        let tab_width = doc.tab_width;
        let indent_width = count * doc.indent_width();
        let changes: Vec<helix_core::Change> = selected_lines(&doc.text, selection)
            .into_iter()
            .filter_map(|line_idx| {
                let mut width = 0;
                let mut pos = 0;
                for ch in doc.text.line(line_idx).chars() {
                    match ch {
                        ' ' => width += 1,
                        '\t' => width = (width / tab_width + 1) * tab_width,
                        _ => break,
                    }
                    pos += 1;
                    if width >= indent_width {
                        break;
                    }
                }
                let start = doc.text.line_to_char(line_idx);
                (pos > 0).then_some((start, start + pos, None))
            })
            .collect();
        Transaction::change(&doc.text, changes.into_iter())
    });
    exit_select_mode(state);
}

pub fn add_newline(state: &mut State, direction: Direction) {
    let count = count_of(state);
    edit(state, |doc, selection, _| {
        let text = &doc.text;
        let changes = selection.iter().map(|range| {
            let (start, end) = range.line_range(text.slice(..));
            let line = match direction {
                Direction::Backward => start,
                Direction::Forward => end + 1,
            };
            let pos = text.line_to_char(line);
            (
                pos,
                pos,
                Some(doc.line_ending.as_str().repeat(count).into()),
            )
        });
        Transaction::change(text, changes)
    });
}

pub fn reflow(state: &mut State, width: Option<NonZeroUsize>) {
    edit(state, |doc, selection, _| {
        let width = width.map_or(doc.text_width, NonZeroUsize::get);
        Transaction::change_by_selection(&doc.text, selection, |range| {
            let fragment = range.fragment(doc.text.slice(..));
            let reflowed = helix_core::wrap::reflow_hard_wrap(&fragment, width);
            (range.from(), range.to(), Some(reflowed))
        })
    })
}
