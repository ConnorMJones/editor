//! The window's own chrome: client-side decorations and the title bar.

use gpui::{
    Bounds, BoxShadow, ClickEvent, Context, Corners, CursorStyle, Decorations, Div, Edges,
    HitboxBehavior, Hsla, IntoElement, MouseButton, MouseMoveEvent, Pixels, Point, ResizeEdge,
    Size, Stateful, Tiling, Window, WindowButton, WindowButtonLayout, WindowControlArea, canvas,
    div, point, prelude::*, px, size, transparent_black,
};
use model::command::Command;
use state::apply;

use super::{Drag, Workspace};
use crate::fonts::hsla;

pub const TITLE_HEIGHT: Pixels = px(32.0);
pub const SHADOW: Pixels = px(10.0);
pub const ROUNDING: Pixels = px(10.0);
pub const BORDER: Pixels = px(1.0);

/// Port of zed's `resize_edge`: the window edge or corner within reach of `pos`.
pub fn resize_edge(pos: Point<Pixels>, window: Size<Pixels>, tiling: Tiling) -> Option<ResizeEdge> {
    let inner = Bounds::new(Point::default(), window).inset(SHADOW * 1.5);
    let corner = size(SHADOW * 1.5, SHADOW * 1.5);
    let near = |x: Pixels, y: Pixels| Bounds::new(point(x, y), corner).contains(&pos);
    let (right, bottom) = (window.width - corner.width, window.height - corner.height);
    match () {
        _ if inner.contains(&pos) => None,
        _ if !tiling.top && near(px(0.), px(0.)) => Some(ResizeEdge::TopLeft),
        _ if !tiling.top && near(right, px(0.)) => Some(ResizeEdge::TopRight),
        _ if !tiling.bottom && near(px(0.), bottom) => Some(ResizeEdge::BottomLeft),
        _ if !tiling.bottom && near(right, bottom) => Some(ResizeEdge::BottomRight),
        _ if !tiling.top && pos.y < SHADOW => Some(ResizeEdge::Top),
        _ if !tiling.bottom && pos.y > window.height - SHADOW => Some(ResizeEdge::Bottom),
        _ if !tiling.left && pos.x < SHADOW => Some(ResizeEdge::Left),
        _ if !tiling.right && pos.x > window.width - SHADOW => Some(ResizeEdge::Right),
        _ => None,
    }
}

fn tiling(window: &Window) -> Tiling {
    match window.window_decorations() {
        Decorations::Server => Tiling::tiled(),
        Decorations::Client { tiling } => tiling,
    }
}

/// `width` on each edge not tiled against a neighbour or the screen.
fn free_edges(tiling: Tiling, width: Pixels) -> Edges<Pixels> {
    let free = |tiled: bool| match tiled {
        true => px(0.),
        false => width,
    };
    Edges {
        top: free(tiling.top),
        right: free(tiling.right),
        bottom: free(tiling.bottom),
        left: free(tiling.left),
    }
}

/// A corner rounds only when both its edges are free.
fn free_corners(tiling: Tiling) -> Corners<Pixels> {
    let edges = free_edges(tiling, ROUNDING);
    Corners {
        top_left: edges.top.min(edges.left),
        top_right: edges.top.min(edges.right),
        bottom_right: edges.bottom.min(edges.right),
        bottom_left: edges.bottom.min(edges.left),
    }
}

pub fn corners(window: &Window) -> Corners<Pixels> {
    free_corners(tiling(window))
}

pub fn round<E: Styled>(element: E, corners: Corners<Pixels>) -> E {
    element
        .rounded_tl(corners.top_left)
        .rounded_tr(corners.top_right)
        .rounded_br(corners.bottom_right)
        .rounded_bl(corners.bottom_left)
}

