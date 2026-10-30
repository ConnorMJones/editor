use std::ops::Index;

use crate::color::Base16::{self, *};
use crate::color::{Palette, Rgb};
use crate::config::ThemeConfig;
use crate::git::GitStatus;
use crate::markdown::CalloutKind;
use crate::mode::Mode;

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
pub const CURSOR_TEXT: Base16 = Base00;

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
    /// Conflict marker and label rows, dimmer than the sections they open.
    OursMarker,
    TheirsMarker,
    BaseMarker,
}

impl Background {
    pub const COUNT: usize = 10;
    pub const ALL: [Background; Background::COUNT] = [
        Background::Added,
        Background::Removed,
        Background::Modified,
        Background::Filler,
        Background::Ours,
        Background::Theirs,
        Background::Base,
        Background::OursMarker,
        Background::TheirsMarker,
        Background::BaseMarker,
    ];
}

/// Colours of diff and conflict marks; each can be overridden under `[theme.diff]`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Tone {
    Added,
    Removed,
    Modified,
    Ours,
    Theirs,
    Base,
}

impl Tone {
    pub const COUNT: usize = 6;
    pub const ALL: [Tone; Tone::COUNT] = [
        Tone::Added,
        Tone::Removed,
        Tone::Modified,
        Tone::Ours,
        Tone::Theirs,
        Tone::Base,
    ];
}

/// Text of the label rows around a conflict.
pub const CONFLICT_LABEL: Style = Style::fg(Base04).with(Modifiers::ITALIC);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Alpha(u8); // private: checked by its constructor

impl Alpha {
    pub const OPAQUE: Alpha = Alpha(100);

    pub const fn percent(percent: u8) -> Alpha {
        assert!(percent <= 100, "alpha is a percentage");
        Alpha(percent)
    }

    /// `None` above 100.
    pub const fn new(percent: u8) -> Option<Alpha> {
        match percent <= 100 {
            true => Some(Alpha(percent)),
            false => None,
        }
    }

    pub fn get(self) -> u8 {
        self.0
    }

