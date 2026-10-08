use std::cell::Cell;
use std::ops::Range;
use std::path::PathBuf;
use std::rc::Rc;

use gpui::{
    AnyElement, Bounds, BoxShadow, ClickEvent, ClipboardItem, Context, CursorStyle, Decorations,
    Div, ElementId, ElementInputHandler, EntityInputHandler, HitboxBehavior, Hsla, IntoElement,
    KeyDownEvent, MouseButton, MouseMoveEvent, Pixels, Point, Render, ResizeEdge, ScrollStrategy,
    Size, Stateful, Tiling, UTF16Selection, UniformListScrollHandle, Window, WindowButton,
    WindowButtonLayout, WindowControlArea, canvas, div, point, prelude::*, px, size,
    transparent_black, uniform_list,
};
use helix_core::text_annotations::TextAnnotations;
use helix_core::visual_offset_from_anchor;
use model::command::{Command, PickerCommand};
use model::register::Clipboard;
use model::window::Axis;
use slotmap::Key as _;
use state::buffer::{Content, Disk};
use state::display::{self, Gutter, Indent};
use state::job::{self, JobResult};
use state::keymap::{self, Context as KeyContext};
use state::layout::{Node, PaneId, SplitPath, View};
use state::picker::{Items, PALETTE, Search};
use state::tree::{self, Fold, RowKind, TreeEdit};
use state::watch::{self, Watcher};
use state::{
    Effect, Focus, Overlay, Repo, Sidebar, SidebarWidth, State, apply, input, register, session,
    workspace,
};

use crate::preview::PreviewElement;
use crate::text::TextElement;
use crate::theme::Theme;

pub struct Workspace {
    pub state: State,
    pub theme: Theme,
    pub effects: Vec<Effect>,
    pub focus: gpui::FocusHandle,
    pub watcher: Watcher,
    pub tree_scroll: UniformListScrollHandle,
    pub picker_scroll: UniformListScrollHandle,
    /// Uncommitted IME composition, drawn at the primary cursor.
    pub preedit: String,
    pub session: SessionFile,
    pub drag: Drag,
    /// Window edge under the pointer; a change redraws so the cursor follows it.
    pub resize_edge: Option<ResizeEdge>,
    /// Bounds drawn last frame, which drags measure against.
    pub areas: Rc<Areas>,
}

/// Where the session is saved on exit, per `session.restore`.
pub enum SessionFile {
    Disabled,
    At(PathBuf),
}

/// Chrome held by the left mouse button.
#[derive(Clone, Debug, PartialEq)]
pub enum Drag {
    Idle,
    /// Pressed on the title bar; moving starts a compositor window move.
    Title,
    Sidebar,
    Divider {
        path: SplitPath,
        divider: usize,
        axis: Axis,
    },
}

#[derive(Default)]
pub struct Areas {
    pub editor: Cell<Bounds<Pixels>>,
    pub sidebar: Cell<Bounds<Pixels>>,
}

const INDENT: f32 = 14.0;
const TITLE_HEIGHT: Pixels = px(32.0);
/// Hit width of split and sidebar dividers; a 1px line is drawn in the middle.
const DIVIDER: Pixels = px(5.0);
const SHADOW: Pixels = px(10.0);
const ROUNDING: Pixels = px(10.0);
const BORDER: Pixels = px(1.0);

