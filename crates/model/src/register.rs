/// Which system clipboard a register maps to.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Clipboard {
    /// `+`
    System,
    /// `*`, the X11/Wayland primary selection.
    Primary,
}

/// Helix registers; special registers compute their values on read.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Register {
    /// `_` discards writes and reads empty.
    BlackHole,
    /// `#` reads the 1-based index of each selection.
    SelectionIndices,
    /// `.` reads the selected text.
    SelectionContents,
    /// `%` reads the buffer path.
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
