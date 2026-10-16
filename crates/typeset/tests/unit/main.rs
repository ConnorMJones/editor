mod document;
mod examples;
mod math;
mod mermaid;

use typeset::{FontSource, Svg, Typesetter};

pub fn typesetter() -> Typesetter {
    Typesetter::new(FontSource::Embedded, "Libertinus Serif")
        .expect("embedded fonts include a math font")
}

/// Parses the SVG with usvg and checks it has a drawable, non-empty size.
pub fn assert_valid(svg: &Svg) -> usvg::Tree {
    assert!(
        svg.width.0 > 0.0 && svg.height.0 > 0.0,
        "empty size: {svg:?}"
    );
    let tree =
        usvg::Tree::from_data(&svg.bytes, &usvg::Options::default()).unwrap_or_else(|error| {
            panic!(
                "invalid SVG ({error}): {}",
                String::from_utf8_lossy(&svg.bytes)
            )
        });
    assert!(tree.size().width() > 0.0 && tree.size().height() > 0.0);
    assert!(tree.root().has_children(), "nothing drawn");
    tree
}
