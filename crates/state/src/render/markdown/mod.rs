//! Markdown and typst documents as the rendered view shows them.

mod highlight;
mod parse;
mod targets;

pub use highlight::{highlight, highlight_code};
pub(crate) use parse::OPTIONS;
pub use parse::parse;
pub(crate) use targets::link_target;
pub use targets::locate;

use std::collections::HashMap;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use model::markdown::CalloutKind;
use model::theme::Style;
use model::typeset::Display;

use crate::project::links::{Index, Resolution};

/// What a rendered view can show, by file extension.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format {
    Markdown,
    Typst,
}

impl Format {
    pub fn of(path: &Path) -> Option<Format> {
        match path.extension().and_then(|ext| ext.to_str()) {
            Some("md" | "markdown") => Some(Format::Markdown),
            Some("typ") => Some(Format::Typst),
            Some(_) | None => None,
        }
    }
}

pub struct Markdown {
    /// Pre-order: a block's descendants follow it, up to its `end`.
    pub blocks: Vec<Block>,
    /// Top-level blocks, one per rendered list item, in order.
    pub roots: Vec<usize>,
    /// Hash of each root's source, parallel to `roots`; equal hashes keep measured heights.
    pub hashes: Vec<u64>,
    /// Highlight runs of code blocks by `Code::hash`, carried over between parses.
    pub highlights: HashMap<u64, Box<[Run]>>,
    /// Copy of the buffer text the blocks were parsed from, reused by the next parse.
    pub source: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Block {
    pub kind: BlockKind,
    /// Byte range in the source.
    pub bytes: Range<usize>,
    /// Zero-based source lines, end exclusive.
    pub lines: Range<usize>,
    /// Index past the block's last descendant.
    pub end: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub enum BlockKind {
    Heading {
        level: Level,
        text: Inline,
    },
    Paragraph(Inline),
    /// Holds `Item`s.
    List(ListKind),
    /// Holds blocks; a tight item's text arrives as a paragraph.
    Item(Task),
    Quote,
    Callout(Callout),
    Code(Code),
    Math(Math),
    Mermaid(Mermaid),
    /// Holds `Row`s; the first one is the header.
    Table(Box<[Column]>),
    Row(RowKind),
    Cell(Inline),
    Image(Image),
    /// `![[...]]` on its own line.
    Embed(Embed),
    Rule,
    /// Raw HTML, shown as code.
    Html(Arc<str>),
    /// A footnote definition holding blocks.
    Footnote {
        label: Arc<str>,
    },
    /// YAML front matter, shown as code.
    Frontmatter(Arc<str>),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum Level {
    H1 = 1,
    H2,
    H3,
    H4,
    H5,
    H6,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ListKind {
    Bullet,
    Ordered { start: u64 },
}

/// A list item's checkbox; the marker is the byte offset of its `[`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Task {
    None,
    Open { marker: usize },
    Done { marker: usize },
}

/// An Obsidian callout: a quote whose first line is `[!kind]`, optionally `+`/`-` foldable.
#[derive(Clone, Debug, PartialEq)]
pub struct Callout {
    pub kind: CalloutKind,
    /// The text after the marker, or the kind's name when empty.
    pub title: Arc<str>,
    pub fold: CalloutFold,
    /// Position among the document's callouts; keys a view's fold toggles.
    pub index: u32,
}

/// Whether a callout can fold, and its fold before the view toggles it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CalloutFold {
    Static,
    /// `[!kind]+`
    Open,
    /// `[!kind]-`
    Closed,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Code {
    /// The first word of the fence's info string.
    pub lang: Option<Arc<str>>,
    /// Without the final line break.
    pub text: Arc<str>,
    /// Keys `Markdown::highlights`.
    pub hash: u64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Math {
    pub tex: Arc<str>,
    pub display: Display,
    pub figure: Figure,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Mermaid {
    pub src: Arc<str>,
    pub figure: Figure,
}

/// A typeset figure by content: keys `State::figures`, with the look it is drawn in.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Figure {
    pub kind: FigureKind,
    pub hash: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FigureKind {
    Math(Display),
    Mermaid,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Column {
    None,
    Left,
    Center,
    Right,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RowKind {
    Head,
    Body,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Image {
    /// As written, before resolution.
    pub src: Arc<str>,
    pub alt: Arc<str>,
    pub file: ImageFile,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ImageFile {
    Local(PathBuf),
    Remote(Arc<str>),
    Missing,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Embed {
    pub link: WikiLink,
    pub kind: EmbedKind,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EmbedKind {
    Image(ImageFile),
    /// A note or other file, shown as a link to it.
    Note(Resolution),
}

/// Text with typed styling spans and links over byte ranges of `text`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Inline {
    pub text: Arc<str>,
    /// Sorted, non-overlapping and covering `text`.
    pub spans: Box<[Span]>,
    /// Sorted and non-overlapping.
    pub links: Box<[Link]>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Span {
    pub range: Range<usize>,
    pub style: Emphasis,
    pub kind: SpanKind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpanKind {
    Text,
    Code,
    Math(Figure),
    Html,
    /// `[^label]`; the text is the label.
    Footnote,
    /// An image within other text; the text is its alt text or source.
    Image,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Emphasis(u8);

impl Emphasis {
    pub const NONE: Emphasis = Emphasis(0);
    pub const STRONG: Emphasis = Emphasis(1);
    pub const ITALIC: Emphasis = Emphasis(2);
    pub const STRIKE: Emphasis = Emphasis(4);
    pub const LINK: Emphasis = Emphasis(8);
    /// A link to a note or file the vault does not have.
    pub const MISSING: Emphasis = Emphasis(16);

    pub const fn contains(self, other: Emphasis) -> bool {
        self.0 & other.0 == other.0
    }

    pub const fn with(self, other: Emphasis) -> Emphasis {
        Emphasis(self.0 | other.0)
    }

    pub const fn without(self, other: Emphasis) -> Emphasis {
        Emphasis(self.0 & !other.0)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Link {
    pub range: Range<usize>,
    pub target: LinkTarget,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LinkTarget {
    /// Has a scheme, like `https:` or `mailto:`.
    Url(String),
    /// A file path as written, percent-decoded, with an optional `#heading`.
    Path {
        path: String,
        heading: Option<String>,
    },
    /// `#heading` within the same note.
    Heading(String),
    Wiki(WikiLink),
}

/// `[[target#heading|alias]]`; `target` is empty for `[[#heading]]`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WikiLink {
    pub target: String,
    pub heading: Option<String>,
    pub alias: Option<String>,
}

impl WikiLink {
    pub fn parse(dest: &str, alias: Option<String>) -> WikiLink {
        let (target, heading) = match dest.split_once('#') {
            Some((target, heading)) => (target, Some(heading.trim().to_owned())),
            None => (dest, None),
        };
        WikiLink {
            target: target.trim().to_owned(),
            heading,
            alias,
        }
    }
}

/// A style change in a code block: `style` holds up to byte `end` of the text.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Run {
    pub end: usize,
    pub style: Style,
}

impl Markdown {
    /// Index of the top-level item holding source `line`, or the nearest one before it.
    pub fn item_at_line(&self, line: usize) -> usize {
        let after = self
            .roots
            .partition_point(|&root| self.blocks[root].lines.start <= line);
        after.saturating_sub(1)
    }

    /// First source line of top-level item `item`.
    pub fn item_line(&self, item: usize) -> Option<usize> {
        let root = *self.roots.get(item)?;
        Some(self.blocks[root].lines.start)
    }

    /// Children of block `index`, in order.
    pub fn children(&self, index: usize) -> impl Iterator<Item = usize> + '_ {
        let end = self.blocks[index].end;
        let mut next = index + 1;
        std::iter::from_fn(move || match next < end {
            true => {
                let child = next;
                next = self.blocks[child].end;
                Some(child)
            }
            false => None,
        })
    }
}

impl Block {
    /// The block's own text, for blocks that hold inline content.
    pub fn inline(&self) -> Option<&Inline> {
        match &self.kind {
            BlockKind::Heading { text, .. } => Some(text),
            BlockKind::Paragraph(text) | BlockKind::Cell(text) => Some(text),
            BlockKind::List(_)
            | BlockKind::Item(_)
            | BlockKind::Quote
            | BlockKind::Callout(_)
            | BlockKind::Code(_)
            | BlockKind::Math(_)
            | BlockKind::Mermaid(_)
            | BlockKind::Table(_)
            | BlockKind::Row(_)
            | BlockKind::Image(_)
            | BlockKind::Embed(_)
            | BlockKind::Rule
            | BlockKind::Html(_)
            | BlockKind::Footnote { .. }
            | BlockKind::Frontmatter(_) => None,
        }
    }
}

/// Images resolve from the note's directory, then the vault root, then by vault resolution.
#[derive(Clone, Copy)]
pub struct Base<'a> {
    pub dir: &'a Path,
    pub root: &'a Path,
    /// The note's folder within the vault; empty at the root and for notes outside it.
    pub folder: &'a str,
    pub vault: &'a Index,
}

pub fn hash(text: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    text.hash(&mut hasher);
    hasher.finish()
}
