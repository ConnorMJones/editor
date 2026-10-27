//! `.pdt` and `.bdt` files: versioned little-endian binary holding each stroke's samples and
//! brush, shapes, text and images, with meshes rebuilt on read.

use std::sync::Arc;

use model::color::Rgb;
use model::theme::{Alpha, Color};

use crate::brush::{Brush, Curve, Fraction, InvalidFraction, InvalidSize, Paint, Size, WidthFrom};
use crate::doc::{
    Background, Board, Drawing, Embedded, Geometry, ImageInk, Ink, InvalidLayerName,
    InvalidPageSize, InvalidSpacing, Layer, LayerIndex, LayerName, Layers, Mark, Marks, PageSize,
    Paged, Pattern, Placement, Sensor, Shape, ShapeInk, Spacing, StrokeError, Visibility,
};
use crate::geom::{Affine, Local, Point, Rect, Vector};
use crate::input::{
    Elapsed, InvalidElapsed, InvalidPressure, InvalidTilt, InvalidWindow, Model, Pressure, Sample,
    Tilt, WindowParams,
};
use crate::text::{EmptyFamily, FontFamily, Fonts, Inked, TextInk, TextStyle, Wrap};

const PAGED: [u8; 4] = *b"PDT\0";
const BOARD: [u8; 4] = *b"BDT\0";
/// The only version read or written until the first release.
pub const VERSION: u16 = 1;

pub const HAS_PRESSURE: u8 = 1;
pub const HAS_TILT: u8 = 2;

