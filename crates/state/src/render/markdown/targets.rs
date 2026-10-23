//! Where links and images point: URLs, paths on disk, and vault notes.

use std::path::Path;
use std::sync::Arc;

use pulldown_cmark::LinkType;

use crate::project::links::{self, Dest, Resolution};

use super::{Base, ImageFile, LinkTarget, WikiLink};

const IMAGE_EXTENSIONS: [&str; 8] = ["png", "jpg", "jpeg", "gif", "webp", "svg", "bmp", "avif"];

/// A URL, or a path tried from the note's directory and the vault root, then by vault resolution.
pub fn locate(src: &str, base: Base) -> ImageFile {
    match has_scheme(src) {
        true => ImageFile::Remote(Arc::from(src)),
        false => {
            let path = Path::new(src);
            let candidates = match path.is_absolute() {
                true => [Some(path.to_owned()), None],
                false => [Some(base.dir.join(path)), Some(base.root.join(path))],
            };
            let found = candidates
                .into_iter()
                .flatten()
                .find(|candidate| candidate.is_file());
            match found {
                Some(found) => ImageFile::Local(found),
                None => vault_image(base, Dest::Path(src)),
            }
        }
    }
}

pub fn vault_image(base: Base, dest: Dest<&str>) -> ImageFile {
    match links::resolve(base.vault, base.folder, dest) {
        Resolution::File(path) => ImageFile::Local(path.to_path(base.root)),
        Resolution::Missing(_) | Resolution::Invalid => ImageFile::Missing,
    }
}

fn has_scheme(dest: &str) -> bool {
    match dest.split_once(':') {
        Some((scheme, _)) => {
            scheme.len() > 1
                && scheme.starts_with(|c: char| c.is_ascii_alphabetic())
                && scheme
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '.' | '-'))
        }
        None => false,
    }
}

pub fn percent_decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let hex = |b: u8| (b as char).to_digit(16);
        match (
            bytes[i],
            bytes.get(i + 1).copied().and_then(hex),
            bytes.get(i + 2).copied().and_then(hex),
        ) {
            (b'%', Some(high), Some(low)) => {
                out.push((high * 16 + low) as u8);
                i += 3;
            }
            (byte, _, _) => {
                out.push(byte);
                i += 1;
            }
        }
    }
    String::from_utf8(out).unwrap_or_else(|_| text.to_owned())
}

pub fn link_target(link_type: LinkType, dest: &str, alias: Option<String>) -> LinkTarget {
    match link_type {
        LinkType::WikiLink { .. } => LinkTarget::Wiki(WikiLink::parse(dest, alias)),
        LinkType::Email => LinkTarget::Url(format!("mailto:{dest}")),
        LinkType::Autolink
        | LinkType::Inline
        | LinkType::Reference
        | LinkType::ReferenceUnknown
        | LinkType::Collapsed
        | LinkType::CollapsedUnknown
        | LinkType::Shortcut
        | LinkType::ShortcutUnknown => match (dest.strip_prefix('#'), has_scheme(dest)) {
            (Some(heading), _) => LinkTarget::Heading(percent_decode(heading)),
            (None, true) => LinkTarget::Url(dest.to_owned()),
            (None, false) => {
                let (path, heading) = match dest.split_once('#') {
                    Some((path, heading)) => (path, Some(percent_decode(heading))),
                    None => (dest, None),
                };
                LinkTarget::Path {
                    path: percent_decode(path),
                    heading,
                }
            }
        },
    }
}

pub fn is_image_path(target: &str) -> bool {
    Path::new(target)
        .extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| IMAGE_EXTENSIONS.contains(&ext.to_ascii_lowercase().as_str()))
}
