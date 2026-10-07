use std::path::Path;

use model::key::{Key, KeyCode};
use state::buffer::{Content, TextDoc};
use state::layout::View;
use state::{Effect, Root, State, input, job};

pub struct Harness {
    pub state: State,
    pub quit: bool,
    effects: Vec<Effect>,
}

impl Harness {
    pub fn new(root: &Path) -> Harness {
        let root = Root::open(root).expect("temp dir is a directory");
        let state =
            State::new(root, helix_core::config::default_lang_loader()).expect("keymap builds");
        Harness {
            state,
            quit: false,
            effects: Vec::new(),
        }
    }

    /// Types keys in helix notation-ish: plain chars, `<esc>`, `<ret>`, `<bs>`, `<C-x>`.
    pub fn keys(&mut self, keys: &str) {
        let mut rest = keys;
        while let Some(c) = rest.chars().next() {
            let (key, len) = match (c, rest.find('>')) {
                ('<', Some(end)) => (special(&rest[1..end]), end + 1),
                _ => (Key::char(c), c.len_utf8()),
            };
            rest = &rest[len..];
            input::key(&mut self.state, key, &mut self.effects);
            self.drain();
        }
    }

    /// Runs queued jobs synchronously, as the host would on a background thread.
    fn drain(&mut self) {
        while let Some(effect) = self.effects.pop() {
            match effect {
                Effect::Job(work) => {
                    let result = job::run(work);
                    job::complete(&mut self.state, result, &mut self.effects);
                }
                Effect::Quit => self.quit = true,
            }
        }
    }

    pub fn doc(&self) -> &TextDoc {
        match &self.state.layout.panes[self.state.layout.focus].view {
            View::Text(view) => match &self.state.buffers[view.buffer].content {
                Content::Text(doc) => doc,
            },
            View::Empty => panic!("focused pane has no buffer"),
        }
    }
}

fn special(name: &str) -> Key {
    match name {
        "esc" => Key::code(KeyCode::Esc),
        "ret" => Key::code(KeyCode::Enter),
        "bs" => Key::code(KeyCode::Backspace),
        ctrl if ctrl.starts_with("C-") => Key::ctrl(ctrl[2..].chars().next().expect("C-<char>")),
        other => panic!("unknown key <{other}>"),
    }
}