    pub fn fraction(self) -> f32 {
        f32::from(self.0) / 100.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Color {
    pub rgb: Rgb,
    pub alpha: Alpha,
}

impl Color {
    pub const fn opaque(rgb: Rgb) -> Color {
        Color {
            rgb,
            alpha: Alpha::OPAQUE,
        }
    }
}

pub struct Theme {
    pub background: Color,
    pub surface: Color,
    pub text: Color,
    pub muted: Color,
    /// Colours of syntax `Style` slots.
    pub syntax: Slots,
    pub mode: ModeColors,
    pub editor: EditorColors,
    pub completion: Completion,
    pub chrome: Chrome,
    pub tones: Tones,
    pub staged: Tones,
    pub rows: Rows,
    pub prose: Prose,
    pub reader: ReaderColors,
}

#[derive(Clone, Copy)]
pub struct ReaderColors {
    pub selection: Color,
    pub highlight: Color,
    pub hit: Color,
    pub current_hit: Color,
}

pub struct Slots(pub [Color; 16]);

pub struct ModeColors {
    pub fg: Color,
    pub bg: [Color; Mode::COUNT],
}

#[derive(Clone, Copy)]
pub struct EditorColors {
    pub selection: Color,
    pub cursorline: Color,
    pub cursor_text: Color,
    pub cursor_primary: Color,
    pub cursor_secondary: Color,
    pub line_number: Color,
    pub line_number_cursor: Color,
    pub scrollbar_track: Color,
    pub scrollbar_thumb: Color,
    pub scrollbar_thumb_dragged: Color,
}

pub struct Completion {
    pub background: Color,
    pub selected: Color,
    pub text: Color,
    pub dir: Color,
}

pub struct Chrome {
    pub border: Color,
    pub selection: Color,
    /// The cursor row of a list, whichever part of the window has the keys.
    pub picked: Color,
    pub unsaved: Color,
    pub accent: Color,
    pub title: Color,
    pub title_text: Color,
    pub hover: Color,
    pub close_hover: Color,
    pub vcs: [Color; GitStatus::COUNT],
}

#[derive(Clone, Copy)]
pub struct Tones(pub [Color; Tone::COUNT]);

#[derive(Clone, Copy)]
pub struct Rows(pub [Color; Background::COUNT]);

pub struct Prose {
    pub link: Color,
    pub missing_link: Color,
    pub code: Color,
    pub math: Color,
    pub marker: Color,
    pub quote_border: Color,
    pub quote_text: Color,
    pub checkbox: Color,
    pub error: Color,
    pub warning: Color,
    pub callouts: [Color; CalloutKind::COUNT],
}

impl Theme {
    pub fn new(palette: &Palette, config: &ThemeConfig) -> Theme {
        let slot = |slot: Base16| Color::opaque(palette[slot]);
        let tint = |color: Color, percent: u8| Color {
            alpha: Alpha::percent(percent),
            ..color
        };
        let diff = &config.diff;
        let tones = Tones(Tone::ALL.map(|tone| {
            let (set, default) = match tone {
                Tone::Added => (diff.added, Base0B),
                Tone::Removed => (diff.removed, Base08),
                Tone::Modified => (diff.modified, Base09),
                Tone::Ours => (diff.ours, Base0B),
                Tone::Theirs => (diff.theirs, Base09),
                Tone::Base => (diff.base, Base0E),
            };
            set.map_or(slot(default), Color::opaque)
        }));
        let background = config.background.map_or(slot(Base00), Color::opaque);
        Theme {
            background,
            surface: slot(Base01),
            text: slot(Base05),
            muted: slot(Base03),
            syntax: Slots(palette.0.map(Color::opaque)),
            mode: ModeColors {
                fg: slot(Base00),
                bg: [slot(Base0D), slot(Base0B), slot(Base0E)],
            },
            editor: EditorColors {
                selection: slot(Base02),
                cursorline: slot(Base01),
                cursor_text: slot(CURSOR_TEXT),
                cursor_primary: slot(Base06),
                cursor_secondary: slot(Base05),
                line_number: slot(Base03),
                line_number_cursor: slot(Base05),
                scrollbar_track: slot(Base01),
                scrollbar_thumb: slot(Base03),
                scrollbar_thumb_dragged: slot(Base04),
            },
            completion: Completion {
                background: slot(Base01),
                selected: slot(Base02),
                text: slot(Base05),
                dir: slot(Base0D),
            },
            chrome: Chrome {
                border: slot(Base02),
                selection: slot(Base02),
                picked: tint(slot(Base0D), 30),
                unsaved: slot(Base0A),
                accent: slot(Base0D),
                title: slot(Base01),
                title_text: slot(Base04),
                hover: tint(slot(Base02), 50),
                close_hover: slot(Base08),
                vcs: [
                    slot(Base03),
                    slot(Base0C),
                    slot(Base0B),
                    slot(Base08),
                    slot(Base0A),
                    slot(Base0E),
                ],
            },
            staged: Tones(tones.0.map(|tone| tint(tone, 40))),
            rows: Rows(Background::ALL.map(|role| match role {
                Background::Added => tint(tones[Tone::Added], 14),
                Background::Removed => tint(tones[Tone::Removed], 14),
                Background::Modified => tint(tones[Tone::Modified], 14),
                Background::Filler => slot(Base01),
                Background::Ours => tint(tones[Tone::Ours], 16),
                Background::Theirs => tint(tones[Tone::Theirs], 16),
                Background::Base => tint(tones[Tone::Base], 16),
                Background::OursMarker => tint(tones[Tone::Ours], 8),
                Background::TheirsMarker => tint(tones[Tone::Theirs], 8),
                Background::BaseMarker => tint(tones[Tone::Base], 8),
            })),
            reader: ReaderColors {
                selection: tint(slot(Base0D), 35),
                highlight: tint(slot(Base0A), 45),
                hit: tint(slot(Base0E), 25),
                current_hit: tint(slot(Base0E), 55),
            },
            tones,
            prose: Prose {
                link: slot(Base0D),
                missing_link: slot(Base03),
                code: slot(Base09),
                math: slot(Base0E),
                marker: slot(Base03),
                quote_border: slot(Base03),
                quote_text: slot(Base04),
                checkbox: slot(Base03),
                error: slot(Base08),
                warning: slot(Base0A),
                callouts: CalloutKind::ALL.map(|kind| {
                    slot(match kind {
                        CalloutKind::Note | CalloutKind::Info | CalloutKind::Todo => Base0D,
                        CalloutKind::Abstract | CalloutKind::Tip => Base0C,
                        CalloutKind::Success => Base0B,
                        CalloutKind::Question => Base0A,
                        CalloutKind::Warning => Base09,
                        CalloutKind::Failure | CalloutKind::Danger | CalloutKind::Bug => Base08,
                        CalloutKind::Example => Base0E,
                        CalloutKind::Quote => Base04,
                    })
                }),
            },
        }
    }
}

impl Index<Base16> for Slots {
    type Output = Color;

    fn index(&self, slot: Base16) -> &Color {
        &self.0[slot as usize]
    }
}

impl Index<Mode> for ModeColors {
    type Output = Color;

    fn index(&self, mode: Mode) -> &Color {
        &self.bg[mode as usize]
    }
}

impl Index<GitStatus> for Chrome {
    type Output = Color;

    fn index(&self, status: GitStatus) -> &Color {
        &self.vcs[status as usize]
    }
}

impl Index<Tone> for Tones {
    type Output = Color;

    fn index(&self, tone: Tone) -> &Color {
        &self.0[tone as usize]
    }
}

impl Index<Background> for Rows {
    type Output = Color;

    fn index(&self, role: Background) -> &Color {
        &self.0[role as usize]
    }
}

impl Index<CalloutKind> for Prose {
    type Output = Color;

    fn index(&self, kind: CalloutKind) -> &Color {
        &self.callouts[kind as usize]
    }
}
