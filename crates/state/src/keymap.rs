use model::command::Command;
use model::key::{Key, KeyCode};

/// Which binding table applies; derived from mode and overlay each key press.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Context {
    Normal,
    Insert,
    Select,
    Prompt,
}

impl Context {
    pub const COUNT: usize = 4;
    pub const ALL: [Context; Context::COUNT] = [
        Context::Normal,
        Context::Insert,
        Context::Select,
        Context::Prompt,
    ];
}

pub enum Trie {
    Leaf(Command),
    Node(Vec<(Key, Trie)>),
}

pub enum Resolved<'a> {
    Command(&'a Command),
    Pending,
    Unbound,
}

pub struct Keymap {
    pub tries: [Trie; Context::COUNT],
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum KeymapError {
    #[error("{context:?}: binding {keys:?} conflicts with an existing prefix or leaf")]
    Conflict { context: Context, keys: Vec<Key> },
    #[error("{context:?}: empty key sequence")]
    Empty { context: Context },
}

type Bindings = &'static [(&'static [Key], Command)];

const SAVE_ALL: [Key; 2] = [Key::ctrl('k'), Key::ctrl('s')];

static NORMAL: Bindings = &[
    (&[Key::char('i')], Command::InsertMode),
    (&[Key::char('a')], Command::AppendMode),
    (&[Key::char('v')], Command::SelectMode),
    (&[Key::char('h')], Command::MoveCharLeft),
    (&[Key::code(KeyCode::Left)], Command::MoveCharLeft),
    (&[Key::char('l')], Command::MoveCharRight),
    (&[Key::code(KeyCode::Right)], Command::MoveCharRight),
    (&[Key::char('k')], Command::MoveLineUp),
    (&[Key::code(KeyCode::Up)], Command::MoveLineUp),
    (&[Key::char('j')], Command::MoveLineDown),
    (&[Key::code(KeyCode::Down)], Command::MoveLineDown),
    (&[Key::char('u')], Command::Undo),
    (&[Key::char('U')], Command::Redo),
    (&[Key::char(':')], Command::CommandMode),
    (&SAVE_ALL, Command::WriteAll),
];

static INSERT: Bindings = &[
    (&[Key::code(KeyCode::Esc)], Command::NormalMode),
    (&[Key::code(KeyCode::Enter)], Command::InsertNewline),
    (
        &[Key::code(KeyCode::Backspace)],
        Command::DeleteCharBackward,
    ),
    (&[Key::code(KeyCode::Tab)], Command::InsertChar('\t')),
    (&[Key::code(KeyCode::Left)], Command::MoveCharLeft),
    (&[Key::code(KeyCode::Right)], Command::MoveCharRight),
    (&[Key::code(KeyCode::Up)], Command::MoveLineUp),
    (&[Key::code(KeyCode::Down)], Command::MoveLineDown),
    (&SAVE_ALL, Command::WriteAll),
];

static SELECT: Bindings = &[
    (&[Key::code(KeyCode::Esc)], Command::NormalMode),
    (&[Key::char('v')], Command::NormalMode),
    (&[Key::char(':')], Command::CommandMode),
    (&SAVE_ALL, Command::WriteAll),
];

static PROMPT: Bindings = &[
    (&[Key::code(KeyCode::Esc)], Command::PromptCancel),
    (&[Key::ctrl('c')], Command::PromptCancel),
    (&[Key::code(KeyCode::Enter)], Command::PromptSubmit),
    (&[Key::code(KeyCode::Backspace)], Command::PromptBackspace),
];

fn table(context: Context) -> Bindings {
    match context {
        Context::Normal => NORMAL,
        Context::Insert => INSERT,
        Context::Select => SELECT,
        Context::Prompt => PROMPT,
    }
}

fn insert(node: &mut Vec<(Key, Trie)>, keys: &[Key], command: &Command) -> Result<(), ()> {
    match keys {
        [] => Err(()),
        [key, rest @ ..] => {
            let slot = node.iter().position(|(k, _)| k == key);
            match (slot, rest) {
                (None, []) => {
                    node.push((*key, Trie::Leaf(command.clone())));
                    Ok(())
                }
                (None, _) => {
                    let mut child = Vec::new();
                    insert(&mut child, rest, command)?;
                    node.push((*key, Trie::Node(child)));
                    Ok(())
                }
                (Some(i), _) => match (&mut node[i].1, rest) {
                    (Trie::Node(child), [_, ..]) => insert(child, rest, command),
                    _ => Err(()),
                },
            }
        }
    }
}

fn build_trie(context: Context, bindings: Bindings) -> Result<Trie, KeymapError> {
    let mut root = Vec::with_capacity(bindings.len());
    for (keys, command) in bindings {
        match keys.is_empty() {
            true => return Err(KeymapError::Empty { context }),
            false => insert(&mut root, keys, command).map_err(|()| KeymapError::Conflict {
                context,
                keys: keys.to_vec(),
            })?,
        }
    }
    Ok(Trie::Node(root))
}

impl Keymap {
    pub fn helix() -> Result<Keymap, KeymapError> {
        let [normal, insert, select, prompt] = Context::ALL;
        Ok(Keymap {
            tries: [
                build_trie(normal, table(normal))?,
                build_trie(insert, table(insert))?,
                build_trie(select, table(select))?,
                build_trie(prompt, table(prompt))?,
            ],
        })
    }

    pub fn resolve(&self, context: Context, keys: &[Key]) -> Resolved<'_> {
        let mut trie = &self.tries[context as usize];
        for key in keys {
            trie = match trie {
                Trie::Node(children) => match children.iter().find(|(k, _)| k == key) {
                    Some((_, child)) => child,
                    None => return Resolved::Unbound,
                },
                Trie::Leaf(_) => return Resolved::Unbound,
            };
        }
        match trie {
            Trie::Leaf(command) => Resolved::Command(command),
            Trie::Node(_) => Resolved::Pending,
        }
    }
}
