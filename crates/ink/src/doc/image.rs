//! Pictures embedded in a drawing: the encoded file, shown over a rectangle of the surface.

use std::sync::Arc;

use model::picture::{Bitmap, Encoding};

use crate::doc::{Built, GeometryId, PageSize};
use crate::geom::{Point, Rect};
use crate::mesh::Mesh;

/// An image's bytes as read from its file, and the rectangle it fills in its own space, from
/// the origin; the host decodes the bytes when it draws them.
#[derive(Debug)]
pub struct ImageInk {
    pub encoded: Arc<[u8]>,
    pub encoding: Embedded,
    pub extent: PageSize,
    /// Two triangles over the rectangle, for hit tests and selection.
    pub built: Built,
}

impl ImageInk {
    pub fn new(encoded: Arc<[u8]>, encoding: Embedded, extent: PageSize) -> ImageInk {
        let (width, height) = (extent.width.get(), extent.height.get());
        let corners = [
            Point::new(0.0, 0.0),
            Point::new(width, 0.0),
            Point::new(width, height),
            Point::new(0.0, height),
        ];
        ImageInk {
            encoded,
            encoding,
            extent,
            built: Built {
                id: GeometryId::next(),
                mesh: Mesh {
                    vertices: corners.to_vec(),
                    triangles: vec![[0, 1, 2], [0, 2, 3]],
                },
                bounds: Rect::spanning(corners[0], corners[2]),
            },
        }
    }
}

/// How a drawing stores a picture: compressed encodings only, so anything else is converted when
/// it is put in.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Embedded {
    Png,
    Jpeg,
    Webp,
    Gif,
    Svg,
}

impl Embedded {
    /// `None` for an encoding that has to be converted first.
    pub fn of(encoding: Encoding) -> Option<Embedded> {
        match encoding {
            Encoding::Bitmap(Bitmap::Png) => Some(Embedded::Png),
            Encoding::Bitmap(Bitmap::Jpeg) => Some(Embedded::Jpeg),
            Encoding::Bitmap(Bitmap::Webp) => Some(Embedded::Webp),
            Encoding::Bitmap(Bitmap::Gif) => Some(Embedded::Gif),
            Encoding::Svg => Some(Embedded::Svg),
            Encoding::Bitmap(Bitmap::Bmp) => None,
        }
    }

    pub fn encoding(self) -> Encoding {
        match self {
            Embedded::Png => Encoding::Bitmap(Bitmap::Png),
            Embedded::Jpeg => Encoding::Bitmap(Bitmap::Jpeg),
            Embedded::Webp => Encoding::Bitmap(Bitmap::Webp),
            Embedded::Gif => Encoding::Bitmap(Bitmap::Gif),
            Embedded::Svg => Encoding::Svg,
        }
    }
}
