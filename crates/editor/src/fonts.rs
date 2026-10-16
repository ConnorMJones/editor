use gpui::{Font, Hsla, Pixels, Rgba, Size, font, px};
use model::config::FontConfig;
use model::theme::{Color, Modifiers};

/// Line height of rendered prose, relative to its size.
pub const PROSE_LINE: f32 = 1.5;

pub struct Fonts {
    /// Per `Modifiers` bold/italic combination.
    pub mono: [Font; 4],
    pub mono_size: Pixels,
    /// Monospace cell; every text-grid position is a multiple of it.
    pub cell: Size<Pixels>,
    pub ui: Font,
    pub ui_size: Pixels,
    /// Rendered prose per `Modifiers` bold/italic combination.
    pub prose: [Font; 4],
    pub prose_size: Pixels,
    /// Distance from a prose line's baseline to its bottom, at `prose_size` and `PROSE_LINE`.
    pub prose_depth: Pixels,
}

impl Fonts {
    /// `prose_extent` is the prose font's ascent and descent at 1px.
    pub fn new(config: &FontConfig, cell: Size<Pixels>, prose_extent: [f32; 2]) -> Fonts {
        let prose_size = config.prose().size_px;
        let [ascent, descent] = prose_extent.map(|extent| extent * prose_size);
        let styled = |family: &str| {
            let plain = font(family.to_owned());
            [
                plain.clone(),
                plain.clone().bold(),
                plain.clone().italic(),
                plain.bold().italic(),
            ]
        };
        Fonts {
            mono: styled(&config.mono.family),
            mono_size: px(config.mono.size_px),
            cell,
            ui: font(config.ui.family.clone()),
            ui_size: px(config.ui.size_px),
            prose: styled(&config.prose().family),
            prose_size: px(prose_size),
            // GPUI centres the glyph box in the line.
            prose_depth: px((prose_size * PROSE_LINE - ascent - descent) / 2.0 + descent),
        }
    }

    pub fn mono_font(&self, modifiers: Modifiers) -> &Font {
        &self.mono[(modifiers & (Modifiers::BOLD | Modifiers::ITALIC)).bits() as usize]
    }
}

pub fn hsla(color: Color) -> Hsla {
    Rgba {
        r: f32::from(color.rgb.r) / 255.0,
        g: f32::from(color.rgb.g) / 255.0,
        b: f32::from(color.rgb.b) / 255.0,
        a: color.alpha.fraction(),
    }
    .into()
}
