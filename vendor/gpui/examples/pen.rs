#![cfg_attr(target_family = "wasm", no_main)]

//! Draw with a drawing tablet: stroke width follows pen pressure and the
//! eraser end removes strokes. Run with `RUST_LOG=debug` to log every event.

#[path = "example_support/fonts.rs"]
mod example_support;

use gpui::{
    App, Context, PathBuilder, PathStyle, PenEvent, PenPhase, PenTool, Pixels, Point,
    StrokeOptions, Window, WindowOptions, canvas, div, prelude::*, px, rgb,
};
use gpui_platform::application;
use lyon::tessellation::LineCap;

const MAX_WIDTH: f32 = 16.0;
const ERASER_RADIUS: f32 = 12.0;

struct Sample {
    position: Point<Pixels>,
    pressure: f32,
}

struct PenExample {
    strokes: Vec<Vec<Sample>>,
    last_event: Option<PenEvent>,
}

impl PenExample {
    fn on_pen(&mut self, event: &PenEvent, _: &mut Window, cx: &mut Context<Self>) {
        match event.phase {
            PenPhase::Enter | PenPhase::Down | PenPhase::Up | PenPhase::Leave => log::info!(
                "pen {:?} {:?} at ({:.1}, {:.1}) pressure={:.3} tilt=({:.1}, {:.1}) buttons={:?}",
                event.phase,
                event.tool,
                event.position.x.as_f32(),
                event.position.y.as_f32(),
                event.pressure,
                event.tilt.x,
                event.tilt.y,
                event.buttons,
            ),
            PenPhase::Hover | PenPhase::Move => log::debug!(
                "pen {:?} {:?} at ({:.1}, {:.1}) pressure={:.3} distance={:.3}",
                event.phase,
                event.tool,
                event.position.x.as_f32(),
                event.position.y.as_f32(),
                event.pressure,
                event.distance,
            ),
        }

        let sample = Sample {
            position: event.position,
            pressure: event.pressure,
        };
        match (event.phase, event.tool) {
            (PenPhase::Down | PenPhase::Move | PenPhase::Up, PenTool::Eraser) => {
                let radius = px(ERASER_RADIUS);
                self.strokes.retain(|stroke| {
                    !stroke.iter().any(|s| {
                        (s.position.x - event.position.x).abs() < radius
                            && (s.position.y - event.position.y).abs() < radius
                    })
                });
            }
            (PenPhase::Down, _) => self.strokes.push(vec![sample]),
            (PenPhase::Move | PenPhase::Up, _) => {
                if let Some(stroke) = self.strokes.last_mut() {
                    stroke.push(sample);
                }
            }
            (PenPhase::Enter | PenPhase::Hover | PenPhase::Leave, _) => {}
        }
        self.last_event = Some(event.clone());
        cx.notify();
    }
}

impl Render for PenExample {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let status = match &self.last_event {
            Some(event) => format!(
                "{:?} {:?}  pressure {:.3}  tilt ({:.1}, {:.1})  distance {:.3}  buttons {:?}",
                event.tool,
                event.phase,
                event.pressure,
                event.tilt.x,
                event.tilt.y,
                event.distance,
                event.buttons,
            ),
            None => "Bring the pen near the tablet".to_string(),
        };
        let segments: Vec<(Point<Pixels>, Point<Pixels>, f32)> = self
            .strokes
            .iter()
            .flat_map(|stroke| {
                stroke.windows(2).map(|pair| {
                    let width = (pair[0].pressure + pair[1].pressure) * 0.5 * MAX_WIDTH;
                    (pair[0].position, pair[1].position, width.max(0.5))
                })
            })
            .collect();

        div()
            .bg(gpui::white())
            .size_full()
            .flex()
            .flex_col()
            .child(
                div()
                    .flex()
                    .gap_2()
                    .p_2()
                    .items_center()
                    .bg(rgb(0xeeeeee))
                    .child(
                        div()
                            .id("clear")
                            .px_2()
                            .border_1()
                            .border_color(rgb(0x888888))
                            .rounded_sm()
                            .bg(rgb(0xffffff))
                            .hover(|style| style.bg(rgb(0xdddddd)))
                            .child("Clear")
                            .on_click(cx.listener(|this, _, _, cx| {
                                log::info!("clear clicked");
                                this.strokes.clear();
                                cx.notify();
                            })),
                    )
                    .child(status),
            )
            .child(
                div()
                    .id("canvas")
                    .size_full()
                    .on_pen_event(cx.listener(Self::on_pen))
                    .child(
                        canvas(
                            |_, _, _| {},
                            move |_, _, window, _| {
                                for (from, to, width) in segments {
                                    let mut builder =
                                        PathBuilder::fill().with_style(PathStyle::Stroke(
                                            StrokeOptions::default()
                                                .with_line_width(width)
                                                .with_line_cap(LineCap::Round),
                                        ));
                                    builder.move_to(from);
                                    builder.line_to(to);
                                    if let Ok(path) = builder.build() {
                                        window.paint_path(path, gpui::black());
                                    }
                                }
                            },
                        )
                        .size_full(),
                    ),
            )
    }
}

fn run_example() {
    application().run(|cx: &mut App| {
        if !example_support::load_fonts(cx) {
            return;
        }
        cx.open_window(
            WindowOptions {
                focus: true,
                ..Default::default()
            },
            |_, cx| {
                cx.new(|_| PenExample {
                    strokes: Vec::new(),
                    last_event: None,
                })
            },
        )
        .unwrap();
        cx.on_window_closed(|cx, _| cx.quit()).detach();
        cx.activate(true);
    });
}

#[cfg(not(target_family = "wasm"))]
fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    run_example();
}

#[cfg(target_family = "wasm")]
#[wasm_bindgen::prelude::wasm_bindgen(start)]
pub fn start() {
    gpui_platform::web_init();
    run_example();
}