/// Port of zed's `resize_edge`: the window edge or corner within reach of `pos`.
fn resize_edge(pos: Point<Pixels>, window: Size<Pixels>, tiling: Tiling) -> Option<ResizeEdge> {
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

/// Rounds every corner whose two edges are untiled.
fn round_corners<E: Styled>(element: E, tiling: Tiling) -> E {
    let element = match tiling.top || tiling.left {
        true => element,
        false => element.rounded_tl(ROUNDING),
    };
    let element = match tiling.top || tiling.right {
        true => element,
        false => element.rounded_tr(ROUNDING),
    };
    let element = match tiling.bottom || tiling.left {
        true => element,
        false => element.rounded_bl(ROUNDING),
    };
    match tiling.bottom || tiling.right {
        true => element,
        false => element.rounded_br(ROUNDING),
    }
}

/// A draggable gap between panes along `axis`, or beside the sidebar.
fn divider(axis: Axis, color: Hsla) -> Div {
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
    fn on_key(&mut self, event: &KeyDownEvent, cx: &mut Context<Self>) {
        let Some(key) = crate::input::translate(&event.keystroke) else {
            return;
        };
        match (input::route(&self.state, key), &event.keystroke.key_char) {
            // Left to propagate so the platform delivers it through the input handler.
            (input::Route::Text, Some(_)) => return,
            (input::Route::Text, None) | (input::Route::Keymap, _) => {}
        }
        cx.stop_propagation();
        input::key(&mut self.state, key, &mut self.effects);
        self.changed(cx);
    }

    /// Runs queued effects, keeps list cursors in view and redraws after a state change.
    fn changed(&mut self, cx: &mut Context<Self>) {
        self.tree_scroll
            .scroll_to_item(self.state.tree.cursor, ScrollStrategy::Nearest);
        match &self.state.overlay {
            Overlay::Picker(picker) => self
                .picker_scroll
                .scroll_to_item(picker.cursor, ScrollStrategy::Nearest),
            Overlay::None | Overlay::Prompt(_) => {}
        }
        self.flush(cx);
        cx.notify();
    }

    fn on_job(&mut self, result: JobResult, cx: &mut Context<Self>) {
        job::complete(&mut self.state, result, &mut self.effects);
        self.flush(cx);
        cx.notify();
    }

    pub fn on_paths(&mut self, paths: Vec<PathBuf>, cx: &mut Context<Self>) {
        watch::changed(&mut self.state, paths, &mut self.effects);
        self.flush(cx);
        cx.notify();
    }

    fn on_drag(&mut self, event: &MouseMoveEvent, window: &mut Window, cx: &mut Context<Self>) {
        match (&self.drag, event.pressed_button) {
            (Drag::Idle, _) => {}
            // The button was released outside the window.
            (_, None | Some(MouseButton::Right | MouseButton::Middle))
            | (_, Some(MouseButton::Navigate(_))) => self.drag = Drag::Idle,
            (Drag::Title, Some(MouseButton::Left)) => {
                self.drag = Drag::Idle;
                window.start_window_move();
            }
            (Drag::Sidebar, Some(MouseButton::Left)) => {
                let left = self.areas.sidebar.get().origin.x;
                self.state.sidebar_width = SidebarWidth::new((event.position.x - left) / px(1.));
                cx.notify();
            }
            (
                Drag::Divider {
                    path,
                    divider,
                    axis,
                },
                Some(MouseButton::Left),
            ) => {
                let area = self.areas.editor.get();
                let at = match axis {
                    Axis::Row => (event.position.x - area.origin.x) / area.size.width,
                    Axis::Column => (event.position.y - area.origin.y) / area.size.height,
                };
                self.state.layout.drag_divider(path, *divider, at);
                cx.notify();
            }
        }
    }

    pub fn save_session(&self) {
        match &self.session {
            SessionFile::At(path) => match session::save(path, &session::snapshot(&self.state)) {
                Ok(()) => {}
                Err(error) => tracing::warn!(?path, %error, "saving session failed"),
            },
            SessionFile::Disabled => {}
        }
    }

    pub fn flush(&mut self, cx: &mut Context<Self>) {
        // Clipboard reads apply synchronously and may queue more effects.
        while !self.effects.is_empty() {
            for effect in std::mem::take(&mut self.effects) {
                match effect {
                    Effect::Job(work) => {
                        let task = cx.background_spawn(async move { job::run(work) });
                        cx.spawn(async move |this, cx| {
                            let result = task.await;
                            this.update(cx, |workspace, cx| workspace.on_job(result, cx))
                        })
                        .detach();
                    }
                    Effect::Quit => {
                        self.save_session();
                        cx.quit()
                    }
                    Effect::Watch(dirs) => watch::watch(&mut self.watcher, &dirs),
                    Effect::WriteClipboard { clipboard, text } => {
                        let item = ClipboardItem::new_string(text);
                        match clipboard {
                            Clipboard::System => cx.write_to_clipboard(item),
                            Clipboard::Primary => cx.write_to_primary(item),
                        }
                    }
                    Effect::ReadClipboard { clipboard, then } => {
                        let item = match clipboard {
                            Clipboard::System => cx.read_from_clipboard(),
                            Clipboard::Primary => cx.read_from_primary(),
                        };
                        let text = item.and_then(|item| item.text());
                        register::clipboard_contents(&mut self.state, clipboard, then, text);
                    }
                }
            }
        }
    }

    /// Zed's client-side decorations: shadow, rounded border and resize edges unless tiled.
    fn decorations(
        &self,
        content: Stateful<Div>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let decorations = window.window_decorations();
        let tiling = match decorations {
            Decorations::Server => Tiling::tiled(),
            Decorations::Client { tiling } => tiling,
        };
        window.set_client_inset(match decorations {
            Decorations::Server => px(0.),
            Decorations::Client { .. } => SHADOW,
        });
        let resizable = window.is_resizable();
        let backdrop = round_corners(div().id("window-backdrop"), tiling)
            .size_full()
            .bg(transparent_black());
        let backdrop = match (decorations, resizable) {
            (Decorations::Server, _) | (Decorations::Client { .. }, false) => backdrop,
            (Decorations::Client { .. }, true) => backdrop
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
                }),
        };
        let backdrop = match tiling.top {
            true => backdrop,
            false => backdrop.pt(SHADOW),
        };
        let backdrop = match tiling.bottom {
            true => backdrop,
            false => backdrop.pb(SHADOW),
        };
        let backdrop = match tiling.left {
            true => backdrop,
            false => backdrop.pl(SHADOW),
        };
        let backdrop = match tiling.right {
            true => backdrop,
            false => backdrop.pr(SHADOW),
        };
        let frame = round_corners(div(), tiling)
            .cursor(CursorStyle::Arrow)
            .size_full()
            .border_color(self.theme.chrome.border)
            .on_mouse_move(|_, _, cx| cx.stop_propagation());
        let frame = match tiling.top {
            true => frame,
            false => frame.border_t(BORDER),
        };
        let frame = match tiling.bottom {
            true => frame,
            false => frame.border_b(BORDER),
        };
        let frame = match tiling.left {
            true => frame,
            false => frame.border_l(BORDER),
        };
        let frame = match tiling.right {
            true => frame,
            false => frame.border_r(BORDER),
        };
        let frame = match tiling.is_tiled() {
            true => frame,
            false => frame.shadow(vec![BoxShadow {
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
            }]),
        };
        let backdrop = backdrop.child(frame.child(content));
        match (decorations, resizable) {
            (Decorations::Server, _) | (Decorations::Client { .. }, false) => backdrop,
            (Decorations::Client { .. }, true) => backdrop.child(
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
                            Some(ResizeEdge::Top | ResizeEdge::Bottom) => CursorStyle::ResizeUpDown,
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
            ),
        }
    }

    /// Drag to move, double-click to maximize, right-click for the window menu, and the
    /// platform's window buttons on either side of the project name.
    fn titlebar(&self, window: &Window, cx: &mut Context<Self>) -> Stateful<Div> {
        let theme = &self.theme;
        let controls = window.window_controls();
        let tiling = match window.window_decorations() {
            Decorations::Server => Tiling::tiled(),
            Decorations::Client { tiling } => tiling,
        };
        let layout = cx
            .button_layout()
            .unwrap_or_else(WindowButtonLayout::linux_default);
        let maximized = window.is_maximized();
        let icon = theme.chrome.title_text;
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
                            theme.chrome.hover,
                        ),
                        (WindowButton::Maximize, false) => (
                            "maximize",
                            div()
                                .size(px(10.))
                                .border_1()
                                .border_color(icon)
                                .into_any_element(),
                            theme.chrome.hover,
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
                            theme.chrome.hover,
                        ),
                        (WindowButton::Close, _) => (
                            "close",
                            div().text_color(icon).child("✕").into_any_element(),
                            theme.chrome.close_hover,
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
        let root = self.state.root.path();
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
            .bg(theme.chrome.title)
            .text_color(theme.chrome.title_text)
            .font(theme.ui.clone())
            .text_size(theme.ui_size)
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _, _| this.drag = Drag::Title),
            )
            .children(left)
            .child(div().flex_1())
            .child(div().text_color(theme.text).child(name))
            .child(
                div()
                    .pl_2()
                    .text_color(theme.text_muted)
                    .child(root.display().to_string()),
            )
            .child(div().flex_1())
            .children(right);
        let bar = match (tiling.top || tiling.left, tiling.top || tiling.right) {
            (true, true) => bar,
            (false, true) => bar.rounded_tl(ROUNDING),
            (true, false) => bar.rounded_tr(ROUNDING),
            (false, false) => bar.rounded_tl(ROUNDING).rounded_tr(ROUNDING),
        };
        let bar = match controls.maximize && window.is_resizable() {
            true => bar.on_click(|event, window, _| match event.click_count() {
                2 => window.zoom_window(),
                _ => {}
            }),
            false => bar,
        };
        match controls.window_menu {
            true => bar.on_mouse_down(MouseButton::Right, |event, window, _| {
                window.show_window_menu(event.position)
            }),
            false => bar,
        }
    }

    /// Helix statusline: mode, file name, modification; selections, position, encoding.
    fn statusline(&self, pane: PaneId) -> Div {
        let theme = &self.theme;
        let state = &self.state;
        let focused = pane == state.layout.focus;
        let mode = state.input.mode;
        let (file, flags, selections, position) =
            match state.layout.panes.get(pane).map(|pane| &pane.view) {
                Some(View::Text(view)) => {
                    let buffer = &state.buffers[view.buffer];
                    let name = buffer
                        .path
                        .strip_prefix(state.root.path())
                        .unwrap_or(&buffer.path)
                        .display()
                        .to_string();
                    match &buffer.content {
                        Content::Text { doc, disk, .. } => {
                            let cursor = view.selection.primary().cursor(doc.text.slice(..));
                            let line = doc.text.char_to_line(cursor);
                            let column = cursor - doc.text.line_to_char(line);
                            let marker = match doc.is_modified() {
                                true => "[+]",
                                false => "",
                            };
                            let disk = match disk {
                                Disk::Synced | Disk::New => "",
                                Disk::ChangedExternally => " [changed on disk]",
                                Disk::Deleted => " [deleted]",
                            };
                            let selections = match view.selection.len() {
                                1 => "1 sel".to_owned(),
                                n => format!("{n} sels"),
                            };
                            (
                                name,
                                format!("{marker}{disk}"),
                                selections,
                                format!("{}:{}", line + 1, column + 1),
                            )
                        }
                        Content::Loading { .. } => {
                            (name, "[loading]".to_owned(), String::new(), String::new())
                        }
                    }
                }
                Some(View::Empty) | None => (
                    "[scratch]".to_owned(),
                    String::new(),
                    String::new(),
                    String::new(),
                ),
            };
        // Buffers are always UTF-8; helix shows the encoding only when it differs.
        let (mode_label, mode_bg, mode_fg) = match focused {
            true => (mode.label(), theme.mode[mode], theme.mode.fg),
            false => ("   ", theme.surface, theme.text_muted),
        };
        let pending: String = match focused {
            true => state
                .input
                .pending
                .as_slice()
                .iter()
                .map(|key| key.to_string())
                .collect(),
            false => String::new(),
        };
        let text = match focused {
            true => theme.text,
            false => theme.text_muted,
        };
        div()
            .flex()
            .flex_row()
            .bg(theme.surface)
            .text_color(text)
            .font(theme.ui.clone())
            .text_size(theme.ui_size)
            .child(
                div()
                    .px_2()
                    .bg(mode_bg)
                    .text_color(mode_fg)
                    .child(mode_label),
            )
            .child(div().pl_2().child(file))
            .child(div().pl_1().text_color(theme.chrome.unsaved).child(flags))
            .child(div().flex_1())
            .child(div().px_2().text_color(theme.text_muted).child(pending))
            .child(div().px_2().child(selections))
            .child(div().px_2().child(position))
    }

    /// `path` holds the child indices from the root down to `node`.
    fn node(&self, node: &Node, path: &mut SplitPath, cx: &mut Context<Self>) -> AnyElement {
        match node {
            Node::Pane(pane) => {
                let pane = *pane;
                let body =
                    match self.state.layout.panes.get(pane).map(|pane| &pane.view) {
                        Some(View::Text(view)) => match &self.state.buffers[view.buffer].content {
                            Content::Text { .. } => div()
                                .flex_1()
                                .min_h_0()
                                .overflow_hidden()
                                .child(TextElement {
                                    workspace: cx.entity(),
                                    pane,
                                }),
                            Content::Loading { .. } => div()
                                .flex_1()
                                .p_2()
                                .text_color(self.theme.text_muted)
                                .child("loading…"),
                        },
                        Some(View::Empty) | None => div().flex_1(),
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
                            workspace::focus_pane(&mut this.state, pane);
                            window.focus(&this.focus, cx);
                            this.changed(cx);
                        }),
                    )
                    .child(body)
                    .child(self.statusline(pane))
                    .into_any_element()
            }
            Node::Split { axis, children } => {
                let axis = *axis;
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
                                divider(axis, self.theme.chrome.border).on_mouse_down(
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

    /// One tab per open buffer: click shows it, middle-click or × closes it.
    fn tabline(&self, cx: &mut Context<Self>) -> Option<Div> {
        let theme = &self.theme;
        let state = &self.state;
        match state.tabs.len() {
            0 => return None,
            _ => {}
        }
        let current = match state.layout.panes.get(state.layout.focus).map(|p| &p.view) {
            Some(View::Text(view)) => Some(view.buffer),
            Some(View::Empty) | None => None,
        };
        let tabs: Vec<_> = state
            .tabs
            .iter()
            .map(|&id| {
                let buffer = &state.buffers[id];
                let name = buffer
                    .path
                    .file_name()
                    .map(|name| name.to_string_lossy().into_owned())
                    .unwrap_or_default();
                let (bg, fg) = match current == Some(id) {
                    true => (theme.background, theme.text),
                    false => (theme.surface, theme.text_muted),
                };
                let marker = match &buffer.content {
                    Content::Text { doc, .. } => match doc.is_modified() {
                        true => "●",
                        false => "",
                    },
                    Content::Loading { .. } => "…",
                };
                let key = id.data().as_ffi();
                div()
                    .id(ElementId::NamedInteger("tab".into(), key))
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_1()
                    .pl_2()
                    .pr_1()
                    .bg(bg)
                    .text_color(fg)
                    .border_r_1()
                    .border_color(theme.chrome.border)
                    .hover(|style| style.bg(theme.chrome.hover))
                    .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                        workspace::select_tab(&mut this.state, id);
                        window.focus(&this.focus, cx);
                        this.changed(cx);
                    }))
                    .on_mouse_down(
                        MouseButton::Middle,
                        cx.listener(move |this, _, _, cx| {
                            apply::close_buffer(&mut this.state, id, apply::Force::No);
                            this.changed(cx);
                        }),
                    )
                    .child(name)
                    .child(div().text_color(theme.chrome.unsaved).child(marker))
                    .child(
                        div()
                            .id(ElementId::NamedInteger("tab-close".into(), key))
                            .px_1()
                            .rounded(px(3.))
                            .text_color(theme.text_muted)
                            .hover(|style| style.bg(theme.chrome.hover).text_color(theme.text))
                            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                cx.stop_propagation();
                                apply::close_buffer(&mut this.state, id, apply::Force::No);
                                this.changed(cx);
                            }))
                            .child("×"),
                    )
            })
            .collect();
        Some(
            div()
                .flex_none()
                .flex()
                .flex_row()
                .bg(theme.surface)
                .border_b_1()
                .border_color(theme.chrome.border)
                .font(theme.ui.clone())
                .text_size(theme.ui_size)
                .children(tabs),
        )
    }

    fn sidebar(&self, focus: Focus, cx: &mut Context<Self>) -> Div {
        let theme = &self.theme;
        let border = match focus {
            Focus::Sidebar => theme.chrome.accent,
            Focus::Editor => theme.chrome.border,
        };
        let areas = self.areas.clone();
        div()
            .flex_none()
            .h_full()
            .flex()
            .flex_row()
            .child(
                div()
                    .relative()
                    .w(px(self.state.sidebar_width.get()))
                    .h_full()
                    .flex()
                    .flex_col()
                    .bg(theme.surface)
                    .font(theme.ui.clone())
                    .text_size(theme.ui_size)
                    .child(
                        canvas(
                            move |bounds, _, _| areas.sidebar.set(bounds),
                            |_, _, _, _| {},
                        )
                        .absolute()
                        .size_full(),
                    )
                    .child(
                        uniform_list(
                            "file-tree",
                            self.state.tree.rows.len(),
                            cx.processor(|this, range: Range<usize>, _window, cx| {
                                let state = &this.state;
                                let theme = &this.theme;
                                let focused =
                                    matches!(state.sidebar, Sidebar::Files(Focus::Sidebar));
                                range
                                    .map(|index| {
                                        let row = &state.tree.rows[index];
                                        let status = match &state.git.repo {
                                            Repo::Found { snapshot, .. } => snapshot.get(&row.path),
                                            Repo::None | Repo::Unknown => None,
                                        };
                                        let color = match status {
                                            Some(status) => theme.chrome[status],
                                            None => theme.text,
                                        };
                                        let icon = match row.kind {
                                            RowKind::File => "  ",
                                            RowKind::Dir(Fold::Collapsed) => "▸ ",
                                            RowKind::Dir(Fold::Expanded) => "▾ ",
                                        };
                                        let unsaved =
                                            match state.tree.unsaved.paths.contains(&row.path) {
                                                true => "●",
                                                false => "",
                                            };
                                        let bg = match (index == state.tree.cursor, focused) {
                                            (true, true) => theme.chrome.selection,
                                            (true, false) => theme.background,
                                            (false, _) => theme.surface,
                                        };
                                        div()
                                            .id(index)
                                            .flex()
                                            .flex_row()
                                            .bg(bg)
                                            .hover(|style| style.bg(theme.chrome.hover))
                                            .pl(px(4.0 + INDENT * row.depth as f32))
                                            .pr_2()
                                            .text_color(color)
                                            .on_click(cx.listener(
                                                move |this, _: &ClickEvent, window, cx| {
                                                    workspace::click_tree(
                                                        &mut this.state,
                                                        index,
                                                        &mut this.effects,
                                                    );
                                                    window.focus(&this.focus, cx);
                                                    this.changed(cx);
                                                },
                                            ))
                                            .child(div().text_color(theme.text_muted).child(icon))
                                            .child(
                                                div()
                                                    .flex_1()
                                                    .overflow_hidden()
                                                    .child(row.path.name().to_owned()),
                                            )
                                            .child(
                                                div()
                                                    .text_color(theme.chrome.unsaved)
                                                    .child(unsaved),
                                            )
                                    })
                                    .collect()
                            }),
                        )
                        .track_scroll(&self.tree_scroll)
                        .flex_1(),
                    ),
            )
            .child(divider(Axis::Row, border).on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    this.drag = Drag::Sidebar;
                    cx.stop_propagation();
                }),
            ))
    }

    fn picker(&self, window: &mut Window, cx: &mut Context<Self>) -> Option<Stateful<Div>> {
        let Overlay::Picker(picker) = &self.state.overlay else {
            return None;
        };
        let theme = &self.theme;
        let previewed = match picker.items {
            Items::Files(_) | Items::Buffers(_) | Items::Grep(_) => true,
            Items::Commands => false,
        };
        let viewport = window.viewport_size();
        let width = match previewed {
            true => viewport.width * 0.85,
            false => viewport.width * 0.6,
        };
        let status = match &picker.items {
            Items::Grep(grep) => match grep.search {
                Search::Idle => String::new(),
                Search::Running => "searching…".to_owned(),
                Search::Done => format!("{} hits", grep.hits.len()),
                Search::Truncated => format!("{}+ hits", grep.hits.len()),
            },
            Items::Files(_) | Items::Buffers(_) | Items::Commands => {
                format!("{}/{}", picker.matches.len(), picker.items.len())
            }
        };
        let list = uniform_list(
            "picker",
            picker.matches.len(),
            cx.processor(|this, range: Range<usize>, _window, cx| {
                let Overlay::Picker(picker) = &this.state.overlay else {
                    return Vec::new();
                };
                let theme = &this.theme;
                range
                    .map(|row| {
                        let index = picker.matches[row];
                        let (label, detail) = match &picker.items {
                            Items::Files(_) | Items::Buffers(_) => {
                                (picker.items.label(index).to_owned(), String::new())
                            }
                            Items::Commands => {
                                let (name, command) = &PALETTE[index as usize];
                                let keys = keymap::keys_for(KeyContext::Normal, command)
                                    .map(|keys| {
                                        keys.iter()
                                            .map(|key| key.to_string())
                                            .collect::<Vec<_>>()
                                            .join(" ")
                                    })
                                    .unwrap_or_default();
                                ((*name).to_owned(), keys)
                            }
                            Items::Grep(grep) => {
                                let hit = &grep.hits[index as usize];
                                (
                                    format!("{}:{}: {}", hit.path, hit.line + 1, hit.text),
                                    String::new(),
                                )
                            }
                        };
                        let bg = match row == picker.cursor {
                            true => theme.chrome.selection,
                            false => theme.surface,
                        };
                        div()
                            .id(row)
                            .flex()
                            .flex_row()
                            .px_2()
                            .bg(bg)
                            .hover(|style| style.bg(theme.chrome.hover))
                            .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                                workspace::click_picker(&mut this.state, row, &mut this.effects);
                                window.focus(&this.focus, cx);
                                this.changed(cx);
                            }))
                            .child(div().flex_1().overflow_hidden().child(label))
                            .child(div().text_color(theme.text_muted).child(detail))
                    })
                    .collect()
            }),
        )
        .track_scroll(&self.picker_scroll)
        .flex_1();
        let body = div().flex_1().min_h_0().flex().flex_row().child(
            div()
                .h_full()
                .min_w_0()
                .flex()
                .flex_col()
                .w(match previewed {
                    true => width * 0.45,
                    false => width,
                })
                .child(list),
        );
        let body = match previewed {
            true => body.child(
                div()
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .overflow_hidden()
                    .pl_2()
                    .bg(theme.background)
                    .border_l_1()
                    .border_color(theme.chrome.border)
                    .child(PreviewElement {
                        workspace: cx.entity(),
                    }),
            ),
            false => body,
        };
        Some(
            div()
                .id("picker-backdrop")
                .absolute()
                .top_0()
                .left_0()
                .size_full()
                .flex()
                .justify_center()
                .items_center()
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|this, _, _, cx| {
                        workspace::picker(
                            &mut this.state,
                            PickerCommand::Cancel,
                            &mut this.effects,
                        );
                        this.changed(cx);
                    }),
                )
                .child(
                    div()
                        .id("picker")
                        .w(width)
                        .h(viewport.height * 0.7)
                        .flex()
                        .flex_col()
                        .bg(theme.surface)
                        .border_1()
                        .border_color(theme.chrome.accent)
                        .font(theme.ui.clone())
                        .text_size(theme.ui_size)
                        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                        .child(
                            div()
                                .flex()
                                .flex_row()
                                .px_2()
                                .border_b_1()
                                .border_color(theme.chrome.border)
                                .child(div().flex_1().child(format!("> {}", picker.query)))
                                .child(div().text_color(theme.text_muted).child(status)),
                        )
                        .child(body),
                ),
        )
    }
}

