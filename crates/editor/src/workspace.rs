use std::cell::{Cell, RefCell};
use std::ops::Range;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::mpsc;

use gpui::{
    AnyElement, Bounds, BoxShadow, ClickEvent, ClipboardItem, Context, CursorStyle, Decorations,
    Deferred, Div, ElementInputHandler, EntityInputHandler, HitboxBehavior, Hsla, IntoElement,
    KeyDownEvent, MouseButton, MouseDownEvent, MouseMoveEvent, Pixels, Point, Render, ResizeEdge,
    ScrollHandle, ScrollStrategy, SharedString, Size, Stateful, Tiling, UTF16Selection,
    UniformListScrollHandle, Window, WindowButton, WindowButtonLayout, WindowControlArea, anchored,
    canvas, deferred, div, point, prelude::*, px, size, transparent_black, uniform_list,
};
use helix_core::visual_offset_from_anchor;
use model::command::{ChangesCommand, Command, PickerCommand, Shown};
use model::config::TitleBar;
use model::git::{Change, GitStatus, Group, Kind};
use model::register::Clipboard;
use model::theme::Theme;
use model::units::Px;
use model::window::Axis;
use slotmap::SecondaryMap;
use state::git::conflict;
use state::git::diff::{self, Bases, Removal, Side};
use state::git::diffview::{Left, RIGHT};
use state::git::stage::Discard;
use state::input::keymap::{self, Context as KeyContext};
use state::input::picker::{Grep, Items, PALETTE, Referrers, Search};
use state::job::{self, JobResult};
use state::project::path::RelPath;
use state::project::watch::{self, Watcher};
use state::render::pages::Fit;
use state::render::typeset::Request;
use state::sidebar::changes::{self, ListRow};
use state::sidebar::tree::{self, Fold, NewEntry, RowKind, TreeAction, TreeEdit, TreeTarget};
use state::text::buffer::{Content, Disk, Highlight};
use state::view::display::{self, Gutter, Indent};
use state::view::layout::{Node, Pane, PaneId, SplitPath, Tabs};
use state::view::mode;
use state::view::scroll;
use state::view::tab::{Beside, InvalidShare, Share, Showing};
use state::{
    Confirm, Dragged, Effect, Files, Focus, Overlay, Repo, Sidebar, SidebarWidth, State, apply,
    input, project::session, text::registers, view::panes,
};

use crate::fonts::{Fonts, hsla};
use crate::preview::PreviewElement;
use crate::rendered::raster::{Rasterized, Rasters};
use crate::rendered::{RenderedPane, Shape};
use crate::text::{Part, TextElement};

pub struct Workspace {
    pub state: State,
    pub theme: Theme,
    pub fonts: Fonts,
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
    /// The file tree context menu, opened by a right click at `at`.
    pub tree_menu: Option<TreeMenu>,
    /// Lists of the panes showing a rendered view.
    pub rendered: SecondaryMap<PaneId, RenderedPane>,
    /// Requests to the typeset thread.
    pub typeset: mpsc::Sender<Request>,
    /// Rasters of the typeset SVGs drawn last frame.
    pub rasters: Rc<RefCell<Rasters>>,
    /// Height of the window's content as last drawn; rendered lists lay out this far past their viewport.
    pub window_height: Pixels,
}

/// Where the session is saved on exit, per `session.restore`.
/// An open file tree context menu.
pub struct TreeMenu {
    pub target: TreeTarget,
    pub at: Point<Pixels>,
}

/// A file tree entry being dragged to another folder.
pub struct DraggedEntry {
    pub path: RelPath,
}

/// What follows the pointer while dragging a tree entry.
pub struct DragPreview {
    pub name: SharedString,
    pub bg: Hsla,
    pub fg: Hsla,
}

impl Render for DragPreview {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .px_2()
            .rounded_sm()
            .bg(self.bg)
            .text_color(self.fg)
            .child(self.name.clone())
    }
}

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
    /// The divider between a split's source and rendering in `pane`.
    Split {
        pane: PaneId,
    },
}

