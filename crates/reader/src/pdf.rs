use std::collections::HashMap;
use std::ops::Range;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock};

use hayro::RenderCache;
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
use kurbo::{Affine, BezPath, Rect};
use model::geom::{Point, Surface};
use model::sheet::{PageSize, Stack};

pub const TILE: u16 = 512;
pub const LEVELS_PER_DOUBLING: f32 = 4.0;
pub const FINEST: Level = Level(40);

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
    #[error("{0:?} is outside its page")]
    NoBlock(TileBlock),
}

#[derive(Debug)]
pub struct PageText {
    pub text: String,
    pub chars: Vec<PdfChar>,
}

impl PageText {
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

    pub fn byte(&self, char: usize) -> usize {
        self.chars.get(char).map_or(self.text.len(), |char| char.at)
    }

    pub fn chars_in(&self, bytes: Range<usize>) -> Range<usize> {
        let first = |at: usize| self.chars.partition_point(|char| char.at < at);
        first(bytes.start)..first(bytes.end)
    }

    pub fn slice(&self, chars: Range<usize>) -> &str {
        let start = self.byte(chars.start);
        self.text[start..self.byte(chars.end).max(start)].trim_end()
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PdfChar {
    pub at: usize,
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

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Level(pub i8);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Tile {
    pub page: usize,
    pub level: Level,
    pub column: u16,
    pub row: u16,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct TileBlock {
    pub page: usize,
    pub level: Level,
    pub columns: Range<u16>,
    pub rows: Range<u16>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Grid {
    pub width: u32,
    pub height: u32,
    pub columns: u16,
    pub rows: u16,
}

pub struct Bgra {
    pub width: u16,
    pub height: u16,
    pub pixels: Vec<u8>,
}

impl Level {
    pub fn covering(pixels_per_point: f32) -> Level {
        let level = (pixels_per_point.log2() * LEVELS_PER_DOUBLING).ceil();
        Level(level.clamp(f32::from(i8::MIN), f32::from(FINEST.0)) as i8)
    }

    pub fn whole_page(size: PageSize) -> Level {
        let longest = size.width.get().max(size.height.get());
        let level = ((f32::from(TILE) / longest).log2() * LEVELS_PER_DOUBLING).floor();
        Level(level.clamp(f32::from(i8::MIN), f32::from(FINEST.0)) as i8)
    }

    pub fn pixels_per_point(self) -> f32 {
        (f32::from(self.0) / LEVELS_PER_DOUBLING).exp2()
    }
}

impl Grid {
    pub fn of(size: PageSize, level: Level) -> Grid {
        let side = |points: f32| ((points * level.pixels_per_point()).ceil() as u32).max(1);
        let (width, height) = (side(size.width.get()), side(size.height.get()));
        let cells = |pixels: u32| pixels.div_ceil(u32::from(TILE)) as u16;
        Grid {
            width,
            height,
            columns: cells(width),
            rows: cells(height),
        }
    }
}

impl Document {
    pub fn open(bytes: Arc<[u8]>) -> Result<Document, PdfError> {
        let _span = tracing::info_span!("pdf open", bytes = bytes.len()).entered();
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
        let _span = tracing::info_span!("pdf text", page).entered();
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
    pub fn render_tiles(&self, block: TileBlock) -> Result<Vec<(Tile, Bgra)>, PdfError> {
        let _span = tracing::info_span!(
            "pdf tiles render",
            page = block.page,
            level = block.level.0,
            columns = ?block.columns,
            rows = ?block.rows
        )
        .entered();
        let (read, size) = (self.pdf.pages().get(block.page))
            .zip(self.pages.sizes().get(block.page))
            .ok_or(PdfError::NoPage(block.page))?;
        let grid = Grid::of(*size, block.level);
        let edge = u32::from(TILE);
        let (left, top) = (
            u32::from(block.columns.start) * edge,
            u32::from(block.rows.start) * edge,
        );
        let (right, bottom) = (
            (u32::from(block.columns.end) * edge).min(grid.width),
            (u32::from(block.rows.end) * edge).min(grid.height),
        );
        let side = |from: u32, to: u32| match to.checked_sub(from).map(u16::try_from) {
            Some(Ok(pixels @ 1..)) => Ok(pixels),
            Some(Ok(0) | Err(_)) | None => Err(PdfError::NoBlock(block.clone())),
        };
        let (width, height) = (side(left, right)?, side(top, bottom)?);
        let scale = f64::from(block.level.pixels_per_point());
        let transform =
            Affine::translate((-f64::from(left), -f64::from(top))) * Affine::scale(scale);
        let mut pixmap = hayro::render_region(
            read,
            &RenderCache::new(),
            &InterpreterSettings::default(),
            transform,
            width,
            height,
            WHITE,
        );
        for pixel in pixmap.data_as_u8_slice_mut().chunks_exact_mut(4) {
            pixel.swap(0, 2);
        }
        let region = pixmap.data_as_u8_slice();
        let stride = usize::from(width) * 4;
        let tiles = (block.rows.clone())
            .flat_map(|row| block.columns.clone().map(move |column| (column, row)))
            .map(|(column, row)| {
                let (x, y) = (u32::from(column) * edge - left, u32::from(row) * edge - top);
                let tile_width = (u32::from(width) - x).min(edge) as usize;
                let tile_height = (u32::from(height) - y).min(edge) as usize;
                let mut pixels = Vec::with_capacity(tile_width * tile_height * 4);
                for line in 0..tile_height {
                    let start = (y as usize + line) * stride + x as usize * 4;
                    pixels.extend_from_slice(&region[start..start + tile_width * 4]);
                }
                let tile = Tile {
                    page: block.page,
                    level: block.level,
                    column,
                    row,
                };
                let bgra = Bgra {
                    width: tile_width as u16,
                    height: tile_height as u16,
                    pixels,
                };
                (tile, bgra)
            })
            .collect();
        Ok(tiles)
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