impl Workspace {
    /// Zed's client-side decorations: shadow, rounded border and resize edges unless tiled.
    pub fn decorations(
        &self,
        content: Stateful<Div>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let decorations = window.window_decorations();
        let tiling = tiling(window);
        window.set_client_inset(match decorations {
            Decorations::Server => px(0.),
            Decorations::Client { .. } => SHADOW,
        });
        let resizing = match (decorations, window.is_resizable()) {
            (Decorations::Client { tiling }, true) => Some(tiling),
            (Decorations::Server, _) | (Decorations::Client { .. }, false) => None,
        };
        let corners = free_corners(tiling);
        let inset = free_edges(tiling, SHADOW);
        let border = free_edges(tiling, BORDER);
        let backdrop = round(div().id("window-backdrop"), corners)
            .size_full()
            .bg(transparent_black())
            .pt(inset.top)
            .pr(inset.right)
            .pb(inset.bottom)
            .pl(inset.left)
            .when_some(resizing, |backdrop, tiling| {
                backdrop
                    .on_mouse_move(
                        cx.listener(move |this, event: &MouseMoveEvent, window, cx| {
                            let size = window.window_bounds().get_bounds().size;
                            let edge = resize_edge(event.position, size, tiling);
                            match edge == this.resize_edge {
                                true => {}
                                false => {
                                    this.resize_edge = edge;
                                    cx.notify();
                                }
                            }
                        }),
                    )
                    .on_mouse_down(MouseButton::Left, move |event, window, _| {
                        let size = window.window_bounds().get_bounds().size;
                        match resize_edge(event.position, size, tiling) {
                            Some(edge) => window.start_window_resize(edge),
                            None => {}
                        }
                    })
            });
        let frame = round(div(), corners)
            .cursor(CursorStyle::Arrow)
            .size_full()
            .border_color(hsla(self.theme.chrome.border))
            .border_t(border.top)
            .border_r(border.right)
            .border_b(border.bottom)
            .border_l(border.left)
            .on_mouse_move(|_, _, cx| cx.stop_propagation())
            .when(!tiling.is_tiled(), |frame| {
                frame.shadow(vec![BoxShadow {
                    color: Hsla {
                        h: 0.,
                        s: 0.,
                        l: 0.,
                        a: 0.4,
                    },
                    offset: point(px(0.), px(0.)),
                    blur_radius: SHADOW / 2.,
                    spread_radius: px(0.),
                    inset: false,
                }])
            });
        backdrop
            .child(frame.child(content))
            .when_some(resizing, |backdrop, tiling| {
                backdrop.child(
                    canvas(
                        |_, window, _| {
                            window.insert_hitbox(
                                Bounds::new(
                                    point(px(0.), px(0.)),
                                    window.window_bounds().get_bounds().size,
                                ),
                                HitboxBehavior::Normal,
                            )
                        },
                        move |_, hitbox, window, _| {
                            let size = window.window_bounds().get_bounds().size;
                            let style = match resize_edge(window.mouse_position(), size, tiling) {
                                None => return,
                                Some(ResizeEdge::Top | ResizeEdge::Bottom) => {
                                    CursorStyle::ResizeUpDown
                                }
                                Some(ResizeEdge::Left | ResizeEdge::Right) => {
                                    CursorStyle::ResizeLeftRight
                                }
                                Some(ResizeEdge::TopLeft | ResizeEdge::BottomRight) => {
                                    CursorStyle::ResizeUpLeftDownRight
                                }
                                Some(ResizeEdge::TopRight | ResizeEdge::BottomLeft) => {
                                    CursorStyle::ResizeUpRightDownLeft
                                }
                            };
                            window.set_cursor_style(style, &hitbox);
                        },
                    )
                    .size_full()
                    .absolute(),
                )
            })
    }

