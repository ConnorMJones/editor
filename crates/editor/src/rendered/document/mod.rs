//! Document views: markdown drawn as a list of blocks.

mod inline;
mod media;

use inline::inline;
use media::{code_box, figure_element, picture, placeholder};

use std::cell::RefCell;
use std::rc::Rc;

use gpui::{
    AnyElement, ClickEvent, Context, Div, IntoElement, SharedString, TextAlign, TextRun,
    WeakEntity, div, prelude::*, px,
};
use model::markdown::CalloutKind;
use model::theme::{Modifiers, Theme};
use state::project::links::{self, Origin as LinkOrigin, Resolution};
use state::render::document::{BlockClick, DocumentState, Parsed};
use state::render::markdown::{
    self, BlockKind, CalloutFold, Column, EmbedKind, LinkTarget, ListKind, Markdown, RowKind, Task,
};
use state::render::typeset::Figures;
use state::text::buffer::BufferId;
use state::view::layout::PaneId;

use super::update;
use crate::fonts::{Fonts, PROSE_LINE, hsla};
use crate::pictures::Pictures;
use crate::workspace::Workspace;

/// Reading column width in ems of the prose, about 75 characters.
const MEASURE: f32 = 45.0;
/// Heading sizes relative to prose, `#` to `######`.
const SCALES: [f32; 6] = [1.6, 1.4, 1.25, 1.12, 1.05, 1.0];

/// What a block's drawing needs from the workspace.
struct Draw<'a> {
    theme: &'a Theme,
    fonts: &'a Fonts,
    view: &'a DocumentState,
    markdown: &'a Markdown,
    figures: &'a Figures,
    pictures: &'a Rc<RefCell<Pictures>>,
    pane: PaneId,
    buffer: BufferId,
    version: u64,
    workspace: WeakEntity<Workspace>,
}

/// How a block's text is inked: done tasks read struck through and muted.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Ink {
    Plain,
    Done,
}

/// Root block `index` of `pane`'s document view, in the reading column.
pub fn item(
    this: &Workspace,
    pane: PaneId,
    buffer: BufferId,
    view: &DocumentState,
    index: usize,
    cx: &mut Context<Workspace>,
) -> AnyElement {
    let (state, theme, fonts) = (&this.state, &this.theme, &this.fonts);
    let Some(Parsed {
        version, markdown, ..
    }) = state.documents.get(buffer)
    else {
        return div().into_any_element();
    };
    let Some(root) = markdown.roots.get(index) else {
        return div().into_any_element();
    };
    let draw = Draw {
        theme,
        fonts,
        view,
        markdown,
        figures: &state.figures,
        pictures: &this.pictures,
        pane,
        buffer,
        version: *version,
        workspace: cx.weak_entity(),
    };
    let top = match index {
        0 => fonts.prose_size * 1.5,
        _ => fonts.prose_size * 0.375,
    };
    div()
        .w_full()
        .flex()
        .justify_center()
        .px_6()
        .pt(top)
        .pb(fonts.prose_size * 0.375)
        .font(fonts.prose[0].clone())
        .text_size(fonts.prose_size)
        .line_height(fonts.prose_size * PROSE_LINE)
        .text_color(hsla(theme.text))
        .child(
            div()
                .w_full()
                .max_w(fonts.prose_size * MEASURE)
                .child(block(&draw, *root, Ink::Plain)),
        )
        .into_any_element()
}

/// A block that puts the source cursor on its first line when clicked.
fn clickable(draw: &Draw, index: usize) -> gpui::Stateful<Div> {
    let (workspace, pane) = (draw.workspace.clone(), draw.pane);
    let click = BlockClick {
        line: draw.markdown.blocks[index].lines.start,
        version: draw.version,
    };
    div()
        .id(("block", index))
        .on_click(move |_: &ClickEvent, window, cx| {
            cx.stop_propagation();
            update(&workspace, window, cx, |this| {
                state::render::document::click(&mut this.state, pane, click)
            });
        })
}

