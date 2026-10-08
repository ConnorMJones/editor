use std::ops::Range;
use std::path::PathBuf;

use gpui::{
    AnyElement, Bounds, ClipboardItem, Context, Div, ElementInputHandler, Entity,
    EntityInputHandler, IntoElement, KeyDownEvent, Pixels, Render, ScrollStrategy, UTF16Selection,
    UniformListScrollHandle, Window, canvas, div, point, prelude::*, px, uniform_list,
};
use helix_core::text_annotations::TextAnnotations;
use helix_core::visual_offset_from_anchor;
use model::register::Clipboard;
use model::window::Axis;
use state::buffer::{Buffer, Content, Disk};
use state::display::{self, Gutter, Indent};
use state::job::{self, JobResult};
use state::keymap::{self, Context as KeyContext};
use state::layout::{Node, PaneId, View};
use state::path::RelPath;
use state::picker::{Items, PALETTE, Search};
use state::tree::{Fold, RowKind, TreeEdit};
use state::watch::{self, Watcher};
use state::{Effect, Focus, Overlay, Repo, Sidebar, State, input, register, session};

use crate::text::TextElement;
use crate::theme::Theme;

pub struct Workspace {
    pub state: State,
    pub theme: Theme,
    pub effects: Vec<Effect>,
    pub focus: gpui::FocusHandle,
    pub watcher: Watcher,
    pub tree_scroll: UniformListScrollHandle,
    /// Uncommitted IME composition, drawn at the primary cursor.
    pub preedit: String,
    pub session: SessionFile,
}

/// Where the session is saved on exit, per `session.restore`.
pub enum SessionFile {
    Disabled,
    At(PathBuf),
}

const SIDEBAR_WIDTH: f32 = 280.0;
const INDENT: f32 = 14.0;

