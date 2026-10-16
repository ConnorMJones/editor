use gpui::{Div, Pixels, SharedString, div, prelude::*};
use model::command::PromptMotion;
use model::theme::Theme;
use state::Overlay;
use state::input::prompt::{self, Candidate, Chosen};

use crate::fonts::{Fonts, hsla};

/// Completion rows shown above the prompt; helix's menu height.
const MENU_ROWS: f32 = 10.;

/// The open prompt drawn over the bottom line, with its completions above it.
pub fn prompt(overlay: &Overlay, theme: &Theme, fonts: &Fonts, line_height: Pixels) -> Option<Div> {
    let prompt = match overlay {
        Overlay::Prompt(prompt) => prompt,
        Overlay::None | Overlay::Picker(_) | Overlay::Confirm(_) => return None,
    };
    let line = prompt.line.as_str();
    let end = prompt::target(line, prompt.cursor, PromptMotion::CharForward);
    let under = match &line[prompt.cursor..end] {
        "" => " ",
        under => under,
    };
    let cursor = div()
        .bg(hsla(theme.editor.cursor_primary))
        .text_color(hsla(theme.editor.cursor_text))
        .child(SharedString::from(under.to_owned()));
    let input = div()
        .px_2()
        .h(line_height)
        .flex()
        .flex_row()
        .bg(hsla(theme.background))
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
                Chosen::At(chosen) if chosen == index => theme.completion.selected,
                Chosen::At(_) | Chosen::Nothing => theme.completion.background,
            };
            let fg = match candidate {
                Candidate::Dir(_) => theme.completion.dir,
                Candidate::Command(_) | Candidate::File(_) => theme.completion.text,
            };
            div()
                .h(line_height)
                .w(fonts.cell.width * width as f32)
                .bg(hsla(bg))
                .text_color(hsla(fg))
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
                .bg(hsla(theme.completion.background))
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
