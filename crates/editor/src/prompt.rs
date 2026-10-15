use gpui::{Div, Pixels, SharedString, div, prelude::*};
use model::command::PromptMotion;
use model::theme as roles;
use state::Overlay;
use state::input::prompt::{self, Candidate, Chosen};

use crate::theme::Theme;

/// Completion rows shown above the prompt; helix's menu height.
const MENU_ROWS: f32 = 10.;

/// The open prompt drawn over the bottom line, with its completions above it.
pub fn prompt(overlay: &Overlay, theme: &Theme, line_height: Pixels) -> Option<Div> {
    let prompt = match overlay {
        Overlay::Prompt(prompt) => prompt,
        Overlay::None | Overlay::Picker(_) => return None,
    };
    let line = prompt.line.as_str();
    let end = prompt::target(line, prompt.cursor, PromptMotion::CharForward);
    let under = match &line[prompt.cursor..end] {
        "" => " ",
        under => under,
    };
    let cursor = div()
        .bg(theme.colors[roles::CURSOR_PRIMARY.bg])
        .text_color(theme.colors[roles::CURSOR_PRIMARY.fg])
        .child(SharedString::from(under.to_owned()));
    let input = div()
        .px_2()
        .h(line_height)
        .flex()
        .flex_row()
        .bg(theme.background)
        .child(prompt.kind.label())
        .child(SharedString::from(line[..prompt.cursor].to_owned()))
        .child(cursor)
        .child(SharedString::from(line[end..].to_owned()));

    let completion = &prompt.completion;
    let width = completion
        .candidates
        .iter()
        .map(|candidate| candidate.text().chars().count())
        .max()
        .unwrap_or(0)
        + 2;
    let items = completion
        .candidates
        .iter()
        .enumerate()
        .map(|(index, candidate)| {
            let bg = match completion.chosen {
                Chosen::At(chosen) if chosen == index => roles::MENU_SELECTED,
                Chosen::At(_) | Chosen::Nothing => roles::MENU,
            };
            let fg = match candidate {
                Candidate::Dir(_) => roles::MENU_DIR,
                Candidate::Command(_) | Candidate::File(_) => roles::MENU_TEXT,
            };
            div()
                .h(line_height)
                .w(theme.cell.width * width as f32)
                .bg(theme.colors[bg])
                .text_color(theme.colors[fg])
                .child(SharedString::from(candidate.text().to_owned()))
        });
    let menu = match completion.candidates.is_empty() {
        true => None,
        false => Some(
            div()
                .flex()
                .flex_row()
                .flex_wrap()
                .max_h(line_height * MENU_ROWS)
                .overflow_hidden()
                .px_2()
                .bg(theme.colors[roles::MENU])
                .children(items),
        ),
    };
    Some(
        div()
            .absolute()
            .left_0()
            .right_0()
            .bottom_0()
            .flex()
            .flex_col()
            .children(menu)
            .child(input),
    )
}
