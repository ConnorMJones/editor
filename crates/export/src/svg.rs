//! SVG with glyphs drawn as outlines and images as data URIs, so it shows the same anywhere;
//! PNG is that SVG drawn by resvg.

use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::fmt::Write;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use ink::doc::{Embedded, Ink};
use ink::text::{Fonts, GlyphKey};
use lyon_tessellation::path::Event;
use resvg::{tiny_skia, usvg};

use crate::ExportError;
use crate::scene::{Scene, wound};

// Writing to a `String` cannot fail, so `write!`'s result is dropped throughout.

/// Sized in points.
pub fn svg(scene: &Scene, fonts: &Fonts) -> String {
    let (min, max) = (scene.area.min(), scene.area.max());
    let (width, height) = (max.x - min.x, max.y - min.y);
    let mut defs = String::new();
    let mut body = String::new();
    for fill in scene.paper() {
        let _ = write!(body, r#"<path fill="{}" d=""#, hex(fill.color));
        for rect in &fill.rects {
            let (a, b) = (rect.min(), rect.max());
            let _ = write!(body, "M{} {}H{}V{}H{}Z", a.x, a.y, b.x, b.y, a.x);
        }
        body.push_str(r#""/>"#);
    }
    let mut glyphs: HashMap<GlyphKey, usize> = HashMap::new();
    for mark in &scene.marks {
        let [a, b, c, d, e, f] = mark.placement.forward().coefficients;
        let matrix = format!("matrix({a} {b} {c} {d} {e} {f})");
        let (color, opacity) = (hex(mark.paint.color.rgb), mark.paint.color.alpha.fraction());
        match &mark.ink {
            Ink::Stroke(_) | Ink::Shape(_) => {
                let _ = write!(
                    body,
                    r#"<path transform="{matrix}" fill="{color}" fill-opacity="{opacity}" d=""#
                );
                for [a, b, c] in wound(&mark.built().mesh) {
                    let _ = write!(body, "M{} {}L{} {}L{} {}Z", a.x, a.y, b.x, b.y, c.x, c.y);
                }
                body.push_str(r#""/>"#);
            }
            Ink::Text(text) => {
                let _ = write!(
                    body,
                    r#"<g transform="{matrix}" fill="{color}" fill-opacity="{opacity}">"#
                );
                for placed in &text.glyphs {
                    let next = glyphs.len();
                    let id = match glyphs.entry(placed.key) {
                        Entry::Occupied(found) => *found.get(),
                        Entry::Vacant(slot) => {
                            let _ = write!(defs, r#"<path id="g{next}" d=""#);
                            for event in fonts.glyph(placed.key).outline.iter() {
                                let _ = match event {
                                    Event::Begin { at } => write!(defs, "M{} {}", at.x, at.y),
                                    Event::Line { to, .. } => write!(defs, "L{} {}", to.x, to.y),
                                    Event::Quadratic { ctrl, to, .. } => {
                                        write!(defs, "Q{} {} {} {}", ctrl.x, ctrl.y, to.x, to.y)
                                    }
                                    Event::Cubic {
                                        ctrl1, ctrl2, to, ..
                                    } => write!(
                                        defs,
                                        "C{} {} {} {} {} {}",
                                        ctrl1.x, ctrl1.y, ctrl2.x, ctrl2.y, to.x, to.y
                                    ),
                                    Event::End { close: true, .. } => write!(defs, "Z"),
                                    Event::End { close: false, .. } => Ok(()),
                                };
                            }
                            defs.push_str(r#""/>"#);
                            *slot.insert(next)
                        }
                    };
                    let _ = write!(
                        body,
                        r##"<use href="#g{id}" transform="translate({} {}) scale({})"/>"##,
                        placed.origin.x, placed.origin.y, placed.size
                    );
                }
                body.push_str("</g>");
            }
            Ink::Image(image) => {
                let mime = match image.encoding {
                    Embedded::Png => "image/png",
                    Embedded::Jpeg => "image/jpeg",
                    Embedded::Webp => "image/webp",
                    Embedded::Gif => "image/gif",
                    Embedded::Svg => "image/svg+xml",
                };
                let _ = write!(
                    body,
                    r#"<image transform="{matrix}" width="{}" height="{}" preserveAspectRatio="none" href="data:{mime};base64,{}"/>"#,
                    image.extent.width.get(),
                    image.extent.height.get(),
                    STANDARD.encode(&image.encoded)
                );
            }
        }
    }
    format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="{width}pt" height="{height}pt" viewBox="{} {} {width} {height}"><defs>{defs}</defs>{body}</svg>"#,
        min.x, min.y
    )
}

/// `scale` pixels to the point.
pub fn png(scene: &Scene, fonts: &Fonts, scale: f32) -> Result<Vec<u8>, ExportError> {
    let tree = usvg::Tree::from_str(&svg(scene, fonts), &usvg::Options::default())?;
    let (min, max) = (scene.area.min(), scene.area.max());
    let width = ((max.x - min.x) * scale).ceil() as u32;
    let height = ((max.y - min.y) * scale).ceil() as u32;
    let mut pixmap = tiny_skia::Pixmap::new(width, height).ok_or(ExportError::Empty)?;
    // The tree is sized in points read as pixels at 96 to the inch.
    let size = tree.size();
    let fit = tiny_skia::Transform::from_scale(
        width as f32 / size.width(),
        height as f32 / size.height(),
    );
    resvg::render(&tree, fit, &mut pixmap.as_mut());
    pixmap
        .encode_png()
        .map_err(|error| ExportError::Png(Box::new(error)))
}

fn hex(color: model::color::Rgb) -> String {
    format!("#{:02x}{:02x}{:02x}", color.r, color.g, color.b)
}
