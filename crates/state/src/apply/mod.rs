use helix_core::{Range, Selection, object};
use model::command::{Answer, Command, Direction, Dirs, Motion};
use model::config::OpenAs;
use model::mode::Mode;
use model::notice::Notice;
use model::register::Register;

use crate::drawing;
use crate::git::navigate::{self};
use crate::insert::{self, ImageFrom};
use crate::job::{FsOp, Job};
use crate::sidebar::tree::EntryKind;
use crate::text::buffer::HistoryStep;
use crate::view::panes::Arrive;
use crate::{
    Confirm, Effect, Exit, LastMotion, Overlay, PromptKind, State, Sticky, Waiting,
    git::conflict,
    git::stage,
    input,
    input::prompt,
    project::links,
    render,
    render::document::{self},
    text::edit,
    text::motion,
    text::select,
    view::panes,
    view::{self, mode},
};

pub mod files;
mod focused;

use files::{
    close_active, focused_file, focused_writable, reload_job, renamed, write_focused, write_job,
    write_modified,
};
use focused::{step_history, tree_select};

pub use files::Force;
pub use files::{open, opened, place, read_now, reloaded, written};
pub use focused::{
    Focused, FocusedText, commit_focused, count_of, edit, exit_select_mode, focused, focused_text,
    map_views, movement_of, reselect, select, selected_lines, try_edit, try_select,
};

