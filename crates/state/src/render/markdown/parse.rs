//! Parsing buffer text into the block tree, with pulldown-cmark.

use std::collections::HashMap;
use std::ops::Range;
use std::sync::Arc;

use helix_core::RopeSlice;
use model::markdown::CalloutKind;
use model::typeset::Display;
use pulldown_cmark::{
    Alignment, CodeBlockKind, Event, HeadingLevel, LinkType, Options, Parser, Tag, TagEnd,
};

use crate::project::links::{self, Dest, Resolution};

use super::targets::{is_image_path, link_target, locate, percent_decode, vault_image};
use super::{
    Base, Block, BlockKind, Callout, CalloutFold, Code, Column, Embed, EmbedKind, Emphasis, Figure,
    FigureKind, Image, Inline, Level, Link, LinkTarget, ListKind, Markdown, Math, Mermaid, RowKind,
    Span, SpanKind, Task, WikiLink, hash,
};

fn figure(kind: FigureKind, text: &str) -> Figure {
    Figure {
        kind,
        hash: hash(text),
    }
}

pub(crate) const OPTIONS: Options = Options::ENABLE_TABLES
    .union(Options::ENABLE_TASKLISTS)
    .union(Options::ENABLE_STRIKETHROUGH)
    .union(Options::ENABLE_FOOTNOTES)
    .union(Options::ENABLE_MATH)
    .union(Options::ENABLE_WIKILINKS)
    .union(Options::ENABLE_HEADING_ATTRIBUTES)
    .union(Options::ENABLE_YAML_STYLE_METADATA_BLOCKS);

/// What a paragraph holds so far, to lift a lone image, embed or display formula to a block.
enum Sole {
    Empty,
    Math(String),
    Image(Image),
    Embed(Embed),
    Mixed,
}

#[derive(Clone, Copy)]
enum InlineOwner {
    Paragraph,
    /// Text directly inside a tight list item.
    Implicit,
    Heading(Level),
    Cell,
}

/// Raw text collected until a block's end.
#[derive(Clone, Copy)]
enum RawOwner {
    Code,
    Html,
    Frontmatter,
}

enum Open {
    Container(usize),
    Inline {
        owner: InlineOwner,
        start: usize,
    },
    Raw {
        owner: RawOwner,
        start: usize,
        lang: Option<Arc<str>>,
    },
}

struct Capture {
    link_type: LinkType,
    dest: String,
    alt: String,
}

#[derive(Default)]
struct InlineBuf {
    text: String,
    spans: Vec<Span>,
    links: Vec<Link>,
    link: Option<(usize, LinkTarget)>,
    /// Spans do not merge across this byte, where a link starts or ends.
    fence: usize,
}

impl InlineBuf {
    fn push(&mut self, text: &str, style: Emphasis, kind: SpanKind) {
        let start = self.text.len();
        self.text.push_str(text);
        let end = self.text.len();
        match (self.spans.last_mut(), kind) {
            (Some(last), SpanKind::Text)
                if last.kind == SpanKind::Text && last.style == style && start != self.fence =>
            {
                last.range.end = end
            }
            (Some(_), _) | (None, _) => self.spans.push(Span {
                range: start..end,
                style,
                kind,
            }),
        }
    }

    fn take(&mut self) -> Inline {
        self.link = None;
        self.fence = 0;
        Inline {
            text: Arc::from(std::mem::take(&mut self.text).as_str()),
            spans: std::mem::take(&mut self.spans).into_boxed_slice(),
            links: std::mem::take(&mut self.links).into_boxed_slice(),
        }
    }
}

struct Builder<'a> {
    src: &'a str,
    base: Base<'a>,
    /// Byte offset of every line start after the first.
    breaks: Vec<usize>,
    blocks: Vec<Block>,
    open: Vec<Open>,
    inline: InlineBuf,
    sole: Sole,
    style: Emphasis,
    image: Option<Capture>,
    raw: String,
    callouts: u32,
    /// Inline content before this byte is a callout's header line and is dropped.
    skip: usize,
}

