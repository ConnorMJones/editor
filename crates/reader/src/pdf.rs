//! PDFs parsed and rendered by hayro; pages are measured in points with their rotation applied.

use std::collections::HashMap;
use std::ops::Range;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock};

use hayro::hayro_interpret::font::Glyph;
use hayro::hayro_interpret::hayro_cmap::BfString;
use hayro::hayro_interpret::util::TransformExt;
use hayro::hayro_interpret::{
    BlendMode, ClipPath, Context, Device, GlyphDrawMode, Image, InterpreterCache,
    InterpreterSettings, Paint, PathDrawMode, SoftMask, interpret_page,
};
use hayro::hayro_syntax::Pdf;
use hayro::hayro_syntax::object::dict::keys::OUTLINES;
use hayro::hayro_syntax::object::{Array, Dict, Object, ObjectIdentifier, String as PdfString};
use hayro::vello_cpu::color::palette::css::WHITE;
use hayro::{RenderCache, RenderSettings};
use kurbo::{Affine, BezPath, Rect};
use model::geom::{Point, Surface};
use model::sheet::{PageSize, Stack};

pub struct Document {
    pdf: Pdf, // private: parsed from `bytes`
    pub bytes: Arc<[u8]>,
    pub pages: Stack,
    texts: Box<[Slot]>, // private: one per page, filled once by the claim that took it
}

struct Slot {
    text: OnceLock<Arc<PageText>>,
    claimed: AtomicBool,
}

#[derive(Debug)]
pub struct TextClaim {
    pub document: Arc<Document>,
    pub page: usize,
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

/// A page's text in the order it is drawn; spaces and line breaks the PDF only implies are in
/// `text` without a `PdfChar`.
#[derive(Debug)]
pub struct PageText {
    pub text: String,
    pub chars: Vec<PdfChar>,
}

impl PageText {
    /// Distance off a line counts four times distance along it.
    pub fn nearest(&self, at: Point<Surface>) -> Option<usize> {
        let off = |char: &PdfChar| {
            let xs = char.quad.map(|corner| corner.x);
            let ys = char.quad.map(|corner| corner.y);
            let outside = |at: f32, values: [f32; 4]| {
                let (low, high) = (
                    values.into_iter().fold(f32::INFINITY, f32::min),
                    values.into_iter().fold(f32::NEG_INFINITY, f32::max),
                );
                (low - at).max(at - high).max(0.0)
            };
            outside(at.x, xs) + 4.0 * outside(at.y, ys)
        };
        (self.chars.iter().enumerate())
            .min_by(|(_, a), (_, b)| off(a).total_cmp(&off(b)))
            .map(|(index, _)| index)
    }

