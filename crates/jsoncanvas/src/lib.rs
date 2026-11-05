use std::collections::HashSet;
use std::fmt;
use std::num::NonZeroU32;

pub use model::color::Preset;
use model::color::Rgb;
use serde_json::{Map, Value};

mod edit;

pub use edit::{
    Change, ChangeError, Copies, Edited, HistoryError, KEPT, Labelled, NewEdge, NewNode, Offset,
    Picked, Revision,
};

pub type Fields = Map<String, Value>;

const PRESETS: [(Preset, &str); 6] = [
    (Preset::Red, "1"),
    (Preset::Orange, "2"),
    (Preset::Yellow, "3"),
    (Preset::Green, "4"),
    (Preset::Cyan, "5"),
    (Preset::Purple, "6"),
];

#[derive(Clone, Debug, PartialEq)]
pub struct JsonCanvas {
    // private: a direct change can repeat an id or keep an edge to a removed node; the written
    // file is then not valid JSON Canvas.
    nodes: Vec<Node>,
    edges: Vec<Edge>,
    pub extra: Fields,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct NodeId(String); // private: an empty or repeated id makes the file not valid JSON Canvas

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct EdgeId(String); // private: an empty or repeated id makes the file not valid JSON Canvas

#[derive(Clone, Debug, PartialEq)]
pub struct Node {
    pub id: NodeId,
    pub area: Area,
    pub color: Option<JsonCanvasColor>,
    pub kind: NodeKind,
    pub extra: Fields,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Area {
    pub x: i64,
    pub y: i64,
    pub width: NonZeroU32,
    pub height: NonZeroU32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NodeKind {
    Text(String),
    File {
        file: String,
        subpath: Option<Subpath>,
    },
    Link(String),
    Group {
        label: Option<String>,
        background: Option<Background>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Subpath(String); // private: a subpath without `#` does not point to a heading or block

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Background {
    pub image: String,
    pub style: Option<BackgroundStyle>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BackgroundStyle {
    Cover,
    Ratio,
    Repeat,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Edge {
    pub id: EdgeId,
    pub from: End,
    pub to: End,
    pub color: Option<JsonCanvasColor>,
    pub label: Option<String>,
    pub extra: Fields,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct End {
    pub node: NodeId,
    pub side: Option<Side>,
    pub cap: Option<Cap>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    Top,
    Right,
    Bottom,
    Left,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cap {
    None,
    Arrow,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum JsonCanvasColor {
    Preset(Preset),
    Hex(Rgb),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum At {
    JsonCanvas,
    Node(usize),
    Edge(usize),
}

#[derive(Debug, thiserror::Error)]
pub enum JsonCanvasError {
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error("{at}: {field} is missing")]
    Missing { at: At, field: &'static str },
    #[error("{at}: {field} is not {wanted}")]
    Wrong {
        at: At,
        field: &'static str,
        wanted: &'static str,
    },
    #[error("{at}: \"{found}\" is not a node type")]
    UnknownType { at: At, found: String },
    #[error("two nodes or two edges share the id \"{0}\"")]
    DuplicateId(String),
    #[error("edge \"{edge}\" names node \"{node}\", which is not in the canvas")]
    UnknownNode { edge: String, node: String },
}

impl fmt::Display for At {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            At::JsonCanvas => f.write_str("the canvas"),
            At::Node(index) => write!(f, "node {}", index + 1),
            At::Edge(index) => write!(f, "edge {}", index + 1),
        }
    }
}

impl NodeId {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl EdgeId {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Subpath {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Edge {
    pub fn caps(&self) -> [Cap; 2] {
        [
            self.from.cap.unwrap_or(Cap::None),
            self.to.cap.unwrap_or(Cap::Arrow),
        ]
    }
}

impl Side {
    const ALL: [(Side, &'static str); 4] = [
        (Side::Top, "top"),
        (Side::Right, "right"),
        (Side::Bottom, "bottom"),
        (Side::Left, "left"),
    ];
}

impl Cap {
    const ALL: [(Cap, &'static str); 2] = [(Cap::None, "none"), (Cap::Arrow, "arrow")];
}

impl BackgroundStyle {
    const ALL: [(BackgroundStyle, &'static str); 3] = [
        (BackgroundStyle::Cover, "cover"),
        (BackgroundStyle::Ratio, "ratio"),
        (BackgroundStyle::Repeat, "repeat"),
    ];
}

fn named<T: Copy + PartialEq>(names: &[(T, &'static str)], text: &str) -> Option<T> {
    (names.iter())
        .find(|(_, name)| *name == text)
        .map(|(value, _)| *value)
}

fn name<T: Copy + PartialEq>(names: &[(T, &'static str)], value: T) -> &'static str {
    (names.iter())
        .find(|(known, _)| *known == value)
        .map_or("", |(_, name)| name)
}

struct Taking {
    at: At,
    fields: Fields,
}

impl Taking {
    fn of(at: At, value: Value) -> Result<Taking, JsonCanvasError> {
        match value {
            Value::Object(fields) => Ok(Taking { at, fields }),
            _ => Err(JsonCanvasError::Wrong {
                at,
                field: "the entry",
                wanted: "an object",
            }),
        }
    }

    fn wrong(&self, field: &'static str, wanted: &'static str) -> JsonCanvasError {
        JsonCanvasError::Wrong {
            at: self.at,
            field,
            wanted,
        }
    }

    fn maybe_text(&mut self, field: &'static str) -> Result<Option<String>, JsonCanvasError> {
        match self.fields.remove(field) {
            None => Ok(None),
            Some(Value::String(text)) => Ok(Some(text)),
            Some(_) => Err(self.wrong(field, "a string")),
        }
    }

    fn text(&mut self, field: &'static str) -> Result<String, JsonCanvasError> {
        self.maybe_text(field)?
            .ok_or(JsonCanvasError::Missing { at: self.at, field })
    }

    fn named<T: Copy + PartialEq>(
        &mut self,
        field: &'static str,
        names: &[(T, &'static str)],
        wanted: &'static str,
    ) -> Result<Option<T>, JsonCanvasError> {
        match self.maybe_text(field)? {
            None => Ok(None),
            Some(text) => named(names, &text)
                .map(Some)
                .ok_or(self.wrong(field, wanted)),
        }
    }

    fn int(&mut self, field: &'static str) -> Result<i64, JsonCanvasError> {
        match self.fields.remove(field) {
            None => Err(JsonCanvasError::Missing { at: self.at, field }),
            Some(value) => value.as_i64().ok_or(self.wrong(field, "a whole number")),
        }
    }

    fn extent(&mut self, field: &'static str) -> Result<NonZeroU32, JsonCanvasError> {
        let value = self.int(field)?;
        (u32::try_from(value).ok())
            .and_then(NonZeroU32::new)
            .ok_or(self.wrong(field, "a positive whole number"))
    }

    fn color(&mut self) -> Result<Option<JsonCanvasColor>, JsonCanvasError> {
        let Some(text) = self.maybe_text("color")? else {
            return Ok(None);
        };
        let hex = (text.strip_prefix('#'))
            .filter(|digits| digits.len() == 6)
            .and_then(|digits| u32::from_str_radix(digits, 16).ok());
        match (named(&PRESETS, &text), hex) {
            (Some(preset), _) => Ok(Some(JsonCanvasColor::Preset(preset))),
            (None, Some(hex)) => Ok(Some(JsonCanvasColor::Hex(Rgb::hex(hex)))),
            (None, None) => Err(self.wrong("color", "a preset from 1 to 6 or a #rrggbb colour")),
        }
    }
}

fn node_id(text: String, at: At, field: &'static str) -> Result<String, JsonCanvasError> {
    Some(text)
        .filter(|text| !text.is_empty())
        .ok_or(JsonCanvasError::Wrong {
            at,
            field,
            wanted: "a non-empty id",
        })
}

fn node(index: usize, value: Value) -> Result<Node, JsonCanvasError> {
    let mut taking = Taking::of(At::Node(index), value)?;
    let id = NodeId(node_id(taking.text("id")?, taking.at, "id")?);
    let kind = match taking.text("type")?.as_str() {
        "text" => NodeKind::Text(taking.text("text")?),
        "file" => {
            let file = taking.text("file")?;
            let subpath = match taking.maybe_text("subpath")? {
                Some(subpath) if subpath.starts_with('#') => Some(Subpath(subpath)),
                Some(_) => return Err(taking.wrong("subpath", "a path starting with #")),
                None => None,
            };
            NodeKind::File { file, subpath }
        }
        "link" => NodeKind::Link(taking.text("url")?),
        "group" => {
            let label = taking.maybe_text("label")?;
            let image = taking.maybe_text("background")?;
            let style = taking.named(
                "backgroundStyle",
                &BackgroundStyle::ALL,
                "cover, ratio or repeat",
            )?;
            let background = match (image, style) {
                (Some(image), style) => Some(Background { image, style }),
                (None, None) => None,
                (None, Some(_)) => {
                    return Err(JsonCanvasError::Missing {
                        at: taking.at,
                        field: "background",
                    });
                }
            };
            NodeKind::Group { label, background }
        }
        other => {
            return Err(JsonCanvasError::UnknownType {
                at: taking.at,
                found: other.to_owned(),
            });
        }
    };
    let area = Area {
        x: taking.int("x")?,
        y: taking.int("y")?,
        width: taking.extent("width")?,
        height: taking.extent("height")?,
    };
    let color = taking.color()?;
    Ok(Node {
        id,
        area,
        color,
        kind,
        extra: taking.fields,
    })
}

fn edge(index: usize, value: Value) -> Result<Edge, JsonCanvasError> {
    let mut taking = Taking::of(At::Edge(index), value)?;
    let id = EdgeId(node_id(taking.text("id")?, taking.at, "id")?);
    let mut end = |node: &'static str, side: &'static str, cap: &'static str| {
        Ok::<End, JsonCanvasError>(End {
            node: NodeId(taking.text(node)?),
            side: taking.named(side, &Side::ALL, "top, right, bottom or left")?,
            cap: taking.named(cap, &Cap::ALL, "none or arrow")?,
        })
    };
    let from = end("fromNode", "fromSide", "fromEnd")?;
    let to = end("toNode", "toSide", "toEnd")?;
    let color = taking.color()?;
    let label = taking.maybe_text("label")?;
    Ok(Edge {
        id,
        from,
        to,
        color,
        label,
        extra: taking.fields,
    })
}

fn entries(top: &mut Taking, field: &'static str) -> Result<Vec<Value>, JsonCanvasError> {
    match top.fields.remove(field) {
        None => Ok(Vec::new()),
        Some(Value::Array(entries)) => Ok(entries),
        Some(_) => Err(top.wrong(field, "a list")),
    }
}

pub fn read(text: &str) -> Result<JsonCanvas, JsonCanvasError> {
    let mut top = Taking::of(At::JsonCanvas, serde_json::from_str(text)?)?;
    let nodes = (entries(&mut top, "nodes")?.into_iter().enumerate())
        .map(|(index, value)| node(index, value))
        .collect::<Result<Vec<Node>, JsonCanvasError>>()?;
    let edges = (entries(&mut top, "edges")?.into_iter().enumerate())
        .map(|(index, value)| edge(index, value))
        .collect::<Result<Vec<Edge>, JsonCanvasError>>()?;
    let mut ids = HashSet::new();
    for node in &nodes {
        (ids.insert(node.id.as_str()))
            .then_some(())
            .ok_or_else(|| JsonCanvasError::DuplicateId(node.id.0.clone()))?;
    }
    let mut edge_ids = HashSet::new();
    for edge in &edges {
        (edge_ids.insert(edge.id.as_str()))
            .then_some(())
            .ok_or_else(|| JsonCanvasError::DuplicateId(edge.id.0.clone()))?;
        for end in [&edge.from, &edge.to] {
            (ids.contains(end.node.as_str()))
                .then_some(())
                .ok_or_else(|| JsonCanvasError::UnknownNode {
                    edge: edge.id.0.clone(),
                    node: end.node.0.clone(),
                })?;
        }
    }
    Ok(JsonCanvas {
        nodes,
        edges,
        extra: top.fields,
    })
}

fn color_value(color: JsonCanvasColor) -> Value {
    match color {
        JsonCanvasColor::Preset(preset) => Value::from(name(&PRESETS, preset)),
        JsonCanvasColor::Hex(rgb) => {
            Value::from(format!("#{:02x}{:02x}{:02x}", rgb.r, rgb.g, rgb.b))
        }
    }
}

fn put(fields: &mut Fields, key: &str, value: Option<Value>) {
    fields.extend(value.map(|value| (key.to_owned(), value)));
}

fn node_value(node: &Node) -> Value {
    let mut fields = Fields::new();
    put(&mut fields, "id", Some(Value::from(node.id.as_str())));
    let (kind, own): (&str, [(&str, Option<Value>); 3]) = match &node.kind {
        NodeKind::Text(text) => (
            "text",
            [
                ("text", Some(Value::from(text.as_str()))),
                ("", None),
                ("", None),
            ],
        ),
        NodeKind::File { file, subpath } => (
            "file",
            [
                ("file", Some(Value::from(file.as_str()))),
                (
                    "subpath",
                    subpath
                        .as_ref()
                        .map(|subpath| Value::from(subpath.as_str())),
                ),
                ("", None),
            ],
        ),
        NodeKind::Link(url) => (
            "link",
            [
                ("url", Some(Value::from(url.as_str()))),
                ("", None),
                ("", None),
            ],
        ),
        NodeKind::Group { label, background } => (
            "group",
            [
                ("label", label.as_deref().map(Value::from)),
                (
                    "background",
                    background
                        .as_ref()
                        .map(|background| Value::from(background.image.as_str())),
                ),
                (
                    "backgroundStyle",
                    (background.as_ref())
                        .and_then(|background| background.style)
                        .map(|style| Value::from(name(&BackgroundStyle::ALL, style))),
                ),
            ],
        ),
    };
    put(&mut fields, "type", Some(Value::from(kind)));
    for (key, value) in own {
        put(&mut fields, key, value);
    }
    let Area {
        x,
        y,
        width,
        height,
    } = node.area;
    put(&mut fields, "x", Some(Value::from(x)));
    put(&mut fields, "y", Some(Value::from(y)));
    put(&mut fields, "width", Some(Value::from(width.get())));
    put(&mut fields, "height", Some(Value::from(height.get())));
    put(&mut fields, "color", node.color.map(color_value));
    fields.extend(node.extra.clone());
    Value::Object(fields)
}

fn edge_value(edge: &Edge) -> Value {
    let mut fields = Fields::new();
    put(&mut fields, "id", Some(Value::from(edge.id.as_str())));
    for (end, [node, side, cap]) in [
        (&edge.from, ["fromNode", "fromSide", "fromEnd"]),
        (&edge.to, ["toNode", "toSide", "toEnd"]),
    ] {
        put(&mut fields, node, Some(Value::from(end.node.as_str())));
        put(
            &mut fields,
            side,
            end.side.map(|value| Value::from(name(&Side::ALL, value))),
        );
        put(
            &mut fields,
            cap,
            end.cap.map(|value| Value::from(name(&Cap::ALL, value))),
        );
    }
    put(&mut fields, "color", edge.color.map(color_value));
    put(&mut fields, "label", edge.label.as_deref().map(Value::from));
    fields.extend(edge.extra.clone());
    Value::Object(fields)
}

pub fn write(canvas: &JsonCanvas) -> String {
    let list = |values: Vec<Value>| match values.as_slice() {
        [] => "[]".to_owned(),
        values => {
            let lines: Vec<String> = values.iter().map(|value| format!("\t\t{value}")).collect();
            format!("[\n{}\n\t]", lines.join(",\n"))
        }
    };
    let mut parts = vec![
        format!(
            "\t\"nodes\":{}",
            list(canvas.nodes.iter().map(node_value).collect())
        ),
        format!(
            "\t\"edges\":{}",
            list(canvas.edges.iter().map(edge_value).collect())
        ),
    ];
    parts.extend(
        (canvas.extra.iter())
            .map(|(key, value)| format!("\t{}:{value}", Value::from(key.as_str()))),
    );
    format!("{{\n{}\n}}\n", parts.join(",\n"))
}
