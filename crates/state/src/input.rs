use model::command::Command;
use model::key::{Key, KeyCode, KeySeq, Mods};
use model::mode::Mode;

use crate::apply::apply;
use crate::keymap::{Context, Resolved};
use crate::{Effect, Overlay, State};

pub fn context(state: &State) -> Context {
    match (&state.overlay, state.input.mode) {
        (Overlay::Prompt(_), _) => Context::Prompt,
        (Overlay::None, Mode::Normal) => Context::Normal,
        (Overlay::None, Mode::Insert) => Context::Insert,
        (Overlay::None, Mode::Select) => Context::Select,
    }
}

enum Step {
    Run(Command),
    Wait(KeySeq),
    Unbound,
}

pub fn key(state: &mut State, key: Key, out: &mut Vec<Effect>) {
    state.notice = None;
    let context = context(state);
    let Some(seq) = state.input.pending.push(key) else {
        state.input.pending = KeySeq::EMPTY;
        return;
    };
    let step = match state.keymap.resolve(context, seq.as_slice()) {
        Resolved::Command(command) => Step::Run(command.clone()),
        Resolved::Pending => Step::Wait(seq),
        Resolved::Unbound => Step::Unbound,
    };
    state.input.pending = KeySeq::EMPTY;
    let command = match (step, context, seq.len, key) {
        (Step::Run(command), ..) => Some(command),
        (Step::Wait(seq), ..) => {
            state.input.pending = seq;
            None
        }
        (
            Step::Unbound,
            Context::Insert,
            1,
            Key {
                code: KeyCode::Char(c),
                mods,
            },
        ) => (mods == Mods::empty()).then_some(Command::InsertChar(c)),
        (
            Step::Unbound,
            Context::Prompt,
            1,
            Key {
                code: KeyCode::Char(c),
                mods,
            },
        ) => (mods == Mods::empty()).then_some(Command::PromptInsert(c)),
        (Step::Unbound, ..) => None,
    };
    if let Some(command) = command {
        apply(state, command, out);
    }
}
