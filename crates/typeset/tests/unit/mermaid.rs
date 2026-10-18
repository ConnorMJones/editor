use model::color::Palette;
use typeset::TypesetError;

use crate::{assert_valid, typesetter};

#[test]
fn flowchart_renders() {
    let mut typesetter = typesetter();
    let svg = typesetter
        .mermaid(
            "flowchart TD\n  A[Start] --> B{Ok?}\n  B -->|yes| C[Done]\n  B -->|no| A",
            &Palette::DEFAULT,
        )
        .expect("flowchart");
    assert_valid(&svg);
    let text = String::from_utf8_lossy(&svg.bytes);
    assert!(!text.contains("<foreignObject"), "{text}");
    // Node fill comes from base01 of the palette.
    assert!(text.contains("#2a3448"), "{text}");
}

#[test]
fn sequence_diagram_renders() {
    let mut typesetter = typesetter();
    let svg = typesetter
        .mermaid(
            "sequenceDiagram\n  Alice->>Bob: Hello\n  Bob-->>Alice: Hi\n  Note over Alice,Bob: done",
            &Palette::DEFAULT,
        )
        .expect("sequence diagram");
    assert_valid(&svg);
}

#[test]
fn palette_change_rebuilds_the_renderer() {
    let mut typesetter = typesetter();
    let mut palette = Palette::DEFAULT;
    let first = typesetter
        .mermaid("flowchart LR\n  A --> B", &palette)
        .expect("first");
    palette.0[1] = model::color::Rgb {
        r: 0x12,
        g: 0x34,
        b: 0x56,
    };
    let second = typesetter
        .mermaid("flowchart LR\n  A --> B", &palette)
        .expect("second");
    assert!(!String::from_utf8_lossy(&first.bytes).contains("#123456"));
    assert!(String::from_utf8_lossy(&second.bytes).contains("#123456"));
}

#[test]
fn prose_is_not_a_diagram() {
    let mut typesetter = typesetter();
    let result = typesetter.mermaid("just some words", &Palette::DEFAULT);
    assert!(result.is_err(), "{result:?}");
    let result = typesetter.mermaid("flowchart TD\n  A -->", &Palette::DEFAULT);
    assert!(
        matches!(result, Err(TypesetError::Mermaid(_))),
        "{result:?}"
    );
}

#[test]
fn background_is_transparent() {
    let mut typesetter = typesetter();
    let svg = typesetter
        .mermaid("graph TD\n  A --> B\n", &Palette::DEFAULT)
        .expect("flowchart");
    let text = String::from_utf8_lossy(&svg.bytes);
    let root = &text[..text.find('>').expect("root tag")];
    assert!(!root.contains("background-color:white"), "{root}");
}
