//! Drawings as tabs: the stores of open drawings, and each tab's view, layer, tools and pen.

pub mod command;
pub mod embed;
pub mod frame;
pub mod pen;
pub mod reading;
pub mod typing;

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use ink::brush::{Brush, Curve, Fraction, Paint, Size, WidthFrom};
use ink::doc::{
    Background, Board, Drawing, GeometryId, LayerIndex, Layers, PageIndex, PageLayer, Paged,
};
use ink::history::{Edited, Revision};
use ink::input::{Model, WindowParams};
use ink::text::{FontFamily, TextStyle};
use ink::tool::{Gather, Picked, ToolKind, Tools, Using, still};

use crate::drawing::typing::Open;
pub use library::TextSpan;
use library::{Annotation, Change, DocumentId, Highlight, Library, Notebook};
use model::geom::{Affine, Pane, Surface};
use model::notice::Notice;
use model::sheet::{Measure, PlaneView, SheetView, Stack};
use reader::pdf::Document;
use slotmap::{SlotMap, new_key_type};

use crate::job::Job;
use crate::text::buffer::Disk;
use crate::view::tab::{Opened, Tab};
use crate::{Effect, State};

new_key_type! {
    pub struct PagedId;
    pub struct BoardId;
    pub struct PdfId;
}

pub const PAPER: Background = Background::Plain(model::color::Rgb::hex(0xffffff));

pub type PagedFiles = SlotMap<PagedId, InkFile<Paged>>;
pub type BoardFiles = SlotMap<BoardId, InkFile<Board>>;
pub type PdfFiles = SlotMap<PdfId, PdfFile>;

pub struct PdfFile {
    pub ink: InkFile<Paged>,
    document: Arc<Document>, // private: must stay the underlay of `ink`; only `new` and `replace` set both
    pub notes: Notes,
}

pub struct PdfRead {
    pub document: Arc<Document>,
    pub notes: Notes,
    pub said: Option<Notice>,
}

pub enum Notes {
    Listed(Listed),
    Unlisted(Unlisted),
}

pub struct Listed {
    pub library: Library,
    pub document: DocumentId,
    pub notebook: Notebook,
    pub position: PageIndex,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Unlisted {
    Outside,
    Failed(String),
}

impl Notes {
    pub fn listed(&self) -> Result<&Listed, Notice> {
        match self {
            Notes::Listed(listed) => Ok(listed),
            Notes::Unlisted(Unlisted::Outside) => Err(Notice::NotInLibrary),
            Notes::Unlisted(Unlisted::Failed(reason)) => Err(Notice::LibraryFailed {
                reason: reason.clone(),
            }),
        }
    }
}

impl Listed {
    pub fn write(&self, file: PdfId, change: Change) -> Effect {
        Effect::Job(Job::Annotate {
            library: self.library.clone(),
            file,
            document: self.document,
            change,
        })
    }
}

pub struct InkFile<D> {
    pub path: PathBuf,
    pub edited: Edited<D>,
    pub disk: Disk,
    pub saved: Revision,
}

impl<D: Drawing> InkFile<D> {
    pub fn new(path: PathBuf, drawing: D, disk: Disk) -> InkFile<D> {
        let edited = Edited::new(drawing);
        let saved = edited.revision();
        InkFile {
            path,
            edited,
            disk,
            saved,
        }
    }

    pub fn is_modified(&self) -> bool {
        self.edited.revision() != self.saved
    }
}

impl PdfFile {
    pub fn new(path: PathBuf, read: PdfRead, disk: Disk) -> PdfFile {
        PdfFile {
            ink: InkFile::new(path, Paged::over_pdf(read.document.clone()), disk),
            document: read.document,
            notes: read.notes,
        }
    }

    pub fn document(&self) -> &Arc<Document> {
        &self.document
    }

