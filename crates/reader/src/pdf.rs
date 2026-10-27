//! PDFs parsed and rendered by hayro; pages are measured in points with their rotation applied.

use std::sync::Arc;

use hayro::hayro_interpret::InterpreterSettings;
use hayro::hayro_syntax::Pdf;
use hayro::vello_cpu::color::palette::css::WHITE;
use hayro::{RenderCache, RenderSettings};
use model::sheet::{PageSize, Stack};

pub struct Document {
    pdf: Pdf, // private: parsed from `bytes`
    pub bytes: Arc<[u8]>,
    pub pages: Stack,
}

impl std::fmt::Debug for Document {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Document({} pages)", self.pages.sizes().len())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum PdfError {
    #[error("not a readable PDF: {0:?}")]
    Load(hayro::hayro_syntax::LoadPdfError),
    #[error("the PDF has no pages")]
    NoPages,
    #[error("page {page} has no area")]
    Empty { page: usize },
    #[error("there is no page {0}")]
    NoPage(usize),
}

/// The file's bytes, shared with hayro without a copy.
struct Shared(Arc<[u8]>);

impl AsRef<[u8]> for Shared {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

/// Premultiplied RGBA, opaque: pages are drawn on white.
pub struct Rgba {
    pub width: u16,
    pub height: u16,
    pub pixels: Vec<u8>,
}

impl Document {
    pub fn open(bytes: Arc<[u8]>) -> Result<Document, PdfError> {
        let pdf = Pdf::new(Arc::new(Shared(bytes.clone()))).map_err(PdfError::Load)?;
        let sizes = (pdf.pages().iter().enumerate())
            .map(|(page, read)| {
                let (width, height) = read.render_dimensions();
                PageSize::new(width, height).map_err(|_| PdfError::Empty { page })
            })
            .collect::<Result<Vec<_>, _>>()?;
        let pages = Stack::new(sizes).ok_or(PdfError::NoPages)?;
        Ok(Document { pdf, bytes, pages })
    }

    /// `page` filling `width` by `height` pixels.
    pub fn render(&self, page: usize, width: u16, height: u16) -> Result<Rgba, PdfError> {
        let (read, size) = (self.pdf.pages().get(page))
            .zip(self.pages.sizes().get(page))
            .ok_or(PdfError::NoPage(page))?;
        let settings = RenderSettings {
            x_scale: f32::from(width) / size.width.get(),
            y_scale: f32::from(height) / size.height.get(),
            width: Some(width),
            height: Some(height),
            bg_color: WHITE,
        };
        let pixmap = hayro::render(
            read,
            &RenderCache::new(),
            &InterpreterSettings::default(),
            &settings,
        );
        Ok(Rgba {
            width,
            height,
            pixels: pixmap.data_as_u8_slice().to_vec(),
        })
    }
}

const _: () = {
    const fn shared<T: Send + Sync>() {}
    shared::<Document>()
};
