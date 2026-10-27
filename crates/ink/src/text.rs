//! Text in drawings: shaped with cosmic-text, each glyph's outline filled into a mesh once and
//! placed for every use.

use std::collections::HashMap;
use std::ops::Range;
use std::sync::{Arc, Mutex, PoisonError};

use cosmic_text::{
    Attrs, Buffer, CacheKey, CacheKeyFlags, Command, Family, FontSystem, Metrics, Shaping,
    SwashCache, fontdb,
};
use lyon_tessellation::math::point;
use lyon_tessellation::path::Path;
use lyon_tessellation::{BuffersBuilder, FillOptions, FillTessellator, FillVertex, VertexBuffers};

use crate::brush::Size;
use crate::doc::{Built, GeometryId};
use crate::geom::{Local, Point, Rect};
use crate::mesh::Mesh;

pub use cosmic_text::fontdb::ID as FontId;

/// The size glyph outlines are read at before scaling to ems.
pub const OUTLINE_SIZE: f32 = 64.0;
/// How far a filled outline may stray from the curve, in ems.
pub const TOLERANCE: f32 = 0.001;
/// Line height as a part of the font size.
pub const LINE_HEIGHT: f32 = 1.25;

/// The fonts text is shaped with, shared by the UI thread and the jobs that read drawings.
#[derive(Clone)]
pub struct Fonts(pub Arc<Mutex<Shaper>>);

pub struct Shaper {
    system: FontSystem, // private: `glyphs` holds meshes of this system's fonts
    glyphs: HashMap<GlyphKey, Arc<Glyph>>, // private: `glyphs` holds meshes of this system's fonts
    pub swash: SwashCache,
    pub buffer: Buffer,
    pub tessellator: FillTessellator,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct GlyphKey {
    pub font: FontId,
    pub glyph: u16,
    pub weight: u16,
}

/// A glyph's outline and its fill, in ems with y down from the baseline.
pub struct Glyph {
    pub outline: Path,
    pub vertices: Vec<[f32; 2]>,
    pub triangles: Vec<[u32; 3]>,
}

/// A glyph where layout put it; `origin` is on its baseline, and `text` its bytes in the text.
#[derive(Clone, Debug)]
pub struct Placed {
    pub key: GlyphKey,
    pub origin: Point<Local>,
    pub size: f32,
    pub advance: f32,
    pub text: Range<usize>,
}

/// A font's file, as embedded in exports.
pub struct Face {
    pub data: Vec<u8>,
    pub index: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FontFamily(Box<str>); // private: never empty

#[derive(Debug, thiserror::Error, PartialEq)]
#[error("a font family needs a name")]
pub struct EmptyFamily;

impl FontFamily {
    pub fn new(name: &str) -> Result<FontFamily, EmptyFamily> {
        match name.is_empty() {
            true => Err(EmptyFamily),
            false => Ok(FontFamily(name.into())),
        }
    }

