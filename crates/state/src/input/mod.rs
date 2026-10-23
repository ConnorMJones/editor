pub mod keymap;
pub mod picker;
pub mod prompt;

use std::num::NonZeroUsize;

use model::command::{
    CharTarget, Command, Object, Paste, Pending, PickerCommand, PlainObject, SurroundPair,
    SyntaxObject, TreeCommand,
};
use model::key::{Key, KeyCode, KeySeq};
use model::mode::Mode;
use model::register::Register;

use crate::apply::{apply, commit_focused};
use crate::input::keymap::{Context, Resolved};
use crate::view::tab::{Beside, Showing, Tab};
use crate::{Effect, Files, Focus, Overlay, Sidebar, State, Sticky, view::scroll};

pub fn context(state: &State) -> Context {
    match (&state.overlay, &state.sidebar) {
        (Overlay::Prompt(_), _) => Context::Prompt,
        (Overlay::Picker(_), _) => Context::Picker,
        (Overlay::Confirm(_), _) => Context::Confirm,
        (Overlay::None, Sidebar::Files(Files::Tree)) => Context::Tree,
        (Overlay::None, Sidebar::Files(Files::Edit(_))) => Context::TreeInput,
        (Overlay::None, Sidebar::Changes(Focus::Sidebar)) => Context::Changes,
        (
            Overlay::None,
            Sidebar::Hidden | Sidebar::Files(Files::Editor) | Sidebar::Changes(Focus::Editor),
        ) => match state.layout.focused().active() {
            Some(Tab::Ink(_)) => Context::Drawing,
            Some(Tab::Text(text)) => match text.showing() {
                Showing::Document(_, _, Beside::Alone) => Context::Document,
                Showing::Pages(_, _, Beside::Alone) => Context::Pages,
                // Beside its rendering, the source takes the keys.
                Showing::Source(_)
                | Showing::Diff(..)
                | Showing::Document(_, _, Beside::Source(..))
                | Showing::Pages(_, _, Beside::Source(..)) => mode_context(state),
            },
            None => mode_context(state),
        },
    }
}

fn mode_context(state: &State) -> Context {
    match state.input.mode {
        Mode::Normal => Context::Normal,
        Mode::Insert => Context::Insert,
        Mode::Select => Context::Select,
    }
}

const MAX_COUNT: usize = 100_000_000;

const VIEW_PREFIX: Key = Key::char('z');

pub fn key(state: &mut State, key: Key, out: &mut Vec<Effect>) {
    let _span = tracing::trace_span!("key", ?key).entered();
    state.notice = None;
    match (state.input.awaiting.take(), context(state)) {
        (Some(pending), context) => {
            complete_pending(state, pending, key, out);
            if context == Context::Insert {
                state.input.last_insert.keys.push(key);
            }
        }
        (None, Context::Insert) => {
            insert_key(state, key, out);
            state.input.last_insert.keys.push(key);
        }
        (None, context @ (Context::Normal | Context::Select)) => {
            let digit = match key.code {
                KeyCode::Char(c) if key.mods.is_empty() && state.input.pending.len == 0 => {
                    c.to_digit(10).map(|digit| digit as usize)
                }
                _ => None,
            };
            match (digit, state.input.count, state.input.sticky) {
                (Some(digit), Some(count), _) => {
                    let count = count.get() * 10 + digit;
                    if count <= MAX_COUNT {
                        state.input.count = NonZeroUsize::new(count);
                    }
                }
                (Some(digit @ 1..), None, Sticky::Off)
                    if matches!(state.keymap.resolve(context, &[key]), Resolved::Unbound) =>
                {
                    state.input.count = NonZeroUsize::new(digit);
                }
                (_, _, Sticky::View) => match state.keymap.resolve(context, &[VIEW_PREFIX, key]) {
                    Resolved::Command(command) => {
                        let command = command.clone();
                        run(state, command, out);
                    }
                    Resolved::Pending | Resolved::Unbound => {
                        state.input.sticky = Sticky::Off;
                        state.input.count = None;
                    }
                },
                (Some(_), None, Sticky::Off) | (None, _, Sticky::Off) => {
                    if let Some(command) = step(state, context, key) {
                        run(state, command, out);
                    }
                }
            }
        }
        (
            None,
            context @ (Context::Prompt
            | Context::Tree
            | Context::TreeInput
            | Context::Picker
            | Context::Changes
            | Context::Confirm
            | Context::Document
            | Context::Pages
            | Context::Drawing),
        ) => {
            if let Some(command) = step(state, context, key) {
                run(state, command, out);
            }
        }
    }
    match state.input.mode {
        Mode::Insert => {}
        Mode::Normal | Mode::Select => commit_focused(state),
    }
    let focus = state.layout.focus();
    scroll::follow_cursor(state, focus);
}

