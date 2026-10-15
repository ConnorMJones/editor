use std::io;
use std::num::NonZeroUsize;
use std::path::PathBuf;

use helix_core::chars::char_is_whitespace;
use helix_core::line_ending::{line_end_char_index, str_is_line_ending};
use helix_core::movement::{self, Movement};
use helix_core::syntax::Loader;
use helix_core::unicode::segmentation::UnicodeSegmentation;
use helix_core::{
    Assoc, ChangeSet, Operation, Range, Rope, RopeSlice, Selection, SmallVec, Transaction,
    graphemes, object,
};
use helix_stdx::rope::RopeSliceExt;
use model::command::{Answer, Command, Direction, Dirs, Motion, Paste};
use model::config::OpenAs;
use model::mode::Mode;
use model::notice::Notice;
use model::register::Register;
use slotmap::SlotMap;

use crate::git::changes::Arrive;
use crate::git::diff::Landing;
use crate::git::diffview::{self, Right};
use crate::input::prompt::{Candidate, Chosen};
use crate::job::{FsOp, Job, ReloadPolicy};
use crate::sidebar::tree::EntryKind;
use crate::text::buffer::{
    Buffer, BufferId, Content, Disk, HistoryStep, Jump, Position, Target, TextDoc, Uncommitted,
};
use crate::text::registers::{self, ClipboardUse, Values};
use crate::view::layout::{Layout, Pane, PaneId, TabKey, Tabs, TextView, View};
use crate::{
    Confirm, Effect, Exit, InsertExit, LastMotion, Overlay, Prompt, PromptKind, State, Sticky,
    git::changes, git::conflict, input, input::prompt, project::links, render::rendered,
    text::edit, text::motion, text::select, view::panes, view::scroll,
};

