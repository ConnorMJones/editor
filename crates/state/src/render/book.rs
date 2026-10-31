use std::collections::HashMap;
use std::ops::Range;
use std::path::Path;
use std::sync::Arc;

use library::{Part, PartText, Place, TextSpan, Texts};
use model::href::{Href, percent_decode};
use model::picture::Encoding;
use model::units::{Pt, Px};
use reader::epub::{self, Book, Spot};
use roxmltree::Node;
use simplecss::{AttributeOperator, DeclarationTokenizer, PseudoClass, StyleSheet};

use crate::render::flow::{
    Block, BlockKind, Code, Column, Emphasis, Flow, Image, ImageFile, ImagePlace, InlineBuf, Level,
    Link, ListKind, LocalImage, Origin, RowKind, SpanKind, Task, hash,
};

pub const LEAST_SCALE: f32 = 0.6;
pub const MOST_SCALE: f32 = 2.5;
pub const MOST_MARGIN: f32 = 8.0;
pub const MOST_INDENT: f32 = 8.0;
pub const PT_PER_EM: f32 = 12.0;

pub struct BookFlow {
    pub flow: Flow<InChapter>,
    pub starts: Box<[usize]>,
    pub anchors: HashMap<(usize, Arc<str>), usize>,
    pub texts: Arc<BookTexts>,
}

pub struct BookTexts {
    pub chapters: Vec<Arc<ChapterText>>,
}

