use jsoncanvas::{Cap, JsonCanvasColor, JsonCanvasError, NodeKind, Preset, Side, read, write};

const SAMPLE: &str = include_str!("../fixtures/sample.canvas");

#[test]
fn the_spec_sample_reads_every_kind_of_node_and_edge() {
    let canvas = read(SAMPLE).unwrap();
    let kinds: Vec<&str> = (canvas.nodes().iter())
        .map(|node| match &node.kind {
            NodeKind::Text(_) => "text",
            NodeKind::File { .. } => "file",
            NodeKind::Link(_) => "link",
            NodeKind::Group { .. } => "group",
        })
        .collect();
    assert_eq!(kinds, ["group", "file", "file", "text", "file"]);
    assert_eq!(
        canvas.nodes()[1].color,
        Some(JsonCanvasColor::Preset(Preset::Purple))
    );
    assert_eq!(canvas.nodes()[0].area.x, -300);
    assert_eq!(canvas.nodes()[0].area.width.get(), 610);
    let edge = &canvas.edges()[0];
    assert_eq!(
        (edge.from.side, edge.to.side),
        (Some(Side::Right), Some(Side::Left))
    );
    assert_eq!(edge.caps(), [Cap::None, Cap::Arrow], "the spec's defaults");
}

#[test]
fn writing_and_reading_again_gives_the_same_canvas_with_unknown_fields_kept() {
    let text = r##"{"nodes":[
        {"id":"a","type":"text","text":"hi\nthere","x":0,"y":0,"width":10,"height":20,"color":"#ff8800","styleAttributes":{"shape":"pill"}},
        {"id":"b","type":"file","file":"notes/a.md","subpath":"#Heading","x":-5,"y":7,"width":1,"height":1},
        {"id":"c","type":"link","url":"https://example.com","x":0,"y":0,"width":1,"height":1},
        {"id":"d","type":"group","label":"G","background":"bg.png","backgroundStyle":"repeat","x":0,"y":0,"width":3,"height":3}
    ],"edges":[
        {"id":"e","fromNode":"a","fromEnd":"arrow","toNode":"b","toEnd":"none","color":"2","label":"so"}
    ],"metadata":{"version":"1.0"}}"##;
    let canvas = read(text).unwrap();
    assert_eq!(canvas.nodes()[0].extra["styleAttributes"]["shape"], "pill");
    assert_eq!(canvas.extra["metadata"]["version"], "1.0");
    let written = write(&canvas);
    assert_eq!(read(&written).unwrap(), canvas);
    assert_eq!(
        read(&write(&read(SAMPLE).unwrap())).unwrap(),
        read(SAMPLE).unwrap()
    );
    assert!(
        written.starts_with("{\n\t\"nodes\":[\n\t\t{\"id\":\"a\",\"type\":\"text\""),
        "{written}"
    );
}

#[test]
fn malformed_canvases_are_refused() {
    let node = |fields: &str| {
        format!(
            r#"{{"nodes":[{{"id":"a","type":"text","text":"t","x":0,"y":0,"width":1,"height":1{fields}}}]}}"#
        )
    };
    let refused = |text: &str| read(text).unwrap_err();
    assert!(matches!(refused("not json"), JsonCanvasError::Json(_)));
    assert!(matches!(
        refused(r#"{"nodes":[{"id":"a","type":"text","x":0,"y":0,"width":1,"height":1}]}"#),
        JsonCanvasError::Missing { field: "text", .. }
    ));
    assert!(matches!(
        refused(&node(r#","color":"7""#)),
        JsonCanvasError::Wrong { field: "color", .. }
    ));
    assert!(matches!(
        refused(
            r#"{"nodes":[{"id":"a","type":"text","text":"t","x":0,"y":0,"width":0,"height":1}]}"#
        ),
        JsonCanvasError::Wrong { field: "width", .. }
    ));
    assert!(matches!(
        refused(r#"{"nodes":[{"id":"a","type":"shape","x":0,"y":0,"width":1,"height":1}]}"#),
        JsonCanvasError::UnknownType { .. }
    ));
    assert!(matches!(
        refused(
            r#"{"nodes":[{"id":"a","type":"file","file":"f","subpath":"h","x":0,"y":0,"width":1,"height":1}]}"#
        ),
        JsonCanvasError::Wrong {
            field: "subpath",
            ..
        }
    ));
    let twice = r#"{"nodes":[{"id":"a","type":"text","text":"t","x":0,"y":0,"width":1,"height":1},{"id":"a","type":"text","text":"t","x":0,"y":0,"width":1,"height":1}]}"#;
    assert!(matches!(refused(twice), JsonCanvasError::DuplicateId(id) if id == "a"));
    let dangling = r#"{"nodes":[{"id":"a","type":"text","text":"t","x":0,"y":0,"width":1,"height":1}],"edges":[{"id":"e","fromNode":"a","toNode":"z"}]}"#;
    assert!(matches!(refused(dangling), JsonCanvasError::UnknownNode { node, .. } if node == "z"));
    assert!(
        read("{}").unwrap().nodes().is_empty(),
        "both lists are optional"
    );
}
