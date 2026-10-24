//! The pane tree: splits, their dividers, and each pane's tabs and body.

use gpui::{
    AnyElement, Context, Div, ElementId, Hsla, IntoElement, MouseButton, MouseDownEvent,
    MouseMoveEvent, MouseUpEvent, PenEvent, Pixels, ScrollWheelEvent, canvas, div, prelude::*, px,
};
use model::window::Axis;
use slotmap::Key as _;
use state::drawing::pen::{PenPhase, Stylus};

use state::view::layout::{Node, Pane, SplitPath};
use state::view::mode;
use state::view::panes;
use state::view::tab::{Beside, Share, Showing, Tab};

use super::{Drag, Workspace};
use crate::drawing::{self, DrawingElement};
use crate::fonts::hsla;
use crate::sheet::SheetElement;
use crate::text::TextElement;
use state::view::draws::Part;

/// Hit width of split and sidebar dividers; a 1px line is drawn in the middle.
pub const DIVIDER: Pixels = px(5.0);
/// Columns of the monospace font each side of a split keeps, however far its divider goes.
pub const SPLIT_MIN_COLUMNS: f32 = 12.0;

/// A draggable gap between panes along `axis`, or beside the sidebar.
pub fn divider(axis: Axis, color: Hsla) -> Div {
    match axis {
        Axis::Row => div()
            .flex_none()
            .w(DIVIDER)
            .h_full()
            .flex()
            .justify_center()
            .cursor_col_resize()
            .child(div().w(px(1.)).h_full().bg(color)),
        Axis::Column => div()
            .flex_none()
            .h(DIVIDER)
            .w_full()
            .flex()
            .flex_col()
            .justify_center()
            .cursor_row_resize()
            .child(div().h(px(1.)).w_full().bg(color)),
    }
}

