//! Where links and images point: URLs, paths on disk, and vault notes.

use std::path::Path;
use std::sync::Arc;

use model::href::{Href, percent_decode, scheme};
use model::picture::Encoding;
use pulldown_cmark::LinkType;

use crate::project::links::{self, Dest, Resolution};

use super::{Base, ImageFile, ImagePlace, LinkTarget, LocalImage, WikiLink};

/// A URL, or a path tried from the note's directory and the vault root, then by vault resolution.
pub fn locate(src: &str, base: Base) -> ImageFile {
    match (scheme(src), Encoding::of_path(Path::new(src))) {
        (Some(_), _) => ImageFile::Remote(Arc::from(src)),
        (None, None) => ImageFile::NotPicture,
        (None, Some(encoding)) => {
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
                Some(found) => ImageFile::Local(LocalImage {
                    place: ImagePlace::File(found.into()),
                    encoding,
                }),
                None => vault_image(base, Dest::Path(src), encoding),
            }
        }
    }
}

pub fn vault_image(base: Base, dest: Dest<&str>, encoding: Encoding) -> ImageFile {
    match links::resolve(base.vault, base.folder, dest) {
        Resolution::File(path) => ImageFile::Local(LocalImage {
            place: ImagePlace::File(path.to_path(base.root).into()),
            encoding,
        }),
        Resolution::Missing(_) | Resolution::Invalid => ImageFile::Missing,
    }
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
        | LinkType::ShortcutUnknown => match Href::parse(dest) {
            Href::Url(url) => LinkTarget::Url(url.to_owned()),
            Href::Local {
                path: "",
                anchor: Some(heading),
            } => LinkTarget::Heading(percent_decode(heading)),
            Href::Local { path, anchor } => LinkTarget::Path {
                path: percent_decode(path),
                heading: anchor.map(percent_decode),
            },
        },
    }
}
