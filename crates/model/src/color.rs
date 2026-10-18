use std::fmt;
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

impl Rgb {
    pub const fn hex(value: u32) -> Rgb {
        Rgb {
            r: (value >> 16) as u8,
            g: (value >> 8) as u8,
            b: value as u8,
        }
    }
}

impl FromStr for Rgb {
    type Err = RgbError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let hex = s.strip_prefix('#').unwrap_or(s);
        match hex.len() {
            6 => u32::from_str_radix(hex, 16)
                .map(Rgb::hex)
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

impl Base16 {
    pub const ALL: [Base16; 16] = [
        Base16::Base00,
        Base16::Base01,
        Base16::Base02,
        Base16::Base03,
        Base16::Base04,
        Base16::Base05,
        Base16::Base06,
        Base16::Base07,
        Base16::Base08,
        Base16::Base09,
        Base16::Base0A,
        Base16::Base0B,
        Base16::Base0C,
        Base16::Base0D,
        Base16::Base0E,
        Base16::Base0F,
    ];
}

impl fmt::Display for Base16 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "base{:02X}", *self as u8)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Palette(pub [Rgb; 16]);

impl Palette {
    /// The built-in `default` scheme.
    pub const DEFAULT: Palette = Palette([
        Rgb::hex(0x061229),
        Rgb::hex(0x2a3448),
        Rgb::hex(0x4d5666),
        Rgb::hex(0x717885),
        Rgb::hex(0x9a99a3),
        Rgb::hex(0xb8bbc2),
        Rgb::hex(0xdbdde0),
        Rgb::hex(0xffffff),
        Rgb::hex(0xd07346),
        Rgb::hex(0xf0a000),
        Rgb::hex(0xfbd461),
        Rgb::hex(0x99bf52),
        Rgb::hex(0x72b9bf),
        Rgb::hex(0x5299bf),
        Rgb::hex(0x9989cc),
        Rgb::hex(0xb08060),
    ]);
}

impl Index<Base16> for Palette {
    type Output = Rgb;

    fn index(&self, slot: Base16) -> &Rgb {
        &self.0[slot as usize]
    }
}