impl Builder<'_> {
    fn line(&self, byte: usize) -> usize {
        self.breaks.partition_point(|&start| start <= byte)
    }

    /// Lines of `bytes`, without trailing blank lines.
    fn lines(&self, bytes: &Range<usize>) -> Range<usize> {
        let start = self.line(bytes.start);
        let end = bytes.start + self.src[bytes.clone()].trim_end().len();
        let last = self.line(end.saturating_sub(1).max(bytes.start));
        start..last + 1
    }

    fn push(&mut self, kind: BlockKind, bytes: Range<usize>) -> usize {
        let index = self.blocks.len();
        let lines = self.lines(&bytes);
        self.blocks.push(Block {
            kind,
            bytes,
            lines,
            end: index + 1,
        });
        index
    }

    fn close_container(&mut self, index: usize, end: usize) {
        let end_index = self.blocks.len();
        let block = &mut self.blocks[index];
        block.end = end_index;
        block.bytes.end = end.max(block.bytes.start);
        let bytes = block.bytes.clone();
        self.blocks[index].lines = self.lines(&bytes);
    }

    /// Opens an implicit paragraph for text arriving directly in a container.
    fn ensure_inline(&mut self, at: usize) {
        match self.open.last() {
            Some(Open::Inline { .. } | Open::Raw { .. }) => {}
            Some(Open::Container(_)) | None => {
                self.open.push(Open::Inline {
                    owner: InlineOwner::Implicit,
                    start: at.max(self.skip),
                });
                self.sole = Sole::Empty;
            }
        }
    }

    fn close_implicit(&mut self, end: usize) {
        match self.open.last() {
            Some(Open::Inline {
                owner: InlineOwner::Implicit,
                ..
            }) => self.close_inline(end),
            Some(Open::Inline { .. } | Open::Raw { .. } | Open::Container(_)) | None => {}
        }
    }

    fn close_inline(&mut self, end: usize) {
        let Some(Open::Inline { owner, start }) = self.open.pop() else {
            return;
        };
        let bytes = start..end.max(start);
        let sole = std::mem::replace(&mut self.sole, Sole::Empty);
        let blank = self.inline.text.trim().is_empty();
        let kind = match (owner, sole) {
            (InlineOwner::Heading(level), _) => BlockKind::Heading {
                level,
                text: self.inline.take(),
            },
            (InlineOwner::Cell, _) => BlockKind::Cell(self.inline.take()),
            (InlineOwner::Paragraph | InlineOwner::Implicit, Sole::Math(tex)) => {
                self.inline.take();
                BlockKind::Math(Math {
                    figure: figure(FigureKind::Math(Display::Block), &tex),
                    tex: Arc::from(tex.as_str()),
                    display: Display::Block,
                })
            }
            (InlineOwner::Paragraph | InlineOwner::Implicit, Sole::Image(image)) => {
                self.inline.take();
                BlockKind::Image(image)
            }
            (InlineOwner::Paragraph | InlineOwner::Implicit, Sole::Embed(embed)) => {
                self.inline.take();
                BlockKind::Embed(embed)
            }
            (InlineOwner::Paragraph | InlineOwner::Implicit, Sole::Empty) if blank => {
                self.inline.take();
                return;
            }
            (InlineOwner::Paragraph | InlineOwner::Implicit, Sole::Empty | Sole::Mixed) => {
                BlockKind::Paragraph(self.inline.take())
            }
        };
        self.push(kind, bytes);
    }

    fn mixed(&mut self) {
        self.sole = Sole::Mixed;
    }

    fn text(&mut self, text: &str, range: Range<usize>) {
        match (&mut self.image, self.open.last()) {
            (Some(capture), _) => capture.alt.push_str(text),
            (None, Some(Open::Raw { .. })) => self.raw.push_str(text),
            (None, _) if range.start < self.skip => {}
            (None, _) => {
                self.ensure_inline(range.start);
                match text.trim().is_empty() {
                    true => {}
                    false => self.mixed(),
                }
                self.inline.push(text, self.style, SpanKind::Text);
            }
        }
    }

    fn atom(&mut self, text: &str, kind: SpanKind, range: Range<usize>) {
        match range.start < self.skip {
            true => {}
            false => {
                self.ensure_inline(range.start);
                self.mixed();
                self.inline.push(text, self.style, kind);
            }
        }
    }

    fn start(&mut self, tag: Tag, range: Range<usize>) {
        match &tag {
            Tag::Emphasis
            | Tag::Strong
            | Tag::Strikethrough
            | Tag::Superscript
            | Tag::Subscript
            | Tag::Link { .. }
            | Tag::Image { .. } => {}
            Tag::Paragraph
            | Tag::Heading { .. }
            | Tag::BlockQuote(_)
            | Tag::CodeBlock(_)
            | Tag::HtmlBlock
            | Tag::List(_)
            | Tag::Item
            | Tag::FootnoteDefinition(_)
            | Tag::DefinitionList
            | Tag::DefinitionListTitle
            | Tag::DefinitionListDefinition
            | Tag::Table(_)
            | Tag::TableHead
            | Tag::TableRow
            | Tag::TableCell
            | Tag::MetadataBlock(_) => self.close_implicit(range.start),
        }
        let at = range.start;
        match tag {
            Tag::Paragraph => {
                self.sole = Sole::Empty;
                self.open.push(Open::Inline {
                    owner: InlineOwner::Paragraph,
                    start: at.max(self.skip),
                })
            }
            Tag::Heading { level, .. } => {
                let level = match level {
                    HeadingLevel::H1 => Level::H1,
                    HeadingLevel::H2 => Level::H2,
                    HeadingLevel::H3 => Level::H3,
                    HeadingLevel::H4 => Level::H4,
                    HeadingLevel::H5 => Level::H5,
                    HeadingLevel::H6 => Level::H6,
                };
                self.sole = Sole::Empty;
                self.open.push(Open::Inline {
                    owner: InlineOwner::Heading(level),
                    start: at,
                })
            }
            Tag::TableCell => {
                self.sole = Sole::Empty;
                self.open.push(Open::Inline {
                    owner: InlineOwner::Cell,
                    start: at,
                })
            }
            Tag::BlockQuote(_) => {
                let kind = match self.callout(range.clone()) {
                    Some(callout) => BlockKind::Callout(callout),
                    None => BlockKind::Quote,
                };
                let index = self.push(kind, range);
                self.open.push(Open::Container(index));
            }
            Tag::List(start) => {
                let kind = match start {
                    Some(start) => ListKind::Ordered { start },
                    None => ListKind::Bullet,
                };
                let index = self.push(BlockKind::List(kind), range);
                self.open.push(Open::Container(index));
            }
            Tag::Item => {
                let index = self.push(BlockKind::Item(Task::None), range);
                self.open.push(Open::Container(index));
            }
            Tag::FootnoteDefinition(label) => {
                let kind = BlockKind::Footnote {
                    label: Arc::from(label.as_ref()),
                };
                let index = self.push(kind, range);
                self.open.push(Open::Container(index));
            }
            Tag::Table(columns) => {
                let columns = columns
                    .into_iter()
                    .map(|column| match column {
                        Alignment::None => Column::None,
                        Alignment::Left => Column::Left,
                        Alignment::Center => Column::Center,
                        Alignment::Right => Column::Right,
                    })
                    .collect();
                let index = self.push(BlockKind::Table(columns), range);
                self.open.push(Open::Container(index));
            }
            Tag::TableHead => {
                let index = self.push(BlockKind::Row(RowKind::Head), range);
                self.open.push(Open::Container(index));
            }
            Tag::TableRow => {
                let index = self.push(BlockKind::Row(RowKind::Body), range);
                self.open.push(Open::Container(index));
            }
            Tag::CodeBlock(kind) => {
                let lang = match kind {
                    CodeBlockKind::Fenced(info) => info
                        .split_whitespace()
                        .next()
                        .map(|lang| Arc::from(lang.trim_matches(|c| c == '{' || c == '}'))),
                    CodeBlockKind::Indented => None,
                };
                self.raw.clear();
                self.open.push(Open::Raw {
                    owner: RawOwner::Code,
                    start: at,
                    lang,
                });
            }
            Tag::HtmlBlock => {
                self.raw.clear();
                self.open.push(Open::Raw {
                    owner: RawOwner::Html,
                    start: at,
                    lang: None,
                });
            }
            Tag::MetadataBlock(_) => {
                self.raw.clear();
                self.open.push(Open::Raw {
                    owner: RawOwner::Frontmatter,
                    start: at,
                    lang: None,
                });
            }
            Tag::Emphasis => self.style = self.style.with(Emphasis::ITALIC),
            Tag::Strong => self.style = self.style.with(Emphasis::STRONG),
            Tag::Strikethrough => self.style = self.style.with(Emphasis::STRIKE),
            Tag::Superscript | Tag::Subscript => {}
            Tag::Link {
                link_type,
                dest_url,
                ..
            } => match at < self.skip {
                true => {}
                false => {
                    self.ensure_inline(at);
                    self.mixed();
                    let target = link_target(link_type, &dest_url, None);
                    self.inline.fence = self.inline.text.len();
                    self.inline.link = Some((self.inline.text.len(), target));
                    self.style = self.style.with(Emphasis::LINK);
                }
            },
            Tag::Image {
                link_type,
                dest_url,
                ..
            } => {
                self.ensure_inline(at);
                self.image = Some(Capture {
                    link_type,
                    dest: dest_url.into_string(),
                    alt: String::new(),
                });
            }
            Tag::DefinitionList | Tag::DefinitionListTitle | Tag::DefinitionListDefinition => {}
        }
    }

    fn end(&mut self, tag: TagEnd, range: Range<usize>) {
        match tag {
            TagEnd::Paragraph | TagEnd::Heading(_) | TagEnd::TableCell => {
                self.close_implicit(range.end);
                self.close_inline(range.end)
            }
            TagEnd::BlockQuote(_)
            | TagEnd::List(_)
            | TagEnd::Item
            | TagEnd::FootnoteDefinition
            | TagEnd::Table
            | TagEnd::TableHead
            | TagEnd::TableRow => {
                self.close_implicit(range.end);
                match self.open.pop() {
                    Some(Open::Container(index)) => self.close_container(index, range.end),
                    Some(other) => self.open.push(other),
                    None => {}
                }
            }
            TagEnd::CodeBlock | TagEnd::HtmlBlock | TagEnd::MetadataBlock(_) => {
                let Some(Open::Raw { owner, start, lang }) = self.open.pop() else {
                    return;
                };
                let text = self.raw.strip_suffix('\n').unwrap_or(&self.raw);
                let kind = match (owner, lang.as_deref()) {
                    (RawOwner::Code, Some("mermaid")) => BlockKind::Mermaid(Mermaid {
                        figure: figure(FigureKind::Mermaid, text),
                        src: Arc::from(text),
                    }),
                    (RawOwner::Code, Some("math")) => BlockKind::Math(Math {
                        figure: figure(FigureKind::Math(Display::Block), text),
                        tex: Arc::from(text),
                        display: Display::Block,
                    }),
                    (RawOwner::Code, _) => BlockKind::Code(Code {
                        hash: hash(text) ^ lang.as_deref().map_or(0, hash),
                        lang,
                        text: Arc::from(text),
                    }),
                    (RawOwner::Html, _) => BlockKind::Html(Arc::from(text.trim_end())),
                    (RawOwner::Frontmatter, _) => BlockKind::Frontmatter(Arc::from(text)),
                };
                self.push(kind, start..range.end);
            }
            TagEnd::Emphasis => self.style = self.style.without(Emphasis::ITALIC),
            TagEnd::Strong => self.style = self.style.without(Emphasis::STRONG),
            TagEnd::Strikethrough => self.style = self.style.without(Emphasis::STRIKE),
            TagEnd::Superscript | TagEnd::Subscript => {}
            TagEnd::Link => {
                self.style = self.style.without(Emphasis::LINK);
                match self.inline.link.take() {
                    Some((start, target)) => {
                        let end = self.inline.text.len();
                        let target = match target {
                            LinkTarget::Wiki(link) => {
                                let shown = &self.inline.text[start..end];
                                let alias = match shown == link.target || shown.is_empty() {
                                    true => None,
                                    false => Some(shown.to_owned()),
                                };
                                LinkTarget::Wiki(WikiLink { alias, ..link })
                            }
                            other => other,
                        };
                        // An empty wikilink text shows its target.
                        match (start == end, &target) {
                            (true, LinkTarget::Wiki(link)) => {
                                let shown = link.target.clone();
                                self.inline.push(
                                    &shown,
                                    self.style.with(Emphasis::LINK),
                                    SpanKind::Text,
                                );
                            }
                            (true, _) | (false, _) => {}
                        }
                        let end = self.inline.text.len();
                        let dest = match &target {
                            LinkTarget::Wiki(link) if !link.target.trim().is_empty() => {
                                Some(Dest::Wiki(link.target.as_str()))
                            }
                            LinkTarget::Path { path, .. } => Some(Dest::Path(path.as_str())),
                            LinkTarget::Wiki(_) | LinkTarget::Url(_) | LinkTarget::Heading(_) => {
                                None
                            }
                        };
                        let missing = dest.is_some_and(|dest| {
                            !matches!(
                                links::resolve(self.base.vault, self.base.folder, dest),
                                Resolution::File(_)
                            )
                        });
                        match missing {
                            true => {
                                for span in self.inline.spans.iter_mut().rev() {
                                    match span.range.start >= start {
                                        true => span.style = span.style.with(Emphasis::MISSING),
                                        false => break,
                                    }
                                }
                            }
                            false => {}
                        }
                        self.inline.fence = end;
                        self.inline.links.push(Link {
                            range: start..end,
                            target,
                        });
                    }
                    None => {}
                }
            }
            TagEnd::Image => {
                let Some(capture) = self.image.take() else {
                    return;
                };
                let shown = match capture.alt.is_empty() {
                    true => capture.dest.clone(),
                    false => capture.alt.clone(),
                };
                let lifted = match capture.link_type {
                    LinkType::WikiLink { .. } => {
                        let link = WikiLink::parse(&capture.dest, None);
                        let dest = Dest::Wiki(link.target.as_str());
                        let kind = match is_image_path(&link.target) {
                            true => EmbedKind::Image(vault_image(self.base, dest)),
                            false => EmbedKind::Note(links::resolve(
                                self.base.vault,
                                self.base.folder,
                                dest,
                            )),
                        };
                        let alias = match capture.alt.is_empty() || capture.alt == capture.dest {
                            true => None,
                            false => Some(capture.alt),
                        };
                        Sole::Embed(Embed {
                            link: WikiLink { alias, ..link },
                            kind,
                        })
                    }
                    LinkType::Inline
                    | LinkType::Reference
                    | LinkType::ReferenceUnknown
                    | LinkType::Collapsed
                    | LinkType::CollapsedUnknown
                    | LinkType::Shortcut
                    | LinkType::ShortcutUnknown
                    | LinkType::Autolink
                    | LinkType::Email => Sole::Image(Image {
                        file: locate(&percent_decode(&capture.dest), self.base),
                        src: Arc::from(capture.dest.as_str()),
                        alt: Arc::from(capture.alt.as_str()),
                    }),
                };
                self.sole = match self.sole {
                    Sole::Empty => lifted,
                    Sole::Math(_) | Sole::Image(_) | Sole::Embed(_) | Sole::Mixed => Sole::Mixed,
                };
                self.inline.push(&shown, self.style, SpanKind::Image);
            }
            TagEnd::DefinitionList
            | TagEnd::DefinitionListTitle
            | TagEnd::DefinitionListDefinition => {}
        }
    }

    /// Reads an Obsidian callout header from a quote's first line, and skips that line.
    fn callout(&mut self, range: Range<usize>) -> Option<Callout> {
        let quote = &self.src[range.clone()];
        let line_end = quote.find('\n').map_or(quote.len(), |at| at + 1);
        let first = quote[..line_end].trim_start();
        let rest = first.strip_prefix('>')?.trim_start().strip_prefix("[!")?;
        let (kind, rest) = rest.split_once(']')?;
        let (fold, title) = match rest.as_bytes().first() {
            Some(b'-') => (CalloutFold::Closed, &rest[1..]),
            Some(b'+') => (CalloutFold::Open, &rest[1..]),
            Some(_) | None => (CalloutFold::Static, rest),
        };
        let kind_name = kind.trim();
        let kind = CalloutKind::parse(kind_name);
        let title = match title.trim() {
            "" => match kind {
                CalloutKind::Note if !kind_name.eq_ignore_ascii_case("note") => {
                    let mut chars = kind_name.chars();
                    chars.next().map_or(String::new(), |first| {
                        first.to_uppercase().chain(chars).collect()
                    })
                }
                _ => kind.name().to_owned(),
            },
            title => title.to_owned(),
        };
        self.skip = range.start + line_end;
        let index = self.callouts;
        self.callouts += 1;
        Some(Callout {
            kind,
            title: Arc::from(title.as_str()),
            fold,
            index,
        })
    }

    fn event(&mut self, event: Event, range: Range<usize>) {
        match event {
            Event::Start(tag) => self.start(tag, range),
            Event::End(tag) => self.end(tag, range),
            Event::Text(text) => self.text(&text, range),
            Event::Code(code) => self.atom(&code, SpanKind::Code, range),
            Event::InlineMath(tex) => {
                let figure = figure(FigureKind::Math(Display::Inline), &tex);
                self.atom(&tex, SpanKind::Math(figure), range)
            }
            Event::DisplayMath(tex) => match range.start < self.skip {
                true => {}
                false => {
                    self.ensure_inline(range.start);
                    self.sole = match self.sole {
                        Sole::Empty => Sole::Math(tex.to_string()),
                        Sole::Math(_) | Sole::Image(_) | Sole::Embed(_) | Sole::Mixed => {
                            Sole::Mixed
                        }
                    };
                    let figure = figure(FigureKind::Math(Display::Block), &tex);
                    self.inline.push(&tex, self.style, SpanKind::Math(figure));
                }
            },
            Event::Html(html) => match self.open.last() {
                Some(Open::Raw { .. }) => self.raw.push_str(&html),
                Some(Open::Inline { .. } | Open::Container(_)) | None => {
                    self.atom(&html, SpanKind::Html, range)
                }
            },
            Event::InlineHtml(html) => self.atom(&html, SpanKind::Html, range),
            Event::FootnoteReference(label) => self.atom(&label, SpanKind::Footnote, range),
            Event::SoftBreak => self.text(" ", range),
            Event::HardBreak => self.text("\n", range),
            Event::Rule => {
                self.close_implicit(range.start);
                self.push(BlockKind::Rule, range);
            }
            Event::TaskListMarker(done) => {
                let item = self.open.iter().rev().find_map(|open| match open {
                    Open::Container(index)
                        if matches!(self.blocks[*index].kind, BlockKind::Item(_)) =>
                    {
                        Some(*index)
                    }
                    Open::Container(_) | Open::Inline { .. } | Open::Raw { .. } => None,
                });
                let marker = range.start;
                match item {
                    Some(index) => {
                        self.blocks[index].kind = BlockKind::Item(match done {
                            true => Task::Done { marker },
                            false => Task::Open { marker },
                        })
                    }
                    None => {}
                }
            }
        }
    }
}

