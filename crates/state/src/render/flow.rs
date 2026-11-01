use std::collections::HashMap;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::ops::Range;
use std::path::Path;
use std::sync::Arc;

use model::command::Direction;
use model::markdown::CalloutKind;
use model::picture::Encoding;
use model::theme::Style;
use model::typeset::Display;
use model::units::Px;

use crate::project::links::Resolution;
use crate::render::document::TopItem;

pub struct Flow<O: Origin> {
    pub blocks: Vec<Block<O>>,
    pub roots: Vec<usize>,
    pub hashes: Vec<u64>,
    pub highlights: HashMap<u64, Box<[Run]>>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Block<O: Origin> {
    pub kind: BlockKind<O>,
    pub origin: O,
    pub end: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Lines {
    pub bytes: Range<usize>,
    pub lines: Range<usize>,
}

pub trait Origin: Clone + std::fmt::Debug + PartialEq {
    type Target: Clone + std::fmt::Debug + PartialEq;
    type Task: Clone + Copy + std::fmt::Debug + PartialEq;
    type Own: OwnBlock;
}

pub trait OwnBlock: Clone + std::fmt::Debug + PartialEq {
    fn figure(&self) -> Option<(Figure, &str)>;

    fn image(&self) -> Option<&LocalImage>;
}

impl OwnBlock for NoteBlock {
    fn figure(&self) -> Option<(Figure, &str)> {
        match self {
            NoteBlock::Math(math) => Some((math.figure, &math.tex)),
            NoteBlock::Mermaid(mermaid) => Some((mermaid.figure, &mermaid.src)),
            NoteBlock::Callout(_)
            | NoteBlock::Embed(_)
            | NoteBlock::Html(_)
            | NoteBlock::Footnote { .. }
            | NoteBlock::Frontmatter(_) => None,
        }
    }

    fn image(&self) -> Option<&LocalImage> {
        match self {
            NoteBlock::Embed(Embed {
                kind: EmbedKind::Image(ImageFile::Local(local)),
                ..
            }) => Some(local),
            NoteBlock::Embed(_)
            | NoteBlock::Callout(_)
            | NoteBlock::Math(_)
            | NoteBlock::Mermaid(_)
            | NoteBlock::Html(_)
            | NoteBlock::Footnote { .. }
            | NoteBlock::Frontmatter(_) => None,
        }
    }
}

impl OwnBlock for std::convert::Infallible {
    fn figure(&self) -> Option<(Figure, &str)> {
        match *self {}
    }

    fn image(&self) -> Option<&LocalImage> {
        match *self {}
    }
}

impl Origin for Lines {
    type Target = LinkTarget;
    type Task = Task;
    type Own = NoteBlock;
}

#[derive(Clone, Debug, PartialEq)]
pub enum BlockKind<O: Origin> {
    Heading {
        level: Level,
        text: Inline<O::Target>,
    },
    Paragraph(Inline<O::Target>),
    /// Holds `Item`s.
    List(ListKind),
    /// Holds blocks; a tight item's text arrives as a paragraph.
    Item(O::Task),
    Quote,
    Code(Code),
    /// Holds `Row`s; the first one is the header.
    Table(Box<[Column]>),
    Row(RowKind),
    Cell(Inline<O::Target>),
    Image(Image),
    Rule,
    Own(O::Own),
}

#[derive(Clone, Debug, PartialEq)]
pub enum NoteBlock {
    Callout(Callout),
    Math(Math),
    Mermaid(Mermaid),
    /// `![[...]]` on its own line.
    Embed(Embed),
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

impl Callout {
    pub fn shown(&self, toggled: &[u32]) -> CalloutFold {
        match (self.fold, toggled.binary_search(&self.index).is_ok()) {
            (CalloutFold::Static, _) => CalloutFold::Static,
            (fold, false) => fold,
            (CalloutFold::Open, true) => CalloutFold::Closed,
            (CalloutFold::Closed, true) => CalloutFold::Open,
        }
    }
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
    Local(LocalImage),
    Remote(Arc<str>),
    Missing,
    NotPicture,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum ImagePlace {
    File(Arc<Path>),
    Zipped { archive: Arc<Path>, name: Arc<str> },
}

impl ImagePlace {
    pub fn file(&self) -> &Path {
        match self {
            ImagePlace::File(path) => path,
            ImagePlace::Zipped { archive, .. } => archive,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct LocalImage {
    pub place: ImagePlace,
    pub encoding: Encoding,
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
#[derive(Clone, Debug, PartialEq)]
pub struct Inline<T = LinkTarget> {
    pub text: Arc<str>,
    /// Sorted, non-overlapping and covering `text`.
    pub spans: Box<[Span]>,
    /// Sorted and non-overlapping.
    pub links: Box<[Link<T>]>,
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
    pub const UNDERLINE: Emphasis = Emphasis(32);
    pub const RAISED: Emphasis = Emphasis(64);
    pub const LOWERED: Emphasis = Emphasis(128);

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
pub struct Link<T = LinkTarget> {
    pub range: Range<usize>,
    pub target: T,
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

pub struct InlineBuf<T = LinkTarget> {
    pub text: String,
    pub spans: Vec<Span>,
    pub links: Vec<Link<T>>,
    pub link: Option<(usize, T)>,
    pub fence: usize,
}

impl<T> Default for InlineBuf<T> {
    fn default() -> InlineBuf<T> {
        InlineBuf {
            text: String::new(),
            spans: Vec::new(),
            links: Vec::new(),
            link: None,
            fence: 0,
        }
    }
}

impl<T> Default for Inline<T> {
    fn default() -> Inline<T> {
        Inline {
            text: Arc::from(""),
            spans: Box::new([]),
            links: Box::new([]),
        }
    }
}

impl<T> InlineBuf<T> {
    pub fn push(&mut self, text: &str, style: Emphasis, kind: SpanKind) {
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

    pub fn take(&mut self) -> Inline<T> {
        self.link = None;
        self.fence = 0;
        Inline {
            text: Arc::from(std::mem::take(&mut self.text).as_str()),
            spans: std::mem::take(&mut self.spans).into_boxed_slice(),
            links: std::mem::take(&mut self.links).into_boxed_slice(),
        }
    }
}

impl Flow<Lines> {
    /// Index of the top-level item holding source `line`, or the nearest one before it.
    pub fn item_at_line(&self, line: usize) -> usize {
        let after = self
            .roots
            .partition_point(|&root| self.blocks[root].origin.lines.start <= line);
        after.saturating_sub(1)
    }

    /// First source line of top-level item `item`.
    pub fn item_line(&self, item: usize) -> Option<usize> {
        let root = *self.roots.get(item)?;
        Some(self.blocks[root].origin.lines.start)
    }
}

impl<O: Origin> Flow<O> {
    pub fn heading(&self, at: TopItem, direction: Direction) -> Option<usize> {
        let heading = |item: &usize| {
            matches!(
                self.blocks[self.roots[*item]].kind,
                BlockKind::Heading { .. }
            )
        };
        match direction {
            Direction::Forward => (at.item + 1..self.roots.len()).find(heading),
            Direction::Backward => {
                let before = match at.offset > Px(0.0) {
                    true => at.item + 1,
                    false => at.item,
                };
                (0..before.min(self.roots.len())).rev().find(heading)
            }
        }
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

impl<O: Origin> Block<O> {
    pub fn text(&self) -> Option<&str> {
        match &self.kind {
            BlockKind::Code(code) => Some(&code.text),
            BlockKind::Heading { text, .. }
            | BlockKind::Paragraph(text)
            | BlockKind::Cell(text) => Some(&text.text),
            BlockKind::List(_)
            | BlockKind::Item(_)
            | BlockKind::Quote
            | BlockKind::Table(_)
            | BlockKind::Row(_)
            | BlockKind::Image(_)
            | BlockKind::Rule
            | BlockKind::Own(_) => None,
        }
    }

    /// The block's own text, for blocks that hold inline content.
    pub fn inline(&self) -> Option<&Inline<O::Target>> {
        match &self.kind {
            BlockKind::Heading { text, .. } => Some(text),
            BlockKind::Paragraph(text) | BlockKind::Cell(text) => Some(text),
            BlockKind::List(_)
            | BlockKind::Item(_)
            | BlockKind::Quote
            | BlockKind::Code(_)
            | BlockKind::Table(_)
            | BlockKind::Row(_)
            | BlockKind::Image(_)
            | BlockKind::Rule
            | BlockKind::Own(_) => None,
        }
    }
}

pub fn hash(text: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    text.hash(&mut hasher);
    hasher.finish()
}