/// Text arrives here once composed, so insert mode sees text rather than raw key presses.
impl EntityInputHandler for Workspace {
    fn text_for_range(
        &mut self,
        _range: Range<usize>,
        _adjusted_range: &mut Option<Range<usize>>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<String> {
        None
    }

    fn selected_text_range(
        &mut self,
        _ignore_disabled_input: bool,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        None
    }

    fn marked_text_range(
        &self,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Range<usize>> {
        match self.preedit.is_empty() {
            true => None,
            false => Some(0..self.preedit.encode_utf16().count()),
        }
    }

    fn unmark_text(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        self.preedit.clear();
        cx.notify();
    }

    fn replace_text_in_range(
        &mut self,
        _range: Option<Range<usize>>,
        text: &str,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.preedit.clear();
        input::text(&mut self.state, text, &mut self.effects);
        self.flush(cx);
        cx.notify();
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        _range: Option<Range<usize>>,
        text: &str,
        _selected_range: Option<Range<usize>>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.preedit.clear();
        self.preedit.push_str(text);
        cx.notify();
    }

    fn bounds_for_range(
        &mut self,
        _range: Range<usize>,
        element_bounds: Bounds<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        let state = &self.state;
        let view = match &state.layout.panes.get(state.layout.focus)?.view {
            View::Text(view) => view,
            View::Empty => return None,
        };
        let doc = match &state.buffers.get(view.buffer)?.content {
            Content::Text { doc, .. } => doc,
            Content::Loading { .. } => return None,
        };
        let text = doc.text.slice(..);
        let viewport = view.scroll.viewport;
        let format = display::text_format(viewport, Indent::of(doc, &state.languages));
        let (pos, _) = visual_offset_from_anchor(
            text,
            view.scroll.anchor,
            view.selection.primary().cursor(text),
            &format,
            &TextAnnotations::default(),
            viewport.rows as usize,
        )
        .ok()?;
        let cell = self.theme.cell;
        let col = Gutter::new(text.len_lines()).cols as usize + pos.col;
        Some(Bounds::new(
            point(
                element_bounds.origin.x + cell.width * col as f32,
                element_bounds.origin.y + cell.height * pos.row as f32,
            ),
            cell,
        ))
    }

    fn character_index_for_point(
        &mut self,
        _point: gpui::Point<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<usize> {
        None
    }

    fn accepts_text_input(&self, _window: &mut Window, _cx: &mut Context<Self>) -> bool {
        input::typed(input::context(&self.state), ' ').is_some()
    }
}

impl Render for Workspace {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let State {
            tree,
            root,
            buffers,
            ..
        } = &mut self.state;
        tree::sync_unsaved(&mut tree.unsaved, root.path(), buffers);
        let state = &self.state;
        let bottom = match (&state.overlay, &state.tree.edit, &state.notice) {
            (Overlay::Prompt(_), _, _) => String::new(),
            (_, TreeEdit::Create { line }, _) => {
                format!("new path (end with / for a directory): {line}")
            }
            (_, TreeEdit::Rename { from, line }, _) => format!("rename {from} to: {line}"),
            (_, TreeEdit::Delete { path, line, .. }, _) => {
                format!("delete {path}? type y to confirm: {line}")
            }
            (Overlay::None | Overlay::Picker(_), TreeEdit::None, Some(notice)) => {
                notice.to_string()
            }
            (Overlay::None | Overlay::Picker(_), TreeEdit::None, None) => String::new(),
        };
        let line_height = window.line_height();
        let main = div().flex_1().flex().flex_row().min_h_0();
        let main = match state.sidebar {
            Sidebar::Files(focus) => main.child(self.sidebar(focus, cx)),
            Sidebar::Hidden => main,
        };
        let areas = self.areas.clone();
        let mut path = SplitPath::new();
        let editor = div()
            .relative()
            .flex_1()
            .flex()
            .min_w_0()
            .min_h_0()
            .child(
                canvas(
                    move |bounds, _, _| areas.editor.set(bounds),
                    |_, _, _, _| {},
                )
                .absolute()
                .size_full(),
            )
            .child(self.node(&self.state.layout.root, &mut path, cx));
        let main = main.child(editor);
        let entity = cx.entity();
        let input_focus = self.focus.clone();
        let tiling = match window.window_decorations() {
            Decorations::Server => Tiling::tiled(),
            Decorations::Client { tiling } => tiling,
        };
        let content = round_corners(div().id("workspace"), tiling)
            .key_context("Workspace")
            .track_focus(&self.focus)
            .on_key_down(cx.listener(|this, event, _, cx| this.on_key(event, cx)))
            .on_mouse_move(cx.listener(|this, event, window, cx| this.on_drag(event, window, cx)))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _, _, _| this.drag = Drag::Idle),
            )
            .relative()
            .size_full()
            .flex()
            .flex_col()
            .bg(self.theme.background)
            .text_color(self.theme.text)
            .font(self.theme.mono[0].clone())
            .text_size(self.theme.mono_size)
            // Fallback input handler so the prompt takes text when no text pane is focused.
            .child(
                canvas(
                    |_, _, _| {},
                    move |bounds, (), window, cx| {
                        window.handle_input(
                            &input_focus,
                            ElementInputHandler::new(bounds, entity),
                            cx,
                        )
                    },
                )
                .absolute()
                .size_full(),
            )
            .child(self.titlebar(window, cx))
            .children(self.tabline(cx))
            .child(main)
            .child(div().flex_none().px_2().h(line_height).child(bottom))
            .children(self.picker(window, cx))
            .children(crate::prompt::prompt(
                &self.state.overlay,
                &self.theme,
                line_height,
            ));
        self.decorations(content, window, cx)
    }
}
