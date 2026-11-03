use gpui::{App, Bounds, Entity, Pixels, Window};
use model::geom::{Pane, Point};
use model::sheet::{Measure, Viewport};
use model::units::Px;
use state::view::layout::PaneId;

use crate::fonts::PROSE_LINE;
use crate::workspace::Workspace;

pub fn viewport(bounds: Bounds<Pixels>) -> Option<Viewport> {
    Viewport::new(
        Px(f32::from(bounds.size.width)),
        Px(f32::from(bounds.size.height)),
    )
}

pub fn pane_point(bounds: Bounds<Pixels>, position: gpui::Point<Pixels>) -> Point<Pane> {
    Point::new(
        f32::from(position.x - bounds.origin.x),
        f32::from(position.y - bounds.origin.y),
    )
}

pub fn report_measure(
    workspace: &Entity<Workspace>,
    pane: PaneId,
    viewport: Viewport,
    last: Option<Measure>,
    window: &mut Window,
    cx: &mut App,
) {
    let line = Px(f32::from(workspace.read(cx).fonts.prose_size) * PROSE_LINE);
    let measure = Measure { viewport, line };
    match last {
        Some(seen) if seen == measure => {}
        Some(_) | None => {
            let workspace = workspace.clone();
            window.defer(cx, move |_, cx| {
                workspace.update(cx, |workspace, cx| {
                    state::view::sheet::measured(&mut workspace.state, pane, measure);
                    cx.notify();
                })
            });
        }
    }
}
