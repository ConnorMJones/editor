use gpui::{
    AnyElement, ClickEvent, Context, DispatchPhase, Div, Hsla, Modifiers, MouseButton,
    MouseDownEvent, MouseMoveEvent, MouseUpEvent, PathBuilder, ScrollWheelEvent, SharedString,
    Stateful, canvas, div, point, prelude::*, px,
};
use jsoncanvas::{Area, Cap, JsonCanvasColor, Labelled, Node, NodeKind, Preset};
use model::color::Base16;
use model::command::JsonCanvasCommand;
use model::geom::{Point, Surface};
use slotmap::Key as _;
use state::json_canvas::{
    Adding, Doing, Embedded, GRID, Gesture, JsonCanvasFile, JsonCanvasId, Phase, Place, Pointer,
    SIDES, Snap, Written, along, anchor, curve, surface,
};
use state::reading::Taps;
use state::view::draws::Part;
use state::view::layout::PaneId;
use state::view::tab::Tab;

use crate::fonts::{Fonts, hsla};
use crate::pictures::picture_canvas;
use crate::rendered::document::{self, NoteKey, Tints};
use crate::text::{TextElement, TextSize};
use crate::workspace::{Drag, Workspace};
use crate::{drawing, pane_space};

const LEAST_DETAIL: f32 = 0.3;
const EDGE_WIDTH: f32 = 2.0;
const ARROW: f32 = 12.0;
const CORNER: f32 = 8.0;
const PADDING: f32 = 12.0;
const HANDLE: f32 = 8.0;
const LABEL_EDITOR: f32 = 240.0;
const DOT: f32 = 2.0;
const DOT_SPACING: f32 = 16.0;
const TOOL: f32 = 28.0;
const ICON: f32 = 14.0;
const CARD_TINT: f32 = 0.16;

#[derive(Clone, Copy)]
enum Held {
    Free,
    Down,
}

#[derive(Clone, Copy)]
enum Detail {
    Full,
    Outline,
}

struct Cards<'a> {
    this: &'a Workspace,
    pane: PaneId,
    id: JsonCanvasId,
    file: &'a JsonCanvasFile,
    in_place: Option<&'a Written>,
    fonts: Fonts,
    scale: f32,
    detail: Detail,
}

struct Edged {
    curve: [Point<Surface>; 4],
    caps: [Cap; 2],
    color: Hsla,
    width: f32,
}