    pub fn name(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct TextStyle {
    pub family: FontFamily,
    /// The font size, in points.
    pub size: Size,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Wrap {
    /// Lines break only at newlines.
    Lines,
    /// Lines also break before they grow wider than this.
    At(Size),
}

/// Text with a style and wrap, and the mesh `Fonts::lay_out` builds from them; never blank.
#[derive(Debug)]
pub struct TextInk {
    pub text: Box<str>,
    pub style: TextStyle,
    pub wrap: Wrap,
    pub built: Built,
    pub area: Rect<Local>,
    pub glyphs: Box<[Placed]>,
}

/// Where a caret between two characters stands, in the text's own space.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Caret {
    pub at: Point<Local>,
    pub height: f32,
}

/// Text laid out: a caret before each character and one after the last, and its ink.
#[derive(Debug)]
pub struct Laid {
    pub carets: Box<[Caret]>,
    pub inked: Inked,
    pub area: Rect<Local>,
}

#[derive(Debug)]
pub enum Inked {
    /// Nothing in the text leaves a mark.
    Blank,
    Ink(Arc<TextInk>),
}

impl Fonts {
    /// The system's fonts and `bundled`.
    pub fn new(bundled: &[&'static [u8]]) -> Fonts {
        let mut db = fontdb::Database::new();
        db.load_system_fonts();
        for font in bundled {
            db.load_font_data(font.to_vec());
        }
        let mut system = FontSystem::new_with_locale_and_db("en-US".to_owned(), db);
        let buffer = Buffer::new(&mut system, Metrics::new(12.0, 12.0 * LINE_HEIGHT));
        Fonts(Arc::new(Mutex::new(Shaper {
            system,
            swash: SwashCache::new(),
            buffer,
            glyphs: HashMap::new(),
            tessellator: FillTessellator::new(),
        })))
    }

    pub fn glyph(&self, key: GlyphKey) -> Arc<Glyph> {
        let mut shaper = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        let Shaper {
            system,
            swash,
            glyphs,
            tessellator,
            ..
        } = &mut *shaper;
        cached(system, swash, tessellator, glyphs, key)
    }

    pub fn face(&self, font: FontId) -> Option<Face> {
        let shaper = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        shaper.system.db().with_face_data(font, |data, index| Face {
            data: data.to_vec(),
            index,
        })
    }

    pub fn rewrap(&self, text: &TextInk, wrap: Wrap) -> Arc<TextInk> {
        match self.lay_out(&text.text, &text.style, wrap).inked {
            Inked::Ink(ink) => ink,
            // Wrapping moves glyphs and never drops them, and this text inked before.
            Inked::Blank => unreachable!("text that inked once inks again"),
        }
    }

    pub fn lay_out(&self, text: &str, style: &TextStyle, wrap: Wrap) -> Laid {
        let mut shaper = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        let Shaper {
            system,
            swash,
            buffer,
            glyphs,
            tessellator,
        } = &mut *shaper;
        let size = style.size.get();
        let line_height = size * LINE_HEIGHT;
        buffer.set_metrics(Metrics::new(size, line_height));
        let (width, wrapping) = match wrap {
            Wrap::Lines => (None, cosmic_text::Wrap::None),
            Wrap::At(width) => (Some(width.get()), cosmic_text::Wrap::WordOrGlyph),
        };
        buffer.set_wrap(wrapping);
        buffer.set_size(width, None);
        let attrs = Attrs::new().family(Family::Name(style.family.name()));
        buffer.set_text(text, &attrs, Shaping::Advanced, None);
        buffer.shape_until_scroll(system, false);

        // Byte offsets where each of cosmic-text's lines starts.
        let line_starts: Vec<usize> = std::iter::once(0)
            .chain(text.match_indices('\n').map(|(at, _)| at + 1))
            .collect();
        let mut by_byte: Vec<Option<Caret>> = vec![None; text.len() + 1];
        let mut mesh = Mesh::default();
        let mut placed = Vec::new();
        let (mut widest, mut bottom) = (0.0_f32, line_height);
        for run in buffer.layout_runs() {
            widest = widest.max(run.line_w);
            bottom = bottom.max(run.line_top + run.line_height);
            let start = line_starts[run.line_i];
            let caret = |x: f32| Caret {
                at: Point::new(x, run.line_top),
                height: run.line_height,
            };
            for glyph in run.glyphs {
                let cluster = &run.text[glyph.start..glyph.end];
                let count = cluster.chars().count().max(1) as f32;
                for (index, (offset, _)) in cluster.char_indices().enumerate() {
                    let x = glyph.x + glyph.w * index as f32 / count;
                    by_byte[start + glyph.start + offset].get_or_insert(caret(x));
                }
                let key = GlyphKey {
                    font: glyph.font_id,
                    glyph: glyph.glyph_id,
                    weight: glyph.font_weight.0,
                };
                let shape = cached(system, swash, tessellator, glyphs, key);
                let origin = Point::<Local>::new(
                    glyph.x + glyph.font_size * glyph.x_offset,
                    run.line_y + glyph.y - glyph.font_size * glyph.y_offset,
                );
                placed.push(Placed {
                    key,
                    origin,
                    size: glyph.font_size,
                    advance: glyph.w,
                    text: start + glyph.start..start + glyph.end,
                });
                let first = mesh.vertices.len() as u32;
                mesh.vertices.extend(shape.vertices.iter().map(|[x, y]| {
                    Point::new(
                        origin.x + x * glyph.font_size,
                        origin.y + y * glyph.font_size,
                    )
                }));
                mesh.triangles.extend(
                    (shape.triangles.iter()).map(|triangle| triangle.map(|index| first + index)),
                );
            }
            let end = start + run.text.len();
            by_byte[end].get_or_insert(caret(run.line_w));
        }
        // Bytes no glyph starts on (inside a character, or a newline) take the caret before.
        let mut last = Caret {
            at: Point::new(0.0, 0.0),
            height: line_height,
        };
        let carets = (text.char_indices().map(|(at, _)| at))
            .chain(std::iter::once(text.len()))
            .map(|at| {
                last = by_byte[at].unwrap_or(last);
                last
            })
            .collect();
        let area = Rect::spanning(
            Point::new(0.0, 0.0),
            Point::new(width.unwrap_or(widest), bottom),
        );
        let inked = match mesh.bounds() {
            None => Inked::Blank,
            Some(bounds) => Inked::Ink(Arc::new(TextInk {
                text: text.into(),
                style: style.clone(),
                wrap,
                built: Built {
                    id: GeometryId::next(),
                    mesh,
                    bounds,
                },
                area,
                glyphs: placed.into(),
            })),
        };
        Laid {
            carets,
            inked,
            area,
        }
    }
}

pub fn cached(
    system: &mut FontSystem,
    swash: &mut SwashCache,
    tessellator: &mut FillTessellator,
    glyphs: &mut HashMap<GlyphKey, Arc<Glyph>>,
    key: GlyphKey,
) -> Arc<Glyph> {
    glyphs
        .entry(key)
        .or_insert_with(|| {
            Arc::new('fill: {
    let (cache, _, _) = CacheKey::new(
        key.font,
        key.glyph,
        OUTLINE_SIZE,
        (0.0, 0.0),
        fontdb::Weight(key.weight),
        CacheKeyFlags::DISABLE_HINTING,
    );
    let empty = || Glyph {
        outline: Path::new(),
        vertices: Vec::new(),
        triangles: Vec::new(),
    };
    let Some(commands) = swash.get_outline_commands_uncached(system, cache) else {
        break 'fill empty();
    };
    // Outlines are read y up; ems here run y down.
    let em = |x: f32, y: f32| point(x / OUTLINE_SIZE, -y / OUTLINE_SIZE);
    let mut path = Path::builder();
    let mut open = false;
    for command in commands.iter() {
        match (*command, open) {
            (Command::MoveTo(to), true) => {
                path.end(true);
                path.begin(em(to.x, to.y));
            }
            (Command::MoveTo(to), false) => {
                path.begin(em(to.x, to.y));
                open = true;
            }
            (Command::LineTo(to), true) => {
                path.line_to(em(to.x, to.y));
            }
            (Command::QuadTo(control, to), true) => {
                path.quadratic_bezier_to(em(control.x, control.y), em(to.x, to.y));
            }
            (Command::CurveTo(first, second, to), true) => {
                path.cubic_bezier_to(em(first.x, first.y), em(second.x, second.y), em(to.x, to.y));
            }
            (Command::Close, true) => {
                path.end(true);
                open = false;
            }
            (
                Command::LineTo(_) | Command::QuadTo(..) | Command::CurveTo(..) | Command::Close,
                false,
            ) => {}
        }
    }
    match open {
        true => path.end(true),
        false => {}
    }
    let outline = path.build();
    let mut buffers: VertexBuffers<[f32; 2], u32> = VertexBuffers::new();
    let filled = tessellator.tessellate_path(
        &outline,
        &FillOptions::non_zero().with_tolerance(TOLERANCE),
        &mut BuffersBuilder::new(&mut buffers, |vertex: FillVertex| {
            vertex.position().to_array()
        }),
    );
    match filled {
        Ok(()) => Glyph {
            outline,
            vertices: buffers.vertices,
            triangles: (buffers.indices.chunks_exact(3))
                .map(|triangle| [triangle[0], triangle[1], triangle[2]])
                .collect(),
        },
        Err(error) => {
            tracing::warn!(glyph = key.glyph, %error, "glyph outline could not be filled");
            empty()
        }
    }
            })
        })
        .clone()
}

impl std::fmt::Debug for Fonts {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Fonts")
    }
}
