mod scene;
mod writers;

use std::sync::Arc;

use ink::brush::{Paint, Size};
use ink::doc::{Embedded, ImageInk, Ink, Mark, PageSize, Placement, Shape, ShapeInk, ShapeKind};
use ink::geom::{Local, Point, Surface};
use ink::text::{Fonts, Inked, TextStyle, Wrap};
use model::color::Rgb;
use model::theme::Color;

pub fn paint() -> Paint {
    Paint {
        color: Color::opaque(Rgb::hex(0x1f2328)),
    }
}

/// A rectangle outline from `from` to `to` on the surface.
pub fn rectangle(from: (f32, f32), to: (f32, f32)) -> Mark {
    let local = |(x, y): (f32, f32)| Point::<Local>::new(x, y);
    let shape = Shape::dragged(ShapeKind::Rectangle, local(from), local(to));
    Mark {
        placement: Placement::ALIGNED,
        paint: paint(),
        ink: Ink::Shape(Arc::new(ShapeInk::new(shape, Size::new(2.0).unwrap()))),
    }
}

pub fn fonts() -> Fonts {
    Fonts::new(&model::fonts::BUNDLED)
}

pub fn text(fonts: &Fonts, words: &str, at: (f32, f32)) -> Mark {
    let style = TextStyle {
        family: ink::text::FontFamily::new(model::fonts::SANS).unwrap(),
        size: Size::new(12.0).unwrap(),
    };
    let Inked::Ink(ink) = fonts.lay_out(words, &style, Wrap::Lines).inked else {
        panic!("{words:?} leaves ink");
    };
    Mark {
        placement: Placement::at(Point::<Surface>::new(at.0, at.1)),
        paint: paint(),
        ink: Ink::Text(ink),
    }
}

/// A 4 by 2 PNG over a 40 by 20 point rectangle.
pub fn picture(at: (f32, f32)) -> Mark {
    let png = resvg::tiny_skia::Pixmap::new(4, 2)
        .unwrap()
        .encode_png()
        .unwrap();
    Mark {
        placement: Placement::at(Point::<Surface>::new(at.0, at.1)),
        paint: paint(),
        ink: Ink::Image(Arc::new(ImageInk::new(
            png.into(),
            Embedded::Png,
            PageSize::new(40.0, 20.0).unwrap(),
        ))),
    }
}