pub fn body(this: &Workspace, pane: PaneId, cx: &mut Context<Workspace>) -> Div {
    let state = &this.state;
    let Some(Tab::JsonCanvas(tab)) = state
        .layout
        .panes()
        .get(pane)
        .and_then(|pane| pane.active())
    else {
        return div();
    };
    let (id, view, last) = (tab.file(), tab.view, tab.last);
    let file = &state.json_canvases[id];
    let board = file.canvas();
    (this.card_scroll.borrow_mut())
        .retain(|(at, node), _| *at != pane || board.node(node).is_some());
    let area_of = |node: &jsoncanvas::NodeId| board.node(node).map(|node| tab.shown(node));
    let theme = &this.theme;
    let colour = |color: Option<JsonCanvasColor>, fallback: Hsla| colour(theme, color, fallback);
    let border = hsla(theme.chrome.border);
    let accent = hsla(theme.chrome.accent);
    let groups: Vec<(Area, Hsla)> = (board.nodes().iter())
        .filter(|node| matches!(node.kind, NodeKind::Group { .. }))
        .map(|node| (tab.shown(node), colour(node.color, border)))
        .collect();
    let picked = tab
        .arranging()
        .and_then(|arranging| arranging.picked.as_ref());
    let gesture = tab.arranging().map(|arranging| &arranging.gesture);
    let edges: Vec<Edged> = (board.edges().iter())
        .filter_map(|edge| {
            let (from, to) = (area_of(&edge.from.node)?, area_of(&edge.to.node)?);
            let picked = picked.filter(|picked| picked.holds_edge(&edge.id));
            let (color, width) = match picked {
                Some(_) => (accent, EDGE_WIDTH * 2.0),
                None => (colour(edge.color, hsla(theme.muted)), EDGE_WIDTH),
            };
            Some(Edged {
                curve: curve(from, edge.from.side, to, edge.to.side),
                caps: edge.caps(),
                color,
                width,
            })
        })
        .collect();
    let dots = hsla(theme.surface);
    let workspace = cx.entity();
    let backdrop = canvas(
        {
            move |bounds, window, cx| {
                (workspace.read(cx).areas.drawings.borrow_mut()).insert(pane, bounds);
                match pane_space::viewport(bounds) {
                    Some(viewport) => {
                        pane_space::report_measure(&workspace, pane, viewport, last, window, cx)
                    }
                    None => {}
                }
                bounds
            }
        },
        move |_, bounds, window, _| {
            let Some(viewport) = pane_space::viewport(bounds) else {
                return;
            };
            let frame = view.frame(viewport);
            let on_screen = |at: Point<Surface>| {
                let at = frame.to_pane.apply(at);
                point(bounds.origin.x + px(at.x), bounds.origin.y + px(at.y))
            };
            let mut step = GRID as f32;
            while step * frame.scale < DOT_SPACING {
                step *= 2.0;
            }
            let first = frame.from_pane.apply(Point::new(0.0, 0.0));
            let last = frame.from_pane.apply(Point::new(
                f32::from(bounds.size.width),
                f32::from(bounds.size.height),
            ));
            let columns = ((first.x / step).floor() as i64)..=((last.x / step).ceil() as i64);
            let rows = ((first.y / step).floor() as i64)..=((last.y / step).ceil() as i64);
            for row in rows {
                for column in columns.clone() {
                    let at = on_screen(Point::new(column as f32 * step, row as f32 * step));
                    window.paint_quad(
                        gpui::fill(
                            gpui::Bounds::new(
                                point(at.x - px(DOT * 0.5), at.y - px(DOT * 0.5)),
                                gpui::size(px(DOT), px(DOT)),
                            ),
                            dots,
                        )
                        .corner_radii(px(DOT * 0.5)),
                    );
                }
            }
            for (area, color) in &groups {
                let [min, max] = corners(*area);
                let (min, max) = (on_screen(min), on_screen(max));
                let rect = gpui::Bounds::from_corners(min, max);
                window.paint_quad(
                    gpui::fill(rect, color.opacity(0.06))
                        .border_widths(px(2.0))
                        .border_color(color.opacity(0.6))
                        .corner_radii(px(CORNER * frame.scale)),
                );
            }
            for edge in &edges {
                let [start, a, b, end] = edge.curve.map(on_screen);
                let mut path = PathBuilder::stroke(px(edge.width * frame.scale.max(0.5)));
                path.move_to(start);
                path.cubic_bezier_to(end, a, b);
                match path.build() {
                    Ok(path) => window.paint_path(path, edge.color),
                    Err(error) => tracing::warn!(%error, "canvas edge"),
                }
                for (cap, tip, from) in [(edge.caps[0], start, a), (edge.caps[1], end, b)] {
                    match cap {
                        Cap::None => {}
                        Cap::Arrow => {
                            let along = tip - from;
                            let length = f32::from(along.x).hypot(f32::from(along.y)).max(0.001);
                            let (x, y) = (f32::from(along.x) / length, f32::from(along.y) / length);
                            let size = ARROW * frame.scale.max(0.5);
                            let back = point(tip.x - px(x * size), tip.y - px(y * size));
                            let side = point(px(-y * size * 0.5), px(x * size * 0.5));
                            let mut head = PathBuilder::fill();
                            head.move_to(tip);
                            head.line_to(back + side);
                            head.line_to(back - side);
                            head.close();
                            match head.build() {
                                Ok(head) => window.paint_path(head, edge.color),
                                Err(error) => tracing::warn!(%error, "canvas arrow"),
                            }
                        }
                    }
                }
            }
        },
    )
    .absolute()
    .size_full();
    let base = div()
        .id(("canvas", pane.data().as_ffi()))
        .relative()
        .flex_1()
        .min_h_0()
        .size_full()
        .overflow_hidden()
        .bg(hsla(theme.background))
        .on_scroll_wheel(cx.listener(move |this, event: &ScrollWheelEvent, _, cx| {
            drawing::wheel(this, pane, event, cx)
        }))
        .on_mouse_down(
            MouseButton::Middle,
            cx.listener(move |this, event: &MouseDownEvent, _, _| {
                this.drag = Drag::Pan {
                    pane,
                    last: event.position,
                }
            }),
        )
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(move |this, event: &MouseDownEvent, window, cx| {
                window.focus(&this.focus, cx);
                let taps = Taps::of(event.click_count);
                pointing(
                    this,
                    pane,
                    Phase::Down,
                    taps,
                    event.position,
                    event.modifiers,
                    cx,
                )
            }),
        )
        .child(backdrop);
    let Some(viewport) = last.map(|measure| measure.viewport) else {
        return div().size_full().child(base);
    };
    let in_place = match &tab.doing {
        Doing::Writing(writing) => match writing.place {
            Place::InPlace => Some(&writing.written),
            Place::Beside(_) => None,
        },
        Doing::Arranging(_) => None,
    };
    let frame = view.frame(viewport);
    let scale = frame.scale;
    let cards = Cards {
        this,
        pane,
        id,
        file,
        in_place,
        fonts: this.fonts.scaled(scale),
        scale,
        detail: match scale.total_cmp(&LEAST_DETAIL) {
            std::cmp::Ordering::Less => Detail::Outline,
            std::cmp::Ordering::Equal | std::cmp::Ordering::Greater => Detail::Full,
        },
    };
    let fonts = &cards.fonts;
    let place = |at: Point<Surface>| frame.to_pane.apply(at);
    let entity = cx.entity();
    let label_editor = || {
        div()
            .w(px(LABEL_EDITOR * scale))
            .h(this.fonts.cell.height * scale + px(6.0))
            .p(px(2.0))
            .bg(hsla(theme.background))
            .border_1()
            .border_color(accent)
            .rounded_sm()
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .child(TextElement {
                workspace: entity.clone(),
                pane,
                part: Part::Draft,
                size: TextSize::Scaled(scale),
            })
            .into_any_element()
    };
    let labels = (board.nodes().iter()).filter_map(|node| {
        let at = place(corners(tab.shown(node))[0]);
        let shown = match (&node.kind, in_place) {
            (NodeKind::Group { .. }, Some(Written::Label(Labelled::Group(id))))
                if *id == node.id =>
            {
                label_editor()
            }
            (
                NodeKind::Group {
                    label: Some(label), ..
                },
                _,
            ) => div()
                .text_size(fonts.ui_size)
                .text_color(hsla(theme.muted))
                .child(SharedString::from(label.clone()))
                .into_any_element(),
            (
                NodeKind::Group { label: None, .. }
                | NodeKind::Text(_)
                | NodeKind::File { .. }
                | NodeKind::Link(_),
                _,
            ) => return None,
        };
        Some(
            div()
                .absolute()
                .left(px(at.x))
                .top(px(at.y))
                .size_0()
                .flex()
                .flex_col()
                .justify_end()
                .child(div().flex_none().child(shown))
                .into_any_element(),
        )
    });
    let edge_labels = (board.edges().iter()).filter_map(|edge| {
        let middle = along(
            curve(
                area_of(&edge.from.node)?,
                edge.from.side,
                area_of(&edge.to.node)?,
                edge.to.side,
            ),
            0.5,
        );
        let at = place(middle);
        let shown = match (&edge.label, in_place) {
            (_, Some(Written::Label(Labelled::Edge(id)))) if *id == edge.id => label_editor(),
            (Some(label), _) => div()
                .whitespace_nowrap()
                .px(fonts.ui_size * 0.4)
                .rounded_sm()
                .bg(hsla(theme.background))
                .text_size(fonts.ui_size)
                .text_color(hsla(theme.text))
                .child(SharedString::from(label.clone()))
                .into_any_element(),
            (None, _) => return None,
        };
        Some(
            div()
                .absolute()
                .left(px(at.x))
                .top(px(at.y))
                .size_0()
                .flex()
                .items_center()
                .justify_center()
                .child(div().flex_none().child(shown))
                .into_any_element(),
        )
    });
    let labels: Vec<AnyElement> = labels.collect();
    let edge_labels: Vec<AnyElement> = edge_labels.collect();
    let shown: Vec<AnyElement> = (board.nodes().iter().enumerate())
        .filter(|(_, node)| !matches!(node.kind, NodeKind::Group { .. }))
        .map(|(index, node)| {
            let area = tab.shown(node);
            cards.card(index, node, area, place(corners(area)[0]), cx)
        })
        .collect();
    let dot_fill = hsla(theme.background);
    let picked_ids: Vec<jsoncanvas::NodeId> = (picked.iter())
        .flat_map(|picked| picked.nodes())
        .cloned()
        .collect();
    let picked: Vec<Area> = (picked.iter())
        .flat_map(|picked| picked.nodes())
        .filter_map(area_of)
        .collect();
    let around = (picked.iter().copied().map(surface)).reduce(|a, b| a.union(&b));
    let boxing = match gesture {
        Some(Gesture::Boxing { from, to, .. }) => Some([*from, *to]),
        Some(
            Gesture::Idle
            | Gesture::Pressed { .. }
            | Gesture::Moving(_)
            | Gesture::Resizing(_)
            | Gesture::Connecting { .. },
        )
        | None => None,
    };
    let connecting = match gesture {
        Some(Gesture::Connecting { side, area, to, .. }) => Some((anchor(*area, *side).0, *to)),
        Some(
            Gesture::Idle
            | Gesture::Pressed { .. }
            | Gesture::Moving(_)
            | Gesture::Resizing(_)
            | Gesture::Boxing { .. },
        )
        | None => None,
    };
    let held = match gesture {
        Some(Gesture::Idle) | None => Held::Free,
        Some(
            Gesture::Pressed { .. }
            | Gesture::Moving(_)
            | Gesture::Resizing(_)
            | Gesture::Boxing { .. }
            | Gesture::Connecting { .. },
        ) => Held::Down,
    };
    let workspace = cx.entity();
    let overlay = canvas(
        |_, _, _| (),
        move |bounds, _, window, _| {
            let on_screen = |at: Point<Surface>| {
                let at = frame.to_pane.apply(at);
                point(bounds.origin.x + px(at.x), bounds.origin.y + px(at.y))
            };
            for area in &picked {
                let [min, max] = corners(*area);
                let rect = gpui::Bounds::from_corners(on_screen(min), on_screen(max));
                window.paint_quad(
                    gpui::outline(rect, accent, gpui::BorderStyle::Solid)
                        .corner_radii(px(CORNER * frame.scale)),
                );
                for side in SIDES {
                    let at = on_screen(anchor(*area, side).0);
                    let dot = px(HANDLE);
                    window.paint_quad(
                        gpui::fill(
                            gpui::Bounds::new(
                                point(at.x - dot * 0.5, at.y - dot * 0.5),
                                gpui::size(dot, dot),
                            ),
                            dot_fill,
                        )
                        .border_widths(px(1.5))
                        .border_color(accent)
                        .corner_radii(dot * 0.5),
                    );
                }
                for corner in [min, Point::new(max.x, min.y), max, Point::new(min.x, max.y)] {
                    let at = on_screen(corner);
                    let side = px(HANDLE);
                    window.paint_quad(gpui::fill(
                        gpui::Bounds::new(
                            point(at.x - side * 0.5, at.y - side * 0.5),
                            gpui::size(side, side),
                        ),
                        accent,
                    ));
                }
            }
            match connecting {
                Some((start, end)) => {
                    let mut line = PathBuilder::stroke(px(EDGE_WIDTH));
                    line.move_to(on_screen(start));
                    line.line_to(on_screen(end));
                    match line.build() {
                        Ok(line) => window.paint_path(line, accent),
                        Err(error) => tracing::warn!(%error, "canvas connection"),
                    }
                }
                None => {}
            }
            match boxing {
                Some([from, to]) => {
                    let rect = gpui::Bounds::from_corners(on_screen(from), on_screen(to));
                    window.paint_quad(
                        gpui::fill(rect, accent.opacity(0.1))
                            .border_widths(px(1.0))
                            .border_color(accent),
                    );
                }
                None => {}
            }
            match held {
                Held::Free => {}
                Held::Down => {
                    window.on_mouse_event({
                        let workspace = workspace.clone();
                        move |event: &MouseMoveEvent, phase, _, cx| match phase {
                            DispatchPhase::Bubble => workspace.update(cx, |this, cx| {
                                pointing(
                                    this,
                                    pane,
                                    Phase::Move,
                                    Taps::One,
                                    event.position,
                                    event.modifiers,
                                    cx,
                                )
                            }),
                            DispatchPhase::Capture => {}
                        }
                    });
                    window.on_mouse_event({
                        let workspace = workspace.clone();
                        move |event: &MouseUpEvent, phase, _, cx| match (phase, event.button) {
                            (DispatchPhase::Bubble, MouseButton::Left) => {
                                workspace.update(cx, |this, cx| {
                                    pointing(
                                        this,
                                        pane,
                                        Phase::Up,
                                        Taps::One,
                                        event.position,
                                        event.modifiers,
                                        cx,
                                    )
                                })
                            }
                            _ => {}
                        }
                    });
                }
            }
        },
    )
    .absolute()
    .size_full();
    let tools = match (gesture, around) {
        (Some(Gesture::Idle), Some(around)) => {
            let icons: &[Icon] = match picked_ids.as_slice() {
                [only]
                    if matches!(
                        board.node(only).map(|node| &node.kind),
                        Some(NodeKind::Text(_))
                    ) =>
                {
                    &[Icon::Delete, Icon::Colour, Icon::Focus, Icon::Edit]
                }
                _ => &[Icon::Delete, Icon::Colour, Icon::Focus],
            };
            let top = place(around.min());
            let middle = place(Point::new((around.min().x + around.max().x) / 2.0, 0.0));
            Some(toolbar(this, pane, (middle.x, top.y), icons, cx))
        }
        (
            Some(
                Gesture::Idle
                | Gesture::Pressed { .. }
                | Gesture::Moving(_)
                | Gesture::Resizing(_)
                | Gesture::Boxing { .. }
                | Gesture::Connecting { .. },
            )
            | None,
            _,
        ) => None,
    };
    div().size_full().child(
        base.children(labels)
            .children(edge_labels)
            .children(shown)
            .child(overlay)
            .children(tools),
    )
}

