/// Which system clipboard a register maps to.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Clipboard {
    System,
    /// The X11/Wayland primary selection.
    Primary,
}

/// Helix registers; special registers compute their values on read.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Register {
    BlackHole,
    SelectionIndices,
    SelectionContents,
    DocumentPath,
    Clipboard(Clipboard),
    Stored(Stored),
}

/// A register that keeps written values.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Stored(char);

impl Stored {
    pub fn char(self) -> char {
        self.0
    }
}

impl Register {
    pub const DEFAULT: Register = Register::Stored(Stored('"'));
    pub const SEARCH: Register = Register::Stored(Stored('/'));
    /// History of the `:` prompt.
    pub const COMMAND: Register = Register::Stored(Stored(':'));

    pub fn from_char(c: char) -> Register {
        match c {
            '_' => Register::BlackHole,
            '#' => Register::SelectionIndices,
            '.' => Register::SelectionContents,
            '%' => Register::DocumentPath,
            '+' => Register::Clipboard(Clipboard::System),
            '*' => Register::Clipboard(Clipboard::Primary),
            c => Register::Stored(Stored(c)),
        }
    }

    pub fn char(self) -> char {
        match self {
            Register::BlackHole => '_',
            Register::SelectionIndices => '#',
            Register::SelectionContents => '.',
            Register::DocumentPath => '%',
            Register::Clipboard(Clipboard::System) => '+',
            Register::Clipboard(Clipboard::Primary) => '*',
            Register::Stored(stored) => stored.0,
        }
    }
}
