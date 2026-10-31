use model::command::{ViewCommand, ZoomBy};
use model::geom::{Pane, Point, Vector};
use model::notice::Notice;
use model::scroll_bar::Thumb;
use model::sheet::{Measure, PlaneView, SheetView, Stack, Viewport, Zoom};
use model::window::Side;

use crate::State;
use crate::drawing::{InkTabs, PagedFiles, PagedId, PdfFiles, PdfId};
use crate::render::pages::Compiles;
use crate::text::buffer::BufferId;
use crate::view::layout::PaneId;
use crate::view::tab::{Showing, ShowingMut, Tab};
use crate::viewer::{ImageFiles, ImageId};

pub const PAN_STEP: f32 = 0.125;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Wheel {
    Pan(Vector<Pane>),
    Zoom { at: Point<Pane>, factor: f32 },
}

pub enum Viewed<'a> {
    Pages {
        view: &'a mut SheetView,
        stack: &'a Stack,
        last: &'a mut Option<Measure>,
    },
    Plane {
        view: &'a mut PlaneView,
        last: &'a mut Option<Measure>,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stacked {
    Paged(PagedId),
    Pdf(PdfId),
    Image(ImageId),
    Typst(BufferId),
}

pub enum Sheeted<'a> {
    Pages {
        view: &'a SheetView,
        last: Option<Measure>,
        of: Stacked,
    },
    Plane {
        view: &'a PlaneView,
        last: Option<Measure>,
    },
}

pub enum SheetedMut<'a> {
    Pages {
        view: &'a mut SheetView,
        last: &'a mut Option<Measure>,
        of: Stacked,
    },
    Plane {
        view: &'a mut PlaneView,
        last: &'a mut Option<Measure>,
    },
}

#[derive(Clone, Copy)]
pub struct Stacks<'a> {
    pub pageds: &'a PagedFiles,
    pub pdfs: &'a PdfFiles,
    pub images: &'a ImageFiles,
    pub compiles: &'a Compiles,
}

impl<'a> Stacks<'a> {
    pub fn get(self, of: Stacked) -> Option<&'a Stack> {
        match of {
            Stacked::Paged(id) => Some(self.pageds[id].edited.drawing().stack()),
            Stacked::Pdf(id) => Some(self.pdfs[id].pages()),
            Stacked::Image(id) => Some(&self.images[id].stack),
            Stacked::Typst(buffer) => Some(&self.compiles.get(buffer)?.pages.as_ref()?.stack),
        }
    }
}

impl Tab {
    pub fn sheet(&self) -> Option<Sheeted<'_>> {
        match self {
            Tab::Ink(InkTabs::Paged(tab)) => Some(Sheeted::Pages {
                view: &tab.view,
                last: tab.hand.last,
                of: Stacked::Paged(tab.file()),
            }),
            Tab::Ink(InkTabs::Pdf(tab)) => Some(Sheeted::Pages {
                view: &tab.view,
                last: tab.hand.last,
                of: Stacked::Pdf(tab.file()),
            }),
            Tab::Ink(InkTabs::Board(tab)) => Some(Sheeted::Plane {
                view: &tab.view,
                last: tab.hand.last,
            }),
            Tab::Image(tab) => Some(Sheeted::Pages {
                view: &tab.view,
                last: tab.last,
                of: Stacked::Image(tab.file()),
            }),
            Tab::Text(text) => match text.showing() {
                Showing::Pages(source, pages, _) => Some(Sheeted::Pages {
                    view: &pages.view,
                    last: pages.last,
                    of: Stacked::Typst(source.buffer()),
                }),
                Showing::Source(_) | Showing::Diff(..) | Showing::Document(..) => None,
            },
            Tab::Book(_) => None,
        }
    }

    pub fn sheet_mut(&mut self) -> Option<SheetedMut<'_>> {
        match self {
            Tab::Ink(InkTabs::Paged(tab)) => Some(SheetedMut::Pages {
                of: Stacked::Paged(tab.file()),
                view: &mut tab.view,
                last: &mut tab.hand.last,
            }),
            Tab::Ink(InkTabs::Pdf(tab)) => Some(SheetedMut::Pages {
                of: Stacked::Pdf(tab.file()),
                view: &mut tab.view,
                last: &mut tab.hand.last,
            }),
            Tab::Ink(InkTabs::Board(tab)) => Some(SheetedMut::Plane {
                view: &mut tab.view,
                last: &mut tab.hand.last,
            }),
            Tab::Image(tab) => Some(SheetedMut::Pages {
                of: Stacked::Image(tab.file()),
                view: &mut tab.view,
                last: &mut tab.last,
            }),
            Tab::Text(text) => match text.showing_mut() {
                ShowingMut::Pages(source, pages, _) => Some(SheetedMut::Pages {
                    of: Stacked::Typst(source.buffer()),
                    view: &mut pages.view,
                    last: &mut pages.last,
                }),
                ShowingMut::Source(_) | ShowingMut::Diff(..) | ShowingMut::Document(..) => None,
            },
            Tab::Book(_) => None,
        }
    }
}