    pub fn slice(&self, chars: Range<usize>) -> &str {
        let start = self
            .chars
            .get(chars.start)
            .map_or(self.text.len(), |char| char.at);
        let end = self
            .chars
            .get(chars.end)
            .map_or(self.text.len(), |char| char.at);
        self.text[start..end.max(start)].trim_end()
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PdfChar {
    /// Where the character starts in `PageText::text`.
    pub at: usize,
    /// Bottom left, bottom right, top right, top left.
    pub quad: [Point<Surface>; 4],
}

struct Drawn {
    text: String,
    quad: [Point<Surface>; 4],
    origin: kurbo::Point,
    em: f64,
}

struct Glyphs(Vec<Drawn>);

impl<'a> Device<'a> for Glyphs {
    fn set_soft_mask(&mut self, _: Option<SoftMask<'a>>) {}
    fn set_blend_mode(&mut self, _: BlendMode) {}
    fn draw_path(&mut self, _: &BezPath, _: Affine, _: &Paint<'a>, _: &PathDrawMode) {}
    fn push_clip_path(&mut self, _: &ClipPath) {}
    fn push_transparency_group(&mut self, _: f32, _: Option<SoftMask<'a>>, _: BlendMode) {}
    fn draw_image(&mut self, _: Image<'a, '_>, _: Affine) {}
    fn pop_clip_path(&mut self) {}
    fn pop_transparency_group(&mut self) {}

    fn draw_glyph(
        &mut self,
        glyph: &Glyph<'a>,
        transform: Affine,
        glyph_transform: Affine,
        _: &Paint<'a>,
        _: &GlyphDrawMode,
    ) {
        let text = match glyph.as_unicode() {
            Some(BfString::Char(char)) => char.to_string(),
            Some(BfString::String(text)) => text,
            None => return,
        };
        // Glyph space is 1000 units to the em; a glyph without a known advance takes half an em.
        let advance = match glyph {
            Glyph::Outline(outline) => outline.advance_width().map_or(500.0, f64::from),
            Glyph::Type3(_) => 500.0,
        };
        let onto = transform * glyph_transform;
        let corner = |x: f64, y: f64| {
            let at = onto * kurbo::Point::new(x, y);
            Point::new(at.x as f32, at.y as f32)
        };
        let origin = onto * kurbo::Point::ORIGIN;
        self.0.push(Drawn {
            text,
            quad: [
                corner(0.0, -200.0),
                corner(advance, -200.0),
                corner(advance, 800.0),
                corner(0.0, 800.0),
            ],
            origin,
            em: (onto * kurbo::Point::new(1000.0, 0.0)).distance(origin),
        });
    }
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
        let texts = (0..pages.sizes().len())
            .map(|_| Slot {
                text: OnceLock::new(),
                claimed: AtomicBool::new(false),
            })
            .collect();
        Ok(Document {
            pdf,
            bytes,
            pages,
            texts,
        })
    }

    pub fn text(&self, page: usize) -> Option<&Arc<PageText>> {
        self.texts.get(page)?.text.get()
    }

    /// `None` once another claim has the page, or past the last page.
    pub fn claim_text(self: &Arc<Self>, page: usize) -> Option<TextClaim> {
        let slot = self.texts.get(page)?;
        (!slot.claimed.swap(true, Ordering::Relaxed)).then(|| TextClaim {
            document: self.clone(),
            page,
        })
    }

    /// Read now when no job has it; a page another job is still reading is read again.
    pub fn read_text(self: &Arc<Self>, page: usize) -> Result<Arc<PageText>, PdfError> {
        match (self.text(page), self.claim_text(page)) {
            (Some(text), _) => Ok(text.clone()),
            (None, Some(claim)) => claim.read(),
            (None, None) => self.extract(page).map(Arc::new),
        }
    }

    fn extract(&self, page: usize) -> Result<PageText, PdfError> {
        let (read, size) = (self.pdf.pages().get(page))
            .zip(self.pages.sizes().get(page))
            .ok_or(PdfError::NoPage(page))?;
        let cache = InterpreterCache::new();
        let area = Rect::new(
            0.0,
            0.0,
            f64::from(size.width.get()),
            f64::from(size.height.get()),
        );
        let mut context = Context::new(
            read.initial_transform(true).to_kurbo(),
            area,
            &cache,
            read.xref(),
            InterpreterSettings::default(),
        );
        let mut glyphs = Glyphs(Vec::new());
        interpret_page(read, &mut context, &mut glyphs);
        let mut text = PageText {
            text: String::new(),
            chars: Vec::with_capacity(glyphs.0.len()),
        };
        let mut last: Option<&Drawn> = None;
        for drawn in &glyphs.0 {
            let between = match last {
                None => None,
                Some(last) if (drawn.origin.y - last.origin.y).abs() > last.em * 0.5 => Some('\n'),
                Some(last)
                    if drawn.origin.x - f64::from(last.quad[1].x) > last.em * 0.15
                        || drawn.origin.x < f64::from(last.quad[0].x) =>
                {
                    Some(' ')
                }
                Some(_) => None,
            };
            text.text.extend(between);
            text.chars.push(PdfChar {
                at: text.text.len(),
                quad: drawn.quad,
            });
            text.text.push_str(&drawn.text);
            last = Some(drawn);
        }
        Ok(text)
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

impl TextClaim {
    pub fn read(self) -> Result<Arc<PageText>, PdfError> {
        let text = Arc::new(self.document.extract(self.page)?);
        Ok(self.document.texts[self.page]
            .text
            .get_or_init(|| text)
            .clone())
    }
}

/// `page` is `None` when the destination can't be found.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Heading {
    pub title: String,
    pub depth: usize,
    pub page: Option<usize>,
}

/// Most entries read from an outline, so a cyclic one ends.
pub const OUTLINE_ENTRIES: usize = 100_000;

impl Document {
    pub fn outline(&self) -> Vec<Heading> {
        let xref = self.pdf.xref();
        let Some(catalog) = xref.get::<Dict<'_>>(xref.root_id()) else {
            return Vec::new();
        };
        let pages: HashMap<ObjectIdentifier, usize> = (self.pdf.pages().iter().enumerate())
            .filter_map(|(index, page)| Some((page.raw().obj_id()?, index)))
            .collect();
        let tree =
            (catalog.get::<Dict<'_>>(b"Names")).and_then(|names| names.get::<Dict<'_>>(b"Dests"));
        let named = |key: &[u8]| -> Option<Object<'_>> {
            match catalog
                .get::<Dict<'_>>(b"Dests")
                .and_then(|dests| dests.get::<Object<'_>>(key))
            {
                Some(found) => Some(found),
                None => {
                    let mut nodes: Vec<(Dict<'_>, usize)> =
                        tree.clone().into_iter().map(|node| (node, 0)).collect();
                    while let Some((node, depth)) = nodes.pop() {
                        let mut names = node
                            .get::<Array<'_>>(b"Names")
                            .into_iter()
                            .flat_map(|names| names.iter::<Object<'_>>());
                        while let (Some(name), Some(value)) = (names.next(), names.next()) {
                            match name {
                                Object::String(name) if name.as_ref() == key => return Some(value),
                                _ => {}
                            }
                        }
                        nodes.extend(
                            node.get::<Array<'_>>(b"Kids")
                                .into_iter()
                                .flat_map(|kids| kids.iter::<Dict<'_>>())
                                .map(|kid| (kid, depth + 1))
                                .filter(|_| depth < 32),
                        );
                    }
                    None
                }
            }
        };
        let page_of = |dest: Object<'_>| -> Option<usize> {
            let dest = match dest {
                Object::Name(name) => named(name.as_ref())?,
                Object::String(name) => named(name.as_ref())?,
                other => other,
            };
            let array = match dest {
                Object::Array(array) => array,
                Object::Dict(dict) => dict.get::<Array<'_>>(b"D")?,
                _ => return None,
            };
            pages
                .get(&array.raw_iter().next()?.as_obj_ref()?.into())
                .copied()
        };
        let mut headings = Vec::new();
        let first = (catalog.get::<Dict<'_>>(OUTLINES))
            .and_then(|outlines| outlines.get::<Dict<'_>>(b"First"));
        let mut pending: Vec<(Dict<'_>, usize)> = first.into_iter().map(|item| (item, 0)).collect();
        while let Some((item, depth)) = pending.pop()
            && headings.len() < OUTLINE_ENTRIES
        {
            let title = item
                .get::<PdfString<'_>>(b"Title")
                .map(|title| match title.as_ref() {
                    [0xfe, 0xff, utf16 @ ..] => String::from_utf16_lossy(
                        &(utf16.chunks_exact(2))
                            .map(|pair| u16::from_be_bytes([pair[0], pair[1]]))
                            .collect::<Vec<_>>(),
                    ),
                    latin => latin.iter().map(|&byte| char::from(byte)).collect(),
                });
            let dest = item
                .get::<Object<'_>>(b"Dest")
                .or_else(|| item.get::<Dict<'_>>(b"A")?.get::<Object<'_>>(b"D"));
            headings.push(Heading {
                title: title.unwrap_or_default(),
                depth,
                page: dest.and_then(page_of),
            });
            pending.extend(item.get::<Dict<'_>>(b"Next").map(|next| (next, depth)));
            pending.extend(
                item.get::<Dict<'_>>(b"First")
                    .map(|child| (child, depth + 1)),
            );
        }
        headings
    }
}
