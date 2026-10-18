use std::fmt;
use std::io;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::color::{Base16, Palette, Rgb};

/// A colour scheme: the built-in default, or `<name>.toml` in the schemes folder.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(try_from = "String")]
pub enum Scheme {
    Default,
    File(SchemeName),
}

/// A scheme file's name without its extension; one plain path component.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SchemeName(String);

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
#[error("scheme name {0:?} must be one file name without '/', '.' or an extension")]
pub struct SchemeNameError(String);

#[derive(Debug, thiserror::Error)]
pub enum SchemeError {
    #[error("cannot read scheme {path:?}: {source}")]
    Read { path: PathBuf, source: io::Error },
    #[error("invalid scheme {path:?}: {source}")]
    Parse {
        path: PathBuf,
        source: toml::de::Error,
    },
    #[error("scheme {path:?} has no `inherits` and is missing {}", list(slots))]
    Missing { path: PathBuf, slots: Vec<Base16> },
    #[error("schemes inherit in a cycle: {}", list(names))]
    Cycle { names: Vec<SchemeName> },
}

/// One scheme file; slots it leaves out come from `inherits`.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SchemeFile {
    inherits: Option<Scheme>,
    base00: Option<Rgb>,
    base01: Option<Rgb>,
    base02: Option<Rgb>,
    base03: Option<Rgb>,
    base04: Option<Rgb>,
    base05: Option<Rgb>,
    base06: Option<Rgb>,
    base07: Option<Rgb>,
    base08: Option<Rgb>,
    base09: Option<Rgb>,
    #[serde(rename = "base0A")]
    base0a: Option<Rgb>,
    #[serde(rename = "base0B")]
    base0b: Option<Rgb>,
    #[serde(rename = "base0C")]
    base0c: Option<Rgb>,
    #[serde(rename = "base0D")]
    base0d: Option<Rgb>,
    #[serde(rename = "base0E")]
    base0e: Option<Rgb>,
    #[serde(rename = "base0F")]
    base0f: Option<Rgb>,
}

impl TryFrom<String> for Scheme {
    type Error = SchemeNameError;

    fn try_from(name: String) -> Result<Scheme, SchemeNameError> {
        match name.as_str() {
            "default" => Ok(Scheme::Default),
            _ => SchemeName::try_from(name).map(Scheme::File),
        }
    }
}

impl TryFrom<String> for SchemeName {
    type Error = SchemeNameError;

    fn try_from(name: String) -> Result<SchemeName, SchemeNameError> {
        match name.as_str() {
            "" => Err(SchemeNameError(name)),
            plain if plain.contains(['/', '\\', '.']) => Err(SchemeNameError(name)),
            _ => Ok(SchemeName(name)),
        }
    }
}

impl fmt::Display for SchemeName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl SchemeFile {
    fn slots(&self) -> [Option<Rgb>; 16] {
        [
            self.base00,
            self.base01,
            self.base02,
            self.base03,
            self.base04,
            self.base05,
            self.base06,
            self.base07,
            self.base08,
            self.base09,
            self.base0a,
            self.base0b,
            self.base0c,
            self.base0d,
            self.base0e,
            self.base0f,
        ]
    }
}

/// Resolves `scheme` and everything it inherits from, reading files from `dir`.
pub fn load(scheme: &Scheme, dir: &Path) -> Result<Palette, SchemeError> {
    let mut chain: Vec<(SchemeName, SchemeFile)> = Vec::new();
    let mut next = scheme.clone();
    let base = loop {
        let name = match next {
            Scheme::Default => break Palette::DEFAULT,
            Scheme::File(name) => name,
        };
        match chain.iter().position(|(seen, ..)| *seen == name) {
            Some(start) => {
                let mut names: Vec<SchemeName> =
                    chain.drain(start..).map(|(name, ..)| name).collect();
                names.push(name);
                return Err(SchemeError::Cycle { names });
            }
            None => {}
        }
        let path = dir.join(format!("{name}.toml"));
        let text = std::fs::read_to_string(&path).map_err(|source| SchemeError::Read {
            path: path.clone(),
            source,
        })?;
        let file: SchemeFile = toml::from_str(&text).map_err(|source| SchemeError::Parse {
            path: path.clone(),
            source,
        })?;
        match file.inherits.clone() {
            Some(parent) => {
                next = parent;
                chain.push((name, file));
            }
            None => {
                let mut colours = [Rgb::hex(0); 16];
                let mut missing = Vec::new();
                for ((slot, colour), value) in
                    Base16::ALL.into_iter().zip(&mut colours).zip(file.slots())
                {
                    match value {
                        Some(rgb) => *colour = rgb,
                        None => missing.push(slot),
                    }
                }
                match missing.as_slice() {
                    [] => break Palette(colours),
                    _ => {
                        return Err(SchemeError::Missing {
                            path,
                            slots: missing,
                        });
                    }
                }
            }
        }
    };
    Ok(chain.iter().rev().fold(base, |mut palette, (.., file)| {
        for (colour, slot) in palette.0.iter_mut().zip(file.slots()) {
            match slot {
                Some(rgb) => *colour = rgb,
                None => {}
            }
        }
        palette
    }))
}

fn list<T: fmt::Display>(items: &[T]) -> String {
    items
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(", ")
}
