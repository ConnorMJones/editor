use std::fmt;

bitflags::bitflags! {
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
    pub struct Mods: u8 {
        const CTRL = 1;
        const ALT = 2;
        /// Only set for non-character keys; shifted characters arrive as their shifted char.
        const SHIFT = 4;
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum KeyCode {
    Char(char),
    Enter,
    Esc,
    Tab,
    Backspace,
    Delete,
    Left,
    Right,
    Up,
    Down,
    Home,
    End,
    PageUp,
    PageDown,
    F(u8),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Key {
    pub code: KeyCode,
    pub mods: Mods,
}

impl Key {
    pub const fn char(c: char) -> Key {
        Key {
            code: KeyCode::Char(c),
            mods: Mods::empty(),
        }
    }

    pub const fn ctrl(c: char) -> Key {
        Key {
            code: KeyCode::Char(c),
            mods: Mods::CTRL,
        }
    }

    pub const fn alt(c: char) -> Key {
        Key {
            code: KeyCode::Char(c),
            mods: Mods::ALT,
        }
    }

    pub const fn code(code: KeyCode) -> Key {
        Key {
            code,
            mods: Mods::empty(),
        }
    }
}

/// Helix notation: `C-k`, `A-x`, `S-tab`, `esc`, `space`.
impl fmt::Display for Key {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (flag, prefix) in [(Mods::CTRL, "C-"), (Mods::ALT, "A-"), (Mods::SHIFT, "S-")] {
            if self.mods.contains(flag) {
                f.write_str(prefix)?;
            }
        }
        match self.code {
            KeyCode::Char(' ') => f.write_str("space"),
            KeyCode::Char(c) => write!(f, "{c}"),
            KeyCode::Enter => f.write_str("ret"),
            KeyCode::Esc => f.write_str("esc"),
            KeyCode::Tab => f.write_str("tab"),
            KeyCode::Backspace => f.write_str("backspace"),
            KeyCode::Delete => f.write_str("del"),
            KeyCode::Left => f.write_str("left"),
            KeyCode::Right => f.write_str("right"),
            KeyCode::Up => f.write_str("up"),
            KeyCode::Down => f.write_str("down"),
            KeyCode::Home => f.write_str("home"),
            KeyCode::End => f.write_str("end"),
            KeyCode::PageUp => f.write_str("pageup"),
            KeyCode::PageDown => f.write_str("pagedown"),
            KeyCode::F(n) => write!(f, "F{n}"),
        }
    }
}

/// Keys typed so far towards a multi-key binding.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KeySeq {
    pub keys: [Key; KeySeq::MAX],
    pub len: usize,
}

impl KeySeq {
    pub const MAX: usize = 4;

    pub const EMPTY: KeySeq = KeySeq {
        keys: [Key::code(KeyCode::Esc); KeySeq::MAX],
        len: 0,
    };

    pub fn as_slice(&self) -> &[Key] {
        &self.keys[..self.len]
    }

    /// Returns `None` when the sequence is already at `MAX`.
    pub fn push(self, key: Key) -> Option<KeySeq> {
        match self.len < KeySeq::MAX {
            true => {
                let mut next = self;
                next.keys[next.len] = key;
                next.len += 1;
                Some(next)
            }
            false => None,
        }
    }
}