/// Parses `text` into blocks; `reuse` lends its buffers and highlight cache to the new parse.
pub fn parse(text: RopeSlice, base: Base, reuse: Option<Markdown>) -> Markdown {
    let (mut blocks, mut roots, mut hashes, mut highlights, mut source) = match reuse {
        Some(old) => (
            old.blocks,
            old.roots,
            old.hashes,
            old.highlights,
            old.source,
        ),
        None => (
            Vec::new(),
            Vec::new(),
            Vec::new(),
            HashMap::new(),
            String::new(),
        ),
    };
    hashes.clear();
    blocks.clear();
    roots.clear();
    source.clear();
    source.reserve(text.len_bytes());
    for chunk in text.chunks() {
        source.push_str(chunk);
    }
    let src = source.as_str();
    let mut builder = Builder {
        src,
        base,
        breaks: src
            .bytes()
            .enumerate()
            .filter(|(_, byte)| *byte == b'\n')
            .map(|(at, _)| at + 1)
            .collect(),
        blocks,
        open: Vec::new(),
        inline: InlineBuf::default(),
        sole: Sole::Empty,
        style: Emphasis::NONE,
        image: None,
        raw: String::new(),
        callouts: 0,
        skip: 0,
    };
    for (event, range) in Parser::new_ext(src, OPTIONS).into_offset_iter() {
        builder.event(event, range);
    }
    while let Some(open) = builder.open.pop() {
        match open {
            Open::Container(index) => builder.close_container(index, src.len()),
            inline @ Open::Inline { .. } => {
                builder.open.push(inline);
                builder.close_inline(src.len());
            }
            Open::Raw { .. } => {}
        }
    }
    let blocks = builder.blocks;
    let mut at = 0;
    while at < blocks.len() {
        roots.push(at);
        hashes.push(hash(&src[blocks[at].bytes.clone()]));
        at = blocks[at].end;
    }
    let mut live: Vec<u64> = blocks
        .iter()
        .filter_map(|block| match &block.kind {
            BlockKind::Code(code) => Some(code.hash),
            _ => None,
        })
        .collect();
    live.sort_unstable();
    highlights.retain(|hash, _| live.binary_search(hash).is_ok());
    Markdown {
        blocks,
        roots,
        hashes,
        highlights,
        source,
    }
}