fn block(draw: &Draw, index: usize, ink: Ink) -> AnyElement {
    let theme = draw.theme;
    let fonts = draw.fonts;
    let markdown = draw.markdown;
    let target = &markdown.blocks[index];
    let children = || markdown.children(index);
    match &target.kind {
        BlockKind::Heading { level, text } => {
            let scale = SCALES[*level as usize - 1];
            let size = fonts.prose_size * scale;
            let heading = clickable(draw, index)
                .font_weight(gpui::FontWeight::BOLD)
                .text_size(size)
                .line_height(size * 1.3)
                .mt(size * 0.4)
                .child(inline(draw, text, index, ink, Modifiers::BOLD));
            match level {
                markdown::Level::H1 | markdown::Level::H2 => heading
                    .pb(fonts.prose_size * 0.25)
                    .border_b_1()
                    .border_color(hsla(theme.chrome.border))
                    .into_any_element(),
                markdown::Level::H3
                | markdown::Level::H4
                | markdown::Level::H5
                | markdown::Level::H6 => heading.into_any_element(),
            }
        }
        BlockKind::Paragraph(text) => clickable(draw, index)
            .child(inline(draw, text, index, ink, Modifiers::empty()))
            .into_any_element(),
        BlockKind::List(kind) => {
            let depth = (markdown.blocks[..index].iter())
                .filter(|outer| matches!(outer.kind, BlockKind::List(_)) && outer.end >= target.end)
                .count();
            let bullet = match depth % 2 {
                0 => "•",
                _ => "◦",
            };
            div()
                .flex()
                .flex_col()
                .gap_1()
                .children(children().enumerate().map(|(n, item)| {
                    let task = match markdown.blocks[item].kind {
                        BlockKind::Item(task) => task,
                        _ => Task::None,
                    };
                    let marker = match (task, kind) {
                        (Task::Open { marker }, _) => checkbox(draw, marker, Ink::Plain),
                        (Task::Done { marker }, _) => checkbox(draw, marker, Ink::Done),
                        (Task::None, ListKind::Bullet) => div()
                            .text_color(hsla(theme.prose.marker))
                            .child(bullet)
                            .into_any_element(),
                        (Task::None, ListKind::Ordered { start }) => div()
                            .text_color(hsla(theme.prose.marker))
                            .child(format!("{}.", start + n as u64))
                            .into_any_element(),
                    };
                    let ink = match task {
                        Task::Done { .. } => Ink::Done,
                        Task::Open { .. } | Task::None => ink,
                    };
                    div()
                        .flex()
                        .flex_row()
                        .gap_2()
                        .child(
                            div()
                                .flex_none()
                                .min_w(fonts.prose_size * 1.125)
                                .flex()
                                .justify_end()
                                .child(marker),
                        )
                        .child(
                            div().flex_1().min_w_0().flex().flex_col().gap_1().children(
                                markdown.children(item).map(|child| block(draw, child, ink)),
                            ),
                        )
                }))
                .into_any_element()
        }
        BlockKind::Item(_) => div()
            .children(children().map(|child| block(draw, child, ink)))
            .into_any_element(),
        BlockKind::Quote => div()
            .flex()
            .flex_col()
            .gap_2()
            .pl_4()
            .border_l(px(3.))
            .border_color(hsla(theme.prose.quote_border))
            .text_color(hsla(theme.prose.quote_text))
            .children(children().map(|child| block(draw, child, ink)))
            .into_any_element(),
        BlockKind::Callout(callout) => {
            let color = hsla(theme.prose[callout.kind]);
            let icon = match callout.kind {
                CalloutKind::Note => "✎",
                CalloutKind::Abstract | CalloutKind::Example => "☰",
                CalloutKind::Info => "ℹ",
                CalloutKind::Todo => "☑",
                CalloutKind::Tip => "✦",
                CalloutKind::Success => "✔",
                CalloutKind::Question => "?",
                CalloutKind::Warning => "⚠",
                CalloutKind::Failure => "✘",
                CalloutKind::Danger => "⚡",
                CalloutKind::Bug => "✱",
                CalloutKind::Quote => "❝",
            };
            let fold = draw.view.fold(callout);
            let chevron = match fold {
                CalloutFold::Static => "",
                CalloutFold::Open => "▾",
                CalloutFold::Closed => "▸",
            };
            let (workspace, pane, ordinal) = (draw.workspace.clone(), draw.pane, callout.index);
            let header = div()
                .id(("callout", index))
                .flex()
                .flex_row()
                .items_center()
                .gap_2()
                .text_color(color)
                .font_weight(gpui::FontWeight::BOLD)
                .child(icon)
                .child(
                    div()
                        .flex_1()
                        .child(SharedString::from(callout.title.clone())),
                )
                .child(chevron);
            let header = match fold {
                CalloutFold::Static => header,
                CalloutFold::Open | CalloutFold::Closed => {
                    header
                        .cursor_pointer()
                        .on_click(move |_: &ClickEvent, window, cx| {
                            cx.stop_propagation();
                            update(&workspace, window, cx, |this| {
                                state::render::document::toggle_fold(&mut this.state, pane, ordinal)
                            });
                        })
                }
            };
            let body = match fold {
                CalloutFold::Static | CalloutFold::Open => Some(
                    div()
                        .flex()
                        .flex_col()
                        .gap_2()
                        .children(children().map(|child| block(draw, child, ink))),
                ),
                CalloutFold::Closed => None,
            };
            div()
                .flex()
                .flex_col()
                .gap_2()
                .px_3()
                .py_2()
                .rounded_sm()
                .border_l(px(3.))
                .border_color(color)
                .bg(color.opacity(0.08))
                .child(header)
                .children(body)
                .into_any_element()
        }
        BlockKind::Code(code) => {
            let runs = markdown.highlights.get(&code.hash).map(|runs| {
                let mut start = 0;
                runs.iter()
                    .map(|run| {
                        let len = run.end - start;
                        start = run.end;
                        TextRun {
                            len,
                            font: fonts.mono_font(run.style.modifiers).clone(),
                            color: run
                                .style
                                .fg
                                .map_or(hsla(theme.text), |slot| hsla(theme.syntax[slot])),
                            background_color: None,
                            underline: None,
                            strikethrough: None,
                        }
                    })
                    .collect::<Vec<_>>()
            });
            clickable(draw, index)
                .child(code_box(
                    theme,
                    fonts,
                    code.lang.clone().map(SharedString::from),
                    SharedString::from(code.text.clone()),
                    runs.filter(|runs| !runs.is_empty()),
                ))
                .into_any_element()
        }
        BlockKind::Math(math) => clickable(draw, index)
            .child(figure_element(draw, math.figure, &math.tex))
            .into_any_element(),
        BlockKind::Mermaid(mermaid) => clickable(draw, index)
            .child(figure_element(draw, mermaid.figure, &mermaid.src))
            .into_any_element(),
        BlockKind::Table(columns) => {
            let columns = columns.clone();
            div()
                .flex()
                .flex_col()
                .border_1()
                .border_color(hsla(theme.chrome.border))
                .children(children().map(|row| {
                    let head = match markdown.blocks[row].kind {
                        BlockKind::Row(RowKind::Head) => RowKind::Head,
                        _ => RowKind::Body,
                    };
                    let line = div()
                        .flex()
                        .flex_row()
                        .border_b_1()
                        .border_color(hsla(theme.chrome.border));
                    let line = match head {
                        RowKind::Head => line
                            .bg(hsla(theme.surface))
                            .font_weight(gpui::FontWeight::BOLD),
                        RowKind::Body => line,
                    };
                    line.children(markdown.children(row).enumerate().map(|(n, cell)| {
                        let align = match columns.get(n).copied().unwrap_or(Column::None) {
                            Column::None | Column::Left => TextAlign::Left,
                            Column::Center => TextAlign::Center,
                            Column::Right => TextAlign::Right,
                        };
                        let content = match &markdown.blocks[cell].kind {
                            BlockKind::Cell(text) => {
                                let modifiers = match head {
                                    RowKind::Head => Modifiers::BOLD,
                                    RowKind::Body => Modifiers::empty(),
                                };
                                Some(inline(draw, text, cell, ink, modifiers))
                            }
                            _ => None,
                        };
                        div()
                            .flex_1()
                            .min_w_0()
                            .px_2()
                            .py_1()
                            .border_r_1()
                            .border_color(hsla(theme.chrome.border))
                            .text_align(align)
                            .children(content)
                    }))
                }))
                .into_any_element()
        }
        BlockKind::Row(_) | BlockKind::Cell(_) => div().into_any_element(),
        BlockKind::Image(image) => clickable(draw, index)
            .child(picture(draw, &image.file, &image.src))
            .into_any_element(),
        BlockKind::Embed(embed) => match &embed.kind {
            EmbedKind::Image(file) => clickable(draw, index)
                .child(picture(draw, file, &embed.link.target))
                .into_any_element(),
            EmbedKind::Note(resolution) => {
                let (label, shown) = match resolution {
                    Resolution::File(path) => ("embedded note", format!("⧉ {path}")),
                    Resolution::Missing(path) => ("embedded note not found", format!("⧉ {path}")),
                    Resolution::Invalid => ("embed", format!("⧉ {}", embed.link.target)),
                };
                let (workspace, pane, buffer) = (draw.workspace.clone(), draw.pane, draw.buffer);
                let target = LinkTarget::Wiki(embed.link.clone());
                div()
                    .id(("embed", index))
                    .cursor_pointer()
                    .on_click(move |_: &ClickEvent, window, cx| {
                        cx.stop_propagation();
                        update(&workspace, window, cx, |this| {
                            links::follow(
                                &mut this.state,
                                buffer,
                                &target,
                                LinkOrigin::Document(pane),
                                &mut this.effects,
                            )
                        });
                    })
                    .child(placeholder(theme, fonts, label, SharedString::from(shown)))
                    .into_any_element()
            }
        },
        BlockKind::Rule => div()
            .my_2()
            .h(px(1.))
            .w_full()
            .bg(hsla(theme.chrome.border))
            .into_any_element(),
        BlockKind::Html(html) => clickable(draw, index)
            .child(code_box(
                theme,
                fonts,
                Some(SharedString::from("html")),
                SharedString::from(html.clone()),
                None,
            ))
            .into_any_element(),
        BlockKind::Frontmatter(text) => clickable(draw, index)
            .child(code_box(
                theme,
                fonts,
                Some(SharedString::from("properties")),
                SharedString::from(text.clone()),
                None,
            ))
            .into_any_element(),
        BlockKind::Footnote { label } => div()
            .flex()
            .flex_row()
            .gap_2()
            .text_size(fonts.prose_size * 0.9)
            .text_color(hsla(theme.muted))
            .child(
                div()
                    .flex_none()
                    .child(SharedString::from(format!("[{label}]"))),
            )
            .child(
                div()
                    .flex_1()
                    .flex()
                    .flex_col()
                    .children(children().map(|child| block(draw, child, ink))),
            )
            .into_any_element(),
    }
}

/// A clickable task checkbox flipping the `[ ]` at byte `marker` of the source.
fn checkbox(draw: &Draw, marker: usize, ink: Ink) -> AnyElement {
    let theme = draw.theme;
    let fonts = draw.fonts;
    let (workspace, buffer, version) = (draw.workspace.clone(), draw.buffer, draw.version);
    let accent = hsla(theme.chrome.accent);
    let box_ = div()
        .id(("task", marker))
        .mt(fonts.prose_size * 0.25)
        .size(fonts.prose_size)
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(3.))
        .border_1()
        .cursor_pointer()
        .text_size(fonts.prose_size * 0.75)
        .hover(move |style| style.border_color(accent))
        .on_click(move |_: &ClickEvent, window, cx| {
            cx.stop_propagation();
            update(&workspace, window, cx, |this| {
                state::render::document::toggle_task(&mut this.state, buffer, version, marker)
            });
        });
    match ink {
        Ink::Plain => box_.border_color(hsla(theme.prose.checkbox)),
        Ink::Done => box_
            .border_color(accent)
            .bg(accent)
            .text_color(hsla(theme.background))
            .child("✓"),
    }
    .into_any_element()
}