fn colour(theme: &model::theme::Theme, color: Option<JsonCanvasColor>, fallback: Hsla) -> Hsla {
    match color {
        None => fallback,
        Some(JsonCanvasColor::Hex(rgb)) => drawing::solid(rgb),
        Some(JsonCanvasColor::Preset(preset)) => hsla(
            theme.syntax[match preset {
                Preset::Red => Base16::Base08,
                Preset::Orange => Base16::Base09,
                Preset::Yellow => Base16::Base0A,
                Preset::Green => Base16::Base0B,
                Preset::Cyan => Base16::Base0C,
                Preset::Purple => Base16::Base0E,
            }],
        ),
    }
}

impl Cards<'_> {
    fn card(
        &self,
        index: usize,
        node: &Node,
        area: Area,
        at: Point<model::geom::Pane>,
        cx: &mut Context<Workspace>,
    ) -> AnyElement {
        let Cards {
            this,
            pane,
            id,
            file,
            in_place,
            ref fonts,
            scale,
            detail,
        } = *self;
        let theme = &this.theme;
        let border = colour(theme, node.color, hsla(theme.chrome.border));
        let (width, height) = (
            area.width.get() as f32 * scale,
            area.height.get() as f32 * scale,
        );
        let shell = div()
            .id(("card", index))
            .absolute()
            .left(px(at.x))
            .top(px(at.y))
            .w(px(width))
            .h(px(height))
            .rounded(px(CORNER * scale))
            .border_2()
            .border_color(border)
            .bg(match node.color {
                Some(_) => hsla(theme.background).blend(border.opacity(CARD_TINT)),
                None => hsla(theme.surface),
            })
            .overflow_hidden()
            .font(fonts.prose[0].clone())
            .text_size(fonts.prose_size)
            .text_color(hsla(theme.text));
        let name = |path: &std::path::Path| {
            SharedString::from(
                path.file_name()
                    .map_or(String::new(), |name| name.to_string_lossy().into_owned()),
            )
        };
        let opens = cx.listener(
            move |this, event: &ClickEvent, _, cx| match event.click_count() {
                2 => {
                    state::json_canvas::open_node(
                        &mut this.state,
                        pane,
                        id,
                        index,
                        &mut this.effects,
                    );
                    this.changed(cx);
                }
                _ => {}
            },
        );
        let plain = |text: SharedString| {
            div()
                .p(px(PADDING * scale))
                .text_color(hsla(theme.muted))
                .child(text)
                .into_any_element()
        };
        let markdown = |cx: &mut Context<Workspace>| {
            let flow = &file.card(&node.id)?.flow;
            let none = |_: usize| Tints::new();
            let items: Vec<AnyElement> = (0..flow.roots.len())
                .map(|root| {
                    let from = document::From {
                        margins: document::Margins::Card,
                        fonts,
                        flow,
                        key: NoteKey::Card(id, index),
                        version: 0,
                        toggled: &[],
                        marked: &none,
                    };
                    document::item(this, pane, from, root, cx)
                })
                .collect();
            let scroll = (this.card_scroll.borrow_mut())
                .entry((pane, node.id.clone()))
                .or_default()
                .clone();
            let held = scroll.clone();
            Some(
                div()
                    .size_full()
                    .on_scroll_wheel(move |_, _, cx| {
                        match held.max_offset().y.partial_cmp(&px(0.0)) {
                            Some(std::cmp::Ordering::Greater) => cx.stop_propagation(),
                            Some(std::cmp::Ordering::Less | std::cmp::Ordering::Equal) | None => {}
                        }
                    })
                    .child(
                        div()
                            .id(("card-text", index))
                            .track_scroll(&scroll)
                            .size_full()
                            .overflow_y_scroll()
                            .p(px(PADDING * scale))
                            .flex()
                            .flex_col()
                            .gap(fonts.prose_size * 0.5)
                            .children(items),
                    )
                    .into_any_element(),
            )
        };
        let written =
            in_place.filter(|written| matches!(written, Written::Card(id) if *id == node.id));
        let content = match (written, &node.kind, file.files.get(&node.id), detail) {
            (Some(_), _, _, _) => div()
                .size_full()
                .overflow_hidden()
                .child(TextElement {
                    workspace: cx.entity(),
                    pane,
                    part: Part::Draft,
                    size: TextSize::Scaled(scale),
                })
                .into_any_element(),
            (None, NodeKind::Text(text), _, Detail::Outline) => plain(SharedString::from(
                text.lines().next().unwrap_or_default().to_owned(),
            )),
            (None, NodeKind::Text(text), _, Detail::Full) => {
                markdown(cx).unwrap_or_else(|| plain(SharedString::from(text.clone())))
            }
            (None, NodeKind::File { .. }, Some(Embedded::Note { path, .. }), Detail::Outline) => {
                plain(name(path))
            }
            (None, NodeKind::File { .. }, Some(Embedded::Note { path, .. }), Detail::Full) => {
                markdown(cx).unwrap_or_else(|| plain(name(path)))
            }
            (None, NodeKind::File { .. }, Some(Embedded::Picture(picture)), _) => {
                picture_canvas(&this.pictures, &cx.weak_entity(), picture.clone())
                    .size_full()
                    .into_any_element()
            }
            (None, NodeKind::File { .. }, Some(Embedded::Other { path, .. }), _) => {
                plain(name(path))
            }
            (None, NodeKind::File { file, .. }, Some(Embedded::Missing(_)), _) => {
                plain(SharedString::from(format!("{file} (missing)")))
            }
            (None, NodeKind::File { file, .. }, None, _) => plain(SharedString::from(file.clone())),
            (None, NodeKind::Link(url), _, _) => div()
                .p(px(PADDING * scale))
                .text_color(hsla(theme.prose.link))
                .underline()
                .child(SharedString::from(url.clone()))
                .into_any_element(),
            (None, NodeKind::Group { .. }, _, _) => div().into_any_element(),
        };
        let shell = match &node.kind {
            NodeKind::Link(_) | NodeKind::File { .. } => {
                let hover = hsla(theme.chrome.hover);
                shell.cursor_pointer().hover(move |style| style.bg(hover))
            }
            NodeKind::Text(_) | NodeKind::Group { .. } => shell,
        };
        shell.on_click(opens).child(content).into_any_element()
    }
}

