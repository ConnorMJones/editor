use std::cell::Cell;
use std::ops::Range;
use std::path::PathBuf;
use std::rc::Rc;

use gpui::{
    AnyElement, Bounds, BoxShadow, ClickEvent, ClipboardItem, Context, CursorStyle, Decorations,
    Div, ElementInputHandler, EntityInputHandler, HitboxBehavior, Hsla, IntoElement, KeyDownEvent,
    MouseButton, MouseMoveEvent, Pixels, Point, Render, ResizeEdge, ScrollHandle, ScrollStrategy,
    Size, Stateful, Tiling, UTF16Selection, UniformListScrollHandle, Window, WindowButton,
    WindowButtonLayout, WindowControlArea, canvas, div, point, prelude::*, px, size,
    transparent_black, uniform_list,
};
use helix_core::visual_offset_from_anchor;
use model::command::{ChangesCommand, Command, PickerCommand};
use model::config::TitleBar;
use model::git::{Change, GitStatus, Group, Kind};
use model::register::Clipboard;
use model::window::Axis;
use slotmap::SecondaryMap;
use state::buffer::{Content, Disk};
use state::changes::{self, Confirm, Discard, ListRow};
use state::conflict;
use state::diff::{self, Bases, Removal, Side};
use state::diffview::{Left, RIGHT};
use state::display::{self, Gutter, Indent};
use state::job::{self, JobResult};
use state::keymap::{self, Context as KeyContext};
use state::layout::{Node, Pane, PaneId, SplitPath, Tabs, View};
use state::picker::{Items, PALETTE, Search};
use state::scroll;
use state::tree::{self, Fold, RowKind, TreeEdit};
use state::watch::{self, Watcher};
use state::{
    Effect, Focus, Overlay, Repo, Sidebar, SidebarWidth, State, apply, input, register, session,
    workspace,
};

use crate::preview::PreviewElement;
use crate::text::{Part, TextElement};
use crate::theme::Theme;