#[derive(Default)]
pub struct Areas {
    pub editor: Cell<Bounds<Pixels>>,
    pub sidebar: Cell<Bounds<Pixels>>,
    /// The body of each pane showing a split, as last laid out.
    pub splits: RefCell<SecondaryMap<PaneId, Bounds<Pixels>>>,
}

const INDENT: f32 = 14.0;
const TITLE_HEIGHT: Pixels = px(32.0);
/// Hit width of split and sidebar dividers; a 1px line is drawn in the middle.
const DIVIDER: Pixels = px(5.0);
/// Columns of the monospace font each side of a split keeps, however far its divider goes.
const SPLIT_MIN_COLUMNS: f32 = 12.0;
/// Sidebar widths in ems of the interface font.
const SIDEBAR_DEFAULT: f32 = 20.0;
const SIDEBAR_MIN: f32 = 8.0;
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
        // Any key dismisses the tree menu; keys keep their usual meaning.
        self.tree_menu = None;
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
        match &self.state.project.git {
            Repo::Found { snapshot, .. } => {
                match changes::cursor_row(
                    &snapshot.changes,
                    &self.state.changes.folds,
                    self.state.changes.cursor.as_ref(),
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
            Overlay::None | Overlay::Prompt(_) | Overlay::Confirm(_) => {}
        }
        let focus = self.state.layout.focus();
        let active = (self.state.layout.panes().get(focus))
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
        self.on_jobs([result], cx)
    }

    pub fn on_jobs(
        &mut self,
        results: impl IntoIterator<Item = JobResult>,
        cx: &mut Context<Self>,
    ) {
        let _span = tracing::info_span!("complete jobs").entered();
        for result in results {
            job::complete(&mut self.state, result, &mut self.effects);
        }
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
                let width = fit_sidebar(
                    event.position.x - left,
                    &self.fonts,
                    window.viewport_size().width,
                );
                self.state.sidebar_width = SidebarWidth::Dragged(
                    Dragged::new(Px(width / px(1.))).expect("a fitted width is positive"),
                );
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
            (Drag::Split { pane }, Some(MouseButton::Left)) => {
                let Some(area) = self.areas.splits.borrow().get(*pane).copied() else {
                    return;
                };
                let least = self.fonts.cell.width * SPLIT_MIN_COLUMNS;
                let room = area.size.width - DIVIDER;
                let source = (event.position.x - area.origin.x)
                    .min(room - least)
                    .max(least);
                match Share::new(source / room) {
                    Ok(share) => mode::drag(&mut self.state, *pane, share),
                    // A pane narrower than both sides' least keeps the share it had.
                    Err(InvalidShare(_)) => {}
                }
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

    /// Syncs the state once per change, brings the lists along and runs what they queued.
    pub fn flush(&mut self, cx: &mut Context<Self>) {
        let _span = tracing::trace_span!("flush").entered();
        loop {
            state::sync(&mut self.state, &mut self.effects);
            crate::rendered::sync(self, cx);
            let effects = std::mem::take(&mut self.effects);
            match effects.as_slice() {
                [] => break,
                [_, ..] => {}
            }
            for effect in effects {
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
                    Effect::OpenUrl(url) => cx.open_url(&url),
                    Effect::Typeset(request) => match self.typeset.send(request) {
                        Ok(()) => {}
                        Err(_) => tracing::error!("the typeset thread is gone"),
                    },
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
                        registers::clipboard_contents(&mut self.state, clipboard, then, text);
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
            .border_color(hsla(self.theme.chrome.border))
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
        let fonts = &self.fonts;
        let controls = window.window_controls();
        let tiling = match window.window_decorations() {
            Decorations::Server => Tiling::tiled(),
            Decorations::Client { tiling } => tiling,
        };
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
        let fonts = &self.fonts;
        let state = &self.state;
        let focused = pane == state.layout.focus();
        let mode = state.input.mode;
        let (file, flags, hunks, conflicts, selections, position) =
            match state.layout.panes().get(pane).and_then(Pane::active) {
                Some(shown) => {
                    let view = shown.text();
                    let buffer = &state.buffers[view.buffer()];
                    let name = buffer
                        .path
                        .strip_prefix(state.project.root.path())
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
                                Disk::Synced { .. } | Disk::New => "",
                                Disk::ChangedExternally { .. } => " [changed on disk]",
                                Disk::Deleted => " [deleted]",
                            };
                            let highlight = match doc.highlight {
                                Highlight::Failed { .. } => " [parse failed]",
                                Highlight::Plain
                                | Highlight::Parsing { .. }
                                | Highlight::Parsed(_) => "",
                            };
                            let selections = match view.selection.len() {
                                1 => "1 sel".to_owned(),
                                n => format!("{n} sels"),
                            };
                            let hunks = match (shown.shown(), &diff.bases) {
                                (Shown::Source, _) => String::new(),
                                (Shown::Render, _) => "rendered".to_owned(),
                                (Shown::Split, _) => "split".to_owned(),
                                (Shown::Diff, Bases::Tracked(tracked)) => {
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
                                (Shown::Diff, Bases::Untracked) => "diff: untracked".to_owned(),
                                (Shown::Diff, Bases::NoRepo) => {
                                    "diff: not in a repository".to_owned()
                                }
                                (Shown::Diff, Bases::Loading { .. }) => "diff".to_owned(),
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
                                format!("{marker}{disk}{highlight}"),
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
            true => (mode.label(), hsla(theme.mode[mode]), hsla(theme.mode.fg)),
            false => ("   ", hsla(theme.surface), hsla(theme.muted)),
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
            true => hsla(theme.text),
            false => hsla(theme.muted),
        };
        div()
            .flex()
            .flex_row()
            .bg(hsla(theme.surface))
            .text_color(text)
            .font(fonts.ui.clone())
            .text_size(fonts.ui_size)
            .child(
                div()
                    .px_2()
                    .bg(mode_bg)
                    .text_color(mode_fg)
                    .child(mode_label),
            )
            .child(div().pl_2().child(file))
            .child(
                div()
                    .pl_1()
                    .text_color(hsla(theme.chrome.unsaved))
                    .child(flags),
            )
            .child(div().flex_1())
            .child(div().px_2().text_color(hsla(theme.muted)).child(pending))
            .child(div().px_2().child(hunks))
            .child(
                div()
                    .px_2()
                    .text_color(hsla(theme.chrome.close_hover))
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
                    Some(view) => {
                        match (&self.state.buffers[view.buffer()].content, view.showing()) {
                            (Content::Text { .. }, Showing::Source(_)) => div()
                                .flex_1()
                                .min_h_0()
                                .overflow_hidden()
                                .child(element(Part::Text)),
                            // Equal halves, so both sides wrap at the same width.
                            (Content::Text { .. }, Showing::Diff(..)) => div()
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
                            (
                                Content::Text { .. },
                                Showing::Document(_, _, Beside::Source(_, share))
                                | Showing::Pages(_, _, Beside::Source(_, share)),
                            ) => {
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
                            (
                                Content::Text { .. },
                                Showing::Document(_, _, Beside::Alone)
                                | Showing::Pages(_, _, Beside::Alone),
                            ) => div()
                                .flex_1()
                                .min_h_0()
                                .flex()
                                .flex_col()
                                .overflow_hidden()
                                .bg(hsla(self.theme.background))
                                .child(crate::rendered::body(self, pane, cx)),
                            (Content::Loading { .. }, _) => div()
                                .flex_1()
                                .p_2()
                                .text_color(hsla(self.theme.muted))
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
                            panes::focus_pane(&mut this.state, pane);
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

    fn sidebar(&self, focus: Focus, window: Pixels, cx: &mut Context<Self>) -> Div {
        let theme = &self.theme;
        let fonts = &self.fonts;
        let drop_bg = hsla(theme.chrome.selection);
        let border = match focus {
            Focus::Sidebar => hsla(theme.chrome.accent),
            Focus::Editor => hsla(theme.chrome.border),
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
                    .w(fit_sidebar(
                        match self.state.sidebar_width {
                            SidebarWidth::Default => fonts.ui_size * SIDEBAR_DEFAULT,
                            SidebarWidth::Dragged(width) => px(width.get().0),
                        },
                        fonts,
                        window,
                    ))
                    .h_full()
                    .flex()
                    .flex_col()
                    .bg(hsla(theme.surface))
                    .font(fonts.ui.clone())
                    .text_size(fonts.ui_size)
                    .child(
                        canvas(
                            move |bounds, _, _| areas.sidebar.set(bounds),
                            |_, _, _, _| {},
                        )
                        .absolute()
                        .size_full(),
                    )
                    .children(match (&self.state.sidebar, &self.state.project.git) {
                        (Sidebar::Changes(_), Repo::Found { snapshot, .. }) => {
                            let note = match snapshot.unlisted.len() {
                                0 => None,
                                1 => Some("1 change is not listed; see the log".to_owned()),
                                count => {
                                    Some(format!("{count} changes are not listed; see the log"))
                                }
                            };
                            note.map(|note| {
                                div()
                                    .px_2()
                                    .py_1()
                                    .text_color(hsla(theme.muted))
                                    .child(note)
                            })
                        }
                        (Sidebar::Hidden | Sidebar::Files(_), _)
                        | (Sidebar::Changes(_), Repo::None | Repo::Unknown) => None,
                    })
                    .child(match self.state.sidebar {
                        Sidebar::Changes(_) => uniform_list(
                            "changes",
                            match &self.state.project.git {
                                Repo::Found { snapshot, .. } => {
                                    changes::list_len(&snapshot.changes, &self.state.changes.folds)
                                }
                                Repo::None | Repo::Unknown => 0,
                            },
                            cx.processor(|this, range: Range<usize>, _window, cx| {
                                let state = &this.state;
                                let theme = &this.theme;
                                let Repo::Found { snapshot, .. } = &state.project.git else {
                                    return Vec::new();
                                };
                                let entries = &snapshot.changes;
                                let focused =
                                    matches!(state.sidebar, Sidebar::Changes(Focus::Sidebar));
                                let folds = &state.changes.folds;
                                let cursor = changes::cursor_row(
                                    entries,
                                    folds,
                                    state.changes.cursor.as_ref(),
                                );
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
                                                (true, true) => hsla(theme.chrome.selection),
                                                (true, false) => hsla(theme.background),
                                                (false, _) => hsla(theme.surface),
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
                                                .hover(|style| style.bg(hsla(theme.chrome.hover)))
                                                .text_color(hsla(theme.muted))
                                                .on_click(cx.listener(
                                                    move |this, _: &ClickEvent, window, cx| {
                                                        panes::click_changes(
                                                            &mut this.state,
                                                            row,
                                                            panes::Click::Single,
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
                                                (true, true) => hsla(theme.chrome.selection),
                                                (true, false) => hsla(theme.background),
                                                (false, _) => hsla(theme.surface),
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
                                            let dir = changed.path.folder_path();
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
                                                .hover(|style| style.bg(hsla(theme.chrome.hover)))
                                                .on_click(cx.listener(
                                                    move |this, event: &ClickEvent, window, cx| {
                                                        let click = match event.click_count() {
                                                            2 => panes::Click::Double,
                                                            _ => panes::Click::Single,
                                                        };
                                                        panes::click_changes(
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
                                                        .text_color(hsla(theme.chrome[status]))
                                                        .child(changed.path.name().to_owned()),
                                                )
                                                .child(
                                                    div()
                                                        .flex_1()
                                                        .min_w_0()
                                                        .overflow_hidden()
                                                        .whitespace_nowrap()
                                                        .text_color(hsla(theme.muted))
                                                        .child(dir.to_owned()),
                                                )
                                                .children(buttons.into_iter().flatten())
                                                .child(
                                                    div()
                                                        .flex_none()
                                                        .text_color(hsla(theme.muted))
                                                        .child(marker),
                                                )
                                                .child(
                                                    div()
                                                        .flex_none()
                                                        .w(px(12.))
                                                        .text_color(hsla(theme.chrome[status]))
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
                        Sidebar::Files(_) | Sidebar::Hidden => div()
                            .id("file-tree-area")
                            .flex_1()
                            .min_h_0()
                            .on_mouse_down(
                                MouseButton::Right,
                                cx.listener(|this, event: &MouseDownEvent, _, cx| {
                                    this.tree_menu = Some(TreeMenu {
                                        target: TreeTarget::Root,
                                        at: event.position,
                                    });
                                    cx.notify();
                                }),
                            )
                            .drag_over::<DraggedEntry>(move |style, _, _, _| style.bg(drop_bg))
                            .on_drop(cx.listener(|this, dragged: &DraggedEntry, window, cx| {
                                panes::move_entry(
                                    &mut this.state,
                                    dragged.path.clone(),
                                    TreeTarget::Root,
                                    &mut this.effects,
                                );
                                window.focus(&this.focus, cx);
                                this.changed(cx);
                            }))
                            .child(
                                uniform_list(
                                    "file-tree",
                                    self.state.tree.rows.len(),
                                    cx.processor(|this, range: Range<usize>, _window, cx| {
                                        let state = &this.state;
                                        let theme = &this.theme;
                                        let (drop_bg, preview_bg, preview_fg) = (
                                            hsla(theme.chrome.selection),
                                            hsla(theme.chrome.title),
                                            hsla(theme.text),
                                        );
                                        let focused = matches!(
                                            state.sidebar,
                                            Sidebar::Files(Files::Tree | Files::Edit(_))
                                        );
                                        range
                                            .map(|index| {
                                                let row = &state.tree.rows[index];
                                                let status = match &state.project.git {
                                                    Repo::Found { snapshot, .. } => {
                                                        snapshot.get(&row.path)
                                                    }
                                                    Repo::None | Repo::Unknown => None,
                                                };
                                                let color = match status {
                                                    Some(status) => hsla(theme.chrome[status]),
                                                    None => hsla(theme.text),
                                                };
                                                let icon = match row.kind {
                                                    RowKind::File => "  ",
                                                    RowKind::Dir(Fold::Collapsed) => "▸ ",
                                                    RowKind::Dir(Fold::Expanded) => "▾ ",
                                                };
                                                let unsaved = match state
                                                    .tree
                                                    .unsaved
                                                    .paths
                                                    .contains(&row.path)
                                                {
                                                    true => "●",
                                                    false => "",
                                                };
                                                let bg = match (index == state.tree.cursor, focused)
                                                {
                                                    (true, true) => hsla(theme.chrome.selection),
                                                    (true, false) => hsla(theme.background),
                                                    (false, _) => hsla(theme.surface),
                                                };
                                                div()
                                            .id(index)
                                            .flex()
                                            .flex_row()
                                            .bg(bg)
                                            .hover(|style| style.bg(hsla(theme.chrome.hover)))
                                            .pl(px(4.0 + INDENT * row.depth as f32))
                                            .pr_2()
                                            .text_color(color)
                                            .on_mouse_down(
                                                MouseButton::Right,
                                                cx.listener(
                                                    move |this, event: &MouseDownEvent, _, cx| {
                                                        this.tree_menu = Some(TreeMenu {
                                                            target: TreeTarget::Row(index),
                                                            at: event.position,
                                                        });
                                                        cx.stop_propagation();
                                                        cx.notify();
                                                    },
                                                ),
                                            )
                                            .on_drag(
                                                DraggedEntry {
                                                    path: row.path.clone(),
                                                },
                                                move |dragged, _, _, cx| {
                                                    cx.new(|_| DragPreview {
                                                        name: dragged.path.name().to_owned().into(),
                                                        bg: preview_bg,
                                                        fg: preview_fg,
                                                    })
                                                },
                                            )
                                            .drag_over::<DraggedEntry>(move |style, _, _, _| {
                                                style.bg(drop_bg)
                                            })
                                            .on_drop(cx.listener(
                                                move |this, dragged: &DraggedEntry, window, cx| {
                                                    panes::move_entry(
                                                        &mut this.state,
                                                        dragged.path.clone(),
                                                        TreeTarget::Row(index),
                                                        &mut this.effects,
                                                    );
                                                    window.focus(&this.focus, cx);
                                                    this.changed(cx);
                                                },
                                            ))
                                            .on_click(cx.listener(
                                                move |this, event: &ClickEvent, window, cx| {
                                                    let click = match event.click_count() {
                                                        2 => panes::Click::Double,
                                                        _ => panes::Click::Single,
                                                    };
                                                    panes::click_tree(
                                                        &mut this.state,
                                                        index,
                                                        click,
                                                        &mut this.effects,
                                                    );
                                                    window.focus(&this.focus, cx);
                                                    this.changed(cx);
                                                },
                                            ))
                                            .child(div().text_color(hsla(theme.muted)).child(icon))
                                            .child(
                                                div()
                                                    .flex_1()
                                                    .overflow_hidden()
                                                    .child(row.path.name().to_owned()),
                                            )
                                            .child(
                                                div()
                                                    .text_color(hsla(theme.chrome.unsaved))
                                                    .child(unsaved),
                                            )
                                            })
                                            .collect()
                                    }),
                                )
                                .track_scroll(&self.tree_scroll)
                                .size_full(),
                            )
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

    fn tree_menu(&self, cx: &mut Context<Self>) -> Option<Deferred> {
        let menu = self.tree_menu.as_ref()?;
        let theme = &self.theme;
        let fonts = &self.fonts;
        let target = menu.target;
        const ITEMS: [(TreeAction, &str); 4] = [
            (TreeAction::NewFile, "New File"),
            (TreeAction::NewFolder, "New Folder"),
            (TreeAction::Rename, "Rename"),
            (TreeAction::Delete, "Delete"),
        ];
        let shown = match target {
            TreeTarget::Row(_) => ITEMS.len(),
            TreeTarget::Root => 2,
        };
        let hover = hsla(theme.chrome.hover);
        let items = ITEMS[..shown].iter().map(|&(action, label)| {
            div()
                .id(label)
                .px_3()
                .py_0p5()
                .rounded_sm()
                .hover(move |style| style.bg(hover))
                .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                    this.tree_menu = None;
                    panes::tree_action(&mut this.state, target, action, &mut this.effects);
                    window.focus(&this.focus, cx);
                    this.changed(cx);
                }))
                .child(label)
        });
        Some(
            deferred(
                anchored().position(menu.at).snap_to_window().child(
                    div()
                        .id("tree-menu")
                        .min_w(px(140.))
                        .p_1()
                        .flex()
                        .flex_col()
                        .bg(hsla(theme.surface))
                        .border_1()
                        .border_color(hsla(theme.chrome.border))
                        .rounded_md()
                        .font(fonts.ui.clone())
                        .text_size(fonts.ui_size)
                        .text_color(hsla(theme.text))
                        .on_mouse_down_out(cx.listener(|this, _, _, cx| {
                            this.tree_menu = None;
                            cx.notify();
                        }))
                        .children(items),
                ),
            )
            .with_priority(1),
        )
    }

    fn picker(&self, window: &mut Window, cx: &mut Context<Self>) -> Option<Stateful<Div>> {
        let Overlay::Picker(picker) = &self.state.overlay else {
            return None;
        };
        let theme = &self.theme;
        let fonts = &self.fonts;
        let previewed = match picker.items {
            Items::Files(_) | Items::Buffers(_) | Items::Grep(_) | Items::Backlinks(_) => true,
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
            Items::Backlinks(Referrers {
                search: Search::Running,
                ..
            }) => "indexing notes…".to_owned(),
            Items::Files(_) | Items::Buffers(_) | Items::Commands | Items::Backlinks(_) => {
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
                            Items::Grep(Grep { hits, .. })
                            | Items::Backlinks(Referrers { hits, .. }) => {
                                let hit = &hits[index as usize];
                                (
                                    format!("{}:{}: {}", hit.path, hit.line + 1, hit.text),
                                    String::new(),
                                )
                            }
                        };
                        let bg = match row == picker.cursor {
                            true => hsla(theme.chrome.selection),
                            false => hsla(theme.surface),
                        };
                        div()
                            .id(row)
                            .flex()
                            .flex_row()
                            .px_2()
                            .bg(bg)
                            .hover(|style| style.bg(hsla(theme.chrome.hover)))
                            .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                                panes::click_picker(&mut this.state, row, &mut this.effects);
                                window.focus(&this.focus, cx);
                                this.changed(cx);
                            }))
                            .child(div().flex_1().overflow_hidden().child(label))
                            .child(div().text_color(hsla(theme.muted)).child(detail))
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
                    .bg(hsla(theme.background))
                    .border_l_1()
                    .border_color(hsla(theme.chrome.border))
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
                        panes::picker(&mut this.state, PickerCommand::Cancel, &mut this.effects);
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
                        .bg(hsla(theme.surface))
                        .border_1()
                        .border_color(hsla(theme.chrome.accent))
                        .font(fonts.ui.clone())
                        .text_size(fonts.ui_size)
                        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                        .child(
                            div()
                                .flex()
                                .flex_row()
                                .px_2()
                                .border_b_1()
                                .border_color(hsla(theme.chrome.border))
                                .child(div().flex_1().child(format!("> {}", picker.query)))
                                .child(div().text_color(hsla(theme.muted)).child(status)),
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
        let shown = state.layout.focused().active()?;
        let view = shown.text();
        let doc = match &state.buffers[view.buffer()].content {
            Content::Text { doc, .. } => doc,
            Content::Loading { .. } => return None,
        };
        let text = doc.text.slice(..);
        let (padding, gutter_lines) = match shown.showing() {
            Showing::Document(_, _, Beside::Alone) | Showing::Pages(_, _, Beside::Alone) => {
                return None;
            }
            Showing::Source(_)
            | Showing::Document(_, _, Beside::Source(..))
            | Showing::Pages(_, _, Beside::Source(..)) => {
                (conflict::padding(doc), text.len_lines())
            }
            Showing::Diff(_, diff) => (
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
        let cell = self.fonts.cell;
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
        let _span = tracing::trace_span!("workspace render").entered();
        self.window_height = window.viewport_size().height;
        // Split bodies record their bounds again as this frame lays them out.
        self.areas.splits.borrow_mut().clear();
        let mut rasters = self.rasters.borrow_mut();
        let frame = rasters.frame;
        rasters
            .entries
            .retain(|_, raster| match (raster.drawn == frame, &raster.image) {
                (true, _) => true,
                (false, Rasterized::Ready(image)) => {
                    cx.drop_image(image.clone(), Some(window));
                    false
                }
                (false, Rasterized::Pending | Rasterized::Failed) => false,
            });
        rasters.frame += 1;
        drop(rasters);
        // Typst pages are sized from the viewport their list was last laid out in.
        for cache in self.rendered.values_mut() {
            let size = cache.list.viewport_bounds().size;
            match (&cache.measured.shape, cache.measured.height == size.height) {
                (Shape::Pages { fit: Fit::Page, .. }, false) => cache.list.remeasure(),
                (Shape::Pages { fit: Fit::Page, .. }, true)
                | (
                    Shape::Pages {
                        fit: Fit::Width, ..
                    }
                    | Shape::Document { .. },
                    _,
                ) => {}
            }
            cache.measured.width = size.width;
            cache.measured.height = size.height;
        }
        let State {
            tree,
            project,
            buffers,
            ..
        } = &mut self.state;
        tree::sync_unsaved(&mut tree.unsaved, project.root.path(), buffers);
        let panes = &self.state.layout.panes();
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
            path.strip_prefix(state.project.root.path())
                .unwrap_or(path)
                .display()
                .to_string()
        };
        let bottom = match (&state.overlay, &state.sidebar, &state.notice) {
            (Overlay::Prompt(_), ..) => String::new(),
            (
                Overlay::Confirm(Confirm::Discard {
                    path,
                    how: Discard::Checkout,
                }),
                ..,
            ) => format!("discard changes to {}? y/n", relative(path)),
            (
                Overlay::Confirm(Confirm::Discard {
                    path,
                    how: Discard::Delete(_),
                }),
                ..,
            ) => format!("delete untracked {}? y/n", relative(path)),
            (Overlay::Confirm(Confirm::Create(path)), ..) => format!("create {path}? y/n"),
            (
                Overlay::None | Overlay::Picker(_),
                Sidebar::Files(Files::Edit(TreeEdit::Create {
                    line,
                    kind: NewEntry::Infer,
                })),
                _,
            ) => {
                format!("new file (end with / for a folder): {line}")
            }
            (
                Overlay::None | Overlay::Picker(_),
                Sidebar::Files(Files::Edit(TreeEdit::Create {
                    line,
                    kind: NewEntry::Folder,
                })),
                _,
            ) => {
                format!("new folder: {line}")
            }
            (
                Overlay::None | Overlay::Picker(_),
                Sidebar::Files(Files::Edit(TreeEdit::Rename { from, line })),
                _,
            ) => {
                format!("rename {from} to: {line}")
            }
            (
                Overlay::None | Overlay::Picker(_),
                Sidebar::Files(Files::Edit(TreeEdit::Delete { path, line, .. })),
                _,
            ) => {
                format!("delete {path}? type y to confirm: {line}")
            }
            (
                Overlay::None | Overlay::Picker(_),
                Sidebar::Hidden | Sidebar::Files(Files::Editor | Files::Tree) | Sidebar::Changes(_),
                Some(notice),
            ) => notice.to_string(),
            (
                Overlay::None | Overlay::Picker(_),
                Sidebar::Hidden | Sidebar::Files(Files::Editor | Files::Tree) | Sidebar::Changes(_),
                None,
            ) => String::new(),
        };
        let line_height = window.line_height();
        let main = div().flex_1().flex().flex_row().min_h_0();
        let main = match &state.sidebar {
            Sidebar::Files(Files::Tree | Files::Edit(_)) | Sidebar::Changes(Focus::Sidebar) => {
                main.child(self.sidebar(Focus::Sidebar, window.viewport_size().width, cx))
            }
            Sidebar::Files(Files::Editor) | Sidebar::Changes(Focus::Editor) => {
                main.child(self.sidebar(Focus::Editor, window.viewport_size().width, cx))
            }
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
            .child(self.node(&self.state.layout.root(), &mut path, cx));
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
            .bg(hsla(self.theme.background))
            .text_color(hsla(self.theme.text))
            .font(self.fonts.mono[0].clone())
            .text_size(self.fonts.mono_size)
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
            .children(self.tree_menu(cx))
            .children(crate::prompt::prompt(
                &self.state.overlay,
                &self.theme,
                &self.fonts,
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
    let hover = hsla(theme.chrome.hover);
    let text = hsla(theme.text);
    div()
        .id(id)
        .flex_none()
        .w(px(16.))
        .flex()
        .justify_center()
        .rounded_sm()
        .text_color(hsla(theme.muted))
        .invisible()
        .group_hover("change-row", |style| style.visible())
        .hover(move |style| style.bg(hover).text_color(text))
        .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
            cx.stop_propagation();
            panes::changes_action(&mut this.state, row, command, &mut this.effects);
            window.focus(&this.focus, cx);
            this.changed(cx);
        }))
        .child(glyph)
}

/// Fits a sidebar width between its minimum and the window, as zed fits its docks.
fn fit_sidebar(width: Pixels, fonts: &Fonts, window: Pixels) -> Pixels {
    width.min(window - DIVIDER).max(fonts.ui_size * SIDEBAR_MIN)
}
