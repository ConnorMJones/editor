//! The picker overlay: matches, preview and query line.

use std::ops::Range;

use gpui::{
    ClickEvent, Context, Div, MouseButton, Stateful, Window, div, prelude::*, uniform_list,
};
use model::command::PickerCommand;
use state::input::keymap::{self, Context as KeyContext};
use state::input::picker::{Grep, Items, PALETTE, Referrers, Search};
use state::{Overlay, view::panes};

use super::Workspace;
use crate::fonts::hsla;
use crate::preview::PreviewElement;

impl Workspace {
    pub fn picker(&self, window: &mut Window, cx: &mut Context<Self>) -> Option<Stateful<Div>> {
        let Overlay::Picker(picker) = &self.state.overlay else {
            return None;
        };
        let theme = &self.theme;
        let fonts = &self.fonts;
        let previewed = match picker.items {
            Items::Files(_) | Items::Buffers(_) | Items::Grep(_) | Items::Backlinks(_) => true,
            Items::Commands | Items::Images(_) | Items::Outline(_) => false,
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
            Items::Files(_)
            | Items::Images(_)
            | Items::Buffers(_)
            | Items::Commands
            | Items::Backlinks(_)
            | Items::Outline(_) => {
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
                            Items::Files(_) | Items::Images(_) | Items::Buffers(_) => {
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
                            Items::Outline(headings) => {
                                let heading = &headings[index as usize];
                                (
                                    format!("{}{}", "  ".repeat(heading.depth), heading.title),
                                    heading
                                        .page
                                        .map_or(String::new(), |page| (page + 1).to_string()),
                                )
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
