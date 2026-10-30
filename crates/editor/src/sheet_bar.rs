use gpui::{
    Context, Div, ElementId, MouseButton, MouseDownEvent, Pixels, canvas, div, prelude::*, px,
    relative,
};
use model::sheet::Thumb;
use slotmap::Key as _;
use state::view::layout::PaneId;

use crate::fonts::hsla;
use crate::workspace::{Drag, Workspace};

pub const BAR_WIDTH: Pixels = px(10.);
pub const LEAST_THUMB: Pixels = px(24.);

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OnTrack {
    pub thumb: Thumb,
    pub shown_top: f32,
    pub shown_length: f32,
}

impl OnTrack {
    pub fn of(thumb: Thumb, track: Pixels) -> Option<OnTrack> {
        let shown_length = thumb.length.max(LEAST_THUMB / track).min(1.0);
        let room = Some(1.0 - shown_length).filter(|room| *room > 0.0)?;
        Some(OnTrack {
            thumb,
            shown_top: thumb.top / (1.0 - thumb.length) * room,
            shown_length,
        })
    }

    pub fn top_at(&self, along: f32) -> f32 {
        let room = 1.0 - self.shown_length;
        (along / room * (1.0 - self.thumb.length)).clamp(0.0, 1.0 - self.thumb.length)
    }
}

pub fn sheet_bar(workspace: &Workspace, pane: PaneId, cx: &mut Context<Workspace>) -> Option<Div> {
    let thumb = state::view::sheet::thumb(&workspace.state, pane)?;
    let track = workspace.areas.sheet_bars.borrow().get(pane).copied();
    let on_track = OnTrack::of(thumb, track.map_or(px(1000.), |track| track.size.height))?;
    let editor = &workspace.theme.editor;
    let color = match workspace.drag {
        Drag::SheetThumb { pane: dragged, .. } if dragged == pane => editor.scrollbar_thumb_dragged,
        Drag::SheetThumb { .. }
        | Drag::Idle
        | Drag::Title
        | Drag::Sidebar
        | Drag::Divider { .. }
        | Drag::Split { .. } => editor.scrollbar_thumb,
    };
    let areas = workspace.areas.clone();
    Some(
        div()
            .absolute()
            .top_0()
            .right_0()
            .h_full()
            .w(BAR_WIDTH)
            .bg(hsla(editor.scrollbar_track))
            .child(
                canvas(
                    move |bounds, _, _| {
                        areas.sheet_bars.borrow_mut().insert(pane, bounds);
                    },
                    |_, _, _, _| {},
                )
                .absolute()
                .size_full(),
            )
            .child(
                div()
                    .absolute()
                    .left_0()
                    .w_full()
                    .top(relative(on_track.shown_top))
                    .h(relative(on_track.shown_length))
                    .bg(hsla(color)),
            )
            .child(
                div()
                    .id(ElementId::NamedInteger(
                        "sheet-bar".into(),
                        pane.data().as_ffi(),
                    ))
                    .absolute()
                    .size_full()
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, event: &MouseDownEvent, _, cx| {
                            cx.stop_propagation();
                            let Some(track) = this.areas.sheet_bars.borrow().get(pane).copied()
                            else {
                                return;
                            };
                            let along = (event.position.y - track.origin.y) / track.size.height;
                            let grab = Some(along - on_track.shown_top)
                                .filter(|inside| (0.0..=on_track.shown_length).contains(inside))
                                .unwrap_or(on_track.shown_length * 0.5);
                            state::view::sheet::seek(
                                &mut this.state,
                                pane,
                                on_track.top_at(along - grab),
                            );
                            this.drag = Drag::SheetThumb { pane, grab };
                            this.changed(cx);
                        }),
                    ),
            ),
    )
}
