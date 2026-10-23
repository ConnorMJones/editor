use std::collections::HashMap;

use helix_core::{NATIVE_LINE_ENDING, Selection};
use model::command::Paste;
use model::notice::Notice;
use model::register::{Clipboard, Register};

use crate::apply::{exit_select_mode, focused_text};
use crate::view::layout::{NotInLayout, PaneId};
use crate::{Effect, State, text::edit};

/// What to do with clipboard contents once the host has read them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClipboardUse {
    Paste {
        at: Paste,
        count: usize,
    },
    Replace {
        count: usize,
    },
    /// Middle click: focuses `pane` and pastes before char `pos`.
    PasteAt {
        pane: PaneId,
        pos: usize,
    },
}

pub struct Registers {
    /// Stored registers, plus the last values yanked to each clipboard so multi-selection
    /// yanks paste back per selection.
    pub values: HashMap<Register, Vec<String>>,
    /// Register `n` searches with.
    pub last_search: Register,
}

impl Default for Registers {
    fn default() -> Registers {
        Registers {
            values: HashMap::new(),
            last_search: Register::SEARCH,
        }
    }
}

/// Register contents; clipboards are read by the host first.
pub enum Values {
    Ready(Vec<String>),
    Clipboard(Clipboard),
}

impl Registers {
    pub fn write(
        &mut self,
        register: Register,
        values: Vec<String>,
        out: &mut Vec<Effect>,
    ) -> Result<(), Notice> {
        match register {
            Register::BlackHole => Ok(()),
            Register::SelectionIndices | Register::SelectionContents | Register::DocumentPath => {
                Err(Notice::RegisterReadOnly {
                    register: register.char(),
                })
            }
            Register::Clipboard(clipboard) => {
                out.push(Effect::WriteClipboard {
                    clipboard,
                    text: values.join(NATIVE_LINE_ENDING.as_str()),
                });
                self.values.insert(register, values);
                Ok(())
            }
            Register::Stored(_) => {
                self.values.insert(register, values);
                Ok(())
            }
        }
    }

    /// Adds `value` as the newest entry, keeping older ones as history.
    pub fn push(
        &mut self,
        register: Register,
        value: String,
        out: &mut Vec<Effect>,
    ) -> Result<(), Notice> {
        match register {
            Register::Clipboard(_)
            | Register::BlackHole
            | Register::SelectionIndices
            | Register::SelectionContents
            | Register::DocumentPath => self.write(register, vec![value], out),
            Register::Stored(_) => {
                self.values.entry(register).or_default().insert(0, value);
                Ok(())
            }
        }
    }
}

pub fn read(state: &mut State, register: Register) -> Values {
    match register {
        Register::BlackHole => Values::Ready(Vec::new()),
        Register::SelectionIndices => Values::Ready(
            match focused_text(&mut state.layout, &mut state.buffers, &mut state.notice) {
                Some((view, _)) => (1..=view.selection.len()).map(|i| i.to_string()).collect(),
                None => Vec::new(),
            },
        ),
        Register::SelectionContents => Values::Ready(
            match focused_text(&mut state.layout, &mut state.buffers, &mut state.notice) {
                Some((view, doc)) => view
                    .selection
                    .fragments(doc.text.slice(..))
                    .map(|fragment| fragment.into_owned())
                    .collect(),
                None => Vec::new(),
            },
        ),
        Register::DocumentPath => {
            let path = state
                .layout
                .focused()
                .active_text()
                .map(|view| &state.buffers[view.buffer()])
                .map(|buffer| {
                    let path = buffer.path.strip_prefix(state.project.root.path());
                    path.unwrap_or(&buffer.path).display().to_string()
                });
            Values::Ready(path.into_iter().collect())
        }
        Register::Clipboard(clipboard) => Values::Clipboard(clipboard),
        Register::Stored(_) => Values::Ready(
            state
                .registers
                .values
                .get(&register)
                .cloned()
                .unwrap_or_default(),
        ),
    }
}

/// The newest value, answering clipboards from what was last yanked to them.
pub fn first(state: &mut State, register: Register) -> Option<String> {
    match read(state, register) {
        Values::Ready(values) => values.into_iter().next(),
        Values::Clipboard(_) => state
            .registers
            .values
            .get(&register)
            .and_then(|values| values.first().cloned()),
    }
}

/// Completes an `Effect::ReadClipboard`; `text` is `None` when the clipboard holds no text.
pub fn clipboard_contents(
    state: &mut State,
    clipboard: Clipboard,
    then: ClipboardUse,
    text: Option<String>,
) {
    let saved = state.registers.values.get(&Register::Clipboard(clipboard));
    let values = match (text, saved) {
        (None, Some(saved)) => saved.clone(),
        (None, None) => Vec::new(),
        (Some(text), Some(saved)) if saved.join(NATIVE_LINE_ENDING.as_str()) == text => {
            saved.clone()
        }
        (Some(text), Some(_) | None) => vec![text],
    };
    match then {
        ClipboardUse::Paste { at, count } => edit::paste(state, &values, at, count),
        ClipboardUse::Replace { count } => edit::replace_with(state, &values, count),
        ClipboardUse::PasteAt { pane, pos } => {
            match state.layout.set_focus(pane) {
                Ok(()) => {}
                Err(NotInLayout(_)) => return,
            }
            match focused_text(&mut state.layout, &mut state.buffers, &mut state.notice) {
                Some((view, doc)) => {
                    view.selection = Selection::point(pos.min(doc.text.len_chars()));
                    edit::paste(state, &values, Paste::Before, 1);
                }
                None => {}
            }
        }
    }
    exit_select_mode(state);
}