pub struct Workspace {
    pub state: State,
    pub theme: Theme,
    pub title_bar: TitleBar,
    pub effects: Vec<Effect>,
    pub focus: gpui::FocusHandle,
    pub watcher: Watcher,
    pub tree_scroll: UniformListScrollHandle,
    pub changes_scroll: UniformListScrollHandle,
    pub picker_scroll: UniformListScrollHandle,
    /// Uncommitted IME composition, drawn at the primary cursor.
    pub preedit: String,
    pub session: SessionFile,
    pub drag: Drag,
    /// Window edge under the pointer; a change redraws so the cursor follows it.
    pub resize_edge: Option<ResizeEdge>,
    /// Bounds drawn last frame, which drags measure against.
    pub areas: Rc<Areas>,
    /// Horizontal scroll of each pane's tab bar.
    pub tab_scroll: SecondaryMap<PaneId, ScrollHandle>,
    /// Focused pane and active tab last scrolled into view.
    pub shown_tab: Option<(PaneId, usize)>,
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
    pub fn changed(&mut self, cx: &mut Context<Self>) {
        self.tree_scroll
            .scroll_to_item(self.state.tree.cursor, ScrollStrategy::Nearest);
        match &self.state.git.repo {
            Repo::Found { snapshot, .. } => {
                match changes::cursor_row(
                    &snapshot.changes,
                    &self.state.git.folds,
                    self.state.git.cursor.as_ref(),
                ) {
                    Some(row) => self
                        .changes_scroll
                        .scroll_to_item(row, ScrollStrategy::Nearest),
                    None => {}
                }
            }
            Repo::None | Repo::Unknown => {}
        }
        match &self.state.overlay {
            Overlay::Picker(picker) => self
                .picker_scroll
                .scroll_to_item(picker.cursor, ScrollStrategy::Nearest),
            Overlay::None | Overlay::Prompt(_) => {}
        }
        let focus = self.state.layout.focus;
        let active = (self.state.layout.panes.get(focus))
            .and_then(|pane| pane.tabs.as_ref())
            .map(Tabs::active);
        match (active, self.tab_scroll.get(focus)) {
            (Some(index), Some(scroll)) if self.shown_tab != Some((focus, index)) => {
                scroll.scroll_to_item(index);
                self.shown_tab = Some((focus, index));
            }
            (Some(_) | None, _) => {}
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
                        register::clipboard_contents(
                            &mut self.state,
                            clipboard,
                            then,
                            text,
                            &mut self.effects,
                        );
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
        let (file, flags, hunks, conflicts, selections, position) =
            match state.layout.panes.get(pane).and_then(Pane::active) {
                Some(shown) => {
                    let view = shown.text();
                    let buffer = &state.buffers[view.buffer()];
                    let name = buffer
                        .path
                        .strip_prefix(state.root.path())
                        .unwrap_or(&buffer.path)
                        .display()
                        .to_string();
                    match &buffer.content {
                        Content::Text {
                            doc, disk, diff, ..
                        } => {
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
                            let hunks = match (shown, &diff.bases) {
                                (View::Text(_), _) => String::new(),
                                (View::Diff(_), Bases::Tracked(tracked)) => {
                                    let at = diff::hunk_at(
                                        &tracked.hunks,
                                        Side::After,
                                        line as u32,
                                        Removal::Include,
                                    );
                                    match (at, tracked.hunks.len()) {
                                        (_, 0) => "diff: no changes".to_owned(),
                                        (Some(at), count) => {
                                            format!("diff: hunk {}/{count}", at + 1)
                                        }
                                        (None, count) => format!("diff: {count} hunks"),
                                    }
                                }
                                (View::Diff(_), Bases::Untracked) => "diff: untracked".to_owned(),
                                (View::Diff(_), Bases::NoRepo) => {
                                    "diff: not in a repository".to_owned()
                                }
                                (View::Diff(_), Bases::Loading) => "diff".to_owned(),
                            };
                            let list = &conflict::current(doc).list;
                            let at = list.partition_point(|conflict| conflict.close < line);
                            let conflicts = match (list.len(), list.get(at)) {
                                (0, _) => String::new(),
                                (count, Some(inside)) if inside.open <= line => {
                                    format!("conflict {}/{count}", at + 1)
                                }
                                (1, _) => "1 conflict".to_owned(),
                                (count, _) => format!("{count} conflicts"),
                            };
                            (
                                name,
                                format!("{marker}{disk}"),
                                hunks,
                                conflicts,
                                selections,
                                format!("{}:{}", line + 1, column + 1),
                            )
                        }
                        Content::Loading { .. } => (
                            name,
                            "[loading]".to_owned(),
                            String::new(),
                            String::new(),
                            String::new(),
                            String::new(),
                        ),
                    }
                }
                None => (
                    "[scratch]".to_owned(),
                    String::new(),
                    String::new(),
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
            .child(div().px_2().child(hunks))
            .child(
                div()
                    .px_2()
                    .text_color(theme.chrome.close_hover)
                    .child(conflicts),
            )
            .child(div().px_2().child(selections))
            .child(div().px_2().child(position))
    }

    /// `path` holds the child indices from the root down to `node`.
    fn node(&self, node: &Node, path: &mut SplitPath, cx: &mut Context<Self>) -> AnyElement {
        match node {
            Node::Pane(pane) => {
                let pane = *pane;
                let target = self.state.layout.panes.get(pane);
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
                    Some(view) => {
                        match (&self.state.buffers[view.buffer()].content, view) {
                            (Content::Text { .. }, View::Text(_)) => div()
                                .flex_1()
                                .min_h_0()
                                .overflow_hidden()
                                .child(element(Part::Text)),
                            // Equal halves, so both sides wrap at the same width.
                            (Content::Text { .. }, View::Diff(_)) => div()
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
                                .child(div().flex_none().w(px(1.)).bg(self.theme.chrome.border))
                                .child(
                                    div()
                                        .flex_1()
                                        .min_w_0()
                                        .overflow_hidden()
                                        .child(element(Part::Buffer)),
                                ),
                            (Content::Loading { .. }, _) => div()
                                .flex_1()
                                .p_2()
                                .text_color(self.theme.text_muted)
                                .child("loading…"),
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
                            workspace::focus_pane(&mut this.state, pane);
                            window.focus(&this.focus, cx);
                            this.changed(cx);
                        }),
                    )
                    .children(bar)
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
                    .child(match self.state.sidebar {
                        Sidebar::Changes(_) => uniform_list(
                            "changes",
                            match &self.state.git.repo {
                                Repo::Found { snapshot, .. } => {
                                    changes::list_len(&snapshot.changes, &self.state.git.folds)
                                }
                                Repo::None | Repo::Unknown => 0,
                            },
                            cx.processor(|this, range: Range<usize>, _window, cx| {
                                let state = &this.state;
                                let theme = &this.theme;
                                let Repo::Found { snapshot, .. } = &state.git.repo else {
                                    return Vec::new();
                                };
                                let entries = &snapshot.changes;
                                let focused =
                                    matches!(state.sidebar, Sidebar::Changes(Focus::Sidebar));
                                let folds = &state.git.folds;
                                let cursor =
                                    changes::cursor_row(entries, folds, state.git.cursor.as_ref());
                                range
                                    .map(|row| match changes::list_row(entries, folds, row) {
                                        Some(ListRow::Header { group, count, fold }) => {
                                            let label = match group {
                                                Group::Conflicts => "Conflicts",
                                                Group::Staged => "Staged Changes",
                                                Group::Changes => "Changes",
                                            };
                                            let chevron = match fold {
                                                Fold::Expanded => "▾",
                                                Fold::Collapsed => "▸",
                                            };
                                            let button = match group {
                                                Group::Staged => row_button(
                                                    theme,
                                                    ("unstage-all", row),
                                                    "−",
                                                    row,
                                                    ChangesCommand::Unstage,
                                                    cx,
                                                ),
                                                Group::Conflicts | Group::Changes => row_button(
                                                    theme,
                                                    ("stage-all", row),
                                                    "+",
                                                    row,
                                                    ChangesCommand::Stage,
                                                    cx,
                                                ),
                                            };
                                            let bg = match (Some(row) == cursor, focused) {
                                                (true, true) => theme.chrome.selection,
                                                (true, false) => theme.background,
                                                (false, _) => theme.surface,
                                            };
                                            div()
                                                .id(row)
                                                .group("change-row")
                                                .w_full()
                                                .flex()
                                                .flex_row()
                                                .gap_1()
                                                .px_2()
                                                .bg(bg)
                                                .hover(|style| style.bg(theme.chrome.hover))
                                                .text_color(theme.text_muted)
                                                .on_click(cx.listener(
                                                    move |this, _: &ClickEvent, window, cx| {
                                                        workspace::click_changes(
                                                            &mut this.state,
                                                            row,
                                                            workspace::Click::Single,
                                                            &mut this.effects,
                                                        );
                                                        window.focus(&this.focus, cx);
                                                        this.changed(cx);
                                                    },
                                                ))
                                                .child(div().flex_none().w(px(12.)).child(chevron))
                                                .child(div().flex_1().child(label))
                                                .child(button)
                                                .child(div().flex_none().child(count.to_string()))
                                        }
                                        Some(ListRow::Entry(index)) => {
                                            let changed = &entries[index];
                                            let change = snapshot.paths.get(&changed.path).copied();
                                            let side = match changed.group {
                                                Group::Staged => change.and_then(Change::staged),
                                                Group::Conflicts | Group::Changes => {
                                                    change.and_then(Change::unstaged)
                                                }
                                            };
                                            let (letter, status) =
                                                match (changed.group, change, side) {
                                                    (Group::Conflicts, ..) => {
                                                        ("C", GitStatus::Conflicted)
                                                    }
                                                    (_, Some(Change::Untracked), _) => {
                                                        ("U", GitStatus::Untracked)
                                                    }
                                                    (_, _, Some(Kind::Added)) => {
                                                        ("A", GitStatus::Added)
                                                    }
                                                    (_, _, Some(Kind::Deleted)) => {
                                                        ("D", GitStatus::Deleted)
                                                    }
                                                    (_, _, Some(Kind::Renamed)) => {
                                                        ("R", GitStatus::Modified)
                                                    }
                                                    (_, _, Some(Kind::TypeChange)) => {
                                                        ("T", GitStatus::Modified)
                                                    }
                                                    (_, _, Some(Kind::Modified) | None) => {
                                                        ("M", GitStatus::Modified)
                                                    }
                                                };
                                            // Staged in full, in part, or not at all.
                                            let marker = match change {
                                                Some(Change::Staged(_)) => "●",
                                                Some(Change::Both { .. }) => "◐",
                                                Some(Change::Unstaged(_) | Change::Untracked) => {
                                                    "○"
                                                }
                                                Some(Change::Conflicted | Change::Ignored)
                                                | None => "",
                                            };
                                            let bg = match (Some(row) == cursor, focused) {
                                                (true, true) => theme.chrome.selection,
                                                (true, false) => theme.background,
                                                (false, _) => theme.surface,
                                            };
                                            let buttons = match changed.group {
                                                Group::Changes => [
                                                    Some(row_button(
                                                        theme,
                                                        ("discard", row),
                                                        "↶",
                                                        row,
                                                        ChangesCommand::Discard,
                                                        cx,
                                                    )),
                                                    Some(row_button(
                                                        theme,
                                                        ("stage", row),
                                                        "+",
                                                        row,
                                                        ChangesCommand::Stage,
                                                        cx,
                                                    )),
                                                ],
                                                Group::Staged => [
                                                    Some(row_button(
                                                        theme,
                                                        ("unstage", row),
                                                        "−",
                                                        row,
                                                        ChangesCommand::Unstage,
                                                        cx,
                                                    )),
                                                    None,
                                                ],
                                                Group::Conflicts => [
                                                    Some(row_button(
                                                        theme,
                                                        ("stage", row),
                                                        "+",
                                                        row,
                                                        ChangesCommand::Stage,
                                                        cx,
                                                    )),
                                                    None,
                                                ],
                                            };
                                            let dir = changed.path.parent().unwrap_or("");
                                            div()
                                                .id(row)
                                                .group("change-row")
                                                .w_full()
                                                .flex()
                                                .flex_row()
                                                .gap_2()
                                                .pl_4()
                                                .pr_2()
                                                .bg(bg)
                                                .hover(|style| style.bg(theme.chrome.hover))
                                                .on_click(cx.listener(
                                                    move |this, event: &ClickEvent, window, cx| {
                                                        let click = match event.click_count() {
                                                            2 => workspace::Click::Double,
                                                            _ => workspace::Click::Single,
                                                        };
                                                        workspace::click_changes(
                                                            &mut this.state,
                                                            row,
                                                            click,
                                                            &mut this.effects,
                                                        );
                                                        window.focus(&this.focus, cx);
                                                        this.changed(cx);
                                                    },
                                                ))
                                                .child(
                                                    div()
                                                        .flex_none()
                                                        .text_color(theme.chrome[status])
                                                        .child(changed.path.name().to_owned()),
                                                )
                                                .child(
                                                    div()
                                                        .flex_1()
                                                        .min_w_0()
                                                        .overflow_hidden()
                                                        .whitespace_nowrap()
                                                        .text_color(theme.text_muted)
                                                        .child(dir.to_owned()),
                                                )
                                                .children(buttons.into_iter().flatten())
                                                .child(
                                                    div()
                                                        .flex_none()
                                                        .text_color(theme.text_muted)
                                                        .child(marker),
                                                )
                                                .child(
                                                    div()
                                                        .flex_none()
                                                        .w(px(12.))
                                                        .text_color(theme.chrome[status])
                                                        .child(letter),
                                                )
                                        }
                                        None => div().id(row),
                                    })
                                    .collect()
                            }),
                        )
                        .track_scroll(&self.changes_scroll)
                        .flex_1()
                        .into_any_element(),
                        Sidebar::Files(_) | Sidebar::Hidden => uniform_list(
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
                                                move |this, event: &ClickEvent, window, cx| {
                                                    let click = match event.click_count() {
                                                        2 => workspace::Click::Double,
                                                        _ => workspace::Click::Single,
                                                    };
                                                    workspace::click_tree(
                                                        &mut this.state,
                                                        index,
                                                        click,
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
                        .flex_1()
                        .into_any_element(),
                    }),
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
        let shown = state.layout.panes.get(state.layout.focus)?.active()?;
        let view = shown.text();
        let doc = match &state.buffers.get(view.buffer())?.content {
            Content::Text { doc, .. } => doc,
            Content::Loading { .. } => return None,
        };
        let text = doc.text.slice(..);
        let (padding, gutter_lines) = match shown {
            View::Text(_) => (conflict::padding(doc), text.len_lines()),
            View::Diff(diff) => (
                diff.align.sides[RIGHT].padding(),
                match &diff.left {
                    Left::Head { doc: head, .. } => text.len_lines().max(head.text.len_lines()),
                    Left::Pending | Left::Absent | Left::Parsing { .. } => text.len_lines(),
                },
            ),
        };
        let viewport = view.scroll.viewport;
        let format = display::text_format(viewport, Indent::of(doc, &state.languages));
        let (pos, _) = visual_offset_from_anchor(
            text,
            view.scroll.anchor,
            view.selection.primary().cursor(text),
            &format,
            &scroll::annotations(text, padding),
            (viewport.rows as isize + view.scroll.rows).max(0) as usize,
        )
        .ok()?;
        let row = pos.row as isize - view.scroll.rows;
        let cell = self.theme.cell;
        let col = Gutter::new(gutter_lines).cols as usize + pos.col;
        Some(Bounds::new(
            point(
                element_bounds.origin.x + cell.width * col as f32,
                element_bounds.origin.y + cell.height * row as f32,
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
        let panes = &self.state.layout.panes;
        self.tab_scroll.retain(|pane, _| panes.contains_key(pane));
        for pane in panes.keys() {
            match self.tab_scroll.contains_key(pane) {
                true => {}
                false => {
                    self.tab_scroll.insert(pane, ScrollHandle::new());
                }
            }
        }
        let state = &self.state;
        let relative = |path: &std::path::Path| {
            path.strip_prefix(state.root.path())
                .unwrap_or(path)
                .display()
                .to_string()
        };
        let bottom = match (
            &state.overlay,
            &state.tree.edit,
            &state.git.confirm,
            &state.notice,
        ) {
            (Overlay::Prompt(_), ..) => String::new(),
            (_, TreeEdit::Create { line }, ..) => {
                format!("new path (end with / for a directory): {line}")
            }
            (_, TreeEdit::Rename { from, line }, ..) => format!("rename {from} to: {line}"),
            (_, TreeEdit::Delete { path, line, .. }, ..) => {
                format!("delete {path}? type y to confirm: {line}")
            }
            (
                _,
                TreeEdit::None,
                Some(Confirm::Discard {
                    path,
                    how: Discard::Checkout,
                }),
                _,
            ) => format!("discard changes to {}? y/n", relative(path)),
            (
                _,
                TreeEdit::None,
                Some(Confirm::Discard {
                    path,
                    how: Discard::Delete(_),
                }),
                _,
            ) => format!("delete untracked {}? y/n", relative(path)),
            (Overlay::None | Overlay::Picker(_), TreeEdit::None, None, Some(notice)) => {
                notice.to_string()
            }
            (Overlay::None | Overlay::Picker(_), TreeEdit::None, None, None) => String::new(),
        };
        let line_height = window.line_height();
        let main = div().flex_1().flex().flex_row().min_h_0();
        let main = match state.sidebar {
            Sidebar::Files(focus) | Sidebar::Changes(focus) => main.child(self.sidebar(focus, cx)),
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
            .children(match self.title_bar {
                TitleBar::Shown => Some(self.titlebar(window, cx)),
                TitleBar::Hidden => None,
            })
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

/// A Changes sidebar row button, shown while the row is hovered, that runs `command` on the row.
fn row_button(
    theme: &Theme,
    id: (&'static str, usize),
    glyph: &'static str,
    row: usize,
    command: ChangesCommand,
    cx: &mut Context<Workspace>,
) -> Stateful<Div> {
    let hover = theme.chrome.hover;
    let text = theme.text;
    div()
        .id(id)
        .flex_none()
        .w(px(16.))
        .flex()
        .justify_center()
        .rounded_sm()
        .text_color(theme.text_muted)
        .invisible()
        .group_hover("change-row", |style| style.visible())
        .hover(move |style| style.bg(hover).text_color(text))
        .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
            cx.stop_propagation();
            workspace::changes_action(&mut this.state, row, command, &mut this.effects);
            window.focus(&this.focus, cx);
            this.changed(cx);
        }))
        .child(glyph)
}
