//! Image files open in tabs, each shown as one page fitted to the pane.

use std::io::Cursor;
use std::path::Path;
use std::sync::Arc;
use std::time::SystemTime;

use model::picture::{Bitmap, Encoding, Picture, Source};
use model::sheet::{Fit, PageSize, SheetView, Stack, Zoom};
use slotmap::{SlotMap, new_key_type};

use crate::text::buffer::Disk;
use model::sheet::Measure;

new_key_type! {
    pub struct ImageId;
}

/// An open image file; the host decodes its pixels when it draws them.
pub struct ImageFile {
    pub path: Arc<Path>,
    pub encoding: Encoding,
    /// One page the image's pixel size.
    pub stack: Stack,
    /// The file's time when read, which names the pixels to decode.
    pub modified: SystemTime,
    pub disk: Disk,
}

pub type ImageFiles = SlotMap<ImageId, ImageFile>;

pub struct ImageTab {
    file: ImageId, // private: a tab's file is its identity
    pub view: SheetView,
    /// The pane as last drawn, for keys that move by part of it.
    pub last: Option<Measure>,
}

impl ImageFile {
    pub fn picture(&self) -> Picture {
        Picture {
            source: Source::File {
                path: self.path.clone(),
                modified: self.modified,
            },
            encoding: self.encoding,
        }
    }
}

impl ImageTab {
    pub fn new(file: ImageId, view: SheetView) -> ImageTab {
        ImageTab {
            file,
            view,
            last: None,
        }
    }

    /// The whole image fitted to the pane.
    pub fn open(file: ImageId) -> ImageTab {
        let view = SheetView {
            fit: Fit::Page,
            zoom: Zoom::ONE,
            ..SheetView::on(model::sheet::PageIndex(0))
        };
        ImageTab::new(file, view)
    }

    pub fn file(&self) -> ImageId {
        self.file
    }

    pub fn copy(&self) -> ImageTab {
        ImageTab::new(self.file, self.view)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ProbeError {
    #[error("{0}")]
    Read(#[from] std::io::Error),
    #[error("{0}")]
    Image(#[from] image::ImageError),
    #[error("{0}")]
    Svg(#[from] usvg::Error),
    #[error("the image has no area")]
    Empty,
}

/// The size an image shows at, in pixels, read from its header without decoding it.
pub fn probe(bytes: &[u8], encoding: Encoding) -> Result<PageSize, ProbeError> {
    let (width, height) = match encoding {
        Encoding::Svg => {
            let tree = usvg::Tree::from_data(bytes, &usvg::Options::default())?;
            (tree.size().width(), tree.size().height())
        }
        Encoding::Bitmap(bitmap) => {
            let format = match bitmap {
                Bitmap::Png => image::ImageFormat::Png,
                Bitmap::Jpeg => image::ImageFormat::Jpeg,
                Bitmap::Webp => image::ImageFormat::WebP,
                Bitmap::Gif => image::ImageFormat::Gif,
                Bitmap::Bmp => image::ImageFormat::Bmp,
            };
            let mut decoder =
                image::ImageReader::with_format(Cursor::new(bytes), format).into_decoder()?;
            let (width, height) = image::ImageDecoder::dimensions(&decoder);
            // A camera's quarter turn swaps the sides the picture shows with.
            match image::ImageDecoder::orientation(&mut decoder)? {
                image::metadata::Orientation::Rotate90
                | image::metadata::Orientation::Rotate270
                | image::metadata::Orientation::Rotate90FlipH
                | image::metadata::Orientation::Rotate270FlipH => (height as f32, width as f32),
                image::metadata::Orientation::NoTransforms
                | image::metadata::Orientation::Rotate180
                | image::metadata::Orientation::FlipHorizontal
                | image::metadata::Orientation::FlipVertical => (width as f32, height as f32),
            }
        }
    };
    PageSize::new(width, height).map_err(|_| ProbeError::Empty)
}

/// An image file's size as read, and the time that names those pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Probed {
    pub size: PageSize,
    pub modified: SystemTime,
}

/// Reads `path`'s header as `encoding`.
pub fn read(path: &Path, encoding: Encoding) -> Result<Probed, ProbeError> {
    let modified = std::fs::metadata(path)?.modified()?;
    let size = probe(&std::fs::read(path)?, encoding)?;
    Ok(Probed { size, modified })
}