fn corners(area: Area) -> [Point<Surface>; 2] {
    let bounds = surface(area);
    [bounds.min(), bounds.max()]
}

fn pointing(
    this: &mut Workspace,
    pane: PaneId,
    phase: Phase,
    taps: Taps,
    position: gpui::Point<gpui::Pixels>,
    modifiers: Modifiers,
    cx: &mut Context<Workspace>,
) {
    let Some((at, viewport)) = drawing::on_pane(this, pane, position) else {
        return;
    };
    let pointer = Pointer {
        at,
        viewport,
        adding: match modifiers.shift {
            true => Adding::Toggle,
            false => Adding::Replace,
        },
        snap: match modifiers.alt {
            true => Snap::Free,
            false => Snap::Grid,
        },
        taps,
    };
    state::json_canvas::pointer(&mut this.state, pane, phase, pointer);
    this.flush(cx);
    cx.notify();
}

#[derive(Clone, Copy)]
enum Icon {
    Delete,
    Colour,
    Focus,
    Edit,
}

fn toolbar(
    this: &Workspace,
    pane: PaneId,
    (middle, top): (f32, f32),
    icons: &[Icon],
    cx: &mut Context<Workspace>,
) -> AnyElement {
    let theme = &this.theme;
    let (text, hover, surface) = (
        hsla(theme.text),
        hsla(theme.chrome.hover),
        hsla(theme.surface),
    );
    let button = |icon: Icon, cx: &mut Context<Workspace>| {
        div()
            .id(("canvas-tool", icon as usize))
            .size(px(TOOL))
            .flex()
            .items_center()
            .justify_center()
            .rounded_sm()
            .hover(move |style| style.bg(hover))
            .cursor_pointer()
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                let command = match icon {
                    Icon::Delete => Some(JsonCanvasCommand::Delete),
                    Icon::Focus => Some(JsonCanvasCommand::Focus),
                    Icon::Edit => Some(JsonCanvasCommand::Write),
                    Icon::Colour => None,
                };
                this.colour_menu = match (command, this.colour_menu) {
                    (Some(_), _) => None,
                    (None, Some(open)) if open == pane => None,
                    (None, Some(_) | None) => Some(pane),
                };
                match command {
                    Some(command) => state::json_canvas::command(&mut this.state, command),
                    None => {}
                }
                window.focus(&this.focus, cx);
                this.changed(cx);
            }))
            .child(icon_shape(icon, text))
    };
    let buttons: Vec<Stateful<Div>> = icons.iter().map(|icon| button(*icon, cx)).collect();
    let swatches = match this.colour_menu {
        Some(open) if open == pane => Some(()),
        Some(_) | None => None,
    }
    .map(|()| {
        let presets = [
            None,
            Some(Preset::Red),
            Some(Preset::Orange),
            Some(Preset::Yellow),
            Some(Preset::Green),
            Some(Preset::Cyan),
            Some(Preset::Purple),
        ];
        div()
            .flex()
            .flex_row()
            .gap(px(4.0))
            .p(px(4.0))
            .rounded_md()
            .bg(surface)
            .children(presets.into_iter().enumerate().map(|(at, preset)| {
                let fill = colour(theme, preset.map(JsonCanvasColor::Preset), surface);
                div()
                    .id(("canvas-swatch", at))
                    .size(px(ICON + 4.0))
                    .rounded_full()
                    .bg(fill)
                    .border_1()
                    .border_color(text)
                    .cursor_pointer()
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                        this.colour_menu = None;
                        state::json_canvas::command(
                            &mut this.state,
                            JsonCanvasCommand::Recolor(preset),
                        );
                        window.focus(&this.focus, cx);
                        this.changed(cx);
                    }))
            }))
    });
    div()
        .absolute()
        .left(px(middle))
        .top(px(top - TOOL - 12.0))
        .size_0()
        .flex()
        .flex_col()
        .items_center()
        .child(
            div()
                .flex_none()
                .flex()
                .flex_col()
                .items_center()
                .gap(px(4.0))
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .p(px(2.0))
                        .rounded_md()
                        .bg(surface)
                        .border_1()
                        .border_color(hsla(theme.chrome.border))
                        .children(buttons),
                )
                .children(swatches),
        )
        .into_any_element()
}