fn modified(buffer: &Buffer) -> bool {
    match &buffer.content {
        Content::Text(doc) => doc.is_modified(),
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
        self.tree_scroll
            .scroll_to_item(self.state.tree.cursor, ScrollStrategy::Nearest);
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
                    let Content::Text(doc) = &buffer.content;
                    let cursor = view.selection.primary().cursor(doc.text.slice(..));
                    let line = doc.text.char_to_line(cursor);
                    let column = cursor - doc.text.line_to_char(line);
                    let name = buffer
                        .path
                        .strip_prefix(state.root.path())
                        .unwrap_or(&buffer.path)
                        .display()
                        .to_string();
                    let marker = match modified(buffer) {
                        true => "[+]",
                        false => "",
                    };
                    let disk = match buffer.disk {
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

    fn node(&self, node: &Node, entity: &Entity<Workspace>) -> AnyElement {
        match node {
            Node::Pane(pane) => {
                div()
                    .flex_1()
                    .flex()
                    .flex_col()
                    .min_w_0()
                    .min_h_0()
                    .overflow_hidden()
                    .child(
                        match self.state.layout.panes.get(*pane).map(|pane| &pane.view) {
                            Some(View::Text(_)) => div()
                                .flex_1()
                                .min_h_0()
                                .overflow_hidden()
                                .child(TextElement {
                                    workspace: entity.clone(),
                                    pane: *pane,
                                }),
                            Some(View::Empty) | None => div().flex_1(),
                        },
                    )
                    .child(self.statusline(*pane))
                    .into_any_element()
            }
            Node::Split { axis, children } => {
                let container = div().flex_1().flex().min_w_0().min_h_0().overflow_hidden();
                let container = match axis {
                    Axis::Row => container.flex_row(),
                    Axis::Column => container.flex_col(),
                };
                let border = self.theme.chrome.border;
                container
                    .children(children.iter().enumerate().map(|(i, child)| {
                        let cell = div()
                            .flex_1()
                            .flex()
                            .min_w_0()
                            .min_h_0()
                            .border_color(border);
                        let cell = match (i, axis) {
                            (0, _) => cell,
                            (_, Axis::Row) => cell.border_l_1(),
                            (_, Axis::Column) => cell.border_t_1(),
                        };
                        cell.child(self.node(child, entity))
                    }))
                    .into_any_element()
            }
        }
    }

    fn tabline(&self) -> Option<Div> {
        let theme = &self.theme;
        let state = &self.state;
        match state.tabs.len() {
            0 | 1 => return None,
            _ => {}
        }
        let current = match state.layout.panes.get(state.layout.focus).map(|p| &p.view) {
            Some(View::Text(view)) => Some(view.buffer),
            Some(View::Empty) | None => None,
        };
        let tabs = state.tabs.iter().map(|&id| {
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
            let marker = match modified(buffer) {
                true => "[+]",
                false => "",
            };
            div()
                .flex()
                .flex_row()
                .px_2()
                .bg(bg)
                .text_color(fg)
                .border_r_1()
                .border_color(theme.chrome.border)
                .child(name)
                .child(div().pl_1().text_color(theme.chrome.unsaved).child(marker))
        });
        Some(
            div()
                .flex()
                .flex_row()
                .bg(theme.surface)
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
        div()
            .w(px(SIDEBAR_WIDTH))
            .h_full()
            .flex()
            .flex_col()
            .bg(theme.surface)
            .border_r_1()
            .border_color(border)
            .font(theme.ui.clone())
            .text_size(theme.ui_size)
            .child(
                uniform_list(
                    "file-tree",
                    self.state.tree.rows.len(),
                    cx.processor(|this, range: std::ops::Range<usize>, _window, _cx| {
                        let state = &this.state;
                        let theme = &this.theme;
                        let root = state.root.path();
                        let dirty: Vec<RelPath> = state
                            .buffers
                            .values()
                            .filter(|buffer| modified(buffer))
                            .filter_map(|buffer| RelPath::new(root, &buffer.path).ok())
                            .collect();
                        let focused = matches!(state.sidebar, Sidebar::Files(Focus::Sidebar));
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
                                let unsaved = match dirty
                                    .iter()
                                    .any(|path| path.is_within(row.path.as_str()))
                                {
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
                                    .pl(px(4.0 + INDENT * row.depth as f32))
                                    .pr_2()
                                    .text_color(color)
                                    .child(div().text_color(theme.text_muted).child(icon))
                                    .child(
                                        div()
                                            .flex_1()
                                            .overflow_hidden()
                                            .child(row.path.name().to_owned()),
                                    )
                                    .child(div().text_color(theme.chrome.unsaved).child(unsaved))
                            })
                            .collect()
                    }),
                )
                .track_scroll(&self.tree_scroll)
                .flex_1(),
            )
    }

    fn picker(&self, window: &mut Window) -> Option<Div> {
        let Overlay::Picker(picker) = &self.state.overlay else {
            return None;
        };
        let theme = &self.theme;
        let height = window.viewport_size().height * 0.6;
        let visible = ((height / window.line_height()) as usize)
            .saturating_sub(2)
            .max(1);
        let start = picker
            .cursor
            .saturating_sub(visible / 2)
            .min(picker.matches.len().saturating_sub(visible));
        let rows = picker.matches[start..(start + visible).min(picker.matches.len())]
            .iter()
            .enumerate()
            .map(|(offset, &index)| {
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
                let bg = match start + offset == picker.cursor {
                    true => theme.chrome.selection,
                    false => theme.surface,
                };
                div()
                    .flex()
                    .flex_row()
                    .px_2()
                    .bg(bg)
                    .child(div().flex_1().overflow_hidden().child(label))
                    .child(div().text_color(theme.text_muted).child(detail))
            });
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
        Some(
            div()
                .absolute()
                .top_0()
                .left_0()
                .size_full()
                .flex()
                .justify_center()
                .items_center()
                .child(
                    div()
                        .w(window.viewport_size().width * 0.7)
                        .h(height)
                        .flex()
                        .flex_col()
                        .bg(theme.surface)
                        .border_1()
                        .border_color(theme.chrome.accent)
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
                        .child(div().flex_1().overflow_hidden().children(rows)),
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
            Content::Text(doc) => doc,
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
        let state = &self.state;
        let bottom = match (&state.overlay, &state.tree.edit, &state.notice) {
            (Overlay::Prompt(prompt), _, _) => format!("{}{}", prompt.kind.label(), prompt.line),
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
        let entity = cx.entity();
        let main = main.child(self.node(&self.state.layout.root, &entity));
        let input_focus = self.focus.clone();
        div()
            .key_context("Workspace")
            .track_focus(&self.focus)
            .on_key_down(cx.listener(|this, event, _, cx| this.on_key(event, cx)))
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
            .children(self.tabline())
            .child(main)
            .child(div().px_2().h(line_height).child(bottom))
            .children(self.picker(window))
    }
}