/// Handles one insert-mode key without recording it; `.` replays through here.
pub fn insert_key(state: &mut State, key: Key, out: &mut Vec<Effect>) {
    if let Some(command) = step(state, Context::Insert, key) {
        run(state, command, out);
    }
}

/// Advances the pending key sequence; returns the command once one is bound.
fn step(state: &mut State, context: Context, key: Key) -> Option<Command> {
    let Some(seq) = state.input.pending.push(key) else {
        state.input.pending = KeySeq::EMPTY;
        return None;
    };
    let resolved = match state.keymap.resolve(context, seq.as_slice()) {
        Resolved::Command(command) => Some(command.clone()),
        Resolved::Pending => {
            state.input.pending = seq;
            return None;
        }
        Resolved::Unbound => None,
    };
    state.input.pending = KeySeq::EMPTY;
    match (resolved, seq.len, key.code) {
        (Some(command), ..) => Some(command),
        (None, 1, KeyCode::Char(c)) if key.mods.is_empty() => typed(context, c),
        (None, ..) => None,
    }
}

/// Runs a command, then drops the count and register unless it awaits another key.
fn run(state: &mut State, command: Command, out: &mut Vec<Effect>) {
    let entry = match state.input.mode {
        Mode::Insert => None,
        Mode::Normal | Mode::Select => Some(command.clone()),
    };
    apply(state, command, out);
    match (entry, state.input.mode) {
        (Some(entry), Mode::Insert) => {
            state.input.last_insert.entry = entry;
            state.input.last_insert.keys.clear();
        }
        (Some(_), Mode::Normal | Mode::Select) | (None, _) => {}
    }
    match state.input.awaiting {
        Some(_) => {}
        None => {
            state.input.count = None;
            state.input.register = None;
        }
    }
}

