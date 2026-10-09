use std::path::{Component, Path};

use gpui::{
    ClickEvent, Context, Div, ElementId, MouseButton, ScrollHandle, Stateful, div, prelude::*, px,
};
use slotmap::Key as _;
use state::apply::Force;
use state::layout::{Pane, PaneId, TabKey, Tabs};
use state::workspace::{self, Click};

use crate::workspace::Workspace;

const TAB_HEIGHT: gpui::Pixels = px(28.);
/// Top edge of every tab; the focused pane's active tab draws it in the accent colour.
const INDICATOR: gpui::Pixels = px(2.);
/// Hover group of a tab, which shows its close button.
const GROUP: &str = "tab";

impl Workspace {
    /// Zed-style tab bar of `pane`: preview tabs in italics, a modified dot that becomes a
    /// close button on hover, and duplicate file names told apart by their directories.
    pub fn tab_bar(
        &self,
        pane: PaneId,
        tabs: &Tabs,
        scroll: &ScrollHandle,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let theme = &self.theme;
        let state = &self.state;
        let focused = pane == state.layout.focus;
        let pane_key = pane.data().as_ffi();
        let bar = div()
            .id(ElementId::NamedInteger("tab-bar".into(), pane_key))
            .flex_none()
            .h(TAB_HEIGHT)
            .w_full()
            .flex()
            .flex_row()
            .overflow_x_scroll()
            .track_scroll(scroll)
            .bg(theme.surface)
            .font(theme.ui.clone())
            .text_size(theme.ui_size);
        let tabs_iter = tabs.items().iter().enumerate().map(|(index, item)| {
            let buffer = &state.buffers[item.view.buffer()];
            let path = buffer.path.as_path();
            let name = path
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_default();
            // Zed's detail: enough trailing parent directories to differ from every other
            // open tab with the same file name.
            let parent = path.parent().unwrap_or(Path::new(""));
            let depth = (state.layout.panes.values())
                .flat_map(Pane::items)
                .map(|other| state.buffers[other.view.buffer()].path.as_path())
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
            let modified = buffer.content.is_modified();
            let (bg, fg, edge, bottom) = match (active, focused) {
                (true, true) => (
                    theme.background,
                    theme.text,
                    theme.chrome.accent,
                    theme.background,
                ),
                (true, false) => (
                    theme.background,
                    theme.text,
                    theme.background,
                    theme.background,
                ),
                (false, _) => (
                    theme.surface,
                    theme.text_muted,
                    theme.surface,
                    theme.chrome.border,
                ),
            };
            let label = div().child(name);
            let label = match tabs.preview() == Some(index) {
                true => label.italic(),
                false => label,
            };
            // Zed marks a diff tab with a muted diff icon before the file name.
            let (kind, close_id, icon) = match item.view.key() {
                TabKey::Text(_) => ("tab", "tab-close", None),
                TabKey::Diff(_) => (
                    "diff-tab",
                    "diff-tab-close",
                    Some(div().text_color(theme.text_muted).child("±")),
                ),
            };
            let key = item.view.buffer().data().as_ffi();
            let close = div()
                .id(ElementId::NamedInteger(close_id.into(), key))
                .absolute()
                .top_0()
                .left_0()
                .size_full()
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(3.))
                .text_color(theme.text_muted)
                .hover(|style| style.bg(theme.chrome.hover).text_color(theme.text))
                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    cx.stop_propagation();
                    workspace::close_tab(&mut this.state, pane, index, Force::No);
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
                .text_color(theme.chrome.unsaved)
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
                .id(ElementId::NamedInteger(kind.into(), key))
                .group(GROUP)
                .flex_none()
                .h_full()
                .flex()
                .flex_col()
                .bg(bg)
                .text_color(fg)
                .border_r_1()
                .border_color(theme.chrome.border)
                .when(!active, |tab| {
                    tab.hover(|style| style.bg(theme.chrome.hover))
                })
                .on_click(cx.listener(move |this, event: &ClickEvent, window, cx| {
                    let click = match event.click_count() {
                        2 => Click::Double,
                        _ => Click::Single,
                    };
                    workspace::click_tab(&mut this.state, pane, index, click);
                    window.focus(&this.focus, cx);
                    this.changed(cx);
                }))
                .on_mouse_down(
                    MouseButton::Middle,
                    cx.listener(move |this, _, _, cx| {
                        cx.stop_propagation();
                        workspace::close_tab(&mut this.state, pane, index, Force::No);
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
                            detail.map(|detail| div().text_color(theme.text_muted).child(detail)),
                        )
                        .child(end),
                )
                .child(div().flex_none().h(px(1.)).bg(bottom))
        });
        bar.children(tabs_iter).child(
            div()
                .flex_1()
                .min_w(px(8.))
                .h_full()
                .flex()
                .flex_col()
                .justify_end()
                .child(div().h(px(1.)).bg(theme.chrome.border)),
        )
    }
}