#[derive(Debug, thiserror::Error, PartialEq)]
pub enum FormatError {
    #[error("not a {0} file")]
    Kind(&'static str),
    #[error("file format version {0} is not one this build reads")]
    Version(u16),
    #[error("the file ends early")]
    Truncated,
    #[error("{0} bytes follow the drawing")]
    Trailing(usize),
    #[error("unknown {what} {tag}")]
    Tag { what: &'static str, tag: u8 },
    #[error("a name or text is not UTF-8")]
    Utf8,
    #[error("alpha {0} is over 100")]
    Alpha(u8),
    #[error("the drawing has no layers")]
    NoLayers,
    #[error("the document has no pages")]
    NoPages,
    #[error("a mark's transform flattens it")]
    Placement,
    #[error("a text mark has nothing to show")]
    BlankText,
    #[error(transparent)]
    Family(#[from] EmptyFamily),
    #[error("{0} is not a coordinate")]
    NotFinite(f32),
    #[error(transparent)]
    LayerName(#[from] InvalidLayerName),
    #[error(transparent)]
    PageSize(#[from] InvalidPageSize),
    #[error(transparent)]
    Spacing(#[from] InvalidSpacing),
    #[error(transparent)]
    Size(#[from] InvalidSize),
    #[error(transparent)]
    Fraction(#[from] InvalidFraction),
    #[error(transparent)]
    Window(#[from] InvalidWindow),
    #[error(transparent)]
    Elapsed(#[from] InvalidElapsed),
    #[error(transparent)]
    Pressure(#[from] InvalidPressure),
    #[error(transparent)]
    Tilt(#[from] InvalidTilt),
    #[error(transparent)]
    Stroke(#[from] StrokeError),
}

pub fn write_paged(paged: &Paged, out: &mut Vec<u8>) {
    header(out, PAGED);
    layers(out, paged.layers());
    out.extend((paged.pages().len() as u32).to_le_bytes());
    for (page, size) in paged.pages().iter().zip(paged.stack().sizes()) {
        f32s(out, &[size.width.get(), size.height.get()]);
        background(out, &page.background);
        for marks in page.marks().iter() {
            write_marks(out, marks);
        }
    }
}

pub fn write_board(board: &Board, out: &mut Vec<u8>) {
    header(out, BOARD);
    layers(out, board.layers());
    background(out, &board.background);
    for marks in board.layer_marks().iter() {
        write_marks(out, marks);
    }
}

pub fn read_paged(bytes: &[u8], fonts: &Fonts) -> Result<Paged, FormatError> {
    let mut read = Reader::open(bytes, PAGED, "paged document", fonts)?;
    let layers = read.layers()?;
    let count = read.u32()? as usize;
    let paged = Paged::build(layers, count, &mut read, Reader::page, Reader::layer_marks)?
        .ok_or(FormatError::NoPages)?;
    read.end()?;
    Ok(paged)
}

pub fn read_board(bytes: &[u8], fonts: &Fonts) -> Result<Board, FormatError> {
    let mut read = Reader::open(bytes, BOARD, "board", fonts)?;
    let layers = read.layers()?;
    let background = read.background()?;
    let board = Board::build(background, layers, &mut read, Reader::layer_marks)?;
    read.end()?;
    Ok(board)
}

pub fn header(out: &mut Vec<u8>, magic: [u8; 4]) {
    out.extend(magic);
    out.extend(VERSION.to_le_bytes());
}

pub fn layers(out: &mut Vec<u8>, layers: &Layers) {
    out.extend((layers.len() as u32).to_le_bytes());
    for (_, layer) in layers.iter() {
        string(out, layer.name.as_str());
        out.push(match layer.visibility {
            Visibility::Shown => 0,
            Visibility::Hidden => 1,
        });
    }
}

pub fn background(out: &mut Vec<u8>, background: &Background) {
    match background {
        Background::Plain(paper) => {
            out.push(0);
            rgb(out, *paper);
        }
        Background::Pattern {
            paper,
            ink,
            pattern,
            spacing,
        } => {
            out.push(1);
            rgb(out, *paper);
            rgb(out, *ink);
            out.push(match pattern {
                Pattern::Lines => 0,
                Pattern::Grid => 1,
                Pattern::Dots => 2,
            });
            f32s(out, &[spacing.get()]);
        }
    }
}

pub fn write_marks(out: &mut Vec<u8>, marks: &Marks) {
    out.extend((marks.len() as u32).to_le_bytes());
    for (_, mark) in marks.iter() {
        out.push(match &mark.ink {
            Ink::Stroke(_) => 0,
            Ink::Shape(_) => 1,
            Ink::Text(_) => 2,
            Ink::Image(_) => 3,
        });
        rgb(out, mark.paint.color.rgb);
        out.push(mark.paint.color.alpha.get());
        f32s(out, &mark.placement.forward().coefficients);
        match &mark.ink {
            Ink::Stroke(geometry) => write_stroke(out, geometry),
            Ink::Shape(shape) => write_shape(out, shape),
            Ink::Text(text) => write_text(out, text),
            Ink::Image(image) => write_image(out, image),
        }
    }
}

pub fn write_stroke(out: &mut Vec<u8>, geometry: &Geometry) {
    let brush = &geometry.brush;
    f32s(out, &[brush.size.get()]);
    match brush.width {
        WidthFrom::Constant => out.push(0),
        WidthFrom::Pressure { curve, thinnest } => {
            out.push(1);
            out.push(match curve {
                Curve::Linear => 0,
                Curve::Sqrt => 1,
                Curve::Square => 2,
            });
            f32s(out, &[thinnest.get()]);
        }
    }
    match brush.model {
        Model::Passthrough => out.push(0),
        Model::SlidingWindow(params) => {
            out.push(1);
            f32s(out, &[params.window.get(), params.upsample.get()]);
        }
    }
    let samples = &geometry.samples;
    out.extend((samples.len() as u32).to_le_bytes());
    let sensors = geometry.sensors;
    let flags = match (sensors.pressure, sensors.tilt) {
        (Sensor::Absent, Sensor::Absent) => 0,
        (Sensor::Read, Sensor::Absent) => HAS_PRESSURE,
        (Sensor::Absent, Sensor::Read) => HAS_TILT,
        (Sensor::Read, Sensor::Read) => HAS_PRESSURE | HAS_TILT,
    };
    out.push(flags);
    for sample in samples.iter() {
        f32s(out, &[sample.at.x, sample.at.y, sample.time.get()]);
        match sample.pressure {
            Some(pressure) => f32s(out, &[pressure.get()]),
            None => {}
        }
        match sample.tilt {
            Some(tilt) => f32s(out, &[tilt.x.get(), tilt.y.get()]),
            None => {}
        }
    }
}

pub fn rgb(out: &mut Vec<u8>, rgb: Rgb) {
    out.extend([rgb.r, rgb.g, rgb.b]);
}

pub fn f32s(out: &mut Vec<u8>, values: &[f32]) {
    for value in values {
        out.extend(value.to_le_bytes());
    }
}

pub struct Reader<'a> {
    pub fonts: &'a Fonts,
    pub bytes: &'a [u8],
}

impl Reader<'_> {
    pub fn take<const N: usize>(&mut self) -> Result<[u8; N], FormatError> {
        let (head, rest) = self
            .bytes
            .split_first_chunk::<N>()
            .ok_or(FormatError::Truncated)?;
        self.bytes = rest;
        Ok(*head)
    }

    pub fn u8(&mut self) -> Result<u8, FormatError> {
        Ok(self.take::<1>()?[0])
    }

    pub fn u32(&mut self) -> Result<u32, FormatError> {
        Ok(u32::from_le_bytes(self.take()?))
    }

    pub fn f32(&mut self) -> Result<f32, FormatError> {
        Ok(f32::from_le_bytes(self.take()?))
    }

    /// A coordinate or coefficient, which must be a number.
    pub fn finite(&mut self) -> Result<f32, FormatError> {
        let value = self.f32()?;
        value
            .is_finite()
            .then_some(value)
            .ok_or(FormatError::NotFinite(value))
    }

    pub fn rgb(&mut self) -> Result<Rgb, FormatError> {
        let [r, g, b] = self.take()?;
        Ok(Rgb { r, g, b })
    }

    pub fn end(&self) -> Result<(), FormatError> {
        match self.bytes.len() {
            0 => Ok(()),
            n => Err(FormatError::Trailing(n)),
        }
    }

    /// Reads the magic and version that start every file.
    pub fn open<'a>(
        bytes: &'a [u8],
        magic: [u8; 4],
        kind: &'static str,
        fonts: &'a Fonts,
    ) -> Result<Reader<'a>, FormatError> {
        let rest = match bytes.split_first_chunk::<4>() {
            Some((found, rest)) if *found == magic => rest,
            Some(_) | None => return Err(FormatError::Kind(kind)),
        };
        let (version, bytes) = rest
            .split_first_chunk::<2>()
            .ok_or(FormatError::Truncated)?;
        match u16::from_le_bytes(*version) {
            VERSION => Ok(Reader { fonts, bytes }),
            other => Err(FormatError::Version(other)),
        }
    }

    pub fn layers(&mut self) -> Result<Layers, FormatError> {
        let count = self.u32()? as usize;
        let mut list = Vec::with_capacity(count.min(self.bytes.len()));
        for _ in 0..count {
            let name = self.string()?;
            let visibility = match self.u8()? {
                0 => Visibility::Shown,
                1 => Visibility::Hidden,
                tag => {
                    return Err(FormatError::Tag {
                        what: "visibility",
                        tag,
                    });
                }
            };
            list.push(Layer {
                name: LayerName::new(name)?,
                visibility,
            });
        }
        Layers::from_list(list).ok_or(FormatError::NoLayers)
    }

    pub fn background(&mut self) -> Result<Background, FormatError> {
        match self.u8()? {
            0 => Ok(Background::Plain(self.rgb()?)),
            1 => Ok(Background::Pattern {
                paper: self.rgb()?,
                ink: self.rgb()?,
                pattern: match self.u8()? {
                    0 => Pattern::Lines,
                    1 => Pattern::Grid,
                    2 => Pattern::Dots,
                    tag => {
                        return Err(FormatError::Tag {
                            what: "pattern",
                            tag,
                        });
                    }
                },
                spacing: Spacing::new(self.f32()?)?,
            }),
            tag => Err(FormatError::Tag {
                what: "background",
                tag,
            }),
        }
    }

    pub fn page(&mut self) -> Result<(PageSize, Background), FormatError> {
        let size = PageSize::new(self.f32()?, self.f32()?)?;
        Ok((size, self.background()?))
    }

    pub fn layer_marks(&mut self, _: LayerIndex) -> Result<Marks, FormatError> {
        let count = self.u32()?;
        let mut marks = Marks::default();
        for _ in 0..count {
            let tag = self.u8()?;
            let rgb = self.rgb()?;
            let alpha = self.u8()?;
            let alpha = Alpha::new(alpha).ok_or(FormatError::Alpha(alpha))?;
            let mut coefficients = [0.0; 6];
            for coefficient in &mut coefficients {
                *coefficient = self.finite()?;
            }
            let placement =
                Placement::new(Affine::new(coefficients)).ok_or(FormatError::Placement)?;
            let ink = match tag {
                0 => Ink::Stroke(Arc::new(self.stroke()?)),
                1 => Ink::Shape(Arc::new(self.shape()?)),
                2 => Ink::Text(self.text()?),
                3 => Ink::Image(Arc::new(self.image()?)),
                tag => return Err(FormatError::Tag { what: "mark", tag }),
            };
            marks.insert(Mark {
                placement,
                paint: Paint {
                    color: Color { rgb, alpha },
                },
                ink,
            });
        }
        Ok(marks)
    }

    pub fn stroke(&mut self) -> Result<Geometry, FormatError> {
        let size = Size::new(self.f32()?)?;
        let width = match self.u8()? {
            0 => WidthFrom::Constant,
            1 => WidthFrom::Pressure {
                curve: match self.u8()? {
                    0 => Curve::Linear,
                    1 => Curve::Sqrt,
                    2 => Curve::Square,
                    tag => return Err(FormatError::Tag { what: "curve", tag }),
                },
                thinnest: Fraction::new(self.f32()?)?,
            },
            tag => return Err(FormatError::Tag { what: "width", tag }),
        };
        let model = match self.u8()? {
            0 => Model::Passthrough,
            1 => Model::SlidingWindow(WindowParams::new(self.f32()?, self.f32()?)?),
            tag => return Err(FormatError::Tag { what: "model", tag }),
        };
        let count = self.u32()? as usize;
        let flags = self.u8()?;
        match flags & !(HAS_PRESSURE | HAS_TILT) {
            0 => {}
            _ => {
                return Err(FormatError::Tag {
                    what: "sample flags",
                    tag: flags,
                });
            }
        }
        let mut samples = Vec::with_capacity(count.min(self.bytes.len() / 12));
        for _ in 0..count {
            let at = self.point()?;
            let time = Elapsed::seconds(self.f32()?)?;
            let pressure = match flags & HAS_PRESSURE {
                0 => None,
                _ => Some(Pressure::new(self.f32()?)?),
            };
            let tilt = match flags & HAS_TILT {
                0 => None,
                _ => Some(Tilt::new(self.f32()?, self.f32()?)?),
            };
            samples.push(Sample {
                at,
                time,
                pressure,
                tilt,
            });
        }
        let brush = Brush { size, width, model };
        Ok(Geometry::new(samples.into_boxed_slice(), brush)?)
    }
}

pub fn write_shape(out: &mut Vec<u8>, ink: &ShapeInk) {
    let point = |out: &mut Vec<u8>, at: Point<Local>| f32s(out, &[at.x, at.y]);
    match ink.shape {
        Shape::Line { from, to } => {
            out.push(0);
            point(out, from);
            point(out, to);
        }
        Shape::Arrow { from, to } => {
            out.push(1);
            point(out, from);
            point(out, to);
        }
        Shape::Rectangle(rect) => {
            out.push(2);
            point(out, rect.min());
            point(out, rect.max());
        }
        Shape::Ellipse { center, radii } => {
            out.push(3);
            point(out, center);
            f32s(out, &[radii.x, radii.y]);
        }
    }
    f32s(out, &[ink.width.get()]);
}

impl Reader<'_> {
    pub fn point(&mut self) -> Result<Point<Local>, FormatError> {
        Ok(Point::new(self.finite()?, self.finite()?))
    }

    pub fn shape(&mut self) -> Result<ShapeInk, FormatError> {
        let shape = match self.u8()? {
            0 => Shape::Line {
                from: self.point()?,
                to: self.point()?,
            },
            1 => Shape::Arrow {
                from: self.point()?,
                to: self.point()?,
            },
            2 => Shape::Rectangle(Rect::spanning(self.point()?, self.point()?)),
            3 => Shape::Ellipse {
                center: self.point()?,
                radii: Vector::new(self.finite()?, self.finite()?),
            },
            tag => return Err(FormatError::Tag { what: "shape", tag }),
        };
        let width = Size::new(self.f32()?)?;
        Ok(ShapeInk::new(shape, width))
    }
}

pub fn string(out: &mut Vec<u8>, text: &str) {
    out.extend((text.len() as u32).to_le_bytes());
    out.extend(text.as_bytes());
}

pub fn write_text(out: &mut Vec<u8>, ink: &TextInk) {
    string(out, &ink.text);
    string(out, ink.style.family.name());
    f32s(out, &[ink.style.size.get()]);
    match ink.wrap {
        Wrap::Lines => out.push(0),
        Wrap::At(width) => {
            out.push(1);
            f32s(out, &[width.get()]);
        }
    }
}

impl Reader<'_> {
    pub fn string(&mut self) -> Result<String, FormatError> {
        let length = self.u32()? as usize;
        let bytes = self.bytes.get(..length).ok_or(FormatError::Truncated)?;
        self.bytes = &self.bytes[length..];
        String::from_utf8(bytes.to_vec()).map_err(|_| FormatError::Utf8)
    }

    pub fn text(&mut self) -> Result<Arc<TextInk>, FormatError> {
        let text = self.string()?;
        let style = TextStyle {
            family: FontFamily::new(&self.string()?)?,
            size: Size::new(self.f32()?)?,
        };
        let wrap = match self.u8()? {
            0 => Wrap::Lines,
            1 => Wrap::At(Size::new(self.f32()?)?),
            tag => return Err(FormatError::Tag { what: "wrap", tag }),
        };
        match self.fonts.lay_out(&text, &style, wrap).inked {
            Inked::Ink(ink) => Ok(ink),
            Inked::Blank => Err(FormatError::BlankText),
        }
    }
}

pub fn write_image(out: &mut Vec<u8>, ink: &ImageInk) {
    out.push(match ink.encoding {
        Embedded::Png => 0,
        Embedded::Jpeg => 1,
        Embedded::Webp => 2,
        Embedded::Gif => 3,
        Embedded::Svg => 4,
    });
    f32s(out, &[ink.extent.width.get(), ink.extent.height.get()]);
    out.extend((ink.encoded.len() as u32).to_le_bytes());
    out.extend(ink.encoded.iter());
}

impl Reader<'_> {
    pub fn image(&mut self) -> Result<ImageInk, FormatError> {
        let encoding = match self.u8()? {
            0 => Embedded::Png,
            1 => Embedded::Jpeg,
            2 => Embedded::Webp,
            3 => Embedded::Gif,
            4 => Embedded::Svg,
            tag => {
                return Err(FormatError::Tag {
                    what: "image encoding",
                    tag,
                });
            }
        };
        let extent = PageSize::new(self.f32()?, self.f32()?)?;
        let length = self.u32()? as usize;
        let bytes = self.bytes.get(..length).ok_or(FormatError::Truncated)?;
        self.bytes = &self.bytes[length..];
        Ok(ImageInk::new(bytes.into(), encoding, extent))
    }
}
