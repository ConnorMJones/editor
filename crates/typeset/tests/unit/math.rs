use model::color::Rgb;
use typeset::{Display, Pt, TypesetError, Typesetter};

use crate::{assert_valid, typesetter};

const WHITE: Rgb = Rgb {
    r: 0xff,
    g: 0xff,
    b: 0xff,
};

#[test]
fn formulas_render_to_valid_svg() {
    let mut typesetter = typesetter();
    let formulas = [
        (r"\frac{a+b}{c^2}", Display::Inline),
        (r"\sum_{i=1}^{n} i = \frac{n(n+1)}{2}", Display::Block),
        (
            r"\begin{pmatrix} 1 & 0 \\ 0 & 1 \end{pmatrix}",
            Display::Block,
        ),
        (
            r"\begin{aligned} f(x) &= x^2 + 2x + 1 \\ &= (x+1)^2 \end{aligned}",
            Display::Block,
        ),
        (
            r"\int_0^\infty e^{-x^2}\,dx = \frac{\sqrt{\pi}}{2}",
            Display::Block,
        ),
        (r"\alpha \leq \beta \Rightarrow \mathbb{R}", Display::Inline),
        (r"\sqrt[3]{x} + \hat{y} + \mathbf{v}", Display::Inline),
        (r"a \xrightarrow{f} b", Display::Inline),
        (r"\text{if } x > 0", Display::Inline),
    ];
    for (tex, display) in formulas {
        let svg = typesetter
            .math(tex, display, WHITE, Pt(16.0))
            .unwrap_or_else(|error| panic!("{tex}: {error}"));
        assert_valid(&svg);
        match display {
            Display::Inline => {
                let baseline = svg.baseline.0;
                assert!(
                    baseline > 0.0 && baseline <= svg.height.0,
                    "{tex}: baseline {baseline} outside 0..{}",
                    svg.height.0
                );
            }
            Display::Block => assert_eq!(svg.baseline, svg.height),
        }
    }
}

#[test]
fn fraction_baseline_sits_between_numerator_and_denominator() {
    let mut typesetter = typesetter();
    let plain = typesetter
        .math("x", Display::Inline, WHITE, Pt(16.0))
        .expect("plain x");
    let fraction = typesetter
        .math(r"\frac{1}{2}", Display::Inline, WHITE, Pt(16.0))
        .expect("fraction");
    let baseline = fraction.baseline.0;
    assert!(fraction.height.0 > plain.height.0);
    assert!(baseline > fraction.height.0 * 0.3 && baseline < fraction.height.0 * 0.8);
}

#[test]
fn size_and_colour_reach_the_output() {
    let mut typesetter = typesetter();
    let small = typesetter
        .math("x+y", Display::Inline, WHITE, Pt(10.0))
        .expect("small");
    let large = typesetter
        .math("x+y", Display::Inline, WHITE, Pt(20.0))
        .expect("large");
    assert!((large.width.0 / small.width.0 - 2.0).abs() < 0.05);
    let red = typesetter
        .math(
            "x",
            Display::Inline,
            Rgb {
                r: 0xd0,
                g: 0x73,
                b: 0x46,
            },
            Pt(16.0),
        )
        .expect("red");
    let text = String::from_utf8_lossy(&red.bytes);
    assert!(text.contains("#d07346"), "{text}");
}

#[test]
fn invalid_latex_is_an_error() {
    let mut typesetter = typesetter();
    // mitex rejects unknown commands and stray braces; typst rejects what mitex lets through.
    for tex in [
        r"\nonexistentcommand{x}",
        "}",
        r"\left( x",
        r"\frac",
        r"\sqrt",
    ] {
        let result = typesetter.math(tex, Display::Inline, WHITE, Pt(16.0));
        assert!(
            matches!(result, Err(TypesetError::Latex(_) | TypesetError::Math(_))),
            "{tex}: {result:?}"
        );
    }
}

#[test]
fn quotes_and_backslashes_survive_escaping() {
    let mut typesetter = typesetter();
    let svg = typesetter
        .math(
            r#"\text{say "hi"} \backslash"#,
            Display::Inline,
            WHITE,
            Pt(16.0),
        )
        .expect("escaped");
    assert_valid(&svg);
}

#[test]
fn system_fonts_load() {
    let mut typesetter =
        Typesetter::new(typeset::FontSource::SystemAndEmbedded, "Libertinus Serif")
            .expect("system fonts");
    let svg = typesetter
        .math(r"e^{i\pi} + 1 = 0", Display::Block, WHITE, Pt(16.0))
        .expect("math");
    assert_valid(&svg);
}

#[test]
fn typesetter_is_send() {
    fn assert_send<T: Send>() {}
    assert_send::<Typesetter>();
}