    pub fn replace(&mut self, read: PdfRead) -> Revision {
        self.document = read.document.clone();
        self.notes = read.notes;
        self.ink.edited.replace(Paged::over_pdf(read.document))
    }
}

pub enum InkTabs {
    Paged(PagedTab),
    Board(BoardTab),
    Pdf(PdfTab),
}

pub struct PagedTab {
    file: PagedId, // private: a tab's file is its identity
    pub view: SheetView,
    pub hand: Hand<PageLayer>,
}

pub struct PdfTab {
    file: PdfId, // private: a tab's file is its identity
    pub view: SheetView,
    pub hand: Hand<PageLayer>,
    pub mode: PdfMode,
    pub found: Found,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Found {
    Nothing,
    Searching {
        query: String,
    },
    /// `hits` is never empty; `at` is the one shown.
    Hits {
        query: String,
        hits: Vec<TextSpan>,
        at: usize,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PdfMode {
    Read(Reading),
    Markup,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ResumeIn {
    Read,
    Markup,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Reading {
    Idle,
    Selecting {
        page: PageIndex,
        anchor: usize,
        head: usize,
    },
    Selected(TextSpan),
    OnHighlight(Highlight),
}

pub struct BoardTab {
    file: BoardId, // private: a tab's file is its identity
    pub view: PlaneView,
    pub hand: Hand<LayerIndex>,
}

pub struct Hand<T> {
    pub layer: LayerIndex,
    pub tip: ToolKind,
    pub tools: Tools,
    pub doing: Doing<T>,
    pub style: TextStyle,
    pub live: GeometryId,
    pub last: Option<Measure>,
}

pub enum Doing<T> {
    Ink { using: Using<T>, pen: Pen },
    Typing(Box<Open<T>>),
}

impl<T> Doing<T> {
    pub const IDLE: Doing<T> = Doing::Ink {
        using: Using::Idle,
        pen: Pen::Up,
    };
}

#[derive(Clone, Copy, Debug)]
pub enum Pen {
    Up,
    Down(Gesture),
}

#[derive(Clone, Copy, Debug)]
pub struct Gesture {
    pub tool: ToolKind,
    pub onto: Affine<Pane, Surface>,
    pub start: Duration,
    pub reach: Size,
}

impl<T: Copy + PartialEq> Hand<T> {
    pub fn new(settings: &InkSettings, layer: LayerIndex) -> Hand<T> {
        Hand {
            layer,
            tip: ToolKind::Brush,
            tools: Tools::new(settings.brush, settings.paint, settings.eraser),
            doing: Doing::IDLE,
            style: settings.text.clone(),
            live: GeometryId::next(),
            last: None,
        }
    }

    pub fn cancel_stroke(&mut self) {
        match self.doing {
            Doing::Ink { .. } => self.doing = Doing::IDLE,
            Doing::Typing(_) => {}
        }
    }

    pub fn pick(&mut self, target: T, picked: Picked) {
        match self.doing {
            Doing::Ink { .. } => {
                self.tip = ToolKind::Select(Gather::Box);
                self.doing = Doing::Ink {
                    using: still(target, picked),
                    pen: Pen::Up,
                };
            }
            Doing::Typing(_) => {}
        }
    }

    pub fn copy(&self) -> Hand<T> {
        Hand {
            tip: self.tip,
            style: self.style.clone(),
            last: self.last,
            tools: Tools::new(self.tools.brush, self.tools.paint, self.tools.eraser),
            ..Hand::new(&InkSettings::default(), self.layer)
        }
    }
}

impl PagedTab {
    pub fn new(file: PagedId, view: SheetView, hand: Hand<PageLayer>) -> PagedTab {
        PagedTab { file, view, hand }
    }

    pub fn file(&self) -> PagedId {
        self.file
    }
}

impl PdfTab {
    pub fn new(file: PdfId, view: SheetView, hand: Hand<PageLayer>, mode: PdfMode) -> PdfTab {
        PdfTab {
            file,
            view,
            hand,
            mode,
            found: Found::Nothing,
        }
    }

    pub fn file(&self) -> PdfId {
        self.file
    }

    pub fn show_page(&mut self, pages: &Stack, page: PageIndex) {
        let viewport = self.hand.last.map(|measure| measure.viewport);
        self.view.go_to(pages, page, viewport);
    }

    pub fn show(&mut self, pages: &Stack, annotation: Annotation) {
        self.show_page(pages, annotation.place().0);
        self.mode = PdfMode::Read(match annotation {
            Annotation::Highlight(highlight) => Reading::OnHighlight(highlight),
            Annotation::Lost(_) | Annotation::Bookmark(_) => Reading::Idle,
        });
    }
}

impl BoardTab {
    pub fn new(file: BoardId, view: PlaneView, hand: Hand<LayerIndex>) -> BoardTab {
        BoardTab { file, view, hand }
    }

    pub fn file(&self) -> BoardId {
        self.file
    }
}

impl InkTabs {
    pub fn copy(&self) -> InkTabs {
        match self {
            InkTabs::Paged(tab) => {
                InkTabs::Paged(PagedTab::new(tab.file, tab.view, tab.hand.copy()))
            }
            InkTabs::Board(tab) => {
                InkTabs::Board(BoardTab::new(tab.file, tab.view, tab.hand.copy()))
            }
            InkTabs::Pdf(tab) => {
                let mode = match tab.mode {
                    PdfMode::Read(_) => PdfMode::Read(Reading::Idle),
                    PdfMode::Markup => PdfMode::Markup,
                };
                InkTabs::Pdf(PdfTab::new(tab.file, tab.view, tab.hand.copy(), mode))
            }
        }
    }

    pub fn opened(&self) -> Opened {
        match self {
            InkTabs::Paged(tab) => Opened::Paged(tab.file),
            InkTabs::Board(tab) => Opened::Board(tab.file),
            InkTabs::Pdf(tab) => Opened::Pdf(tab.file),
        }
    }

    pub fn reloaded(&mut self) {
        match self {
            InkTabs::Paged(tab) => tab.hand.cancel_stroke(),
            InkTabs::Board(tab) => tab.hand.cancel_stroke(),
            InkTabs::Pdf(tab) => {
                tab.hand.cancel_stroke();
                tab.found = Found::Nothing;
                tab.mode = match tab.mode {
                    PdfMode::Read(_) => PdfMode::Read(Reading::Idle),
                    PdfMode::Markup => PdfMode::Markup,
                };
            }
        }
    }
}

#[derive(Clone, Debug)]
pub struct InkSettings {
    pub brush: Brush,
    pub paint: Paint,
    pub eraser: Size,
    pub text: TextStyle,
}

impl Default for InkSettings {
    fn default() -> InkSettings {
        InkSettings {
            brush: Brush {
                size: Size::new(1.5).expect("a positive size"),
                width: WidthFrom::Pressure {
                    curve: Curve::Sqrt,
                    thinnest: Fraction::new(0.4).expect("a fraction"),
                },
                model: Model::SlidingWindow(WindowParams::DEFAULT),
            },
            paint: Paint {
                color: model::theme::Color::opaque(model::color::Rgb::hex(0x1f2328)),
            },
            eraser: Size::new(12.0).expect("a positive size"),
            text: TextStyle {
                family: FontFamily::new(model::fonts::SANS).expect("a family name"),
                size: Size::new(12.0).expect("a positive size"),
            },
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Saved {
    Paged {
        layer: LayerIndex,
        view: SheetView,
    },
    Board {
        layer: LayerIndex,
        view: PlaneView,
    },
    Pdf {
        layer: LayerIndex,
        view: SheetView,
        mode: ResumeIn,
    },
}

pub fn open_paged(state: &State, id: PagedId) -> InkTabs {
    let view = SheetView::start(state.pageds[id].edited.drawing().stack());
    InkTabs::Paged(PagedTab::new(
        id,
        view,
        Hand::new(&state.ink, LayerIndex(0)),
    ))
}

pub fn open_pdf(state: &State, id: PdfId) -> InkTabs {
    let file = &state.pdfs[id];
    let stack = file.ink.edited.drawing().stack();
    let mut view = SheetView::start(stack);
    match &file.notes {
        Notes::Listed(listed) => view.go_to(stack, listed.position, None),
        Notes::Unlisted(_) => {}
    }
    InkTabs::Pdf(PdfTab::new(
        id,
        view,
        Hand::new(&state.ink, LayerIndex(0)),
        PdfMode::Read(Reading::Idle),
    ))
}

pub fn open_board(state: &State, id: BoardId) -> InkTabs {
    InkTabs::Board(BoardTab::new(
        id,
        PlaneView::START,
        Hand::new(&state.ink, LayerIndex(0)),
    ))
}

pub fn reopen(state: &State, opened: Opened, saved: Saved) -> Option<Tab> {
    let at = |layers: &Layers, layer: LayerIndex| match layers.get(layer) {
        Some(_) => layer,
        None => LayerIndex(0),
    };
    match (opened, saved) {
        (Opened::Paged(id), Saved::Paged { layer, view }) => {
            let layer = at(state.pageds[id].edited.drawing().layers(), layer);
            let hand = Hand::new(&state.ink, layer);
            Some(Tab::Ink(InkTabs::Paged(PagedTab::new(id, view, hand))))
        }
        (Opened::Board(id), Saved::Board { layer, view }) => {
            let layer = at(state.boards[id].edited.drawing().layers(), layer);
            let hand = Hand::new(&state.ink, layer);
            Some(Tab::Ink(InkTabs::Board(BoardTab::new(id, view, hand))))
        }
        (Opened::Pdf(id), Saved::Pdf { layer, view, mode }) => {
            let layer = at(state.pdfs[id].ink.edited.drawing().layers(), layer);
            let hand = Hand::new(&state.ink, layer);
            let mode = match mode {
                ResumeIn::Read => PdfMode::Read(Reading::Idle),
                ResumeIn::Markup => PdfMode::Markup,
            };
            Some(Tab::Ink(InkTabs::Pdf(PdfTab::new(id, view, hand, mode))))
        }
        (Opened::Text(_) | Opened::Image(_), _)
        | (Opened::Paged(_), Saved::Board { .. } | Saved::Pdf { .. })
        | (Opened::Board(_), Saved::Paged { .. } | Saved::Pdf { .. })
        | (Opened::Pdf(_), Saved::Paged { .. } | Saved::Board { .. }) => None,
    }
}
