//! Drawings as tabs: the stores of open drawings, and each tab's view, layer, tools and pen.

pub mod command;
pub mod embed;
pub mod frame;
pub mod markup;
pub mod pdf_text;
pub mod pen;
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
use crate::reading::{Found, Notes, Reading};
pub use library::TextSpan;
use model::geom::{Affine, Pane, Surface};
use model::notice::Notice;
use model::sheet::{Measure, PlaneView, SheetView, Stack};
use reader::pdf::Document;
use slotmap::{SlotMap, new_key_type};

use crate::State;
use crate::text::buffer::Disk;
use crate::view::tab::{Opened, Tab};

new_key_type! {
    pub struct PagedId;
    pub struct BoardId;
    pub struct PdfId;
}

pub const PAPER: Background = Background::Plain(model::color::Rgb::WHITE);

pub type PagedFiles = SlotMap<PagedId, InkFile<Paged>>;
pub type BoardFiles = SlotMap<BoardId, InkFile<Board>>;
pub type PdfFiles = SlotMap<PdfId, PdfFile>;

pub struct PdfFile {
    pub ink: InkFile<Paged>,
    document: Arc<Document>, // private: must stay the underlay of `ink`; only `new`, `replace` and `flattened` set both
    on_disk: Arc<Document>,  // private: the bytes the file holds, which the next save appends to
    pub notes: Notes,
}

pub struct PdfRead {
    pub on_disk: Arc<Document>,
    pub drawn: Arc<Document>,
    pub ink: Paged,
    pub notes: Notes,
    pub said: Option<Notice>,
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
            ink: InkFile::new(path, read.ink, disk),
            document: read.drawn,
            on_disk: read.on_disk,
            notes: read.notes,
        }
    }

    pub fn on_disk(&self) -> &Arc<Document> {
        &self.on_disk
    }

    pub fn saved(&mut self, on_disk: Arc<Document>, revision: Revision, disk: Disk) {
        self.on_disk = on_disk;
        self.ink.saved = revision;
        self.ink.disk = disk;
    }

    pub fn flattened(&mut self, on_disk: Arc<Document>, disk: Disk) {
        self.document = on_disk.clone();
        self.ink.saved = self.ink.edited.replace(Paged::over_pdf(on_disk.clone()));
        self.on_disk = on_disk;
        self.ink.disk = disk;
    }

    pub fn pages(&self) -> &Stack {
        self.ink.edited.drawing().stack()
    }

    pub fn document(&self) -> &Arc<Document> {
        &self.document
    }

    pub fn replace(&mut self, read: PdfRead) -> Revision {
        self.document = read.drawn;
        self.on_disk = read.on_disk;
        self.notes = read.notes;
        self.ink.edited.replace(read.ink)
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

#[derive(Debug, PartialEq, Eq)]
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

impl ResumeIn {
    pub fn mode(self) -> PdfMode {
        match self {
            ResumeIn::Read => PdfMode::Read(Reading::Idle),
            ResumeIn::Markup => PdfMode::Markup,
        }
    }
}

impl PdfMode {
    pub fn resume(&self) -> ResumeIn {
        match self {
            PdfMode::Read(_) => ResumeIn::Read,
            PdfMode::Markup => ResumeIn::Markup,
        }
    }
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
                let mode = tab.mode.resume().mode();
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
                tab.mode = tab.mode.resume().mode();
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
    let stack = file.pages();
    let mut view = SheetView::start(stack);
    match &file.notes {
        Notes::Listed(listed) => view.go_to(stack, PageIndex(listed.position.part.0), None),
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
            Some(Tab::Ink(InkTabs::Pdf(PdfTab::new(
                id,
                view,
                hand,
                mode.mode(),
            ))))
        }
        (Opened::Text(_) | Opened::Image(_) | Opened::Book(_) | Opened::JsonCanvas(_), _)
        | (Opened::Paged(_), Saved::Board { .. } | Saved::Pdf { .. })
        | (Opened::Board(_), Saved::Paged { .. } | Saved::Pdf { .. })
        | (Opened::Pdf(_), Saved::Paged { .. } | Saved::Board { .. }) => None,
    }
}
