use gpui::{
    Bounds, Context, Div, ElementId, MouseButton, MouseDownEvent, Pixels, Window, canvas, div,
    fill, point, prelude::*, px, size,
};
use model::scroll_bar::{OnTrack, Thumb};
use model::theme::EditorColors;
use model::units::Px;
use slotmap::Key as _;
use state::view::layout::PaneId;

use crate::fonts::hsla;
use crate::workspace::{Drag, Workspace};

pub const WIDTH: Pixels = px(model::scroll_bar::WIDTH.0);

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Held {
    Free,
    Dragged,
}

pub fn on_track(thumb: Thumb, track: Bounds<Pixels>) -> Option<OnTrack> {
    OnTrack::of(thumb, Px(f32::from(track.size.height)))
}

pub fn along(track: Bounds<Pixels>, y: Pixels) -> f32 {
    (y - track.origin.y) / track.size.height
}

pub fn paint(
    window: &mut Window,
    track: Bounds<Pixels>,
    on: OnTrack,
    held: Held,
    colors: &EditorColors,
) {
    window.paint_quad(fill(track, hsla(colors.scrollbar_track)));
    let thumb = match held {
        Held::Free => colors.scrollbar_thumb,
        Held::Dragged => colors.scrollbar_thumb_dragged,
    };
    window.paint_quad(fill(
        Bounds::new(
            point(
                track.origin.x,
                track.origin.y + track.size.height * on.shown_top,
            ),
            size(track.size.width, track.size.height * on.shown_length),
        ),
        hsla(thumb),
    ));
}

pub fn thumb(workspace: &Workspace, pane: PaneId) -> Option<Thumb> {
    match workspace.rendered.get(pane) {
        Some(list) => list.thumb(),
        None => state::view::sheet::thumb(&workspace.state, pane),
    }
}

pub fn seek(workspace: &mut Workspace, pane: PaneId, top: f32) {
    match workspace.rendered.get(pane) {
        Some(list) => match list.at_fraction(top) {
            Some(at) => crate::rendered::report(&mut workspace.state, pane, list.shows, at),
            None => {}
        },
        None => state::view::sheet::seek(&mut workspace.state, pane, top),
    }
}

pub fn pane_bar(workspace: &Workspace, pane: PaneId, cx: &mut Context<Workspace>) -> Option<Div> {
    let thumb = thumb(workspace, pane)?;
    let held = match workspace.drag {
        Drag::ScrollThumb { pane: dragged, .. } if dragged == pane => Held::Dragged,
        Drag::ScrollThumb { .. }
        | Drag::Idle
        | Drag::Title
        | Drag::Sidebar
        | Drag::Divider { .. }
        | Drag::Split { .. }
        | Drag::Pan { .. } => Held::Free,
    };
    let colors = workspace.theme.editor;
    let areas = workspace.areas.clone();
    Some(
        div()
            .absolute()
            .top_0()
            .right_0()
            .h_full()
            .w(WIDTH)
            .child(
                canvas(
                    move |track, _, _| {
                        areas.scroll_bars.borrow_mut().insert(pane, track);
                        track
                    },
                    move |_, track, window, _| match on_track(thumb, track) {
                        Some(on) => paint(window, track, on, held, &colors),
                        None => {}
                    },
                )
                .absolute()
                .size_full(),
            )
            .child(
                div()
                    .id(ElementId::NamedInteger(
                        "scroll-bar".into(),
                        pane.data().as_ffi(),
                    ))
                    .absolute()
                    .size_full()
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, event: &MouseDownEvent, _, cx| {
                            let Some(track) = this.areas.scroll_bars.borrow().get(pane).copied()
                            else {
                                return;
                            };
                            let Some(on) = on_track(thumb, track) else {
                                return;
                            };
                            cx.stop_propagation();
                            let along = along(track, event.position.y);
                            let grab = on.grab(along);
                            seek(this, pane, on.top_at(along - grab));
                            this.drag = Drag::ScrollThumb { pane, grab };
                            this.changed(cx);
                        }),
                    ),
            ),
    )
}
