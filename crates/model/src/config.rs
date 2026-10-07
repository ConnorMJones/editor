use std::path::PathBuf;

use serde::Deserialize;

use crate::color::Rgb;

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct Config {
    pub theme: ThemeConfig,
    pub font: FontConfig,
    pub session: SessionConfig,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct ThemeConfig {
    pub palette: PathBuf,
    /// Replaces base00 as the editor background when set.
    pub background: Option<Rgb>,
}

impl Default for ThemeConfig {
    fn default() -> Self {
        ThemeConfig {
            palette: PathBuf::from("/etc/stylix/palette.json"),
            background: None,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct FontConfig {
    pub mono: FontSpec,
    pub ui: FontSpec,
}

impl Default for FontConfig {
    fn default() -> Self {
        FontConfig {
            mono: FontSpec {
                family: "Iosevka Nerd Font".to_owned(),
                size_px: 16.0,
            },
            ui: FontSpec {
                family: "DejaVu Sans".to_owned(),
                size_px: 14.0,
            },
        }
    }
}

/// `family` must name an installed font; startup fails otherwise.
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