fn icon_shape(icon: Icon, color: Hsla) -> impl IntoElement {
    canvas(
        |_, _, _| (),
        move |bounds, _, window, _| {
            let (origin, side) = (bounds.origin, f32::from(bounds.size.width));
            let at = |x: f32, y: f32| point(origin.x + px(x * side), origin.y + px(y * side));
            let mut path = PathBuilder::stroke(px(1.5));
            let strokes: &[&[(f32, f32)]] = match icon {
                Icon::Delete => &[
                    &[(0.1, 0.25), (0.9, 0.25)],
                    &[(0.38, 0.25), (0.38, 0.1), (0.62, 0.1), (0.62, 0.25)],
                    &[(0.22, 0.25), (0.28, 0.92), (0.72, 0.92), (0.78, 0.25)],
                ],
                Icon::Colour => &[&[
                    (0.5, 0.08),
                    (0.8, 0.2),
                    (0.92, 0.5),
                    (0.8, 0.8),
                    (0.5, 0.92),
                    (0.2, 0.8),
                    (0.08, 0.5),
                    (0.2, 0.2),
                    (0.5, 0.08),
                ]],
                Icon::Focus => &[
                    &[(0.08, 0.35), (0.08, 0.08), (0.35, 0.08)],
                    &[(0.65, 0.08), (0.92, 0.08), (0.92, 0.35)],
                    &[(0.92, 0.65), (0.92, 0.92), (0.65, 0.92)],
                    &[(0.35, 0.92), (0.08, 0.92), (0.08, 0.65)],
                ],
                Icon::Edit => &[
                    &[
                        (0.15, 0.85),
                        (0.2, 0.65),
                        (0.7, 0.15),
                        (0.85, 0.3),
                        (0.35, 0.8),
                        (0.15, 0.85),
                    ],
                    &[(0.6, 0.25), (0.75, 0.4)],
                ],
            };
            for stroke in strokes {
                let mut points = stroke.iter().map(|(x, y)| at(*x, *y));
                match points.next() {
                    Some(first) => path.move_to(first),
                    None => continue,
                }
                for point in points {
                    path.line_to(point);
                }
            }
            match path.build() {
                Ok(path) => window.paint_path(path, color),
                Err(error) => tracing::warn!(%error, "canvas icon"),
            }
        },
    )
    .size(px(ICON))
}