pub fn apply(state: &mut State, command: Command, out: &mut Vec<Effect>) {
    let _span = tracing::trace_span!("command", ?command).entered();
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
            Mode::Insert => edit::exit_insert(state),
        },
        Command::InsertMode => {
            state.input.mode = Mode::Insert;
            reselect(state, |_, range| Range::new(range.to(), range.from()));
        }
        Command::AppendMode => edit::append_mode(state),
        Command::SelectMode => select::select_mode(state),
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
        Command::RotateSelections(direction) => select::rotate(state, direction),
        Command::RotateSelectionContents(direction) => edit::rotate_contents(state, direction),
        Command::SelectAll => select(state, |doc, _, _| {
            Selection::single(0, doc.text.len_chars())
        }),
        Command::ExtendLineBelow => select::extend_line_below(state),
        Command::ExtendToLineBounds => select::extend_to_line_bounds(state),
        Command::ShrinkToLineBounds => select::shrink_to_line_bounds(state),
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
        Command::TrimSelections => select::trim(state),
        Command::AlignSelections => {
            edit::align_selections(state);
            exit_select_mode(state);
        }
        Command::ExpandSelection => tree_select(state, object::expand_selection),
        Command::ShrinkSelection => tree_select(state, object::shrink_selection),
        Command::SelectSibling(Direction::Forward) => {
            tree_select(state, object::select_next_sibling)
        }
        Command::SelectSibling(Direction::Backward) => {
            tree_select(state, object::select_prev_sibling)
        }
        Command::SelectAllSiblings => tree_select(state, object::select_all_siblings),
        Command::SelectAllChildren => tree_select(state, object::select_all_children),
        Command::SearchNext(direction) => select::search_next(state, direction),
        Command::SearchSelection(bounds) => select::search_selection(state, bounds, out),

        Command::InsertChar(c) => edit::insert_char(state, c),
        Command::InsertNewline => edit::insert_newline(state),
        Command::InsertTab => edit::insert_tab(state),
        Command::SmartTab => edit::smart_tab(state),
        Command::DeleteCharBackward => edit::delete_char_backward(state),
        Command::DeleteCharForward => edit::delete_char_forward(state),
        Command::DeleteWordBackward => edit::delete_word_backward(state),
        Command::DeleteWordForward => edit::delete_word_forward(state),
        Command::KillToLineStart => edit::kill_to_line_start(state),
        Command::Delete(yanking) => {
            edit::delete_selection(state, edit::Operation::Delete, yanking, out)
        }
        Command::Change(yanking) => {
            edit::delete_selection(state, edit::Operation::Change, yanking, out)
        }
        Command::Replace(target) => edit::replace(state, target),
        Command::ChangeCase(case) => edit::change_case(state, case),
        Command::Indent => edit::indent(state),
        Command::Unindent => edit::unindent(state),
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
        Command::AddNewline(direction) => edit::add_newline(state, direction),
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
            edit::paste_from(state, register, at, out);
        }
        Command::PasteFrom(register, at) => edit::paste_from(state, register, at, out),
        Command::ReplaceWithYanked => {
            let register = state.input.register.unwrap_or(Register::DEFAULT);
            edit::replace_from(state, register, out);
        }
        Command::ReplaceFrom(register) => edit::replace_from(state, register, out),

        // Text being typed in a drawing undoes its own edits; otherwise the drawing steps.
        Command::Undo => match focused(&mut state.layout, &mut state.buffers) {
            Focused::NotText => drawing::command::step(state, drawing::command::Step::Undo),
            Focused::Text(_) | Focused::Empty | Focused::ReadOnly => {
                step_history(state, HistoryStep::Undo, Notice::AlreadyOldest)
            }
        },
        Command::Redo => match focused(&mut state.layout, &mut state.buffers) {
            Focused::NotText => drawing::command::step(state, drawing::command::Step::Redo),
            Focused::Text(_) | Focused::Empty | Focused::ReadOnly => {
                step_history(state, HistoryStep::Redo, Notice::AlreadyNewest)
            }
        },
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
            state.overlay = Overlay::Prompt(prompt::open(PromptKind::Command, &state.project.index))
        }
        Command::RegexPrompt(op) => prompt::open_regex(state, op),
        Command::PromptInsert(c) => prompt::insert(state, c),
        Command::Prompt(command) => prompt::command(state, command, out),
        Command::PromptCancel => prompt::cancel(state),
        Command::PromptSubmit => prompt::submit(state, out),

        Command::Open(path) => open(state, path, Arrive::AsIs, None, OpenAs::Permanent, out),
        Command::PasteImage => match insert::into_focused(state, None) {
            Ok(into) => out.push(Effect::ReadClipboardImage { into }),
            Err(notice) => state.notice = Some(notice),
        },
        Command::Image(path) => match (
            files::canonical(state.project.root.path(), &path).and_then(ImageFrom::file),
            insert::into_focused(state, None),
        ) {
            (Ok(image), Ok(into)) => insert::insert(state, into, image, out),
            (Err(notice), _) | (_, Err(notice)) => state.notice = Some(notice),
        },
        Command::GotoFile => links::goto_file(state, out),
        Command::Write => write_focused(state, Dirs::Existing, out),
        Command::ForceWrite => write_focused(state, Dirs::Create, out),
        Command::WriteAs(path, dirs) => {
            let path = helix_stdx::path::normalize(state.project.root.path().join(path));
            match focused_writable(&state.layout).and_then(|file| renamed(state, file, path)) {
                Ok(file) => {
                    let job = write_job(state, file, dirs);
                    out.push(Effect::Job(job));
                }
                Err(refused) => state.notice = Some(refused),
            }
        }
        Command::WriteAll => {
            write_modified(state, out);
        }
        Command::WriteQuit => match focused_writable(&state.layout) {
            Ok(file) => {
                let job = write_job(state, file, Dirs::Existing);
                out.push(Effect::Job(job));
                state.exit = Exit::AfterWrite(Waiting::one(file.into()));
            }
            Err(_) => apply(state, Command::Quit, out),
        },
        Command::WriteAllQuit => match Waiting::new(write_modified(state, out)) {
            Some(waiting) => state.exit = Exit::AfterWrite(waiting),
            None => apply(state, Command::Quit, out),
        },
        Command::Quit => match panes::request_close(state) {
            panes::Close::Allowed => out.push(Effect::Quit),
            panes::Close::Refused => {}
        },
        Command::ForceQuit => out.push(Effect::Quit),
        Command::CloseBuffer => close_active(state, Force::No),
        Command::ForceCloseBuffer => close_active(state, Force::Yes),
        Command::Reload => match focused_file(&state.layout) {
            Ok(opened) => {
                let job = reload_job(state, opened);
                out.push(Effect::Job(job))
            }
            Err(refused) => state.notice = Some(refused),
        },
        Command::ReloadAll => {
            let files: Vec<_> = state.files().collect();
            for opened in files {
                out.push(Effect::Job(reload_job(state, opened)));
            }
        }
        Command::GotoLine(line) => {
            state.input.count = Some(line);
            motion::motion(state, Motion::Line);
        }
        Command::Sort(case, order) => edit::sort(state, case, order),
        Command::Reflow(width) => edit::reflow(state, width),

        Command::Scroll(direction, by) => motion::scroll(state, direction, by),
        Command::AlignView(align) => motion::align_view(state, align),
        Command::StickyView => state.input.sticky = Sticky::View,

        Command::Window(command) => panes::window(state, command, out),
        Command::CycleBuffer(cycle) => panes::cycle_tab(state, cycle),
        Command::Tree(command) => panes::tree(state, command, out),
        Command::Picker(command) => panes::picker(state, command, out),

        Command::GotoHunk(target) => navigate::goto_hunk(state, target, out),
        Command::Hunk(op) => stage::hunk_op(state, op, out),
        Command::File(op) => stage::file_op(state, op, out),
        Command::Confirm(answer) => {
            match (std::mem::replace(&mut state.overlay, Overlay::None), answer) {
                (overlay @ (Overlay::None | Overlay::Prompt(_) | Overlay::Picker(_)), _) => {
                    state.overlay = overlay
                }
                (Overlay::Confirm(Confirm::Discard { .. }), Answer::No) => {
                    state.notice = Some(Notice::DiscardNotConfirmed)
                }
                (Overlay::Confirm(Confirm::Discard { path, how }), Answer::Yes) => {
                    stage::discard(state, path, how, out)
                }
                (Overlay::Confirm(Confirm::Create(_)), Answer::No) => {
                    state.notice = Some(Notice::CreateNotConfirmed)
                }
                (Overlay::Confirm(Confirm::Create(path)), Answer::Yes) => {
                    out.push(Effect::Job(Job::Fs {
                        root: state.project.root.path().to_owned(),
                        op: FsOp::Create {
                            path,
                            kind: EntryKind::File,
                        },
                    }))
                }
            }
        }

        Command::ToggleMode(shown) => mode::toggle(state, shown),
        Command::CycleMode(direction) => mode::cycle(state, direction),
        Command::ResizeSplit(resize) => mode::resize(state, resize),
        Command::Changes(command) => crate::sidebar::changes::list(state, command, out),
        Command::GotoConflict(direction) => conflict::goto(state, direction),
        Command::Resolve(take) => conflict::resolve(state, take),

        Command::Read(command) => render::read(state, command),
        Command::Document(command) => document::command(state, command),
        Command::View(command) => view::sheet::command(state, command),
        Command::Drawing(command) => drawing::command::command(state, command),
        Command::Export(to) => crate::export::export(state, to, out),
    }
}
