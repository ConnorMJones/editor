//! Pictures: where an image's pixels come from and how they are encoded, never the pixels.

use std::num::NonZeroU32;
use std::path::Path;
use std::sync::Arc;
use std::time::SystemTime;

/// A bitmap encoding the decoder reads.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Bitmap {
    Png,
    Jpeg,
    Webp,
    Gif,
    Bmp,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Encoding {
    Bitmap(Bitmap),
    Svg,
}

impl Encoding {
    /// By a file's extension, in any case.
    pub fn of_path(path: &Path) -> Option<Encoding> {
        let extension = path.extension()?.to_str()?.to_ascii_lowercase();
        match extension.as_str() {
            "png" => Some(Encoding::Bitmap(Bitmap::Png)),
            "jpg" | "jpeg" => Some(Encoding::Bitmap(Bitmap::Jpeg)),
            "webp" => Some(Encoding::Bitmap(Bitmap::Webp)),
            "gif" => Some(Encoding::Bitmap(Bitmap::Gif)),
            "bmp" => Some(Encoding::Bitmap(Bitmap::Bmp)),
            "svg" => Some(Encoding::Svg),
            _ => None,
        }
    }
}

impl Bitmap {
    /// By the signature bytes an encoded image starts with.
    pub fn sniff(bytes: &[u8]) -> Option<Bitmap> {
        match bytes {
            [0x89, b'P', b'N', b'G', ..] => Some(Bitmap::Png),
            [0xff, 0xd8, 0xff, ..] => Some(Bitmap::Jpeg),
            [
                b'R',
                b'I',
                b'F',
                b'F',
                _,
                _,
                _,
                _,
                b'W',
                b'E',
                b'B',
                b'P',
                ..,
            ] => Some(Bitmap::Webp),
            [b'G', b'I', b'F', b'8', ..] => Some(Bitmap::Gif),
            [b'B', b'M', ..] => Some(Bitmap::Bmp),
            _ => None,
        }
    }
}

#[derive(Clone, PartialEq, Eq)]
pub enum Source {
    /// `modified` is the file's time as read, so a change on disk is a new picture.
    File {
        path: Arc<Path>,
        modified: SystemTime,
    },
    Bytes(Arc<[u8]>),
}

impl std::fmt::Debug for Source {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Source::File { path, .. } => write!(f, "File({path:?})"),
            Source::Bytes(bytes) => write!(f, "Bytes({} bytes)", bytes.len()),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Picture {
    pub source: Source,
    pub encoding: Encoding,
}

/// A picture's own width and height in pixels.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PixelSize {
    pub width: NonZeroU32,
    pub height: NonZeroU32,
}
