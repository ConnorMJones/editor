//! Drawings written out as PDF, SVG or PNG.

pub mod pdf;
pub mod scene;
pub mod svg;

use ink::doc::{Board, Embedded, PageIndex, Paged};
use ink::text::Fonts;
use model::command::ExportAs;

use crate::scene::{Region, Scene};

#[derive(Debug, thiserror::Error)]
pub enum ExportError {
    #[error("{0}")]
    Pdf(krilla::error::KrillaError),
    #[error("a font could not be embedded")]
    Font,
    #[error("a {encoding:?} image: {reason}")]
    Picture { encoding: Embedded, reason: String },
    #[error("{0}")]
    Svg(#[from] resvg::usvg::Error),
    #[error("{0}")]
    Png(Box<dyn std::error::Error + Send + Sync>),
    #[error("there is nothing to export")]
    Empty,
}

/// What to write, from snapshots of the drawing taken when the export was asked for.
#[derive(Debug)]
pub enum Export {
    /// Every page, as one PDF.
    Pages(Paged),
    Page {
        paged: Paged,
        page: PageIndex,
        to: ExportAs,
    },
    Region {
        board: Board,
        region: Region,
        to: ExportAs,
    },
}

/// Pixels to the point in a PNG.
pub const PNG_SCALE: f32 = 2.0;

pub fn render(export: &Export, fonts: &Fonts) -> Result<Vec<u8>, ExportError> {
    let one = |scene: Scene, to: ExportAs| match to {
        ExportAs::Pdf => pdf::pdf([scene], fonts),
        ExportAs::Svg => Ok(svg::svg(&scene, fonts).into_bytes()),
        ExportAs::Png => svg::png(&scene, fonts, PNG_SCALE),
    };
    match export {
        Export::Pages(paged) => pdf::pdf(
            (0..paged.stack().sizes().len()).map(|page| scene::page(paged, PageIndex(page))),
            fonts,
        ),
        Export::Page { paged, page, to } => one(scene::page(paged, *page), *to),
        Export::Region { board, region, to } => {
            one(scene::region(board, region).ok_or(ExportError::Empty)?, *to)
        }
    }
}
