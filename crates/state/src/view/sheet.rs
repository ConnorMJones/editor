//! Zoom, pan and fit for every tab shown as pages or a plane: typst pages, drawings, images.

use model::command::{ViewCommand, ZoomBy};
use model::geom::{Pane, Point, Vector};
use model::notice::Notice;
use model::sheet::{Measure, PlaneView, SheetView, Stack, Viewport, Zoom};
use model::window::Side;

use crate::State;
use crate::drawing::InkTabs;
use crate::view::layout::PaneId;
use crate::view::tab::{ShowingMut, Tab};

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

pub fn viewed(state: &mut State, pane: PaneId) -> Option<Viewed<'_>> {
    let State {
        layout,
        pageds,
        pdfs,
        compiles,
        images,
        ..
    } = state;
    match layout.find_pane(pane)?.active_mut()? {
        Tab::Ink(InkTabs::Paged(tab)) => Some(Viewed::Pages {
            stack: pageds[tab.file()].edited.drawing().stack(),
            view: &mut tab.view,
            last: &mut tab.hand.last,
        }),
        Tab::Ink(InkTabs::Pdf(tab)) => Some(Viewed::Pages {
            stack: pdfs[tab.file()].edited.drawing().stack(),
            view: &mut tab.view,
            last: &mut tab.hand.last,
        }),
        Tab::Ink(InkTabs::Board(tab)) => Some(Viewed::Plane {
            view: &mut tab.view,
            last: &mut tab.hand.last,
        }),
        Tab::Image(tab) => Some(Viewed::Pages {
            stack: &images[tab.file()].stack,
            view: &mut tab.view,
            last: &mut tab.last,
        }),
        Tab::Text(text) => match text.showing_mut() {
            ShowingMut::Pages(source, pages, _) => Some(Viewed::Pages {
                stack: &compiles.get(source.buffer())?.pages.as_ref()?.stack,
                view: &mut pages.view,
                last: &mut pages.last,
            }),
            ShowingMut::Source(_) | ShowingMut::Diff(..) | ShowingMut::Document(..) => None,
        },
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