    /// Drag to move, double-click to maximize, right-click for the window menu.
    pub fn titlebar(&self, window: &Window, cx: &mut Context<Self>) -> Stateful<Div> {
        let theme = &self.theme;
        let fonts = &self.fonts;
        let controls = window.window_controls();
        let layout = cx
            .button_layout()
            .unwrap_or_else(WindowButtonLayout::linux_default);
        let maximized = window.is_maximized();
        let icon = hsla(theme.chrome.title_text);
        let buttons = |side: [Option<WindowButton>; gpui::MAX_BUTTONS_PER_SIDE],
                       cx: &mut Context<Self>| {
            side.into_iter()
                .flatten()
                .filter(|button| match button {
                    WindowButton::Minimize => controls.minimize,
                    WindowButton::Maximize => controls.maximize,
                    WindowButton::Close => true,
                })
                .map(|button| {
                    let (id, glyph, hover) = match (button, maximized) {
                        (WindowButton::Minimize, _) => (
                            "minimize",
                            div().w(px(10.)).h(px(1.)).bg(icon).into_any_element(),
                            hsla(theme.chrome.hover),
                        ),
                        (WindowButton::Maximize, false) => (
                            "maximize",
                            div()
                                .size(px(10.))
                                .border_1()
                                .border_color(icon)
                                .into_any_element(),
                            hsla(theme.chrome.hover),
                        ),
                        (WindowButton::Maximize, true) => (
                            "restore",
                            div()
                                .size(px(8.))
                                .border_1()
                                .border_color(icon)
                                .shadow(vec![BoxShadow {
                                    color: icon,
                                    offset: point(px(2.), px(-2.)),
                                    blur_radius: px(0.),
                                    spread_radius: px(0.),
                                    inset: false,
                                }])
                                .into_any_element(),
                            hsla(theme.chrome.hover),
                        ),
                        (WindowButton::Close, _) => (
                            "close",
                            div().text_color(icon).child("✕").into_any_element(),
                            hsla(theme.chrome.close_hover),
                        ),
                    };
                    div()
                        .id(id)
                        .w(px(30.))
                        .h(px(22.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded(px(4.))
                        .hover(move |style| style.bg(hover))
                        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                        .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                            cx.stop_propagation();
                            match button {
                                WindowButton::Minimize => window.minimize_window(),
                                WindowButton::Maximize => window.zoom_window(),
                                WindowButton::Close => {
                                    apply::apply(&mut this.state, Command::Quit, &mut this.effects);
                                    window.focus(&this.focus, cx);
                                    this.changed(cx);
                                }
                            }
                        }))
                        .child(glyph)
                })
                .collect::<Vec<_>>()
        };
        let left = buttons(layout.left, cx);
        let right = buttons(layout.right, cx);
        let root = self.state.project.root.path();
        let name = root.file_name().map_or_else(
            || root.display().to_string(),
            |name| name.to_string_lossy().into_owned(),
        );
        let bar = div()
            .id("titlebar")
            .window_control_area(WindowControlArea::Drag)
            .flex_none()
            .h(TITLE_HEIGHT)
            .w_full()
            .flex()
            .flex_row()
            .items_center()
            .gap_1()
            .px_2()
            .bg(hsla(theme.chrome.title))
            .text_color(hsla(theme.chrome.title_text))
            .font(fonts.ui.clone())
            .text_size(fonts.ui_size)
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _, _| this.drag = Drag::Title),
            )
            .children(left)
            .child(div().flex_1())
            .child(div().text_color(hsla(theme.text)).child(name))
            .child(
                div()
                    .pl_2()
                    .text_color(hsla(theme.muted))
                    .child(root.display().to_string()),
            )
            .child(div().flex_1())
            .children(right);
        let corners = corners(window);
        bar.rounded_tl(corners.top_left)
            .rounded_tr(corners.top_right)
            .when(controls.maximize && window.is_resizable(), |bar| {
                bar.on_click(|event, window, _| match event.click_count() {
                    2 => window.zoom_window(),
                    _ => {}
                })
            })
            .when(controls.window_menu, |bar| {
                bar.on_mouse_down(MouseButton::Right, |event, window, _| {
                    window.show_window_menu(event.position)
                })
            })
    }
}
