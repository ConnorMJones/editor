use serde::Deserialize;

use crate::color::Rgb;
use crate::fonts;
use crate::scheme::Scheme;

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct Config {
    pub theme: ThemeConfig,
    pub font: FontConfig,
    pub session: SessionConfig,
    pub tabs: TabsConfig,
    pub window: WindowConfig,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct ThemeConfig {
    pub scheme: Scheme,
    /// Replaces base00 as the editor background when set.
    pub background: Option<Rgb>,
    pub diff: DiffColors,
}

impl Default for ThemeConfig {
    fn default() -> Self {
        ThemeConfig {
            scheme: Scheme::Default,
            background: None,
            diff: DiffColors::default(),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct FontConfig {
    pub mono: FontSpec,
    pub ui: FontSpec,
    /// Rendered markdown and typst prose.
    pub prose: ProseFont,
}

impl Default for FontConfig {
    fn default() -> Self {
        FontConfig {
            mono: FontSpec {
                family: fonts::MONO.to_owned(),
                size_px: 16.0,
            },
            ui: FontSpec {
                family: fonts::SANS.to_owned(),
                size_px: 14.0,
            },
            prose: ProseFont::Ui,
        }
    }
}

/// The prose font: the interface font unless `[font.prose]` names its own.
#[derive(Debug, Deserialize)]
#[serde(from = "Option<FontSpec>")]
pub enum ProseFont {
    Ui,
    Own(FontSpec),
}

impl From<Option<FontSpec>> for ProseFont {
    fn from(spec: Option<FontSpec>) -> ProseFont {
        match spec {
            Some(spec) => ProseFont::Own(spec),
            None => ProseFont::Ui,
        }
    }
}

impl FontConfig {
    pub fn prose(&self) -> &FontSpec {
        match &self.prose {
            ProseFont::Ui => &self.ui,
            ProseFont::Own(spec) => spec,
        }
    }
}

/// `family` must name a bundled or installed font; startup fails otherwise.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FontSpec {
    pub family: String,
    pub size_px: f32,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct SessionConfig {
    pub restore: Restore,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Restore {
    #[default]
    Off,
    Last,
}

/// How files opened from each source join a pane's tabs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct TabsConfig {
    /// Tree clicks and `l`/enter on a file row.
    pub open_from_tree: OpenAs,
    /// File and grep pickers.
    pub open_from_picker: OpenAs,
}

impl Default for TabsConfig {
    fn default() -> Self {
        TabsConfig {
            open_from_tree: OpenAs::Preview,
            open_from_picker: OpenAs::Permanent,
        }
    }
}

/// A preview tab is replaced by the next preview opened in its pane.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OpenAs {
    Preview,
    Permanent,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct WindowConfig {
    pub title_bar: TitleBar,
}

/// The drawn title bar; GNOME offers no server-side one, so hiding it leaves Super+drag to move.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TitleBar {
    #[default]
    Hidden,
    Shown,
}

/// Overrides for diff and conflict colours; unset ones come from the palette.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct DiffColors {
    pub added: Option<Rgb>,
    pub removed: Option<Rgb>,
    pub modified: Option<Rgb>,
    pub ours: Option<Rgb>,
    pub theirs: Option<Rgb>,
    pub base: Option<Rgb>,
}