#[derive(Default)]
pub struct ChapterText {
    pub text: String,
    pub blocks: Vec<TextBlock>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TextBlock {
    pub block: usize,
    pub start: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub struct InChapter {
    pub chapter: usize,
    pub layout: Layout,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Layout {
    pub align: Align,
    pub indent: f32,
    pub margin: f32,
    pub scale: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Align {
    Start,
    Center,
    End,
    Justify,
}

impl Layout {
    pub const PLAIN: Layout = Layout {
        align: Align::Start,
        indent: 0.0,
        margin: 0.0,
        scale: 1.0,
    };
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BookTarget {
    Url(String),
    Spot(Spot),
}

impl Origin for InChapter {
    type Target = BookTarget;
}

#[derive(Clone, Copy)]
struct Style {
    display: Display,
    emphasis: Emphasis,
    layout: Layout,
    face: Face,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Display {
    Block,
    Inline,
    Hidden,
}

#[derive(Clone, Copy)]
enum Container {
    Quote,
    List(ListKind),
    Item,
    Table,
    Row,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Face {
    Prose,
    Code,
}

#[derive(Clone, Copy)]
enum Role {
    Heading(Level),
    Plain,
    Container(Container),
    Cell,
    Pre,
    Image,
    Break,
    Rule,
    Inline,
    Hidden,
}

#[derive(Clone, Copy)]
enum Length {
    Em(f32),
    Rem(f32),
    Percent(f32),
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Within {
    Flow,
    Text,
}

struct Chapter<'a> {
    index: usize,
    path: &'a str,
    file: &'a Arc<Path>,
    book: &'a Book,
    chapters: &'a HashMap<&'a str, usize>,
    rules: Vec<&'a simplecss::Rule<'a>>,
}

struct Builder {
    blocks: Vec<Block<InChapter>>,
    open: Vec<usize>,
    inline: InlineBuf<BookTarget>,
    items: usize,
    anchors: HashMap<(usize, Arc<str>), usize>,
    waiting: Vec<Arc<str>>,
}

#[derive(Clone, Copy)]
struct Css<'a, 'input>(Node<'a, 'input>);

impl simplecss::Element for Css<'_, '_> {
    fn parent_element(&self) -> Option<Self> {
        self.0.parent_element().map(Css)
    }

    fn prev_sibling_element(&self) -> Option<Self> {
        self.0.prev_sibling_element().map(Css)
    }

    fn has_local_name(&self, name: &str) -> bool {
        self.0.tag_name().name().eq_ignore_ascii_case(name)
    }

    fn attribute_matches(&self, local_name: &str, operator: AttributeOperator<'_>) -> bool {
        (self.0.attributes())
            .find(|attribute| attribute.name() == local_name)
            .is_some_and(|attribute| operator.matches(attribute.value()))
    }

    fn pseudo_class_matches(&self, class: PseudoClass<'_>) -> bool {
        match class {
            PseudoClass::FirstChild => self.0.prev_sibling_element().is_none(),
            PseudoClass::Link
            | PseudoClass::Visited
            | PseudoClass::Hover
            | PseudoClass::Active
            | PseudoClass::Focus
            | PseudoClass::Lang(_) => false,
        }
    }
}

impl Role {
    fn of(node: Node<'_, '_>) -> Role {
        match node.tag_name().name().to_ascii_lowercase().as_str() {
            "h1" => Role::Heading(Level::H1),
            "h2" => Role::Heading(Level::H2),
            "h3" => Role::Heading(Level::H3),
            "h4" => Role::Heading(Level::H4),
            "h5" => Role::Heading(Level::H5),
            "h6" => Role::Heading(Level::H6),
            "blockquote" => Role::Container(Container::Quote),
            "ul" => Role::Container(Container::List(ListKind::Bullet)),
            "ol" => Role::Container(Container::List(ListKind::Ordered {
                start: (node.attribute("start"))
                    .and_then(|start| start.trim().parse().ok())
                    .unwrap_or(1),
            })),
            "li" => Role::Container(Container::Item),
            "table" => Role::Container(Container::Table),
            "tr" => Role::Container(Container::Row),
            "td" | "th" => Role::Cell,
            "pre" => Role::Pre,
            "img" | "image" => Role::Image,
            "br" => Role::Break,
            "hr" => Role::Rule,
            "head" | "script" | "style" | "title" | "meta" | "link" | "noscript" | "template" => {
                Role::Hidden
            }
            "p" | "div" | "section" | "article" | "aside" | "nav" | "header" | "footer"
            | "main" | "figure" | "figcaption" | "address" | "center" | "dl" | "dt" | "dd"
            | "body" | "html" | "hgroup" | "details" | "summary" | "caption" | "thead"
            | "tbody" | "tfoot" | "svg" => Role::Plain,
            _ => Role::Inline,
        }
    }
}

impl Length {
    fn parse(value: &str) -> Option<Length> {
        let value = value.trim();
        let number = |unit: &str| value.strip_suffix(unit)?.trim().parse::<f32>().ok();
        match value {
            "0" => Some(Length::Rem(0.0)),
            _ => (number("rem").map(Length::Rem))
                .or_else(|| number("em").map(Length::Em))
                .or_else(|| number("ex").map(|ex| Length::Em(ex / 2.0)))
                .or_else(|| number("px").map(|px| Length::Rem(Pt::from(Px(px)).0 / PT_PER_EM)))
                .or_else(|| number("pt").map(|pt| Length::Rem(pt / PT_PER_EM)))
                .or_else(|| number("%").map(Length::Percent))
                .filter(|length| match length {
                    Length::Em(number) | Length::Rem(number) | Length::Percent(number) => {
                        number.is_finite()
                    }
                }),
        }
    }

    fn ems(self) -> Option<f32> {
        match self {
            Length::Em(ems) | Length::Rem(ems) => Some(ems),
            Length::Percent(_) => None,
        }
    }
}

impl Style {
    const ROOT: Style = Style {
        display: Display::Block,
        emphasis: Emphasis::NONE,
        layout: Layout::PLAIN,
        face: Face::Prose,
    };

    fn of(node: Node<'_, '_>, role: Role, parent: &Style, rules: &[&simplecss::Rule<'_>]) -> Style {
        let tag = node.tag_name().name().to_ascii_lowercase();
        let emphasis = match tag.as_str() {
            "em" | "i" | "cite" | "var" | "dfn" => parent.emphasis.with(Emphasis::ITALIC),
            "strong" | "b" => parent.emphasis.with(Emphasis::STRONG),
            "u" | "ins" => parent.emphasis.with(Emphasis::UNDERLINE),
            "s" | "strike" | "del" => parent.emphasis.with(Emphasis::STRIKE),
            "sup" => parent.emphasis.with(Emphasis::RAISED),
            "sub" => parent.emphasis.with(Emphasis::LOWERED),
            _ => parent.emphasis,
        };
        let mut style = Style {
            display: match role {
                Role::Hidden => Display::Hidden,
                Role::Inline | Role::Break | Role::Image => Display::Inline,
                Role::Heading(_)
                | Role::Plain
                | Role::Container(_)
                | Role::Cell
                | Role::Pre
                | Role::Rule => Display::Block,
            },
            emphasis,
            layout: Layout {
                align: match tag.as_str() {
                    "center" => Align::Center,
                    _ => parent.layout.align,
                },
                ..parent.layout
            },
            face: match tag.as_str() {
                "code" | "kbd" | "samp" | "tt" => Face::Code,
                _ => parent.face,
            },
        };
        let mut margin = 0.0;
        let css = Css(node);
        let matched = (rules.iter())
            .filter(|rule| rule.selector.matches(&css))
            .flat_map(|rule| {
                rule.declarations
                    .iter()
                    .map(|declaration| (declaration.name, declaration.value))
            });
        let inline = (node.attribute("style").into_iter())
            .flat_map(DeclarationTokenizer::from)
            .map(|declaration| (declaration.name, declaration.value));
        for (name, value) in matched.chain(inline) {
            let value = value.trim().to_ascii_lowercase();
            let words = || value.split_whitespace();
            match name {
                "display" => match value.as_str() {
                    "none" => style.display = Display::Hidden,
                    "inline" | "inline-block" => style.display = Display::Inline,
                    "block" | "list-item" | "table" | "flex" | "grid" => {
                        style.display = Display::Block
                    }
                    _ => {}
                },
                "font-weight" => match value.as_str() {
                    "bold" | "bolder" | "600" | "700" | "800" | "900" => {
                        style.emphasis = style.emphasis.with(Emphasis::STRONG)
                    }
                    "normal" | "lighter" | "100" | "200" | "300" | "400" | "500" => {
                        style.emphasis = style.emphasis.without(Emphasis::STRONG)
                    }
                    _ => {}
                },
                "font-style" => match value.as_str() {
                    "italic" | "oblique" => style.emphasis = style.emphasis.with(Emphasis::ITALIC),
                    "normal" => style.emphasis = style.emphasis.without(Emphasis::ITALIC),
                    _ => {}
                },
                "text-decoration" | "text-decoration-line" => {
                    for word in words() {
                        match word {
                            "underline" => {
                                style.emphasis = style.emphasis.with(Emphasis::UNDERLINE)
                            }
                            "line-through" => {
                                style.emphasis = style.emphasis.with(Emphasis::STRIKE)
                            }
                            "none" => {
                                style.emphasis = (style.emphasis)
                                    .without(Emphasis::UNDERLINE)
                                    .without(Emphasis::STRIKE)
                            }
                            _ => {}
                        }
                    }
                }
                "vertical-align" => match value.as_str() {
                    "super" => style.emphasis = style.emphasis.with(Emphasis::RAISED),
                    "sub" => style.emphasis = style.emphasis.with(Emphasis::LOWERED),
                    "baseline" => {
                        style.emphasis = (style.emphasis)
                            .without(Emphasis::RAISED)
                            .without(Emphasis::LOWERED)
                    }
                    _ => {}
                },
                "text-align" => match value.as_str() {
                    "left" | "start" => style.layout.align = Align::Start,
                    "center" => style.layout.align = Align::Center,
                    "right" | "end" => style.layout.align = Align::End,
                    "justify" => style.layout.align = Align::Justify,
                    _ => {}
                },
                "text-indent" => match Length::parse(&value).and_then(Length::ems) {
                    Some(ems) => style.layout.indent = ems.clamp(-MOST_INDENT, MOST_INDENT),
                    None => {}
                },
                "margin-left" => match Length::parse(&value).and_then(Length::ems) {
                    Some(ems) => margin = ems,
                    None => {}
                },
                "margin" => {
                    let sides: Vec<&str> = words().collect();
                    let left = match sides.as_slice() {
                        [all] => Some(*all),
                        [_, sides] | [_, sides, _] => Some(*sides),
                        [_, _, _, left] => Some(*left),
                        _ => None,
                    };
                    match left.and_then(Length::parse).and_then(Length::ems) {
                        Some(ems) => margin = ems,
                        None => {}
                    }
                }
                "font-size" => {
                    let from = parent.layout.scale;
                    let scale = match value.as_str() {
                        "xx-small" => Some(0.6),
                        "x-small" => Some(0.75),
                        "small" => Some(0.89),
                        "medium" => Some(1.0),
                        "large" => Some(1.2),
                        "x-large" => Some(1.5),
                        "xx-large" => Some(2.0),
                        "smaller" => Some(from / 1.2),
                        "larger" => Some(from * 1.2),
                        value => Length::parse(value).map(|length| match length {
                            Length::Em(ems) => from * ems,
                            Length::Rem(ems) => ems,
                            Length::Percent(percent) => from * percent / 100.0,
                        }),
                    };
                    match scale {
                        Some(scale) => style.layout.scale = scale.clamp(LEAST_SCALE, MOST_SCALE),
                        None => {}
                    }
                }
                _ => {}
            }
        }
        style.layout.margin = (parent.layout.margin + margin).clamp(0.0, MOST_MARGIN);
        style
    }
}

impl Builder {
    fn push(&mut self, kind: BlockKind<BookTarget>, origin: InChapter) -> usize {
        let index = self.blocks.len();
        match self.open.as_slice() {
            [] => self.items += 1,
            [_, ..] => {}
        }
        let item = self.items - 1;
        for anchor in self.waiting.drain(..) {
            self.anchors.entry((origin.chapter, anchor)).or_insert(item);
        }
        self.blocks.push(Block {
            kind,
            origin,
            end: index + 1,
        });
        index
    }

    fn open(&mut self, kind: BlockKind<BookTarget>, origin: InChapter) {
        let index = self.push(kind, origin);
        self.open.push(index);
    }

    fn close(&mut self) {
        match self.open.pop() {
            Some(index) => self.blocks[index].end = self.blocks.len(),
            None => {}
        }
    }

    fn trim_end(&mut self) {
        let kept = self.inline.text.trim_end().len();
        self.inline.text.truncate(kept);
        let spans = (self.inline.spans).partition_point(|span| span.range.start < kept);
        self.inline.spans.truncate(spans);
        let links = (self.inline.links).partition_point(|link| link.range.start < kept);
        self.inline.links.truncate(links);
        match self.inline.spans.last_mut() {
            Some(span) => span.range.end = span.range.end.min(kept),
            None => {}
        }
        match self.inline.links.last_mut() {
            Some(link) => link.range.end = link.range.end.min(kept),
            None => {}
        }
    }

    fn flush(&mut self, chapter: usize, layout: Layout) {
        self.trim_end();
        match self.inline.text.is_empty() {
            true => {
                self.inline.take();
            }
            false => {
                let text = self.inline.take();
                self.push(BlockKind::Paragraph(text), InChapter { chapter, layout });
            }
        }
    }

    fn text(&mut self, text: &str, style: &Style) {
        let kind = match style.face {
            Face::Prose => SpanKind::Text,
            Face::Code => SpanKind::Code,
        };
        for (at, word) in text.split(|c: char| c.is_ascii_whitespace()).enumerate() {
            match (at, self.inline.text.chars().last()) {
                (0, _) | (_, None | Some(' ' | '\n')) => {}
                (_, Some(_)) => self.inline.push(" ", style.emphasis, kind),
            }
            match word {
                "" => {}
                word => self.inline.push(word, style.emphasis, kind),
            }
        }
    }
}

impl Chapter<'_> {
    fn target(&self, href: &str) -> Option<BookTarget> {
        match Href::parse(href) {
            Href::Url(url) => Some(BookTarget::Url(url.to_owned())),
            Href::Local { path, anchor } => {
                let chapter = match path {
                    "" => Some(self.index),
                    path => (self.chapters)
                        .get(epub::resolve(self.path, path)?.as_str())
                        .copied(),
                }?;
                Some(BookTarget::Spot(Spot {
                    chapter,
                    anchor: anchor.map(|anchor| percent_decode(anchor).into_boxed_str()),
                }))
            }
        }
    }

    fn image(&self, node: Node<'_, '_>) -> Option<Image> {
        let src = (node.attribute("src"))
            .or_else(|| {
                (node.attributes())
                    .find(|attribute| attribute.name() == "href")
                    .map(|attribute| attribute.value())
            })?
            .trim();
        let name =
            (epub::resolve(self.path, src)).filter(|name| self.book.archive.entry(name).is_some());
        let file = match name
            .as_deref()
            .map(|name| (name, Encoding::of_path(Path::new(name))))
        {
            None => ImageFile::Missing,
            Some((_, None)) => ImageFile::NotPicture,
            Some((name, Some(encoding))) => ImageFile::Local(LocalImage {
                place: ImagePlace::Zipped {
                    archive: self.file.clone(),
                    name: Arc::from(name),
                },
                encoding,
            }),
        };
        Some(Image {
            src: Arc::from(src),
            alt: Arc::from(node.attribute("alt").unwrap_or_default()),
            file,
        })
    }

    fn walk(
        &self,
        node: Node<'_, '_>,
        parent: &Style,
        within: Within,
        here: Layout,
        out: &mut Builder,
    ) {
        match node.node_type() {
            roxmltree::NodeType::Text => {
                return match node.text() {
                    Some(text) => out.text(text, parent),
                    None => {}
                };
            }
            roxmltree::NodeType::Element => {}
            roxmltree::NodeType::Root | roxmltree::NodeType::Comment | roxmltree::NodeType::PI => {
                return;
            }
        }
        let role = Role::of(node);
        let mut style = Style::of(node, role, parent, &self.rules);
        let role = match (style.display, role) {
            (Display::Hidden, _) => return,
            (Display::Inline, Role::Plain) => Role::Inline,
            (Display::Block, Role::Inline) => Role::Plain,
            (Display::Inline | Display::Block, role) => role,
        };
        match node.attribute("id") {
            Some(id) => out.waiting.push(Arc::from(id)),
            None => {}
        }
        let chapter = self.index;
        let link = match (node.tag_name().name(), node.attribute("href")) {
            ("a", Some(href)) => match self.target(href.trim()) {
                Some(target) => {
                    style.emphasis = style.emphasis.with(Emphasis::LINK);
                    Some(target)
                }
                None => {
                    style.emphasis = style.emphasis.with(Emphasis::MISSING);
                    None
                }
            },
            _ => None,
        };
        let children = |within: Within, here: Layout, out: &mut Builder| {
            for child in node.children() {
                self.walk(child, &style, within, here, out);
            }
        };
        match (within, role) {
            (_, Role::Hidden) => {}
            (Within::Text, Role::Image) => match self.image(node) {
                Some(image) => out.inline.push(&image.alt, style.emphasis, SpanKind::Image),
                None => {}
            },
            (_, Role::Break) => match out.inline.text.is_empty() {
                true => {}
                false => {
                    out.trim_end();
                    out.inline.push("\n", style.emphasis, SpanKind::Text)
                }
            },
            (_, Role::Inline) => {
                let start = out.inline.text.len();
                children(within, here, out);
                match link {
                    Some(target) if out.inline.text.len() > start => {
                        let end = out.inline.text.trim_end().len().max(start);
                        out.inline.fence = end;
                        out.inline.links.push(Link {
                            range: start..end,
                            target,
                        })
                    }
                    Some(_) | None => {}
                }
            }
            (Within::Text, _) => {
                out.text(" ", &style);
                children(Within::Text, here, out);
                out.text(" ", &style);
            }
            (Within::Flow, Role::Image) => {
                out.flush(chapter, here);
                match self.image(node) {
                    Some(image) => {
                        out.push(
                            BlockKind::Image(image),
                            InChapter {
                                chapter,
                                layout: style.layout,
                            },
                        );
                    }
                    None => {}
                }
            }
            (Within::Flow, Role::Rule) => {
                out.flush(chapter, here);
                out.push(
                    BlockKind::Rule,
                    InChapter {
                        chapter,
                        layout: style.layout,
                    },
                );
            }
            (Within::Flow, Role::Pre) => {
                out.flush(chapter, here);
                let text: String = (node.descendants())
                    .filter(|node| node.is_text())
                    .filter_map(|node| node.text())
                    .collect();
                let text = text.trim_end_matches(['\n', '\r']);
                out.push(
                    BlockKind::Code(Code {
                        lang: None,
                        hash: hash(text),
                        text: Arc::from(text),
                    }),
                    InChapter {
                        chapter,
                        layout: style.layout,
                    },
                );
            }
            (Within::Flow, Role::Heading(level)) => {
                out.flush(chapter, here);
                children(Within::Text, style.layout, out);
                out.trim_end();
                match out.inline.text.is_empty() {
                    true => {
                        out.inline.take();
                    }
                    false => {
                        let text = out.inline.take();
                        out.push(
                            BlockKind::Heading { level, text },
                            InChapter {
                                chapter,
                                layout: style.layout,
                            },
                        );
                    }
                }
            }
            (Within::Flow, Role::Cell) => {
                out.flush(chapter, here);
                children(Within::Text, style.layout, out);
                out.trim_end();
                let text = out.inline.take();
                out.push(
                    BlockKind::Cell(text),
                    InChapter {
                        chapter,
                        layout: style.layout,
                    },
                );
            }
            (Within::Flow, Role::Plain) => {
                out.flush(chapter, here);
                children(Within::Flow, style.layout, out);
                out.flush(chapter, style.layout);
            }
            (Within::Flow, Role::Container(container)) => {
                out.flush(chapter, here);
                let kind = match container {
                    Container::Quote => BlockKind::Quote,
                    Container::List(kind) => BlockKind::List(kind),
                    Container::Item => BlockKind::Item(Task::None),
                    Container::Table => BlockKind::Table(Box::new([])),
                    Container::Row => BlockKind::Row(
                        match (node.parent_element()).map(|parent| parent.tag_name().name()) {
                            Some("thead") => RowKind::Head,
                            Some(_) | None => RowKind::Body,
                        },
                    ),
                };
                let index = out.blocks.len();
                out.open(
                    kind,
                    InChapter {
                        chapter,
                        layout: style.layout,
                    },
                );
                children(Within::Flow, style.layout, out);
                out.flush(chapter, style.layout);
                out.close();
                match container {
                    Container::Table => {
                        let widest = (index + 1..out.blocks[index].end)
                            .filter(|row| matches!(out.blocks[*row].kind, BlockKind::Row(_)))
                            .map(|row| {
                                (row + 1..out.blocks[row].end)
                                    .filter(|cell| {
                                        matches!(out.blocks[*cell].kind, BlockKind::Cell(_))
                                    })
                                    .count()
                            })
                            .max()
                            .unwrap_or(0);
                        out.blocks[index].kind =
                            BlockKind::Table(vec![Column::None; widest].into());
                    }
                    Container::Quote | Container::List(_) | Container::Item | Container::Row => {}
                }
            }
        }
    }
}

pub fn convert(book: &Book, file: &Arc<Path>) -> Option<BookFlow> {
    let _span = tracing::info_span!("book convert", chapters = book.chapters.len()).entered();
    let chapters: HashMap<&str, usize> = (book.chapters.iter().enumerate())
        .map(|(at, chapter)| (&*chapter.path, at))
        .collect();
    let sheets: HashMap<&str, StyleSheet<'_>> = (book.styles.iter())
        .map(|(path, text)| (&**path, StyleSheet::parse(text)))
        .collect();
    let mut out = Builder {
        blocks: Vec::new(),
        open: Vec::new(),
        inline: InlineBuf::default(),
        items: 0,
        anchors: HashMap::new(),
        waiting: Vec::new(),
    };
    let mut starts = Vec::with_capacity(book.chapters.len());
    for (index, chapter) in book.chapters.iter().enumerate() {
        starts.push(out.items);
        let parsed = epub::parse(&chapter.path, &chapter.xhtml, |root| {
            let inline: Vec<StyleSheet<'_>> = (root.descendants())
                .filter(|node| node.tag_name().name() == "style")
                .filter_map(|node| node.text())
                .map(StyleSheet::parse)
                .collect();
            let mut inline = inline.iter();
            let mut rules: Vec<&simplecss::Rule<'_>> = (root.descendants())
                .filter_map(|node| match node.tag_name().name() {
                    "link" => (node.attribute("href"))
                        .and_then(|href| sheets.get(epub::resolve(&chapter.path, href)?.as_str())),
                    "style" => inline.next(),
                    _ => None,
                })
                .flat_map(|sheet| sheet.rules.iter())
                .collect();
            rules.sort_by_key(|rule| rule.selector.specificity());
            let walker = Chapter {
                index,
                path: &chapter.path,
                file,
                book,
                chapters: &chapters,
                rules,
            };
            let body = (root.children())
                .find(|node| node.tag_name().name() == "body")
                .unwrap_or(root);
            walker.walk(body, &Style::ROOT, Within::Flow, Layout::PLAIN, &mut out);
            out.flush(index, Layout::PLAIN);
            out.waiting.clear();
        });
        match parsed {
            Ok(()) => {}
            Err(error) => {
                out.inline
                    .push(&error.to_string(), Emphasis::MISSING, SpanKind::Text);
                out.flush(index, Layout::PLAIN);
            }
        }
    }
    let last = out.items.checked_sub(1)?;
    let starts = (starts.into_iter()).map(|start| start.min(last)).collect();
    let blocks = out.blocks;
    let mut roots = Vec::with_capacity(out.items);
    let mut at = 0;
    while at < blocks.len() {
        roots.push(at);
        at = blocks[at].end;
    }
    let mut chapters: Vec<ChapterText> = (book.chapters.iter())
        .map(|_| ChapterText::default())
        .collect();
    for (index, block) in blocks.iter().enumerate() {
        match block.text() {
            Some(text) => {
                let chapter = &mut chapters[block.origin.chapter];
                match chapter.blocks.is_empty() {
                    true => {}
                    false => chapter.text.push('\n'),
                }
                chapter.blocks.push(TextBlock {
                    block: index,
                    start: chapter.text.len(),
                });
                chapter.text.push_str(text);
            }
            None => {}
        }
    }
    Some(BookFlow {
        flow: Flow {
            hashes: (0..roots.len() as u64).collect(),
            blocks,
            roots,
            highlights: HashMap::new(),
        },
        starts,
        anchors: out.anchors,
        texts: Arc::new(BookTexts {
            chapters: chapters.into_iter().map(Arc::new).collect(),
        }),
    })
}

impl BookFlow {
    pub fn item(&self, spot: &Spot) -> usize {
        let start = self.starts[spot.chapter];
        (spot.anchor.as_deref())
            .and_then(|anchor| self.anchors.get(&(spot.chapter, Arc::from(anchor))))
            .copied()
            .unwrap_or(start)
    }
}

impl BookFlow {
    pub fn place_of(&self, item: usize) -> Place {
        let root = self.flow.roots[item];
        let chapter = self.flow.blocks[root].origin.chapter;
        let text = &self.texts.chapters[chapter];
        let next = text.blocks.partition_point(|block| block.block < root);
        Place {
            part: Part(chapter),
            char: (text.blocks.get(next)).map_or(text.text.len(), |block| block.start),
        }
    }

    pub fn item_at(&self, at: Place) -> usize {
        let start = self.starts[at.part.0.min(self.starts.len() - 1)];
        let text = (self.texts.chapters.get(at.part.0)).and_then(|text| {
            let within = text.blocks.partition_point(|block| block.start <= at.char);
            text.blocks.get(within.checked_sub(1)?)
        });
        match text {
            Some(found) => self.flow.roots.partition_point(|root| *root <= found.block) - 1,
            None => start,
        }
    }

    pub fn point(&self, block: usize, byte: usize) -> Option<Place> {
        let chapter = self.flow.blocks.get(block)?.origin.chapter;
        let text = &self.texts.chapters[chapter];
        let found = text.blocks[text
            .blocks
            .binary_search_by_key(&block, |at| at.block)
            .ok()?];
        Some(Place {
            part: Part(chapter),
            char: found.start + byte,
        })
    }

    pub fn within(&self, block: usize, span: &TextSpan) -> Option<Range<usize>> {
        let text = self.texts.chapters.get(span.part.0)?;
        let found = text.blocks[text
            .blocks
            .binary_search_by_key(&block, |at| at.block)
            .ok()?];
        let length = self.flow.blocks[block].text()?.len();
        let start = span.chars.start.max(found.start);
        let end = span.chars.end.min(found.start + length);
        (start < end).then(|| start - found.start..end - found.start)
    }
}

impl PartText for ChapterText {
    fn text(&self) -> &str {
        &self.text
    }

    fn byte(&self, char: usize) -> usize {
        char.min(self.text.len())
    }

    fn chars_in(&self, bytes: Range<usize>) -> Range<usize> {
        bytes
    }

    fn after(&self, char: usize) -> usize {
        (self.text.get(char..).and_then(|rest| rest.chars().next()))
            .map_or(self.text.len(), |next| char + next.len_utf8())
    }
}

impl Texts for BookTexts {
    fn parts(&self) -> usize {
        self.chapters.len()
    }

    fn part(&self, part: Part) -> Option<Arc<dyn PartText + Send + Sync>> {
        let text: Arc<dyn PartText + Send + Sync> = self.chapters.get(part.0)?.clone();
        Some(text)
    }
}
