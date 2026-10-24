use std::sync::Arc;

use ink::brush::Size;
use ink::doc::{Ink, Mark, Placement};
use ink::text::{Caret, FontFamily, Inked, Laid, TextInk, TextStyle, Wrap};

use crate::{fonts, paint};

pub fn style() -> TextStyle {
    TextStyle {
        family: FontFamily::new(model::fonts::SANS).unwrap(),
        size: Size::new(12.0).unwrap(),
    }
}

fn laid(text: &str, wrap: Wrap) -> Laid {
    fonts().lay_out(text, &style(), wrap)
}

fn ink(laid: &Laid) -> &Arc<TextInk> {
    match &laid.inked {
        Inked::Ink(ink) => ink,
        Inked::Blank => panic!("the text leaves a mark"),
    }
}

/// A text mark of `text` wrapped at 120 points, aligned with its surface.
pub fn mark(text: &str) -> Mark {
    let laid = fonts().lay_out(text, &style(), Wrap::At(Size::new(120.0).unwrap()));
    Mark {
        placement: Placement::ALIGNED,
        paint: paint(),
        ink: Ink::Text(ink(&laid).clone()),
    }
}

#[test]
fn text_is_inked_within_its_lines_with_a_caret_at_every_boundary() {
    let laid = laid("Hi", Wrap::Lines);
    assert_eq!(laid.carets.len(), 3);
    let bounds = ink(&laid).built.bounds;
    assert!(bounds.min().x >= 0.0 && bounds.min().y >= 0.0, "{bounds:?}");
    assert!(bounds.max().y <= 12.0 * 1.25, "{bounds:?}");
    let xs: Vec<f32> = laid.carets.iter().map(|caret| caret.at.x).collect();
    assert!(xs.windows(2).all(|pair| pair[0] < pair[1]), "{xs:?}");
    assert!((xs[2] - bounds.max().x).abs() < 2.0, "{xs:?} {bounds:?}");
}

#[test]
fn blank_text_leaves_no_mark_but_keeps_its_carets() {
    let laid = laid("  \n", Wrap::Lines);
    assert!(matches!(laid.inked, Inked::Blank));
    assert_eq!(laid.carets.len(), 4);
    let Caret { at, height } = laid.carets[3];
    assert!(
        at.y >= height * 0.9,
        "the caret after a newline is on the next line: {at:?}"
    );
}

#[test]
fn a_wrap_width_breaks_lines_between_words() {
    let unwrapped = laid("first second", Wrap::Lines);
    let wrapped = laid("first second", Wrap::At(Size::new(40.0).unwrap()));
    let second = "first ".len();
    assert_eq!(unwrapped.carets[second].at.y, unwrapped.carets[0].at.y);
    assert!(wrapped.carets[second].at.y > wrapped.carets[0].at.y);
    assert_eq!(wrapped.carets[second].at.x, 0.0);
}

#[test]
fn a_repeated_glyph_reuses_one_outline() {
    let one = ink(&laid("a", Wrap::Lines)).built.mesh.triangles.len();
    let two = ink(&laid("aa", Wrap::Lines)).built.mesh.triangles.len();
    assert!(one > 0);
    assert_eq!(two, one * 2);
}
