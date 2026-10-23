use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::mpsc;

use gpui::{
    Bounds, ClipboardItem, Context, ElementInputHandler, IntoElement, KeyDownEvent, MouseButton,
    MouseMoveEvent, Pixels, Render, ResizeEdge, ScrollHandle, ScrollStrategy,
    UniformListScrollHandle, Window, canvas, div, prelude::*, px,
};
use model::config::TitleBar;
use model::register::Clipboard;
use model::theme::Theme;
use model::units::Px;
use model::window::Axis;
use slotmap::SecondaryMap;
use state::git::stage::Discard;
use state::job::{self, JobResult};
use state::project::watch::{self, Watcher};
use state::render::typeset::Request;
use state::sidebar::changes::{self};
use state::sidebar::tree::{self, NewEntry, TreeEdit};
use state::view::layout::{PaneId, SplitPath, Tabs};
use state::view::mode;
use state::view::tab::{InvalidShare, Share};
use state::{
    Confirm, Dragged, Effect, Files, Focus, Overlay, Repo, Sidebar, SidebarWidth, State, input,
    project::session, text::registers,
};

mod chrome;
mod drops;
mod ime;
mod pane_tree;
mod picker;
mod sidebar;
mod status;

pub use drops::DraggedTab;
pub use sidebar::{DragPreview, DraggedEntry, TreeMenu};

use crate::fonts::{Fonts, hsla};
use crate::rendered::RenderedPane;
use crate::rendered::raster::{Rasterized, Rasters};
use chrome::{corners, round};
use pane_tree::{DIVIDER, SPLIT_MIN_COLUMNS};
use sidebar::fit_sidebar;

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
    /// The pane and zone a dragged file or tab would drop into.
    pub drop_zone: Option<(PaneId, state::view::panes::Zone)>,
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
    pub rendered: SecondaryMap<PaneId, RenderedPane>,
    pub typeset: mpsc::Sender<Request>,
    /// Rasters of the typeset SVGs drawn last frame.
    pub rasters: Rc<RefCell<Rasters>>,
    /// Content height as last drawn; rendered lists lay out this far past their viewport.
    pub window_height: Pixels,
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
    /// Each drawing's body, which pen and mouse positions are measured from.
    pub drawings: RefCell<SecondaryMap<PaneId, Bounds<Pixels>>>,
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
        let content = round(div().id("workspace"), corners(window))
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
