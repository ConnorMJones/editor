use std::ops::Index;

use gpui::{Font, Hsla, Pixels, Rgba, Size, font, px};
use model::color::{Base16, Palette, Rgb};
use model::config::Config;
use model::git::GitStatus;
use model::mode::Mode;
use model::theme::Modifiers;

/// Resolved UI colours and fonts; mirrors the base16 mapping of the user's helix theme.
pub struct Theme {
    pub background: Hsla,
    pub surface: Hsla,
    pub text: Hsla,
    pub text_muted: Hsla,
    pub mode: ModeColors,
    pub colors: Colors,
    /// Monospace font per `Modifiers` bold/italic combination.
    pub mono: [Font; 4],
    pub mono_size: Pixels,
    /// Monospace cell; every text-grid position is a multiple of it.
    pub cell: Size<Pixels>,
    pub ui: Font,
    pub ui_size: Pixels,
    pub chrome: Chrome,
}

pub struct ModeColors {
    pub fg: Hsla,
    pub bg: [Hsla; Mode::COUNT],
}

impl Index<Mode> for ModeColors {
    type Output = Hsla;

    fn index(&self, mode: Mode) -> &Hsla {
        &self.bg[mode as usize]
    }
}

/// The palette as GPUI colours, indexed by base16 slot.
pub struct Colors(pub [Hsla; 16]);

impl Index<Base16> for Colors {
    type Output = Hsla;

    fn index(&self, slot: Base16) -> &Hsla {
        &self.0[slot as usize]
    }
}

fn hsla(c: Rgb) -> Hsla {
    Rgba {
        r: c.r as f32 / 255.0,
        g: c.g as f32 / 255.0,
        b: c.b as f32 / 255.0,
        a: 1.0,
    }
    .into()
}

impl Theme {
    pub fn new(palette: &Palette, config: &Config, cell: Size<Pixels>) -> Theme {
        let colors = Colors(palette.0.map(hsla));
        let mono = font(config.font.mono.family.clone());
        Theme {
            background: hsla(config.theme.background.unwrap_or(palette[Base16::Base00])),
            surface: colors[Base16::Base01],
            text: colors[Base16::Base05],
            text_muted: colors[Base16::Base03],
            mode: ModeColors {
                fg: colors[Base16::Base00],
                bg: [
                    colors[Base16::Base0D],
                    colors[Base16::Base0B],
                    colors[Base16::Base0E],
                ],
            },
            mono: [
                mono.clone(),
                mono.clone().bold(),
                mono.clone().italic(),
                mono.bold().italic(),
            ],
            mono_size: px(config.font.mono.size_px),
            cell,
            ui: font(config.font.ui.family.clone()),
            ui_size: px(config.font.ui.size_px),
            chrome: Chrome {
                border: colors[Base16::Base02],
                selection: colors[Base16::Base02],
                unsaved: colors[Base16::Base0A],
                accent: colors[Base16::Base0D],
                title: colors[Base16::Base01],
                title_text: colors[Base16::Base04],
                hover: colors[Base16::Base02].opacity(0.5),
                close_hover: colors[Base16::Base08],
                vcs: [
                    colors[Base16::Base03],
                    colors[Base16::Base0C],
                    colors[Base16::Base0B],
                    colors[Base16::Base08],
                    colors[Base16::Base0A],
                    colors[Base16::Base0E],
                ],
            },
            colors,
        }
    }

    pub fn mono_font(&self, modifiers: Modifiers) -> &Font {
        &self.mono[(modifiers & (Modifiers::BOLD | Modifiers::ITALIC)).bits() as usize]
    }
}

// Workspace chrome colours: title bar, tab line, tree, pickers.

pub struct Chrome {
    pub border: Hsla,
    pub selection: Hsla,
    pub unsaved: Hsla,
    pub accent: Hsla,
    pub title: Hsla,
    pub title_text: Hsla,
    /// Background of hovered rows, tabs and window controls.
    pub hover: Hsla,
    pub close_hover: Hsla,
    /// Indexed by `GitStatus`.
    pub vcs: [Hsla; GitStatus::COUNT],
}

impl Index<GitStatus> for Chrome {
    type Output = Hsla;

    fn index(&self, status: GitStatus) -> &Hsla {
        &self.vcs[status as usize]
    }
}
