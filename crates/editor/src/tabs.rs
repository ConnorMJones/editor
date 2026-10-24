use std::path::{Component, Path};

use gpui::{
    ClickEvent, Context, Div, ElementId, MouseButton, ScrollHandle, Stateful, div, prelude::*, px,
};
use model::command::Shown;
use slotmap::Key as _;
use state::apply::Force;
use state::view::layout::{Pane, PaneId, Tabs};
use state::view::panes::{self, Click, Dragged};
use state::view::tab::{Opened, Tab};

use crate::fonts::hsla;
use crate::workspace::{DragPreview, DraggedEntry, DraggedTab, Workspace};

/// Top edge of every tab; the focused pane's active tab draws it in the accent colour.
const INDICATOR: gpui::Pixels = px(2.);
/// Hover group of a tab, which shows its close button.
const GROUP: &str = "tab";

impl Workspace {
    /// Zed-style: preview tabs in italics, a modified dot that becomes a close button on hover.
    pub fn tab_bar(
        &self,
        pane: PaneId,
        tabs: &Tabs,
        scroll: &ScrollHandle,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let theme = &self.theme;
        let fonts = &self.fonts;
        let state = &self.state;
        let focused = pane == state.layout.focus();
        let pane_key = pane.data().as_ffi();
        let (preview_bg, preview_fg) = (hsla(theme.chrome.title), hsla(theme.text));
        let drop_bg = hsla(theme.chrome.selection);
        let bar = div()
            .id(ElementId::NamedInteger("tab-bar".into(), pane_key))
            .flex_none()
            .h(fonts.bar)
            .w_full()
            .flex()
            .flex_row()
            .overflow_x_scroll()
            .track_scroll(scroll)
            .bg(hsla(theme.surface))
            .font(fonts.ui.clone())
            .text_size(fonts.ui_size);
        let tabs_iter = tabs.items().iter().enumerate().map(|(index, item)| {
            let opened = item.tab.opened();
            let path = state.path_of(opened);
            let name = path
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_default();
            // Zed's detail: enough parent directories to tell apart tabs with the same file name.
            let parent = path.parent().unwrap_or(Path::new(""));
            let depth = (state.layout.panes().values())
                .flat_map(Pane::items)
                .map(|other| state.path_of(other.tab.opened()))
                .filter(|other| *other != path && other.file_name() == path.file_name())
                .map(|other| {
                    let other = other.parent().unwrap_or(Path::new(""));
                    let shared = (parent.components().rev())
                        .zip(other.components().rev())
                        .take_while(|(a, b)| a == b)
                        .count();
                    shared + 1
                })
                .max();
            let detail = depth.map(|depth| {
                let skip = parent.components().count().saturating_sub(depth);
                (parent.components().skip(skip))
                    .filter(|component| matches!(component, Component::Normal(_)))
                    .collect::<std::path::PathBuf>()
                    .display()
                    .to_string()
            });
            let active = index == tabs.active();
            let modified = state.is_modified(opened);
            let (bg, fg, edge, bottom) = match (active, focused) {
                (true, true) => (
                    hsla(theme.background),
                    hsla(theme.text),
                    hsla(theme.chrome.accent),
                    hsla(theme.background),
                ),
                (true, false) => (
                    hsla(theme.background),
                    hsla(theme.text),
                    hsla(theme.background),
                    hsla(theme.background),
                ),
                (false, _) => (
                    hsla(theme.surface),
                    hsla(theme.muted),
                    hsla(theme.surface),
                    hsla(theme.chrome.border),
                ),
            };
            let preview_name: gpui::SharedString = name.clone().into();
            let label = div().child(name);
            let label = match tabs.preview() == Some(index) {
                true => label.italic(),
                false => label,
            };
            // A diff or rendering gets a muted icon before the file name, as zed marks diffs.
            let icon = match &item.tab {
                Tab::Text(text) => match text.shown() {
                    Shown::Source => None,
                    Shown::Diff => Some("±"),
                    Shown::Render => Some("◉"),
                    Shown::Split => Some("◧"),
                },
                Tab::Ink(_) => Some("✎"),
                Tab::Image(_) => Some("▣"),
            }
            .map(|icon| div().text_color(hsla(theme.muted)).child(icon));
            // Ids from different stores can be equal, so the kind is part of the name.
            let (tab_name, close_name, key) = match opened {
                Opened::Text(buffer) => ("tab-text", "tab-close-text", buffer.data().as_ffi()),
                Opened::Paged(id) => ("tab-paged", "tab-close-paged", id.data().as_ffi()),
                Opened::Board(id) => ("tab-board", "tab-close-board", id.data().as_ffi()),
                Opened::Image(id) => ("tab-image", "tab-close-image", id.data().as_ffi()),
            };
            let close = div()
                .id(ElementId::NamedInteger(close_name.into(), key))
                .absolute()
                .top_0()
                .left_0()
                .size_full()
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(3.))
                .text_color(hsla(theme.muted))
                .hover(|style| {
                    style
                        .bg(hsla(theme.chrome.hover))
                        .text_color(hsla(theme.text))
                })
                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    cx.stop_propagation();
                    panes::close_tab(&mut this.state, pane, index, Force::No);
                    this.changed(cx);
                }))
                .child("×");
            let dot = div()
                .absolute()
                .top_0()
                .left_0()
                .size_full()
                .flex()
                .items_center()
                .justify_center()
                .text_color(hsla(theme.chrome.unsaved))
                .group_hover(GROUP, |style| style.invisible())
                .child("●");
            let hover = |close: Stateful<Div>| {
                close
                    .invisible()
                    .group_hover(GROUP, |style| style.visible())
            };
            let end = div().relative().flex_none().size(px(16.));
            let end = match (modified, active) {
                (true, _) => end.child(dot).child(hover(close)),
                (false, true) => end.child(close),
                (false, false) => end.child(hover(close)),
            };
            div()
                .id(ElementId::NamedInteger(tab_name.into(), key))
                .group(GROUP)
                .flex_none()
                .h_full()
                .flex()
                .flex_col()
                .bg(bg)
                .text_color(fg)
                .border_r_1()
                .border_color(hsla(theme.chrome.border))
                .when(!active, |tab| {
                    tab.hover(|style| style.bg(hsla(theme.chrome.hover)))
                })
                .on_drag(DraggedTab { pane, index }, move |_, _, _, cx| {
                    cx.new(|_| DragPreview {
                        name: preview_name.clone(),
                        bg: preview_bg,
                        fg: preview_fg,
                    })
                })
                .drag_over::<DraggedTab>(move |style, _, _, _| style.bg(drop_bg))
                .drag_over::<DraggedEntry>(move |style, _, _, _| style.bg(drop_bg))
                .on_drop(cx.listener(move |this, dragged: &DraggedTab, window, cx| {
                    let tab = Dragged::Tab {
                        pane: dragged.pane,
                        index: dragged.index,
                    };
                    this.drop_on_tabs(pane, index, tab, window, cx)
                }))
                .on_drop(
                    cx.listener(move |this, dragged: &DraggedEntry, window, cx| {
                        let file = Dragged::File(dragged.path.clone());
                        this.drop_on_tabs(pane, index, file, window, cx)
                    }),
                )
                .on_click(cx.listener(move |this, event: &ClickEvent, window, cx| {
                    let click = Click::of(event.click_count());
                    panes::click_tab(&mut this.state, pane, index, click);
                    window.focus(&this.focus, cx);
                    this.changed(cx);
                }))
                .on_mouse_down(
                    MouseButton::Middle,
                    cx.listener(move |this, _, _, cx| {
                        cx.stop_propagation();
                        panes::close_tab(&mut this.state, pane, index, Force::No);
                        this.changed(cx);
                    }),
                )
                .child(div().flex_none().h(INDICATOR).bg(edge))
                .child(
                    div()
                        .flex_1()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap_1()
                        .pl_3()
                        .pr_1()
                        .children(icon)
                        .child(label)
                        .children(
                            detail.map(|detail| div().text_color(hsla(theme.muted)).child(detail)),
                        )
                        .child(end),
                )
                .child(div().flex_none().h(px(1.)).bg(bottom))
        });
        let end = tabs.items().len();
        bar.children(tabs_iter).child(
            div()
                .id(ElementId::NamedInteger("tab-bar-end".into(), pane_key))
                .drag_over::<DraggedTab>(move |style, _, _, _| style.bg(drop_bg))
                .drag_over::<DraggedEntry>(move |style, _, _, _| style.bg(drop_bg))
                .on_drop(cx.listener(move |this, dragged: &DraggedTab, window, cx| {
                    let tab = Dragged::Tab {
                        pane: dragged.pane,
                        index: dragged.index,
                    };
                    this.drop_on_tabs(pane, end, tab, window, cx)
                }))
                .on_drop(
                    cx.listener(move |this, dragged: &DraggedEntry, window, cx| {
                        let file = Dragged::File(dragged.path.clone());
                        this.drop_on_tabs(pane, end, file, window, cx)
                    }),
                )
                .flex_1()
                .min_w(px(8.))
                .h_full()
                .flex()
                .flex_col()
                .justify_end()
                .child(div().h(px(1.)).bg(hsla(theme.chrome.border))),
        )
    }
}
