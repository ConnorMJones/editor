//! PDF through krilla: meshes as filled paths, text as real text in its embedded fonts.

use std::collections::HashMap;
use std::collections::hash_map::Entry;

use ink::brush::Paint;
use ink::doc::{Embedded, ImageInk, Ink, Mark};
use ink::text::{FontId, Fonts, TextInk};
use krilla::color::rgb;
use krilla::geom::{PathBuilder, Point, Size, Transform};
use krilla::image::Image;
use krilla::num::NormalizedF32;
use krilla::page::PageSettings;
use krilla::paint::{Fill, FillRule};
use krilla::surface::Surface as Canvas;
use krilla::text::{Font, GlyphId, KrillaGlyph};
use krilla::{Data, Document};
use krilla_svg::{SurfaceExt, SvgSettings};
use model::color::Rgb;

use crate::ExportError;
use crate::scene::{Scene, wound};

pub fn pdf<'a>(
    scenes: impl IntoIterator<Item = Scene<'a>>,
    fonts: &Fonts,
) -> Result<Vec<u8>, ExportError> {
    let mut document = Document::new();
    let mut faces = HashMap::new();
    for scene in scenes {
        let (min, max) = (scene.area.min(), scene.area.max());
        let settings =
            PageSettings::from_wh(max.x - min.x, max.y - min.y).ok_or(ExportError::Empty)?;
        let mut page = document.start_page_with(settings);
        let mut surface = page.surface();
        surface.push_transform(&Transform::from_translate(-min.x, -min.y));
        for fill in scene.paper() {
            let mut path = PathBuilder::new();
            for rect in &fill.rects {
                let (min, max) = (rect.min(), rect.max());
                path.move_to(min.x, min.y);
                path.line_to(max.x, min.y);
                path.line_to(max.x, max.y);
                path.line_to(min.x, max.y);
                path.close();
            }
            surface.set_fill(Some(solid(fill.color, 1.0)));
            match path.finish() {
                Some(path) => surface.draw_path(&path),
                None => {}
            }
        }
        for mark in &scene.marks {
            draw(&mut surface, mark, fonts, &mut faces)?;
        }
        surface.pop();
        surface.finish();
        page.finish();
    }
    document.finish().map_err(ExportError::Pdf)
}

fn draw(
    surface: &mut Canvas,
    mark: &Mark,
    fonts: &Fonts,
    faces: &mut HashMap<FontId, Font>,
) -> Result<(), ExportError> {
    let [a, b, c, d, e, f] = mark.placement.forward().coefficients;
    surface.push_transform(&Transform::from_row(a, b, c, d, e, f));
    match &mark.ink {
        Ink::Stroke(_) | Ink::Shape(_) => {
            let mut path = PathBuilder::new();
            for [a, b, c] in wound(&mark.built().mesh) {
                path.move_to(a.x, a.y);
                path.line_to(b.x, b.y);
                path.line_to(c.x, c.y);
                path.close();
            }
            surface.set_fill(Some(paint(mark.paint)));
            match path.finish() {
                Some(path) => surface.draw_path(&path),
                None => {}
            }
        }
        Ink::Text(text) => glyphs(surface, text, mark.paint, fonts, faces)?,
        Ink::Image(image) => picture(surface, image)?,
    }
    surface.pop();
    Ok(())
}

/// Each run of glyphs sharing a font, size and baseline is drawn from its first glyph.
fn glyphs(
    surface: &mut Canvas,
    text: &TextInk,
    color: Paint,
    fonts: &Fonts,
    faces: &mut HashMap<FontId, Font>,
) -> Result<(), ExportError> {
    surface.set_fill(Some(paint(color)));
    let runs = text
        .glyphs
        .chunk_by(|a, b| a.key.font == b.key.font && a.size == b.size && a.origin.y == b.origin.y);
    for run in runs {
        let first = &run[0];
        let font = match faces.entry(first.key.font) {
            Entry::Occupied(found) => found.get().clone(),
            Entry::Vacant(slot) => {
                let face = fonts.face(first.key.font).ok_or(ExportError::Font)?;
                let font = Font::new(face.data.into(), face.index).ok_or(ExportError::Font)?;
                slot.insert(font).clone()
            }
        };
        let size = first.size;
        let mut pen = first.origin.x;
        let placed: Vec<KrillaGlyph> = run
            .iter()
            .map(|glyph| {
                let offset = glyph.origin.x - pen;
                pen += glyph.advance;
                KrillaGlyph::new(
                    GlyphId::new(u32::from(glyph.key.glyph)),
                    glyph.advance / size,
                    offset / size,
                    0.0,
                    0.0,
                    glyph.text.clone(),
                    None,
                )
            })
            .collect();
        surface.draw_glyphs(
            Point::from_xy(first.origin.x, first.origin.y),
            &placed,
            font,
            &text.text,
            size,
            false,
        );
    }
    Ok(())
}

fn picture(surface: &mut Canvas, image: &ImageInk) -> Result<(), ExportError> {
    let size = Size::from_wh(image.extent.width.get(), image.extent.height.get())
        .ok_or(ExportError::Empty)?;
    let data = || Data::from(image.encoded.to_vec());
    let bitmap = match image.encoding {
        Embedded::Svg => {
            let tree =
                resvg::usvg::Tree::from_data(&image.encoded, &resvg::usvg::Options::default())?;
            return surface.draw_svg(&tree, size, SvgSettings::default()).ok_or(
                ExportError::Picture {
                    encoding: image.encoding,
                    reason: "the svg could not be drawn".to_owned(),
                },
            );
        }
        Embedded::Png => Image::from_png(data(), true),
        Embedded::Jpeg => Image::from_jpeg(data(), true),
        Embedded::Webp => Image::from_webp(data(), true),
        Embedded::Gif => Image::from_gif(data(), true),
    };
    let bitmap = bitmap.map_err(|reason| ExportError::Picture {
        encoding: image.encoding,
        reason,
    })?;
    surface.draw_image(bitmap, size);
    Ok(())
}

fn paint(paint: Paint) -> Fill {
    solid(paint.color.rgb, paint.color.alpha.fraction())
}

fn solid(color: Rgb, opacity: f32) -> Fill {
    Fill {
        paint: rgb::Color::new(color.r, color.g, color.b).into(),
        opacity: NormalizedF32::new(opacity).expect("an alpha is a fraction"),
        rule: FillRule::NonZero,
    }
}