impl Workspace {
    /// `path` holds the child indices from the root down to `node`.
    pub fn node(&self, node: &Node, path: &mut SplitPath, cx: &mut Context<Self>) -> AnyElement {
        match node {
            Node::Pane(pane) => {
                let pane = *pane;
                let target = self.state.layout.panes().get(pane);
                let bar = match (
                    target.and_then(|target| target.tabs.as_ref()),
                    self.tab_scroll.get(pane),
                ) {
                    (Some(tabs), Some(scroll)) => Some(self.tab_bar(pane, tabs, scroll, cx)),
                    (Some(_) | None, _) => None,
                };
                let element = |part: Part| TextElement {
                    workspace: cx.entity(),
                    pane,
                    part,
                };
                let body = match target.and_then(Pane::active) {
                    Some(Tab::Ink(_)) => div().flex_1().min_h_0().overflow_hidden().child(
                        div()
                            .id(ElementId::NamedInteger(
                                "drawing-input".into(),
                                pane.data().as_ffi(),
                            ))
                            .size_full()
                            .on_pen_event(cx.listener(move |this, event: &PenEvent, window, cx| {
                                let phase = match event.phase {
                                    gpui::PenPhase::Down => {
                                        window.focus(&this.focus, cx);
                                        PenPhase::Down(drawing::stylus(event.tool))
                                    }
                                    gpui::PenPhase::Move => PenPhase::Move,
                                    gpui::PenPhase::Up => PenPhase::Up,
                                    gpui::PenPhase::Enter
                                    | gpui::PenPhase::Hover
                                    | gpui::PenPhase::Leave => return,
                                };
                                let at = event.position;
                                match drawing::sample(
                                    this,
                                    pane,
                                    at,
                                    event.time,
                                    Some(event.pressure),
                                    Some(event.tilt),
                                ) {
                                    Some(sample) => drawing::input(this, pane, phase, sample, cx),
                                    None => {}
                                }
                            }))
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(move |this, event: &MouseDownEvent, window, cx| {
                                    window.focus(&this.focus, cx);
                                    match drawing::sample(
                                        this,
                                        pane,
                                        event.position,
                                        drawing::now(),
                                        None,
                                        None,
                                    ) {
                                        Some(sample) => {
                                            let phase = PenPhase::Down(Stylus::Tip);
                                            drawing::input(this, pane, phase, sample, cx)
                                        }
                                        None => {}
                                    }
                                }),
                            )
                            .on_mouse_move(cx.listener(
                                move |this, event: &MouseMoveEvent, _, cx| match (
                                    event.pressed_button,
                                    drawing::sample(
                                        this,
                                        pane,
                                        event.position,
                                        drawing::now(),
                                        None,
                                        None,
                                    ),
                                ) {
                                    (Some(MouseButton::Left), Some(sample)) => {
                                        drawing::input(this, pane, PenPhase::Move, sample, cx)
                                    }
                                    (Some(_) | None, _) => {}
                                },
                            ))
                            .on_mouse_up(
                                MouseButton::Left,
                                cx.listener(move |this, event: &MouseUpEvent, _, cx| {
                                    match drawing::sample(
                                        this,
                                        pane,
                                        event.position,
                                        drawing::now(),
                                        None,
                                        None,
                                    ) {
                                        Some(sample) => {
                                            drawing::input(this, pane, PenPhase::Up, sample, cx)
                                        }
                                        None => {}
                                    }
                                }),
                            )
                            .on_scroll_wheel(cx.listener(
                                move |this, event: &ScrollWheelEvent, _, cx| {
                                    drawing::wheel(this, pane, event, cx)
                                },
                            ))
                            .child(DrawingElement {
                                workspace: cx.entity(),
                                pane,
                            }),
                    ),
                    Some(Tab::Image(_)) => div().flex_1().min_h_0().child(SheetElement {
                        workspace: cx.entity(),
                        pane,
                    }),
                    Some(Tab::Text(view)) => {
                        match view.showing() {
                            Showing::Source(_) => div()
                                .flex_1()
                                .min_h_0()
                                .overflow_hidden()
                                .child(element(Part::Text)),
                            // Equal halves, so both sides wrap at the same width.
                            Showing::Diff(..) => div()
                                .flex_1()
                                .min_h_0()
                                .flex()
                                .flex_row()
                                .overflow_hidden()
                                .child(
                                    div()
                                        .flex_1()
                                        .min_w_0()
                                        .overflow_hidden()
                                        .child(element(Part::Head)),
                                )
                                .child(
                                    div()
                                        .flex_none()
                                        .w(px(1.))
                                        .bg(hsla(self.theme.chrome.border)),
                                )
                                .child(
                                    div()
                                        .flex_1()
                                        .min_w_0()
                                        .overflow_hidden()
                                        .child(element(Part::Buffer)),
                                ),
                            Showing::Document(_, _, Beside::Source(_, share))
                            | Showing::Pages(_, _, Beside::Source(_, share)) => {
                                let least = self.fonts.cell.width * SPLIT_MIN_COLUMNS;
                                let areas = self.areas.clone();
                                let side = |grow: f32| {
                                    let mut side =
                                        div().min_w(least).flex_basis(px(0.)).overflow_hidden();
                                    side.style().flex_grow = Some(grow);
                                    side
                                };
                                div()
                                    .flex_1()
                                    .min_h_0()
                                    .relative()
                                    .flex()
                                    .flex_row()
                                    .overflow_hidden()
                                    .child(
                                        canvas(
                                            move |bounds, _, _| {
                                                areas.splits.borrow_mut().insert(pane, bounds);
                                            },
                                            |_, _, _, _| {},
                                        )
                                        .absolute()
                                        .size_full(),
                                    )
                                    .child(side(share.get()).child(element(Part::Text)))
                                    .child(
                                        divider(Axis::Row, hsla(self.theme.chrome.border))
                                            .on_mouse_down(
                                                MouseButton::Left,
                                                cx.listener(
                                                    move |this, event: &MouseDownEvent, _, cx| {
                                                        match event.click_count {
                                                            2 => mode::drag(
                                                                &mut this.state,
                                                                pane,
                                                                Share::EVEN,
                                                            ),
                                                            _ => this.drag = Drag::Split { pane },
                                                        }
                                                        cx.stop_propagation();
                                                        cx.notify();
                                                    },
                                                ),
                                            ),
                                    )
                                    .child(
                                        side(1.0 - share.get())
                                            .flex()
                                            .flex_col()
                                            .overflow_hidden()
                                            .bg(hsla(self.theme.background))
                                            .child(crate::rendered::body(self, pane, cx)),
                                    )
                            }
                            Showing::Document(_, _, Beside::Alone)
                            | Showing::Pages(_, _, Beside::Alone) => div()
                                .flex_1()
                                .min_h_0()
                                .flex()
                                .flex_col()
                                .overflow_hidden()
                                .bg(hsla(self.theme.background))
                                .child(crate::rendered::body(self, pane, cx)),
                        }
                    }
                    None => div().flex_1(),
                };
                div()
                    .flex_1()
                    .flex()
                    .flex_col()
                    .min_w_0()
                    .min_h_0()
                    .overflow_hidden()
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _, window, cx| {
                            panes::focus_pane(&mut this.state, pane);
                            window.focus(&this.focus, cx);
                            this.changed(cx);
                        }),
                    )
                    .children(bar)
                    .child(self.drop_target(pane, body, cx))
                    .child(self.statusline(pane))
                    .into_any_element()
            }
            Node::Split(split) => {
                let (axis, children) = (split.axis(), split.children());
                let container = div()
                    .size_full()
                    .flex()
                    .min_w_0()
                    .min_h_0()
                    .overflow_hidden();
                let mut container = match axis {
                    Axis::Row => container.flex_row(),
                    Axis::Column => container.flex_col(),
                };
                for (index, child) in children.iter().enumerate() {
                    match index {
                        0 => {}
                        _ => {
                            let split = path.clone();
                            container = container.child(
                                divider(axis, hsla(self.theme.chrome.border)).on_mouse_down(
                                    MouseButton::Left,
                                    cx.listener(move |this, _, _, cx| {
                                        this.drag = Drag::Divider {
                                            path: split.clone(),
                                            divider: index - 1,
                                            axis,
                                        };
                                        cx.stop_propagation();
                                    }),
                                ),
                            );
                        }
                    }
                    path.push(index);
                    let mut cell = div()
                        .flex()
                        .min_w_0()
                        .min_h_0()
                        .overflow_hidden()
                        .flex_basis(px(0.));
                    cell.style().flex_grow = Some(child.flex.get());
                    container = container.child(cell.child(self.node(&child.node, path, cx)));
                    path.pop();
                }
                container.into_any_element()
            }
        }
    }
}
