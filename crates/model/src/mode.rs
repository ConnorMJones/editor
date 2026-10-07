/// Helix editing modes; the discriminant indexes per-mode tables.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Mode {
    Normal,
    Insert,
    Select,
}

impl Mode {
    pub const COUNT: usize = 3;

    pub fn label(self) -> &'static str {
        match self {
            Mode::Normal => "NOR",
            Mode::Insert => "INS",
            Mode::Select => "SEL",
        }
    }
}
