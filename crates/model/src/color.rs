use std::ops::Index;
use std::str::FromStr;

use serde::Deserialize;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(try_from = "String")]
pub struct Rgb {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum RgbError {
    #[error("expected 6 hex digits with an optional leading '#', got {0:?}")]
    Length(String),
    #[error("invalid hex digit in {0:?}")]
    Digit(String),
}

impl FromStr for Rgb {
    type Err = RgbError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let hex = s.strip_prefix('#').unwrap_or(s);
        match hex.len() {
            6 => u32::from_str_radix(hex, 16)
                .map(|v| Rgb {
                    r: (v >> 16) as u8,
                    g: (v >> 8) as u8,
                    b: v as u8,
                })
                .map_err(|_| RgbError::Digit(s.to_owned())),
            _ => Err(RgbError::Length(s.to_owned())),
        }
    }
}

impl TryFrom<String> for Rgb {
    type Error = RgbError;

    fn try_from(s: String) -> Result<Self, Self::Error> {
        s.parse()
    }
}

/// Base16 slot names; the discriminant is the palette index.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Base16 {
    Base00,
    Base01,
    Base02,
    Base03,
    Base04,
    Base05,
    Base06,
    Base07,
    Base08,
    Base09,
    Base0A,
    Base0B,
    Base0C,
    Base0D,
    Base0E,
    Base0F,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(from = "PaletteFile")]
pub struct Palette(pub [Rgb; 16]);

impl Index<Base16> for Palette {
    type Output = Rgb;

    fn index(&self, slot: Base16) -> &Rgb {
        &self.0[slot as usize]
    }
}

/// Layout of stylix's `palette.json`; non-colour keys are ignored.
#[derive(Deserialize)]
struct PaletteFile {
    base00: Rgb,
    base01: Rgb,
    base02: Rgb,
    base03: Rgb,
    base04: Rgb,
    base05: Rgb,
    base06: Rgb,
    base07: Rgb,
    base08: Rgb,
    base09: Rgb,
    #[serde(rename = "base0A")]
    base0a: Rgb,
    #[serde(rename = "base0B")]
    base0b: Rgb,
    #[serde(rename = "base0C")]
    base0c: Rgb,
    #[serde(rename = "base0D")]
    base0d: Rgb,
    #[serde(rename = "base0E")]
    base0e: Rgb,
    #[serde(rename = "base0F")]
    base0f: Rgb,
}

impl From<PaletteFile> for Palette {
    fn from(f: PaletteFile) -> Self {
        Palette([
            f.base00, f.base01, f.base02, f.base03, f.base04, f.base05, f.base06, f.base07,
            f.base08, f.base09, f.base0a, f.base0b, f.base0c, f.base0d, f.base0e, f.base0f,
        ])
    }
}