pub fn apply(state: &mut State, command: Command, out: &mut Vec<Effect>) {
    match &command {
        Command::FindChar(..)
        | Command::SelectTextObject(..)
        | Command::GotoSyntaxObject(..)
        | Command::Move(
            Motion::NextParagraph
            | Motion::PrevParagraph
            | Motion::ParentNodeEnd
            | Motion::ParentNodeStart,
        )
        | Command::ExpandSelection
        | Command::ShrinkSelection
        | Command::SelectSibling(_)
        | Command::SelectAllSiblings
        | Command::SelectAllChildren => {
            state.input.last_motion = Some(LastMotion {
                command: command.clone(),
                count: state.input.count,
            });
        }
        _ => {}
    }
    match command {
        Command::NormalMode => match state.input.mode {
            Mode::Normal => {}
            Mode::Select => state.input.mode = Mode::Normal,
            Mode::Insert => {
                let blank_line = match focused(&mut state.layout, &mut state.buffers) {
                    Some((view, doc)) => {
                        let text = doc.text.slice(..);
                        let range = view.selection.primary();
                        let pos = range.cursor(text);
                        let line = range.cursor_line(text);
                        let inserted_blank = match &doc.uncommitted {
                            Uncommitted::Pending { changes, .. } => {
                                match changes.changes().changes() {
                                    [
                                        Operation::Retain(at),
                                        Operation::Insert(inserted),
                                        Operation::Retain(_),
                                    ] => {
                                        let mut graphemes = inserted.graphemes(true);
                                        at + inserted.len() == pos
                                            && graphemes.next().is_some_and(str_is_line_ending)
                                            && graphemes.all(|g| g.chars().all(char_is_whitespace))
                                            && pos == line_end_char_index(&text, line)
                                    }
                                    _ => false,
                                }
                            }
                            Uncommitted::Clean => false,
                        };
                        inserted_blank.then(|| (text.line_to_char(line), pos))
                    }
                    None => None,
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
        },
        Command::InsertMode => {
            state.input.mode = Mode::Insert;
            reselect(state, |_, range| Range::new(range.to(), range.from()));
        }
        Command::AppendMode => {
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
        Command::SelectMode => {
            reselect(state, |text, range| {
                match range.is_empty() && range.head == text.len_chars() {
                    true => Range::new(
                        graphemes::prev_grapheme_boundary(text, range.anchor),
                        range.head,
                    ),
                    false => range,
                }
            });
            state.input.mode = Mode::Select;
        }
        Command::InsertAtLineStart => edit::insert_with_indent(state, edit::Fallback::LineStart),
        Command::InsertAtLineEnd => edit::insert_with_indent(state, edit::Fallback::LineEnd),
        Command::OpenBelow => edit::open(state, Direction::Forward),
        Command::OpenAbove => edit::open(state, Direction::Backward),

        Command::Move(m) => motion::motion(state, m),
        Command::FindChar(find, target) => motion::find_char(state, find, target),
        Command::SelectTextObject(scope, object) => motion::text_object(state, scope, object),
        Command::GotoSyntaxObject(object, direction) => {
            motion::goto_syntax_object(state, object, direction)
        }
        Command::RepeatMotion => {
            let last =
                (state.input.last_motion.as_ref()).map(|last| (last.command.clone(), last.count));
            if let Some((command, recorded)) = last {
                for _ in 0..count_of(state) {
                    state.input.count = recorded;
                    apply(state, command.clone(), out);
                }
            }
        }

        Command::CollapseSelection => {
            reselect(state, |text, range| Range::point(range.cursor(text)))
        }
        Command::FlipSelections => reselect(state, |_, range| range.flip()),
        Command::EnsureSelectionsForward => reselect(state, |_, range| {
            range.with_direction(helix_core::movement::Direction::Forward)
        }),
        Command::KeepPrimarySelection => select(state, |_, selection, _| {
            let range = selection.primary();
            Selection::single(range.anchor, range.head)
        }),
        Command::RemovePrimarySelection => {
            try_select(state, |_, selection, _| match selection.len() {
                1 => Err(Notice::NoSelectionsRemaining),
                _ => Ok(selection.clone().remove(selection.primary_index())),
            })
        }
        Command::RotateSelections(direction) => {
            let count = count_of(state);
            select(state, |_, selection, _| {
                let mut selection = selection.clone();
                let (index, len) = (selection.primary_index(), selection.len());
                selection.set_primary_index(match direction {
                    Direction::Forward => (index + count) % len,
                    Direction::Backward => (index + (len.saturating_sub(count) % len)) % len,
                });
                selection
            })
        }
        Command::RotateSelectionContents(direction) => {
            let rotate = state.input.count.map_or(1, NonZeroUsize::get);
            select(state, |_, selection, _| {
                let len = selection.len();
                let rotate = rotate.min(len);
                let mut selection = selection.clone();
                selection.set_primary_index(match direction {
                    Direction::Forward => (selection.primary_index() + len + rotate) % len,
                    Direction::Backward => (selection.primary_index() + len - rotate) % len,
                });
                selection
            });
            edit(state, |doc, selection, _| {
                let text = doc.text.slice(..);
                let mut fragments: Vec<helix_core::Tendril> = selection
                    .slices(text)
                    .map(|fragment| fragment.chunks().collect())
                    .collect();
                let rotate = rotate.min(fragments.len());
                match direction {
                    Direction::Forward => fragments.rotate_right(rotate),
                    Direction::Backward => fragments.rotate_left(rotate),
                }
                Transaction::change(
                    &doc.text,
                    selection
                        .ranges()
                        .iter()
                        .zip(fragments)
                        .map(|(range, fragment)| (range.from(), range.to(), Some(fragment))),
                )
            });
        }
        Command::SelectAll => select(state, |doc, _, _| {
            Selection::single(0, doc.text.len_chars())
        }),
        Command::ExtendLineBelow => {
            let count = count_of(state);
            select(state, |doc, selection, _| {
                let text = &doc.text;
                selection.clone().transform(|range| {
                    let (start_line, end_line) = range.line_range(text.slice(..));
                    let start = text.line_to_char(start_line);
                    let end = text.line_to_char((end_line + 1).min(text.len_lines()));
                    let lines = match range.from() == start && range.to() == end {
                        true => count + 1,
                        false => count,
                    };
                    Range::new(
                        start,
                        text.line_to_char((end_line + lines).min(text.len_lines())),
                    )
                })
            })
        }
        Command::ExtendToLineBounds => select(state, |doc, selection, _| {
            let text = &doc.text;
            selection.clone().transform(|range| {
                let (start_line, end_line) = range.line_range(text.slice(..));
                let start = text.line_to_char(start_line);
                let end = text.line_to_char((end_line + 1).min(text.len_lines()));
                Range::new(start, end).with_direction(range.direction())
            })
        }),
        Command::ShrinkToLineBounds => select(state, |doc, selection, _| {
            let text = &doc.text;
            selection.clone().transform(|range| {
                let (start_line, end_line) = range.line_range(text.slice(..));
                match start_line == end_line {
                    true => range,
                    false => {
                        let start = match text.line_to_char(start_line) == range.from() {
                            true => range.from(),
                            false => text.line_to_char((start_line + 1).min(text.len_lines())),
                        };
                        let full_end = text.line_to_char((end_line + 1).min(text.len_lines()));
                        let end = match full_end == range.to() {
                            true => full_end,
                            false => text.line_to_char(end_line),
                        };
                        Range::new(start, end).with_direction(range.direction())
                    }
                }
            })
        }),
        Command::SplitSelectionOnNewline => select(state, |doc, selection, _| {
            helix_core::selection::split_on_newline(doc.text.slice(..), selection)
        }),
        Command::MergeSelections => {
            select(state, |_, selection, _| selection.clone().merge_ranges())
        }
        Command::MergeConsecutiveSelections => select(state, |_, selection, _| {
            selection.clone().merge_consecutive_ranges()
        }),
        Command::CopySelectionOnLine(direction) => select::copy_selection_on_line(state, direction),
        Command::TrimSelections => select(state, |doc, selection, _| {
            let text = doc.text.slice(..);
            let ranges: SmallVec<[Range; 1]> = selection
                .iter()
                .filter(|range| {
                    !range.is_empty() && !range.slice(text).chars().all(char_is_whitespace)
                })
                .map(|range| {
                    let start = movement::skip_while(text, range.from(), char_is_whitespace)
                        .unwrap_or(range.from());
                    let end = movement::backwards_skip_while(text, range.to(), char_is_whitespace)
                        .unwrap_or(range.to());
                    Range::new(start, end).with_direction(range.direction())
                })
                .collect();
            let primary = selection.primary();
            match ranges.iter().position(|range| range.overlaps(&primary)) {
                _ if ranges.is_empty() => Selection::point(primary.cursor(text)),
                Some(index) => Selection::new(ranges, index),
                None => {
                    let last = ranges.len() - 1;
                    Selection::new(ranges, last)
                }
            }
        }),
        Command::AlignSelections => {
            edit::align_selections(state);
            exit_select_mode(state);
        }
        Command::ExpandSelection => select(state, |doc, selection, _| match &doc.syntax {
            Some(syntax) => object::expand_selection(syntax, doc.text.slice(..), selection.clone()),
            None => selection.clone(),
        }),
        Command::ShrinkSelection => select(state, |doc, selection, _| match &doc.syntax {
            Some(syntax) => object::shrink_selection(syntax, doc.text.slice(..), selection.clone()),
            None => selection.clone(),
        }),
        Command::SelectSibling(direction) => {
            select(state, |doc, selection, _| match (&doc.syntax, direction) {
                (Some(syntax), Direction::Forward) => {
                    object::select_next_sibling(syntax, doc.text.slice(..), selection.clone())
                }
                (Some(syntax), Direction::Backward) => {
                    object::select_prev_sibling(syntax, doc.text.slice(..), selection.clone())
                }
                (None, _) => selection.clone(),
            })
        }
        Command::SelectAllSiblings => select(state, |doc, selection, _| match &doc.syntax {
            Some(syntax) => {
                object::select_all_siblings(syntax, doc.text.slice(..), selection.clone())
            }
            None => selection.clone(),
        }),
        Command::SelectAllChildren => select(state, |doc, selection, _| match &doc.syntax {
            Some(syntax) => {
                object::select_all_children(syntax, doc.text.slice(..), selection.clone())
            }
            None => selection.clone(),
        }),
        Command::SearchNext(direction) => select::search_next(state, direction),
        Command::SearchSelection(bounds) => select::search_selection(state, bounds, out),

        Command::InsertChar(c) => edit::insert_char(state, c),
        Command::InsertNewline => edit::insert_newline(state),
        Command::InsertTab => edit::insert_tab(state),
        Command::SmartTab => {
            let after_text = match focused(&mut state.layout, &mut state.buffers) {
                Some((view, doc)) => view.selection.ranges().iter().any(|range| {
                    let text = doc.text.slice(..);
                    let cursor = range.cursor(text);
                    let line_start = text.line_to_char(text.char_to_line(cursor));
                    !text
                        .slice(line_start..cursor)
                        .chars()
                        .all(char::is_whitespace)
                }),
                None => false,
            };
            match after_text {
                true => motion::motion(state, Motion::ParentNodeEnd),
                false => edit::insert_tab(state),
            }
        }
        Command::DeleteCharBackward => edit::delete_char_backward(state),
        Command::DeleteCharForward => {
            let count = count_of(state);
            edit::delete_insert_mode(state, Direction::Forward, |text, range| {
                let pos = range.cursor(text);
                (pos, graphemes::nth_next_grapheme_boundary(text, pos, count))
            })
        }
        Command::DeleteWordBackward => {
            let count = count_of(state);
            edit::delete_insert_mode(state, Direction::Backward, |text, range| {
                let anchor = movement::move_prev_word_start(text, *range, count).from();
                let next = Range::new(anchor, range.cursor(text));
                // Exclude the cursor itself unless it sits at the end of the text.
                let next = match next.to() == range.to() && text.len_chars() != range.to() {
                    true => Range::new(
                        next.from(),
                        graphemes::prev_grapheme_boundary(text, range.to()),
                    ),
                    false => next,
                };
                (next.from(), next.to())
            })
        }
        Command::DeleteWordForward => {
            let count = count_of(state);
            edit::delete_insert_mode(state, Direction::Forward, |text, range| {
                let head = movement::move_next_word_end(text, *range, count).to();
                (range.cursor(text), head)
            })
        }
        Command::KillToLineStart => {
            edit::delete_insert_mode(state, Direction::Backward, |text, range| {
                let line = range.cursor_line(text);
                let first_char = text.line_to_char(line);
                let anchor = range.cursor(text);
                let head = match (anchor == first_char && line != 0, text.line(line)) {
                    (true, _) => line_end_char_index(&text, line - 1),
                    (false, line_text) => match line_text.first_non_whitespace_char() {
                        Some(pos) if first_char + pos < anchor => first_char + pos,
                        Some(_) | None => first_char,
                    },
                };
                (head, anchor)
            })
        }
        Command::Delete(yanking) => {
            edit::delete_selection(state, edit::Operation::Delete, yanking, out)
        }
        Command::Change(yanking) => {
            edit::delete_selection(state, edit::Operation::Change, yanking, out)
        }
        Command::Replace(target) => {
            edit(state, |doc, selection, _| {
                let mut buf = [0u8; 4];
                let ch: &str = match target {
                    model::command::CharTarget::Char(c) => c.encode_utf8(&mut buf),
                    model::command::CharTarget::LineEnding => doc.line_ending.as_str(),
                };
                Transaction::change_by_selection(&doc.text, selection, |range| {
                    match range.is_empty() {
                        false => {
                            let text: helix_core::Tendril = doc
                                .text
                                .slice(range.from()..range.to())
                                .graphemes()
                                .map(|_| ch)
                                .collect();
                            (range.from(), range.to(), Some(text))
                        }
                        true => (range.from(), range.to(), None),
                    }
                })
            });
            exit_select_mode(state);
        }
        Command::ChangeCase(case) => {
            edit(state, |doc, selection, _| {
                let text = doc.text.slice(..);
                Transaction::change_by_selection(&doc.text, selection, |range| {
                    let fragment = range.slice(text);
                    let changed: helix_core::Tendril = match case {
                        model::command::Case::Switch => fragment
                            .chars()
                            .flat_map(|c| {
                                let (upper, lower) = (c.is_uppercase(), c.is_lowercase());
                                match (lower, upper) {
                                    (true, _) => CaseChars::Upper(c.to_uppercase()),
                                    (false, true) => CaseChars::Lower(c.to_lowercase()),
                                    (false, false) => CaseChars::Keep(Some(c)),
                                }
                            })
                            .collect(),
                        model::command::Case::Lower => fragment
                            .chunks()
                            .map(|chunk| chunk.to_lowercase())
                            .collect(),
                        model::command::Case::Upper => fragment
                            .chunks()
                            .map(|chunk| chunk.to_uppercase())
                            .collect(),
                    };
                    (range.from(), range.to(), Some(changed))
                })
            });
            exit_select_mode(state);
        }
        Command::Indent => {
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
        Command::Unindent => {
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
        Command::Join(join) => edit::join_selections(state, join),
        Command::ToggleComments => {
            edit::toggle_comments(state);
            exit_select_mode(state);
        }
        Command::SurroundAdd(target) => {
            edit::surround_add(state, target);
            exit_select_mode(state);
        }
        Command::SurroundReplace(from, to) => {
            edit::surround_replace(state, from, to);
            exit_select_mode(state);
        }
        Command::SurroundDelete(pair) => {
            edit::surround_delete(state, pair);
            exit_select_mode(state);
        }
        Command::AddNewline(direction) => {
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
        Command::Increment => edit::increment(state, 1),
        Command::Decrement => edit::increment(state, -1),

        Command::Yank => {
            let register = state.input.register.unwrap_or(Register::DEFAULT);
            edit::yank(state, register, edit::Yank::All, out);
        }
        Command::YankTo(register) => edit::yank(state, register, edit::Yank::All, out),
        Command::YankMainTo(register) => edit::yank(state, register, edit::Yank::Primary, out),
        Command::Paste(at) => {
            let register = state.input.register.unwrap_or(Register::DEFAULT);
            paste_from(state, register, at, out);
        }
        Command::PasteFrom(register, at) => paste_from(state, register, at, out),
        Command::ReplaceWithYanked => {
            let register = state.input.register.unwrap_or(Register::DEFAULT);
            replace_from(state, register, out);
        }
        Command::ReplaceFrom(register) => replace_from(state, register, out),

        Command::Undo => step_history(state, HistoryStep::Undo, Notice::AlreadyOldest),
        Command::Redo => step_history(state, HistoryStep::Redo, Notice::AlreadyNewest),
        Command::Earlier => step_history(state, HistoryStep::Earlier, Notice::AlreadyOldest),
        Command::Later => step_history(state, HistoryStep::Later, Notice::AlreadyNewest),
        Command::CommitUndoCheckpoint => commit_focused(state),
        Command::RepeatInsert => {
            let entry = state.input.last_insert.entry.clone();
            let keys = state.input.last_insert.keys.clone();
            for _ in 0..count_of(state) {
                apply(state, entry.clone(), out);
                for key in &keys {
                    input::insert_key(state, *key, out);
                }
            }
        }

        Command::Await(pending) => state.input.awaiting = Some(pending),

        Command::CommandMode => {
            state.overlay = Overlay::Prompt(prompt::open(PromptKind::Command, &state.files))
        }
        Command::RegexPrompt(op) => match focused(&mut state.layout, &mut state.buffers) {
            Some((view, _)) => {
                let kind = PromptKind::Regex {
                    op,
                    register: state.input.register.unwrap_or(Register::SEARCH),
                    snapshot: view.selection.clone(),
                };
                state.overlay = Overlay::Prompt(prompt::open(kind, &state.files))
            }
            None => state.notice = Some(Notice::NoBuffer),
        },
        Command::PromptInsert(c) => prompt::insert(state, c),
        Command::Prompt(command) => prompt::command(state, command, out),
        Command::PromptCancel => match std::mem::replace(&mut state.overlay, Overlay::None) {
            Overlay::Prompt(Prompt {
                kind: PromptKind::Regex { snapshot, .. },
                ..
            }) => select(state, |_, _, _| snapshot),
            Overlay::Prompt(Prompt {
                kind: PromptKind::Command,
                ..
            })
            | Overlay::None
            | Overlay::Picker(_) => {}
        },
        // Enter on a chosen directory lists it instead of submitting, as in helix.
        Command::PromptSubmit => match &mut state.overlay {
            Overlay::Prompt(prompt)
                if matches!(
                    prompt.completion.chosen,
                    Chosen::At(i) if matches!(prompt.completion.candidates[i], Candidate::Dir(_))
                ) =>
            {
                prompt::complete(prompt, &state.files)
            }
            Overlay::Prompt(_) | Overlay::None | Overlay::Picker(_) => {
                match std::mem::replace(&mut state.overlay, Overlay::None) {
                    Overlay::Prompt(Prompt {
                        kind: PromptKind::Command,
                        line,
                        ..
                    }) => match prompt::remember(state, Register::COMMAND, line, out)
                        .map(|line| prompt::parse(&line))
                    {
                        Some(Ok(command)) => apply(state, command, out),
                        Some(Err(notice)) => state.notice = Some(notice),
                        None => {}
                    },
                    Overlay::Prompt(Prompt {
                        kind:
                            PromptKind::Regex {
                                op,
                                register,
                                snapshot,
                            },
                        line,
                        ..
                    }) => select::submit_regex(state, op, register, snapshot, line, out),
                    Overlay::None | Overlay::Picker(_) => {}
                }
            }
        },

        Command::Open(path) => open(state, path, Arrive::Text, None, OpenAs::Permanent, out),
        Command::GotoFile => links::goto_file(state, out),
        Command::Write => match focused_buffer(state) {
            Some(id) => out.extend(write_job(state, id, Dirs::Existing).map(Effect::Job)),
            None => state.notice = Some(Notice::NoBuffer),
        },
        Command::ForceWrite => match focused_buffer(state) {
            Some(id) => out.extend(write_job(state, id, Dirs::Create).map(Effect::Job)),
            None => state.notice = Some(Notice::NoBuffer),
        },
        Command::WriteAs(path, dirs) => match focused_buffer(state) {
            Some(id) => {
                let path = helix_stdx::path::normalize(state.root.path().join(path));
                match state.buffers.get_mut(id) {
                    Some(Buffer {
                        path: current,
                        content: Content::Text { disk, mtime, .. },
                    }) => {
                        *current = path;
                        *disk = Disk::New;
                        *mtime = None;
                    }
                    Some(Buffer {
                        content: Content::Loading { .. },
                        ..
                    })
                    | None => {}
                }
                out.extend(write_job(state, id, dirs).map(Effect::Job));
            }
            None => state.notice = Some(Notice::NoBuffer),
        },
        Command::WriteAll => {
            for id in modified_buffers(&state.buffers) {
                out.extend(write_job(state, id, Dirs::Existing).map(Effect::Job));
            }
        }
        Command::WriteQuit => match focused_buffer(state) {
            Some(id) => match write_job(state, id, Dirs::Existing) {
                Some(job) => {
                    out.push(Effect::Job(job));
                    state.exit = Exit::AfterWrite { buffers: vec![id] };
                }
                None => {}
            },
            None => apply(state, Command::Quit, out),
        },
        Command::WriteAllQuit => match modified_buffers(&state.buffers) {
            buffers if buffers.is_empty() => apply(state, Command::Quit, out),
            buffers => {
                for id in &buffers {
                    out.extend(write_job(state, *id, Dirs::Existing).map(Effect::Job));
                }
                state.exit = Exit::AfterWrite { buffers };
            }
        },
        Command::Quit => match panes::request_close(state) {
            panes::Close::Allowed => out.push(Effect::Quit),
            panes::Close::Refused => {}
        },
        Command::ForceQuit => out.push(Effect::Quit),
        Command::CloseBuffer => close_active(state, Force::No),
        Command::ForceCloseBuffer => close_active(state, Force::Yes),
        Command::Reload => match focused_buffer(state) {
            Some(id) => out.extend(reload_job(state, id).map(Effect::Job)),
            None => state.notice = Some(Notice::NoBuffer),
        },
        Command::ReloadAll => {
            let ids: Vec<BufferId> = state.buffers.keys().collect();
            for id in ids {
                out.extend(reload_job(state, id).map(Effect::Job));
            }
        }
        Command::GotoLine(line) => {
            state.input.count = Some(line);
            motion::motion(state, Motion::Line);
        }
        Command::Sort(case, order) => edit::sort(state, case, order),
        Command::Reflow(width) => edit(state, |doc, selection, _| {
            let width = width.map_or(doc.text_width, NonZeroUsize::get);
            Transaction::change_by_selection(&doc.text, selection, |range| {
                let fragment = range.fragment(doc.text.slice(..));
                let reflowed = helix_core::wrap::reflow_hard_wrap(&fragment, width);
                (range.from(), range.to(), Some(reflowed))
            })
        }),

        Command::Scroll(direction, by) => motion::scroll(state, direction, by),
        Command::AlignView(align) => motion::align_view(state, align),
        Command::StickyView => state.input.sticky = Sticky::View,

        Command::Window(command) => panes::window(state, command, out),
        Command::CycleBuffer(cycle) => panes::cycle_tab(state, cycle),
        Command::Tree(command) => panes::tree(state, command, out),
        Command::Picker(command) => panes::picker(state, command, out),

        Command::GotoHunk(target) => changes::goto_hunk(state, target, out),
        Command::Hunk(op) => changes::hunk_op(state, op, out),
        Command::File(op) => changes::file_op(state, op, out),
        Command::Confirm(answer) => match (state.confirm.take(), answer) {
            (None, _) => {}
            (Some(Confirm::Discard { .. }), Answer::No) => {
                state.notice = Some(Notice::DiscardNotConfirmed)
            }
            (Some(Confirm::Discard { path, how }), Answer::Yes) => {
                changes::discard(state, path, how, out)
            }
            (Some(Confirm::Create(_)), Answer::No) => {
                state.notice = Some(Notice::CreateNotConfirmed)
            }
            (Some(Confirm::Create(path)), Answer::Yes) => out.push(Effect::Job(Job::Fs {
                root: state.root.path().to_owned(),
                op: FsOp::Create {
                    path,
                    kind: EntryKind::File,
                },
            })),
        },

        Command::ToggleDiff => panes::toggle_diff(state),
        Command::Changes(command) => changes::list(state, command, out),
        Command::GotoConflict(direction) => conflict::goto(state, direction),
        Command::Resolve(take) => conflict::resolve(state, take),

        Command::RenderedSplit => rendered::split(state),
        Command::ToggleRendered => rendered::toggle(state),
        Command::Rendered(command) => rendered::command(state, command),
        Command::ExportPdf => crate::render::typeset::export_pdf(state, out),
    }
}

enum CaseChars {
    Upper(std::char::ToUppercase),
    Lower(std::char::ToLowercase),
    Keep(Option<char>),
}

impl Iterator for CaseChars {
    type Item = char;

    fn next(&mut self) -> Option<char> {
        match self {
            CaseChars::Upper(upper) => upper.next(),
            CaseChars::Lower(lower) => lower.next(),
            CaseChars::Keep(c) => c.take(),
        }
    }
}

/// Sorted, deduplicated lines touched by any range.
pub(crate) fn selected_lines(text: &Rope, selection: &Selection) -> Vec<usize> {
    let mut lines: Vec<usize> = selection
        .iter()
        .flat_map(|range| {
            let (start, end) = range.line_range(text.slice(..));
            start..=end
        })
        .collect();
    lines.sort_unstable();
    lines.dedup();
    lines
}

fn paste_from(state: &mut State, register: Register, at: Paste, out: &mut Vec<Effect>) {
    let count = count_of(state);
    match registers::read(state, register) {
        Values::Ready(values) => {
            edit::paste(state, &values, at, count);
            exit_select_mode(state);
        }
        Values::Clipboard(clipboard) => out.push(Effect::ReadClipboard {
            clipboard,
            then: ClipboardUse::Paste { at, count },
        }),
    }
}

fn replace_from(state: &mut State, register: Register, out: &mut Vec<Effect>) {
    let count = count_of(state);
    match registers::read(state, register) {
        Values::Ready(values) => {
            edit::replace_with(state, &values, count);
            exit_select_mode(state);
        }
        Values::Clipboard(clipboard) => out.push(Effect::ReadClipboard {
            clipboard,
            then: ClipboardUse::Replace { count },
        }),
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Force {
    No,
    Yes,
}

/// Closes the focused pane's active tab.
fn close_active(state: &mut State, force: Force) {
    let focus = state.layout.focus;
    match (state.layout.panes.get(focus))
        .and_then(|pane| pane.tabs.as_ref())
        .map(Tabs::active)
    {
        Some(index) => panes::close_tab(state, focus, index, force),
        None => state.notice = Some(Notice::NoBuffer),
    }
}

pub(crate) fn count_of(state: &State) -> usize {
    state.input.count.map_or(1, NonZeroUsize::get)
}

/// Select mode extends selections where normal mode moves them.
pub(crate) fn movement_of(state: &State) -> Movement {
    match state.input.mode {
        Mode::Select => Movement::Extend,
        Mode::Normal | Mode::Insert => Movement::Move,
    }
}

pub(crate) fn exit_select_mode(state: &mut State) {
    match state.input.mode {
        Mode::Select => state.input.mode = Mode::Normal,
        Mode::Normal | Mode::Insert => {}
    }
}

/// The focused editable text: a text view, or a diff's buffer side unless it is inert; a
/// rendered view takes no edits.
pub(crate) fn focused<'a, 'b>(
    layout: &'a mut Layout,
    buffers: &'b mut SlotMap<BufferId, Buffer>,
) -> Option<(&'a mut TextView, &'b mut TextDoc)> {
    let view = layout.panes.get_mut(layout.focus)?.active_mut()?;
    let content = &mut buffers.get_mut(view.buffer())?.content;
    let right = match view {
        View::Text(_) => Right::Live,
        View::Diff(_) => diffview::right(content),
        View::Rendered(_) => return None,
    };
    match (right, content) {
        (Right::Live, Content::Text { doc, .. }) => Some((view.text_mut(), doc)),
        (Right::Deleted, _) | (Right::Live, Content::Loading { .. }) => None,
    }
}

fn focused_buffer(state: &State) -> Option<BufferId> {
    state
        .layout
        .panes
        .get(state.layout.focus)?
        .active()
        .map(View::buffer)
}

fn modified_buffers(buffers: &SlotMap<BufferId, Buffer>) -> Vec<BufferId> {
    buffers
        .iter()
        .filter(|(_, buffer)| buffer.content.is_modified())
        .map(|(id, _)| id)
        .collect()
}

/// Maps every view of `buffer` in every tab through `changes`, keeping helix's selection
/// invariants.
pub(crate) fn map_views(
    layout: &mut Layout,
    buffer: BufferId,
    changes: &ChangeSet,
    text: RopeSlice,
) {
    for pane in layout.panes.values_mut() {
        for item in pane.items_mut() {
            let view = item.view.text_mut();
            match view.buffer() == buffer {
                true => {
                    view.selection = view.selection.clone().map(changes).ensure_invariants(text);
                    view.scroll.anchor = changes.map_pos(view.scroll.anchor, Assoc::Before);
                }
                false => {}
            }
        }
    }
}

pub(crate) fn try_select(
    state: &mut State,
    f: impl FnOnce(&TextDoc, &Selection, &Loader) -> Result<Selection, Notice>,
) {
    let State {
        layout,
        buffers,
        languages,
        notice,
        ..
    } = state;
    let Some((view, doc)) = focused(layout, buffers) else {
        return;
    };
    match f(doc, &view.selection, languages) {
        Ok(selection) => view.selection = selection.ensure_invariants(doc.text.slice(..)),
        Err(error) => *notice = Some(error),
    }
}

pub(crate) fn select(
    state: &mut State,
    f: impl FnOnce(&TextDoc, &Selection, &Loader) -> Selection,
) {
    try_select(state, |doc, selection, loader| {
        Ok(f(doc, selection, loader))
    });
}

pub(crate) fn reselect(state: &mut State, mut f: impl FnMut(RopeSlice, Range) -> Range) {
    select(state, |doc, selection, _| {
        let text = doc.text.slice(..);
        selection.clone().transform(|range| f(text, range))
    });
}

/// Applies the transaction `f` builds to the focused buffer, maps every view of it and
/// keeps its preview tabs.
pub(crate) fn try_edit(
    state: &mut State,
    f: impl FnOnce(&TextDoc, &Selection, &Loader) -> Result<Transaction, Notice>,
) {
    let State {
        layout,
        buffers,
        languages,
        notice,
        ..
    } = state;
    let focus = layout.focus;
    let Some((view, doc)) = focused(layout, buffers) else {
        *notice = Some(Notice::NoBuffer);
        return;
    };
    let tx = match f(doc, &view.selection, languages) {
        Ok(tx) => tx,
        Err(error) => {
            *notice = Some(error);
            return;
        }
    };
    let buffer = view.buffer();
    doc.apply(&tx, &view.selection, languages);
    let text = doc.text.slice(..);
    map_views(layout, buffer, tx.changes(), text);
    match tx.changes().is_empty() {
        true => {}
        false => layout.keep(buffer),
    }
    match (
        tx.selection(),
        layout.panes.get_mut(focus).and_then(Pane::active_mut),
    ) {
        (Some(selection), Some(view)) => {
            view.text_mut().selection = selection.clone().ensure_invariants(text);
        }
        (Some(_), None) | (None, _) => {}
    }
}

pub(crate) fn edit(
    state: &mut State,
    f: impl FnOnce(&TextDoc, &Selection, &Loader) -> Transaction,
) {
    try_edit(state, |doc, selection, loader| {
        Ok(f(doc, selection, loader))
    });
}

/// Groups the focused buffer's staged edits into an undo revision.
pub(crate) fn commit_focused(state: &mut State) {
    if let Some((view, doc)) = focused(&mut state.layout, &mut state.buffers) {
        match doc.uncommitted {
            Uncommitted::Pending { .. } => doc.commit(&view.selection),
            Uncommitted::Clean => {}
        }
    }
}

fn step_history(state: &mut State, step: HistoryStep, exhausted: Notice) {
    let count = count_of(state);
    let State {
        layout,
        buffers,
        languages,
        notice,
        ..
    } = state;
    let focus = layout.focus;
    for _ in 0..count {
        let Some((view, doc)) = focused(layout, buffers) else {
            return;
        };
        doc.commit(&view.selection);
        let buffer = view.buffer();
        let txs = doc.step_history(step, languages);
        if txs.is_empty() {
            *notice = Some(exhausted);
            return;
        }
        let text = doc.text.slice(..);
        layout.keep(buffer);
        for tx in &txs {
            map_views(layout, buffer, tx.changes(), text);
            match (
                tx.selection(),
                layout.panes.get_mut(focus).and_then(Pane::active_mut),
            ) {
                (Some(selection), Some(view)) => {
                    view.text_mut().selection = selection.clone().ensure_invariants(text);
                }
                (Some(_), None) | (None, _) => {}
            }
        }
    }
}

/// Commits `id` with the selection of a view showing it.
fn commit(state: &mut State, id: BufferId) {
    let selection = state
        .layout
        .panes
        .values()
        .flat_map(|pane| pane.items())
        .find_map(|item| {
            let view = item.view.text();
            (view.buffer() == id).then(|| view.selection.clone())
        })
        .unwrap_or_else(|| Selection::point(0));
    match state.buffers.get_mut(id).map(|b| &mut b.content) {
        Some(Content::Text { doc, .. }) => doc.commit(&selection),
        Some(Content::Loading { .. }) | None => {}
    }
}

/// `None` with a notice while the buffer is still loading; saving keeps its previews.
fn write_job(state: &mut State, id: BufferId, dirs: Dirs) -> Option<Job> {
    commit(state, id);
    let buffer = state.buffers.get(id)?;
    match &buffer.content {
        Content::Text { doc, .. } => {
            state.layout.keep(id);
            Some(Job::Write {
                buffer: id,
                path: buffer.path.clone(),
                text: doc.text.clone(),
                revision: doc.history.current_revision(),
                dirs,
            })
        }
        Content::Loading { .. } => {
            state.notice = Some(Notice::StillLoading {
                path: buffer.path.clone(),
            });
            None
        }
    }
}

/// `:reload` reads the file regardless of mtime and replaces unsaved edits.
fn reload_job(state: &mut State, id: BufferId) -> Option<Job> {
    commit(state, id);
    let buffer = state.buffers.get(id)?;
    match &buffer.content {
        Content::Text { .. } => Some(Job::Reload {
            buffer: id,
            path: buffer.path.clone(),
            mtime: None,
            policy: ReloadPolicy::Discard,
        }),
        Content::Loading { .. } => None,
    }
}

/// Char index of `to`, with the column clamped to the line's end.
pub(crate) fn line_column(text: &Rope, to: Position) -> usize {
    let line = to.line.min(text.len_lines().saturating_sub(1));
    let start = text.line_to_char(line);
    (start + to.column).min(line_end_char_index(&text.slice(..), line))
}

/// The buffer of `path`, read as a job when none has it yet; `None` with a notice when the
/// path cannot be opened.
pub(crate) fn load(state: &mut State, path: PathBuf, out: &mut Vec<Effect>) -> Option<BufferId> {
    let joined = helix_stdx::path::normalize(state.root.path().join(&path));
    let path = match joined.canonicalize() {
        Ok(path) if path.is_dir() => {
            state.notice = Some(Notice::NotAFile { path });
            return None;
        }
        Ok(path) => path,
        Err(error) if error.kind() == io::ErrorKind::NotFound => joined,
        Err(error) => {
            state.notice = Some(Notice::OpenFailed {
                path: joined,
                error,
            });
            return None;
        }
    };
    let existing = state
        .buffers
        .iter()
        .find(|(_, buffer)| buffer.path == path)
        .map(|(id, _)| id);
    match existing {
        Some(id) => Some(id),
        None => {
            let id = state.buffers.insert(Buffer {
                path: path.clone(),
                content: Content::Loading { jump: None },
            });
            out.push(Effect::Job(Job::Open {
                buffer: id,
                path,
                languages: state.languages.clone(),
            }));
            Some(id)
        }
    }
}

/// Shows `path` in the focused pane; `to` places the cursor, once the text or diff arrives if
/// need be.
pub(crate) fn open(
    state: &mut State,
    path: PathBuf,
    arrive: Arrive,
    to: Option<Target>,
    open: OpenAs,
    out: &mut Vec<Effect>,
) {
    let Some(id) = load(state, path, out) else {
        return;
    };
    let focus = state.layout.focus;
    panes::show(state, focus, arrive.key(id), open);
    let Some(to) = to else {
        return;
    };
    match &mut state.buffers[id].content {
        Content::Loading { jump } => {
            *jump = Some(Jump {
                pane: focus,
                arrive,
                to,
            })
        }
        Content::Text { .. } => place(state, focus, id, arrive, to),
    }
}

/// Puts the cursor of `buffer`'s tab in `pane` at `to`; a hunk waits for the buffer's diff.
pub(crate) fn place(state: &mut State, pane: PaneId, buffer: BufferId, arrive: Arrive, to: Target) {
    let State {
        layout,
        buffers,
        previews,
        notice,
        ..
    } = state;
    let (at, line) = match (to, crate::text::buffer::doc(buffers, buffer)) {
        (Target::Hunk(edge), _) => {
            return changes::expect_landing(state, buffer, Landing { pane, edge, arrive });
        }
        (Target::Position(to), Some(doc)) => {
            let at = line_column(&doc.text, to);
            (at, doc.text.char_to_line(at))
        }
        (Target::Heading(heading), Some(doc)) => {
            let parsed = rendered::markdown(previews, buffer, doc);
            match links::heading_line(doc, parsed, &heading) {
                Some(line) => (doc.text.line_to_char(line), line),
                None => return *notice = Some(Notice::HeadingNotFound { heading }),
            }
        }
        (Target::Position(_) | Target::Heading(_), None) => return,
    };
    let view = layout.panes.get_mut(pane).and_then(|target| {
        let index = target.tabs.as_ref()?.find(arrive.key(buffer))?;
        Some(&mut target.items_mut()[index].view)
    });
    match view {
        // `rendered::sync` brings the line's block to the top once parsed.
        Some(View::Rendered(view)) => {
            view.text.selection = Selection::point(at);
            view.scroll = rendered::Scroll::Line(line);
        }
        Some(view @ (View::Text(_) | View::Diff(_))) => {
            view.text_mut().selection = Selection::point(at);
            scroll::follow_cursor(state, pane);
        }
        None => {}
    }
}

/// Drops `id` and every tab showing it.
pub(crate) fn remove_buffer(state: &mut State, id: BufferId) {
    state.buffers.remove(id);
    let panes: SmallVec<[PaneId; 8]> = state.layout.panes.keys().collect();
    for pane in panes {
        // A pane may show the buffer as text, as a diff and rendered.
        for key in [TabKey::Text(id), TabKey::Diff(id), TabKey::Rendered(id)] {
            let index =
                (state.layout.panes.get(pane)).and_then(|target| target.tabs.as_ref()?.find(key));
            match index {
                Some(index) => {
                    state.layout.remove_tab(pane, index);
                }
                None => {}
            }
        }
    }
}
