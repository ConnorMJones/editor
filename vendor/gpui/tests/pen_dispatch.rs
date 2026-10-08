#![cfg(feature = "test-support")]

use gpui::{
    Context, InteractiveElement, IntoElement, MouseButton, ParentElement, PenEvent, PenPhase,
    Pixels, Point, Render, StatefulInteractiveElement, Styled, TestAppContext, Window, div, point,
    px,
};

#[derive(Default)]
struct PenView {
    canvas_phases: Vec<PenPhase>,
    button_mouse_downs: usize,
    button_clicks: usize,
}

impl Render for PenView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .child(
                div()
                    .id("canvas")
                    .absolute()
                    .left(px(0.))
                    .top(px(0.))
                    .size(px(100.))
                    .on_pen_event(cx.listener(|this, event: &PenEvent, _, cx| {
                        this.canvas_phases.push(event.phase);
                        cx.notify();
                    })),
            )
            .child(
                div()
                    .id("button")
                    .absolute()
                    .left(px(200.))
                    .top(px(0.))
                    .size(px(100.))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, _, _| this.button_mouse_downs += 1),
                    )
                    .on_click(cx.listener(|this, _, _, _| this.button_clicks += 1)),
            )
    }
}

fn pen(phase: PenPhase, position: Point<Pixels>) -> PenEvent {
    PenEvent {
        phase,
        position,
        pressure: 0.5,
        ..Default::default()
    }
}

#[gpui::test]
fn stroke_claimed_by_pen_listener_follows_it_outside_bounds(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(|_, _| PenView::default());

    cx.simulate_event(pen(PenPhase::Enter, point(px(50.), px(50.))));
    cx.simulate_event(pen(PenPhase::Down, point(px(50.), px(50.))));
    cx.simulate_event(pen(PenPhase::Move, point(px(250.), px(50.))));
    cx.simulate_event(pen(PenPhase::Up, point(px(250.), px(50.))));

    view.read_with(cx, |view, _| {
        assert_eq!(
            view.canvas_phases,
            [
                PenPhase::Enter,
                PenPhase::Down,
                PenPhase::Move,
                PenPhase::Up
            ]
        );
        assert_eq!(view.button_mouse_downs, 0);
        assert_eq!(view.button_clicks, 0);
    });
}

#[gpui::test]
fn unclaimed_stroke_is_delivered_as_mouse_click(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(|_, _| PenView::default());

    cx.simulate_event(pen(PenPhase::Enter, point(px(250.), px(50.))));
    cx.simulate_event(pen(PenPhase::Down, point(px(250.), px(50.))));
    cx.simulate_event(pen(PenPhase::Move, point(px(50.), px(50.))));
    cx.simulate_event(pen(PenPhase::Move, point(px(250.), px(50.))));
    cx.simulate_event(pen(PenPhase::Up, point(px(250.), px(50.))));
    cx.simulate_event(pen(PenPhase::Leave, point(px(250.), px(50.))));

    view.read_with(cx, |view, _| {
        assert!(view.canvas_phases.is_empty());
        assert_eq!(view.button_mouse_downs, 1);
        assert_eq!(view.button_clicks, 1);
    });
}
