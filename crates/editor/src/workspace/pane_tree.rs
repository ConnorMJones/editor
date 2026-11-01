//! The pane tree: splits, their dividers, and each pane's tabs and body.

use gpui::{
    AnyElement, Context, Div, ElementId, Hsla, IntoElement, MouseButton, MouseDownEvent,
    MouseMoveEvent, MouseUpEvent, PenEvent, Pixels, ScrollWheelEvent, canvas, div, prelude::*, px,
};

use model::window::Axis;
use slotmap::Key as _;
use state::comment::Spread;
use state::drawing::frame;
use state::drawing::pen::{PenPhase, Stylus};
use state::drawing::{InkTabs, PdfMode};
use state::git::diffview::DiffShown;
use state::reading::Taps;

use state::view::layout::{Node, Pane, SplitPath};
use state::view::mode;
use state::view::panes;
use state::view::tab::{Beside, Share, Showing, Tab};

use super::{Drag, Workspace};
use crate::drawing::{self, DrawingElement};
use crate::fonts::hsla;
use crate::reading::{self, Offer};
use crate::sheet::SheetElement;
use crate::text::TextElement;
use state::view::draws::Part;

pub const DIVIDER: Pixels = px(5.0);
pub const SPLIT_MIN_COLUMNS: f32 = 12.0;

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
                let popup = match target.and_then(Pane::active) {
                    Some(Tab::Ink(InkTabs::Pdf(tab))) => {
                        let file = &self.state.pdfs[tab.file()];
                        let shown = match &tab.mode {
                            PdfMode::Read(reading) => {
                                Offer::of(reading).zip(reading.shown(file.document()))
                            }
                            PdfMode::Markup => None,
                        };
                        let corner = shown.as_ref().and_then(|(_, span)| {
                            let end = span.end();
                            let text = file.document().text(end.part.0)?;
                            let last = text.chars.get(end.char.checked_sub(1)?)?;
                            Some((model::sheet::PageIndex(end.part.0), last.quad[0]))
                        });
                        match (shown, corner, tab.hand.last) {
                            (Some((offer, _)), Some((page, corner)), Some(measure)) => {
                                let paged = file.ink.edited.drawing();
                                let frame = frame::paged(paged, &tab.view, measure.viewport);
                                frame.on(page).map(|shown| {
                                    let at = shown.to_pane.apply(corner);
                                    reading::popup(self, offer, cx)
                                        .absolute()
                                        .left(px(at.x))
                                        .top(px(at.y) + self.fonts.ui_line * 0.3)
                                })
                            }
                            (_, _, _) => None,
                        }
                    }
                    Some(Tab::Book(tab)) => Offer::of(&tab.reading).map(|offer| {
                        div()
                            .absolute()
                            .bottom(self.fonts.ui_line)
                            .left_0()
                            .right_0()
                            .flex()
                            .justify_center()
                            .child(reading::popup(self, offer, cx))
                    }),
                    Some(Tab::Ink(_) | Tab::Text(_) | Tab::Image(_)) | None => None,
                };
                let entity = cx.entity();
                let element = |part: Part| TextElement {
                    workspace: entity.clone(),
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
                            .relative()
                            .on_pen_event(cx.listener(move |this, event: &PenEvent, window, cx| {
                                let phase = match event.phase {
                                    gpui::PenPhase::Down => {
                                        window.focus(&this.focus, cx);
                                        PenPhase::Down(
                                            drawing::stylus(event.tool),
                                            Taps::of(event.click_count),
                                        )
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
                                            let phase = PenPhase::Down(
                                                Stylus::Tip,
                                                Taps::of(event.click_count),
                                            );
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
                            })
                            .children(popup),
                    ),
                    Some(Tab::Image(_)) => div().flex_1().min_h_0().child(SheetElement {
                        workspace: cx.entity(),
                        pane,
                    }),
                    Some(Tab::Book(_)) => div()
                        .flex_1()
                        .min_h_0()
                        .flex()
                        .flex_col()
                        .overflow_hidden()
                        .bg(hsla(self.theme.background))
                        .child(crate::rendered::body(self, pane, cx)),
                    Some(Tab::Text(view)) => match view.showing() {
                        Showing::Source(_) => div()
                            .flex_1()
                            .min_h_0()
                            .overflow_hidden()
                            .child(element(Part::Text)),
                        Showing::Diff(text, sides) => {
                            let buffer = &self.state.buffers[text.buffer()];
                            let whole = |part: Part| {
                                div()
                                    .flex_1()
                                    .min_h_0()
                                    .overflow_hidden()
                                    .child(element(part))
                            };
                            match sides.shown(buffer) {
                                DiffShown::Both => self.split_body(
                                    pane,
                                    sides.share,
                                    element(Part::Head).into_any_element(),
                                    element(Part::Buffer).into_any_element(),
                                    cx,
                                ),
                                DiffShown::Head => whole(Part::Head),
                                DiffShown::Buffer => whole(Part::Buffer),
                            }
                        }
                        Showing::Document(_, _, Beside::Source(_, share))
                        | Showing::Pages(_, _, Beside::Source(_, share)) => {
                            let rendered = div()
                                .flex()
                                .flex_col()
                                .size_full()
                                .overflow_hidden()
                                .bg(hsla(self.theme.background))
                                .child(crate::rendered::body(self, pane, cx))
                                .into_any_element();
                            self.split_body(
                                pane,
                                share,
                                element(Part::Text).into_any_element(),
                                rendered,
                                cx,
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
                    },
                    None => div().flex_1(),
                };
                let writing = (target.and_then(Pane::active).and_then(Tab::writing))
                    .map(|writing| (writing.spread, writing.share));
                let body = match writing {
                    Some((Spread::Split, share)) => self.split_body(
                        pane,
                        share,
                        body.size_full().into_any_element(),
                        element(Part::Comment).into_any_element(),
                        cx,
                    ),
                    Some((Spread::Full, _)) => div()
                        .flex_1()
                        .min_h_0()
                        .overflow_hidden()
                        .child(element(Part::Comment)),
                    None => body,
                };
                let body = body
                    .relative()
                    .children(crate::scroll_bar::pane_bar(self, pane, cx));
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

impl Workspace {
    fn split_body(
        &self,
        pane: state::view::layout::PaneId,
        share: Share,
        left: AnyElement,
        right: AnyElement,
        cx: &mut Context<Self>,
    ) -> Div {
        let least = self.fonts.cell.width * SPLIT_MIN_COLUMNS;
        let areas = self.areas.clone();
        let side = |grow: f32| {
            let mut side = div().min_w(least).flex_basis(px(0.)).overflow_hidden();
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
            .child(side(share.get()).child(left))
            .child(
                divider(Axis::Row, hsla(self.theme.chrome.border)).on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |this, event: &MouseDownEvent, _, cx| {
                        match event.click_count {
                            2 => mode::drag(&mut this.state, pane, Share::EVEN),
                            _ => this.drag = Drag::Split { pane },
                        }
                        cx.stop_propagation();
                        cx.notify();
                    }),
                ),
            )
            .child(side(1.0 - share.get()).flex().flex_col().child(right))
    }
}