pub fn viewed(state: &mut State, pane: PaneId) -> Option<Viewed<'_>> {
    let State {
        layout,
        pageds,
        pdfs,
        compiles,
        images,
        ..
    } = state;
    let stacks = Stacks {
        pageds,
        pdfs,
        images,
        compiles,
    };
    match layout.find_pane(pane)?.active_mut()?.sheet_mut()? {
        SheetedMut::Pages { view, last, of } => Some(Viewed::Pages {
            stack: stacks.get(of)?,
            view,
            last,
        }),
        SheetedMut::Plane { view, last } => Some(Viewed::Plane { view, last }),
    }
}

pub fn thumb(state: &State, pane: PaneId) -> Option<Thumb> {
    let stacks = Stacks {
        pageds: &state.pageds,
        pdfs: &state.pdfs,
        images: &state.images,
        compiles: &state.compiles,
    };
    match state.layout.panes().get(pane)?.active()?.sheet()? {
        Sheeted::Pages { view, last, of } => {
            let (stack, viewport) = (stacks.get(of)?, last?.viewport);
            Some(view.frame(stack, viewport).thumb(stack, viewport))
        }
        Sheeted::Plane { .. } => None,
    }
}

pub fn seek(state: &mut State, pane: PaneId, top: f32) {
    match viewed(state, pane) {
        Some(Viewed::Pages {
            view,
            stack,
            last: Some(measure),
        }) => view.seek(stack, measure.viewport, top),
        Some(Viewed::Pages { last: None, .. } | Viewed::Plane { .. }) | None => {}
    }
}

pub fn measured(state: &mut State, pane: PaneId, measure: Measure) {
    match viewed(state, pane) {
        Some(Viewed::Pages { last, .. } | Viewed::Plane { last, .. }) => *last = Some(measure),
        None => {}
    }
}

pub fn wheeled(state: &mut State, pane: PaneId, wheel: Wheel, viewport: Viewport) {
    match (viewed(state, pane), wheel) {
        (Some(Viewed::Pages { view, stack, .. }), Wheel::Pan(by)) => view.pan(stack, viewport, by),
        (Some(Viewed::Pages { view, stack, .. }), Wheel::Zoom { at, factor }) => {
            view.zoom_about(stack, viewport, at, factor)
        }
        (Some(Viewed::Plane { view, .. }), Wheel::Pan(by)) => view.pan(by),
        (Some(Viewed::Plane { view, .. }), Wheel::Zoom { at, factor }) => {
            view.zoom_about(viewport, at, factor)
        }
        (None, _) => {}
    }
}

pub fn command(state: &mut State, command: ViewCommand) {
    let pane = state.layout.focus();
    let result = match (viewed(state, pane), command) {
        (None, _) => Ok(()),
        (Some(Viewed::Pages { view, .. }), ViewCommand::Zoom(by)) => {
            view.zoom = zoom(view.zoom, by);
            Ok(())
        }
        (Some(Viewed::Plane { view, .. }), ViewCommand::Zoom(by)) => {
            view.zoom = zoom(view.zoom, by);
            Ok(())
        }
        (Some(Viewed::Pages { view, stack, last }), ViewCommand::Pan(side)) => {
            match last {
                Some(Measure { viewport, .. }) => view.pan(stack, *viewport, pan(side, *viewport)),
                // Not drawn yet, so there is no pane to move by part of.
                None => {}
            }
            Ok(())
        }
        (Some(Viewed::Plane { view, last }), ViewCommand::Pan(side)) => {
            match last {
                Some(Measure { viewport, .. }) => view.pan(pan(side, *viewport)),
                None => {}
            }
            Ok(())
        }
        (Some(Viewed::Pages { view, stack, last }), ViewCommand::Fit) => {
            view.step_fit(stack, last.map(|measure| measure.viewport));
            Ok(())
        }
        (Some(Viewed::Plane { .. }), ViewCommand::Fit) => Err(Notice::BoardHasNoPages),
    };
    match result {
        Ok(()) => {}
        Err(refused) => state.notice = Some(refused),
    }
}

pub fn zoom(zoom: Zoom, by: ZoomBy) -> Zoom {
    match by {
        ZoomBy::In => zoom.by(Zoom::STEP),
        ZoomBy::Out => zoom.by(1.0 / Zoom::STEP),
        ZoomBy::Reset => Zoom::ONE,
    }
}

pub fn pan(side: Side, viewport: Viewport) -> Vector<Pane> {
    let (x, y) = (
        viewport.width.get() * PAN_STEP,
        viewport.height.get() * PAN_STEP,
    );
    match side {
        Side::Left => Vector::new(-x, 0.0),
        Side::Right => Vector::new(x, 0.0),
        Side::Up => Vector::new(0.0, -y),
        Side::Down => Vector::new(0.0, y),
    }
}