fn complete_pending(state: &mut State, pending: Pending, key: Key, out: &mut Vec<Effect>) {
    let typed = match key.code {
        KeyCode::Char(c) => Some(c),
        _ => None,
    };
    let target = match key.code {
        KeyCode::Char(c) => Some(CharTarget::Char(c)),
        KeyCode::Tab => Some(CharTarget::Char('\t')),
        KeyCode::Enter => Some(CharTarget::LineEnding),
        _ => None,
    };
    let surround = |c: char| match c {
        'm' => SurroundPair::Closest,
        c => SurroundPair::Char(c),
    };
    let command = match (pending, typed, target) {
        (Pending::Find(find), _, Some(target)) => Some(Command::FindChar(find, target)),
        (Pending::Replace, _, Some(target)) => Some(Command::Replace(target)),
        (Pending::SelectRegister, Some(c), _) => {
            state.input.register = Some(Register::from_char(c));
            None
        }
        (Pending::InsertRegister, Some(c), _) => {
            Some(Command::PasteFrom(Register::from_char(c), Paste::Cursor))
        }
        (Pending::TextObject(scope), Some(c), _) => {
            let object = match c {
                'w' => Some(Object::Plain(PlainObject::Word)),
                'W' => Some(Object::Plain(PlainObject::LongWord)),
                'p' => Some(Object::Plain(PlainObject::Paragraph)),
                't' => Some(Object::Syntax(SyntaxObject::Class)),
                'f' => Some(Object::Syntax(SyntaxObject::Function)),
                'a' => Some(Object::Syntax(SyntaxObject::Parameter)),
                'c' => Some(Object::Syntax(SyntaxObject::Comment)),
                'T' => Some(Object::Syntax(SyntaxObject::Test)),
                'e' => Some(Object::Syntax(SyntaxObject::Entry)),
                'x' => Some(Object::Syntax(SyntaxObject::XmlElement)),
                'm' => Some(Object::Plain(PlainObject::ClosestPair)),
                c if !c.is_ascii_alphanumeric() => Some(Object::Plain(PlainObject::Pair(c))),
                _ => None,
            };
            object.map(|object| Command::SelectTextObject(scope, object))
        }
        (Pending::SurroundAdd, Some(c), _) => Some(Command::SurroundAdd(CharTarget::Char(c))),
        (Pending::SurroundAdd, None, Some(CharTarget::LineEnding)) => {
            Some(Command::SurroundAdd(CharTarget::LineEnding))
        }
        (Pending::SurroundReplace, Some(c), _) => {
            Some(Command::Await(Pending::SurroundReplaceWith(surround(c))))
        }
        (Pending::SurroundReplaceWith(from), Some(c), _) => Some(Command::SurroundReplace(from, c)),
        (Pending::SurroundDelete, Some(c), _) => Some(Command::SurroundDelete(surround(c))),
        (
            Pending::Find(_)
            | Pending::Replace
            | Pending::SelectRegister
            | Pending::InsertRegister
            | Pending::TextObject(_)
            | Pending::SurroundAdd
            | Pending::SurroundReplace
            | Pending::SurroundReplaceWith(_)
            | Pending::SurroundDelete,
            _,
            _,
        ) => {
            state.input.count = None;
            state.input.register = None;
            None
        }
    };
    if let Some(command) = command {
        run(state, command, out);
    }
}

/// How the host should deliver a key press.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Route {
    Keymap,
    /// Unbound printable key in a text context; it arrives through `text` instead.
    Text,
}

pub fn route(state: &State, key: Key) -> Route {
    let context = context(state);
    match (
        state.input.awaiting,
        context,
        state.input.pending.len,
        key.code,
    ) {
        (None, _, 0, KeyCode::Char(c)) if key.mods.is_empty() && typed(context, c).is_some() => {
            match state.keymap.resolve(context, &[key]) {
                Resolved::Unbound => Route::Text,
                Resolved::Command(_) | Resolved::Pending => Route::Keymap,
            }
        }
        _ => Route::Keymap,
    }
}

/// Inserts committed text (typed, composed or from an IME) in text contexts.
pub fn text(state: &mut State, text: &str, out: &mut Vec<Effect>) {
    state.notice = None;
    for c in text.chars() {
        let context = context(state);
        match typed(context, c) {
            Some(command) => {
                run(state, command, out);
                if context == Context::Insert {
                    state.input.last_insert.keys.push(Key::char(c));
                }
            }
            None => break,
        }
    }
    let focus = state.layout.focus();
    scroll::follow_cursor(state, focus);
}

/// The command inserting `c` in contexts that take typed text.
pub fn typed(context: Context, c: char) -> Option<Command> {
    match context {
        Context::Insert => Some(Command::InsertChar(c)),
        Context::Prompt => Some(Command::PromptInsert(c)),
        Context::Picker => Some(Command::Picker(PickerCommand::Insert(c))),
        Context::TreeInput => Some(Command::Tree(TreeCommand::EditInsert(c))),
        Context::Normal
        | Context::Select
        | Context::Tree
        | Context::Changes
        | Context::Confirm
        | Context::Document
        | Context::Pages
        | Context::Drawing => None,
    }
}
