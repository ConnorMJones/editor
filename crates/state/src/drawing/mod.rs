//! Drawings as tabs: the stores of open drawings, and each tab's view, layer, tools and pen.

pub mod command;
pub mod frame;
pub mod pen;

use std::path::PathBuf;
use std::time::Duration;

use ink::brush::{Brush, Curve, Fraction, Paint, Size, WidthFrom};
use ink::doc::{Background, Board, Drawing, GeometryId, LayerIndex, Layers, PageLayer, Paged};
use ink::history::{Edited, Revision};
use ink::input::{Model, WindowParams};
use ink::tool::{ToolKind, Tools};
use model::geom::{Affine, Pane, Point, Surface, Vector};
use model::sheet::{PlaneView, SheetView, Viewport};
use slotmap::{SlotMap, new_key_type};

use crate::State;
use crate::text::buffer::Disk;
use crate::view::layout::PaneId;
use crate::view::tab::{Opened, Tab};

new_key_type! {
    pub struct PagedId;
    pub struct BoardId;
}

pub const PAPER: Background = Background::Plain(model::color::Rgb::hex(0xffffff));

pub type PagedFiles = SlotMap<PagedId, InkFile<Paged>>;
pub type BoardFiles = SlotMap<BoardId, InkFile<Board>>;

/// An open drawing; `saved` is the revision last written.
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

pub enum InkTabs {
    Paged(PagedTab),
    Board(BoardTab),
}

/// A paged document's tab, viewed as pages fitted to the pane.
pub struct PagedTab {
    file: PagedId, // private: a tab's file is its identity
    pub view: SheetView,
    pub hand: Hand<PageLayer>,
}

/// A board's tab, viewed as a plane.
pub struct BoardTab {
    file: BoardId, // private: a tab's file is its identity
    pub view: PlaneView,
    pub hand: Hand<LayerIndex>,
}

pub struct Hand<T> {
    pub layer: LayerIndex,
    pub tip: ToolKind,
    pub tools: Tools<T>,
    pub pen: Pen,
    /// Names the stroke in progress for the host, which keeps its mesh.
    pub live: GeometryId,
    /// The pane's size when last drawn, for keys that move by part of it.
    pub last: Option<Viewport>,
}

#[derive(Clone, Copy, Debug)]
pub enum Pen {
    Up,
    Down(Gesture),
}

/// What is fixed when the pen goes down: the tool, how the pane maps onto the surface, the
/// host's clock then, and how far a tap reaches.
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
            pen: Pen::Up,
            live: GeometryId::next(),
            last: None,
        }
    }

    /// Drops the stroke under way, as when the drawing under it changes by other means.
    pub fn cancel_stroke(&mut self) {
        self.tools.cancel();
        self.pen = Pen::Up;
    }

    /// A hand with the same settings and no stroke in progress.
    pub fn copy(&self) -> Hand<T> {
        Hand {
            tip: self.tip,
            last: self.last,
            tools: Tools::new(
                self.tools.brush.brush,
                self.tools.brush.paint,
                self.tools.eraser.size,
            ),
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

impl BoardTab {
    pub fn new(file: BoardId, view: PlaneView, hand: Hand<LayerIndex>) -> BoardTab {
        BoardTab { file, view, hand }
    }

    pub fn file(&self) -> BoardId {
        self.file
    }
}

impl InkTabs {
    /// A tab on the same drawing at the same place, with no stroke in progress.
    pub fn copy(&self) -> InkTabs {
        match self {
            InkTabs::Paged(tab) => {
                InkTabs::Paged(PagedTab::new(tab.file, tab.view, tab.hand.copy()))
            }
            InkTabs::Board(tab) => {
                InkTabs::Board(BoardTab::new(tab.file, tab.view, tab.hand.copy()))
            }
        }
    }

    pub fn opened(&self) -> Opened {
        match self {
            InkTabs::Paged(tab) => Opened::Paged(tab.file),
            InkTabs::Board(tab) => Opened::Board(tab.file),
        }
    }

    /// Drops the stroke or selection under way, as when the drawing is replaced.
    pub fn cancel_strokes(&mut self) {
        match self {
            InkTabs::Paged(tab) => tab.hand.cancel_stroke(),
            InkTabs::Board(tab) => tab.hand.cancel_stroke(),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InkSettings {
    pub brush: Brush,
    pub paint: Paint,
    pub eraser: Size,
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
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Saved {
    Paged { layer: LayerIndex, view: SheetView },
    Board { layer: LayerIndex, view: PlaneView },
}

/// A new tab on a paged document: its first page fitted to the width.
pub fn open_paged(state: &State, id: PagedId) -> InkTabs {
    let view = SheetView::start(state.pageds[id].edited.drawing().stack());
    InkTabs::Paged(PagedTab::new(
        id,
        view,
        Hand::new(&state.ink, LayerIndex(0)),
    ))
}

/// A new tab on a board, at its origin.
pub fn open_board(state: &State, id: BoardId) -> InkTabs {
    InkTabs::Board(BoardTab::new(
        id,
        PlaneView::START,
        Hand::new(&state.ink, LayerIndex(0)),
    ))
}

/// A restored tab on a drawing, at its saved layer while the drawing still has it; `None` when
/// the saved kind isn't the file's.
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
        (Opened::Text(_), _)
        | (Opened::Paged(_), Saved::Board { .. })
        | (Opened::Board(_), Saved::Paged { .. }) => None,
    }
}

/// The host drew `pane`'s drawing at `viewport`.
pub fn measured(state: &mut State, pane: PaneId, viewport: Viewport) {
    match state
        .layout
        .find_pane(pane)
        .and_then(|pane| pane.active_mut())
    {
        Some(Tab::Ink(InkTabs::Paged(tab))) => tab.hand.last = Some(viewport),
        Some(Tab::Ink(InkTabs::Board(tab))) => tab.hand.last = Some(viewport),
        Some(Tab::Text(_)) | None => {}
    }
}

/// The wheel moved `pane`'s drawing by `by` pixels.
pub fn scrolled(state: &mut State, pane: PaneId, by: Vector<Pane>, viewport: Viewport) {
    let State { layout, pageds, .. } = state;
    match layout.find_pane(pane).and_then(|pane| pane.active_mut()) {
        Some(Tab::Ink(InkTabs::Paged(tab))) => {
            let stack = pageds[tab.file].edited.drawing().stack();
            tab.view.pan(stack, viewport, by)
        }
        Some(Tab::Ink(InkTabs::Board(tab))) => tab.view.pan(by),
        Some(Tab::Text(_)) | None => {}
    }
}

/// The wheel zoomed `pane`'s drawing by `factor` about `at`.
pub fn zoomed(state: &mut State, pane: PaneId, at: Point<Pane>, factor: f32, viewport: Viewport) {
    let State { layout, pageds, .. } = state;
    match layout.find_pane(pane).and_then(|pane| pane.active_mut()) {
        Some(Tab::Ink(InkTabs::Paged(tab))) => {
            let stack = pageds[tab.file].edited.drawing().stack();
            tab.view.zoom_about(stack, viewport, at, factor)
        }
        Some(Tab::Ink(InkTabs::Board(tab))) => tab.view.zoom_about(viewport, at, factor),
        Some(Tab::Text(_)) | None => {}
    }
}
