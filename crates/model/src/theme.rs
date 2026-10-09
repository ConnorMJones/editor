use crate::color::Base16::{self, *};

bitflags::bitflags! {
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
    pub struct Modifiers: u8 {
        const BOLD = 1;
        const ITALIC = 2;
        const UNDERLINED = 4;
    }
}

/// A theme entry; `fg: None` keeps the colour of the style underneath.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Style {
    pub fg: Option<Base16>,
    pub modifiers: Modifiers,
}

impl Style {
    pub const fn fg(slot: Base16) -> Style {
        Style {
            fg: Some(slot),
            modifiers: Modifiers::empty(),
        }
    }

    pub const fn with(self, modifiers: Modifiers) -> Style {
        Style {
            fg: self.fg,
            modifiers: self.modifiers.union(modifiers),
        }
    }

    /// Layers `top` over `self` the way helix stacks nested highlights.
    pub const fn patch(self, top: Style) -> Style {
        Style {
            fg: match top.fg {
                Some(fg) => Some(fg),
                None => self.fg,
            },
            modifiers: self.modifiers.union(top.modifiers),
        }
    }
}

const NONE: Style = Style {
    fg: None,
    modifiers: Modifiers::empty(),
};

/// Syntax scopes of the user's helix theme; tree-sitter captures resolve to the
/// longest dotted prefix in this list and the highlight index is the array index.
pub const SCOPES: [(&str, Style); 44] = [
    ("comment", Style::fg(Base03)),
    ("keyword", Style::fg(Base0E)),
    ("keyword.control", Style::fg(Base0E)),
    ("keyword.function", Style::fg(Base0E)),
    ("keyword.operator", Style::fg(Base0E)),
    ("keyword.storage", Style::fg(Base0E)),
    ("operator", Style::fg(Base05)),
    ("punctuation", Style::fg(Base05)),
    ("punctuation.delimiter", Style::fg(Base05)),
    ("punctuation.bracket", Style::fg(Base05)),
    ("function", Style::fg(Base0C)),
    ("function.method", Style::fg(Base0C)),
    ("function.macro", Style::fg(Base09)),
    ("function.builtin", Style::fg(Base0C)),
    ("type", Style::fg(Base0B)),
    ("type.builtin", Style::fg(Base0B)),
    ("constructor", Style::fg(Base0B)),
    ("constant", Style::fg(Base09)),
    ("constant.builtin", Style::fg(Base09)),
    ("constant.numeric", Style::fg(Base09)),
    ("string", Style::fg(Base0A)),
    ("string.regexp", Style::fg(Base0C)),
    ("string.special", Style::fg(Base0C)),
    ("variable", Style::fg(Base0D)),
    ("variable.builtin", Style::fg(Base08)),
    ("variable.parameter", Style::fg(Base0D)),
    ("variable.other.member", Style::fg(Base0D)),
    ("label", Style::fg(Base08)),
    ("attribute", Style::fg(Base09)),
    ("namespace", Style::fg(Base04)),
    ("module", Style::fg(Base04)),
    ("special", Style::fg(Base0C)),
    ("tag", Style::fg(Base08)),
    ("markup.heading", Style::fg(Base0D).with(Modifiers::BOLD)),
    ("markup.bold", NONE.with(Modifiers::BOLD)),
    ("markup.italic", NONE.with(Modifiers::ITALIC)),
    (
        "markup.link.url",
        Style::fg(Base09).with(Modifiers::UNDERLINED),
    ),
    ("markup.link.text", Style::fg(Base0D)),
    ("markup.list", Style::fg(Base08)),
    ("markup.quote", Style::fg(Base0B).with(Modifiers::ITALIC)),
    ("markup.raw", Style::fg(Base0B)),
    ("diff.plus", Style::fg(Base0B)),
    ("diff.minus", Style::fg(Base08)),
    ("diff.delta", Style::fg(Base09)),
];

pub const TEXT: Style = Style::fg(Base05);
pub const WHITESPACE: Style = Style::fg(Base02);
pub const INDENT_GUIDE: Style = Style::fg(Base02);

/// Background colours of the text area.
pub const SELECTION: Base16 = Base02;
pub const CURSORLINE: Base16 = Base01;

pub const LINENR: Base16 = Base03;
pub const LINENR_SELECTED: Base16 = Base05;

pub struct CursorColors {
    pub fg: Base16,
    pub bg: Base16,
}

pub const CURSOR_PRIMARY: CursorColors = CursorColors {
    fg: Base00,
    bg: Base06,
};
pub const CURSOR_SECONDARY: CursorColors = CursorColors {
    fg: Base00,
    bg: Base05,
};

pub const DIFF_PLUS: Base16 = Base0B;
pub const DIFF_MINUS: Base16 = Base08;
pub const DIFF_DELTA: Base16 = Base09;

pub const SCROLLBAR_TRACK: Base16 = Base01;
pub const SCROLLBAR_THUMB: Base16 = Base03;
/// Thumb colour while dragged.
pub const SCROLLBAR_THUMB_ACTIVE: Base16 = Base04;

/// Prompt completion menu, as helix's `ui.menu` and `ui.menu.selected`.
pub const MENU: Base16 = Base01;
pub const MENU_SELECTED: Base16 = Base02;
pub const MENU_TEXT: Base16 = Base05;
pub const MENU_DIR: Base16 = Base0D;

/// Row background roles of diff and conflict views.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Background {
    Added,
    Removed,
    Modified,
    /// Virtual rows that keep two sides of a diff aligned.
    Filler,
    Ours,
    Theirs,
    /// The common ancestor section of a diff3 conflict.
    Base,
}

impl Background {
    pub const COUNT: usize = 7;
    pub const ALL: [Background; Background::COUNT] = [
        Background::Added,
        Background::Removed,
        Background::Modified,
        Background::Filler,
        Background::Ours,
        Background::Theirs,
        Background::Base,
    ];
}

/// A palette slot blended over the editor background at `alpha` percent.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Tint {
    pub slot: Base16,
    pub alpha: u8,
}

pub const fn background(role: Background) -> Tint {
    match role {
        Background::Added => Tint {
            slot: Base0B,
            alpha: 14,
        },
        Background::Removed => Tint {
            slot: Base08,
            alpha: 14,
        },
        Background::Modified => Tint {
            slot: Base09,
            alpha: 14,
        },
        Background::Filler => Tint {
            slot: Base01,
            alpha: 100,
        },
        Background::Ours => Tint {
            slot: Base0D,
            alpha: 16,
        },
        Background::Theirs => Tint {
            slot: Base0E,
            alpha: 16,
        },
        Background::Base => Tint {
            slot: Base03,
            alpha: 16,
        },
    }
}

/// Opacity of gutter signs for hunks that are already staged, in percent.
pub const STAGED_SIGN_ALPHA: u8 = 40;
