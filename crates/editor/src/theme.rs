use std::ops::Index;

use gpui::{Font, Hsla, Pixels, Rgba, font, px};
use model::color::{Base16, Palette, Rgb};
use model::config::Config;
use model::mode::Mode;

/// Resolved UI colours and fonts; mirrors the base16 mapping of the user's helix theme.
pub struct Theme {
    pub background: Hsla,
    pub surface: Hsla,
    pub text: Hsla,
    pub text_muted: Hsla,
    pub mode: ModeColors,
    pub mono: Font,
    pub mono_size: Pixels,
    pub ui: Font,
    pub ui_size: Pixels,
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
    pub fn new(palette: &Palette, config: &Config) -> Theme {
        let p = |slot| hsla(palette[slot]);
        Theme {
            background: hsla(config.theme.background.unwrap_or(palette[Base16::Base00])),
            surface: p(Base16::Base01),
            text: p(Base16::Base05),
            text_muted: p(Base16::Base03),
            mode: ModeColors {
                fg: p(Base16::Base00),
                bg: [p(Base16::Base0D), p(Base16::Base0B), p(Base16::Base0E)],
            },
            mono: font(config.font.mono.family.clone()),
            mono_size: px(config.font.mono.size_px),
            ui: font(config.font.ui.family.clone()),
            ui_size: px(config.font.ui.size_px),
        }
    }
}
