use std::borrow::Cow;

use gpui::{Context, Div, Hsla, MouseButton, MouseDownEvent, div, prelude::*};
use helix_core::SmallVec;
use library::{Comment, TextSpan, Texts};
use model::command::{Annotate, Command, ReadingCommand};
use model::theme::ReaderColors;
use state::apply;
use state::reading::{Found, Notes, Reading};

use crate::fonts::hsla;
use crate::workspace::Workspace;

pub type Marks<'a> = SmallVec<[(Cow<'a, TextSpan>, Hsla); 16]>;

#[derive(Clone, Copy)]
pub enum Offer<'a> {
    Selection,
    Highlight(Option<&'a Comment>),
}

impl Offer<'_> {
    pub fn of(reading: &Reading) -> Option<Offer<'_>> {
        match reading {
            Reading::Selected(_) => Some(Offer::Selection),
            Reading::OnHighlight(highlight) => Some(Offer::Highlight(highlight.comment.as_ref())),
            Reading::Idle | Reading::Selecting { .. } | Reading::Writing(_) => None,
        }
    }
}

pub fn marks<'a>(
    colors: &ReaderColors,
    notes: &'a Notes,
    found: &'a Found,
    reading: &Reading,
    texts: &dyn Texts,
) -> Marks<'a> {
    let mut marks = Marks::new();
    match notes {
        Notes::Listed(listed) => marks.extend(
            (listed.notebook.highlights.iter())
                .map(|highlight| (Cow::Borrowed(&highlight.span), hsla(colors.highlight))),
        ),
        Notes::Unlisted(_) => {}
    }
    match found {
        Found::Hits { hits, at, .. } => {
            marks.extend((hits.iter().enumerate()).map(|(index, hit)| {
                let color = match index == *at {
                    true => colors.current_hit,
                    false => colors.hit,
                };
                (Cow::Borrowed(hit), hsla(color))
            }))
        }
        Found::Nothing | Found::Searching { .. } => {}
    }
    marks.extend((reading.shown(texts)).map(|span| (Cow::Owned(span), hsla(colors.selection))));
    marks
}

pub fn popup(workspace: &Workspace, offer: Offer, cx: &mut Context<Workspace>) -> Div {
    let (theme, fonts) = (&workspace.theme, &workspace.fonts);
    let button = |label: &'static str, command: ReadingCommand| {
        div()
            .id(label)
            .px(fonts.ui_size * 0.6)
            .rounded_sm()
            .cursor_pointer()
            .hover(|style| style.bg(hsla(theme.chrome.hover)))
            .child(label)
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _: &MouseDownEvent, _, cx| {
                    cx.stop_propagation();
                    apply::apply(
                        &mut this.state,
                        Command::Reading(command),
                        &mut this.effects,
                    );
                    this.changed(cx);
                }),
            )
    };
    let buttons = div()
        .h(fonts.ui_line * 1.2)
        .flex()
        .items_center()
        .gap(fonts.ui_size * 0.2);
    let (comment, buttons) = match offer {
        Offer::Selection => (
            None,
            buttons
                .child(button(
                    "Highlight",
                    ReadingCommand::Annotate(Annotate::Highlight),
                ))
                .child(button(
                    "Comment",
                    ReadingCommand::Annotate(Annotate::Comment),
                ))
                .child(button("Copy", ReadingCommand::Copy))
                .child(button("Search", ReadingCommand::FindSelection)),
        ),
        Offer::Highlight(comment) => (
            comment,
            buttons
                .child(button(
                    "Edit comment",
                    ReadingCommand::Annotate(Annotate::Comment),
                ))
                .child(button("Delete", ReadingCommand::Annotate(Annotate::Remove))),
        ),
    };
    div()
        .max_w(fonts.ui_size * 28.0)
        .flex()
        .flex_col()
        .px(fonts.ui_size * 0.2)
        .rounded_md()
        .border_1()
        .border_color(hsla(theme.chrome.border))
        .bg(hsla(theme.surface))
        .text_color(hsla(theme.text))
        .text_size(fonts.ui_size)
        .children(comment.map(|comment| {
            div()
                .px(fonts.ui_size * 0.6)
                .py(fonts.ui_size * 0.3)
                .child(comment.line().into_owned())
        }))
        .child(buttons)
}
