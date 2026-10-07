use gpui::{Context, IntoElement, KeyDownEvent, Render, Window, div, prelude::*};
use state::buffer::Content;
use state::job::{self, JobResult};
use state::layout::View;
use state::{Effect, Overlay, State, input};

use crate::theme::Theme;

pub struct Workspace {
    pub state: State,
    pub theme: Theme,
    pub effects: Vec<Effect>,
    pub focus: gpui::FocusHandle,
}

impl Workspace {
    fn on_key(&mut self, event: &KeyDownEvent, cx: &mut Context<Self>) {
        let Some(key) = crate::input::translate(&event.keystroke) else {
            return;
        };
        input::key(&mut self.state, key, &mut self.effects);
        self.flush(cx);
        cx.notify();
    }

    fn on_job(&mut self, result: JobResult, cx: &mut Context<Self>) {
        job::complete(&mut self.state, result, &mut self.effects);
        self.flush(cx);
        cx.notify();
    }

    fn flush(&mut self, cx: &mut Context<Self>) {
        for effect in self.effects.drain(..) {
            match effect {
                Effect::Job(work) => {
                    let task = cx.background_spawn(async move { job::run(work) });
                    cx.spawn(async move |this, cx| {
                        let result = task.await;
                        this.update(cx, |workspace, cx| workspace.on_job(result, cx))
                    })
                    .detach();
                }
                Effect::Quit => cx.quit(),
            }
        }
    }
}

impl Render for Workspace {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = &self.theme;
        let state = &self.state;
        let mode = state.input.mode;
        let focused = state
            .layout
            .panes
            .get(state.layout.focus)
            .map(|pane| &pane.view);
        let (body, file, position) = match focused {
            Some(View::Text(view)) => {
                let buffer = &state.buffers[view.buffer];
                match &buffer.content {
                    Content::Text(doc) => {
                        let cursor = view.selection.primary().cursor(doc.text.slice(..));
                        let line = doc.text.char_to_line(cursor);
                        let column = cursor - doc.text.line_to_char(line);
                        let rows = window.viewport_size().height / window.line_height();
                        let lines = doc
                            .text
                            .lines()
                            .take(rows.max(1.0) as usize)
                            .map(|line| div().child(line.to_string().trim_end().to_owned()));
                        let name = buffer
                            .path
                            .strip_prefix(state.root.path())
                            .unwrap_or(&buffer.path)
                            .display()
                            .to_string();
                        let modified = match doc.is_modified() {
                            true => " [+]",
                            false => "",
                        };
                        (
                            div().flex_1().overflow_hidden().px_2().children(lines),
                            format!("{name}{modified}"),
                            format!("{}:{}", line + 1, column + 1),
                        )
                    }
                }
            }
            Some(View::Empty) | None => (div().flex_1(), String::new(), String::new()),
        };
        let pending: String = state
            .input
            .pending
            .as_slice()
            .iter()
            .map(|key| key.to_string())
            .collect();
        let bottom = match (&state.overlay, &state.notice) {
            (Overlay::Prompt(prompt), _) => format!(":{}", prompt.line),
            (Overlay::None, Some(notice)) => notice.to_string(),
            (Overlay::None, None) => String::new(),
        };
        div()
            .key_context("Workspace")
            .track_focus(&self.focus)
            .on_key_down(cx.listener(|this, event, _, cx| this.on_key(event, cx)))
            .size_full()
            .flex()
            .flex_col()
            .bg(theme.background)
            .text_color(theme.text)
            .font(theme.mono.clone())
            .text_size(theme.mono_size)
            .child(body)
            .child(
                div()
                    .flex()
                    .flex_row()
                    .bg(theme.surface)
                    .font(theme.ui.clone())
                    .text_size(theme.ui_size)
                    .child(
                        div()
                            .px_2()
                            .bg(theme.mode[mode])
                            .text_color(theme.mode.fg)
                            .child(mode.label()),
                    )
                    .child(div().px_2().child(file))
                    .child(div().flex_1())
                    .child(div().px_2().text_color(theme.text_muted).child(pending))
                    .child(div().px_2().child(position)),
            )
            .child(div().px_2().h(window.line_height()).child(bottom))
    }
}
