//! Files and tabs dropped on panes: which part of a pane a drop would land in, and the tint that
//! shows it, as VS Code shows where a dropped editor goes.

use gpui::{
    Bounds, Context, Div, DragMoveEvent, Hsla, Pixels, Stateful, Window, div, prelude::*, relative,
};
use model::geom::Point;
use model::sheet::Viewport;
use model::units::Px;
use model::window::Side;
use slotmap::Key as _;
use state::view::layout::PaneId;
use state::view::panes::{self, Dragged, DropOn, Zone};

use super::Workspace;
use super::sidebar::DraggedEntry;
use crate::fonts::hsla;

/// A tab being dragged to another place in its bar, another pane, or a pane's edge.
pub struct DraggedTab {
    pub pane: PaneId,
    pub index: usize,
}

impl Workspace {
    /// `body` taking dropped files and tabs, tinted where a drop would land.
    pub fn drop_target(
        &self,
        pane: PaneId,
        body: impl IntoElement,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let tint = match self.drop_zone {
            Some((over, zone)) if over == pane && cx.has_active_drag() => Some(zone),
            Some(_) | None => None,
        };
        let color = hsla(self.theme.chrome.accent).opacity(0.2);
        div()
            .id(("pane-drop", pane.data().as_ffi()))
            .relative()
            .flex_1()
            .min_h_0()
            .flex()
            .flex_col()
            .child(body)
            .on_drag_move(
                cx.listener(move |this, event: &DragMoveEvent<DraggedEntry>, _, cx| {
                    this.hover_drop(pane, event.bounds, event.event.position, cx)
                }),
            )
            .on_drag_move(
                cx.listener(move |this, event: &DragMoveEvent<DraggedTab>, _, cx| {
                    this.hover_drop(pane, event.bounds, event.event.position, cx)
                }),
            )
            .on_drop(
                cx.listener(move |this, dragged: &DraggedEntry, window, cx| {
                    this.drop_on_pane(pane, Dragged::File(dragged.path.clone()), window, cx)
                }),
            )
            .on_drop(cx.listener(move |this, dragged: &DraggedTab, window, cx| {
                let tab = Dragged::Tab {
                    pane: dragged.pane,
                    index: dragged.index,
                };
                this.drop_on_pane(pane, tab, window, cx)
            }))
            .children(tint.map(|zone| overlay(zone, color)))
    }

    /// Records which zone of `pane` the pointer is over while a drag moves.
    fn hover_drop(
        &mut self,
        pane: PaneId,
        bounds: Bounds<Pixels>,
        position: gpui::Point<Pixels>,
        cx: &mut Context<Self>,
    ) {
        let viewport = Viewport::new(
            Px(f32::from(bounds.size.width)),
            Px(f32::from(bounds.size.height)),
        );
        let over = (viewport.filter(|_| bounds.contains(&position))).map(|viewport| {
            let at = Point::new(
                f32::from(position.x - bounds.origin.x),
                f32::from(position.y - bounds.origin.y),
            );
            (pane, Zone::at(at, viewport))
        });
        let before = self.drop_zone;
        self.drop_zone = match (over, before) {
            (Some(over), _) => Some(over),
            // Leaving this pane clears only its own zone; another pane may hold the pointer now.
            (None, Some((left, _))) if left == pane => None,
            (None, kept) => kept,
        };
        match self.drop_zone == before {
            true => {}
            false => cx.notify(),
        }
    }

    fn drop_on_pane(
        &mut self,
        pane: PaneId,
        dragged: Dragged,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let zone = match self.drop_zone.take() {
            Some((over, zone)) if over == pane => zone,
            Some(_) | None => Zone::Centre,
        };
        let on = DropOn::Pane { pane, zone };
        panes::dropped(&mut self.state, dragged, on, &mut self.effects);
        window.focus(&self.focus, cx);
        self.changed(cx);
    }

    pub fn drop_on_tabs(
        &mut self,
        pane: PaneId,
        at: usize,
        dragged: Dragged,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.drop_zone = None;
        panes::dropped(
            &mut self.state,
            dragged,
            DropOn::Tabs { pane, at },
            &mut self.effects,
        );
        window.focus(&self.focus, cx);
        self.changed(cx);
    }
}

/// The part of a pane a drop in `zone` fills: all of it, or the half a split would give.
fn overlay(zone: Zone, color: Hsla) -> Div {
    let tint = div().absolute().bg(color);
    match zone {
        Zone::Centre => tint.top_0().left_0().size_full(),
        Zone::Edge(Side::Left) => tint.top_0().left_0().h_full().w(relative(0.5)),
        Zone::Edge(Side::Right) => tint.top_0().right_0().h_full().w(relative(0.5)),
        Zone::Edge(Side::Up) => tint.top_0().left_0().w_full().h(relative(0.5)),
        Zone::Edge(Side::Down) => tint.bottom_0().left_0().w_full().h(relative(0.5)),
    }
}
