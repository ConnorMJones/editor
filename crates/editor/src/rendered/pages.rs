//! The strip of diagnostics over a typst file's pages; the pages are a `SheetElement`.

use gpui::{AnyElement, ClickEvent, Context, SharedString, div, prelude::*};
use model::typeset::{Diagnostic, Origin, SourceFile};
use state::render::typeset::{Compiled, Outcome, Typst};
use state::view::layout::PaneId;

use super::update;
use crate::fonts::hsla;
use crate::workspace::Workspace;

/// The strip above the pages: the first error or warning, or that a first compile runs.
pub fn header(
    workspace: &Workspace,
    pane: PaneId,
    typst: &Typst,
    cx: &mut Context<Workspace>,
) -> Option<AnyElement> {
    let (state, theme, fonts) = (&workspace.state, &workspace.theme, &workspace.fonts);
    let strip = |color: gpui::Hsla| {
        div()
            .flex_none()
            .w_full()
            .px_3()
            .py_1()
            .font(fonts.ui.clone())
            .text_size(fonts.ui_size)
            .text_color(color)
            .bg(color.opacity(0.12))
            .border_b_1()
            .border_color(hsla(theme.chrome.border))
    };
    // Clicking a diagnostic strip opens the first entry's place.
    let diagnostics = |listed: &[Diagnostic], mark: &str, color: gpui::Hsla| {
        let first = &listed[0];
        let place = match &first.origin {
            Origin::Span {
                file: SourceFile::Project(path),
                start,
                ..
            } => format!(
                "{}:{}:{}: ",
                path.strip_prefix(state.project.root.path())
                    .unwrap_or(path)
                    .display(),
                start.line + 1,
                start.column + 1
            ),
            Origin::Span {
                file: file @ (SourceFile::Embedded(_) | SourceFile::Package(_)),
                start,
                ..
            } => format!("{file}:{}:{}: ", start.line + 1, start.column + 1),
            Origin::Detached => String::new(),
        };
        let more = match listed.len() {
            1 => String::new(),
            count => format!("  (+{} more)", count - 1),
        };
        let workspace = cx.weak_entity();
        strip(color)
            .id("typst-diagnostics")
            .cursor_pointer()
            .on_click(move |_: &ClickEvent, window, cx| {
                cx.stop_propagation();
                update(&workspace, window, cx, |this| {
                    state::render::pages::goto_diagnostic(&mut this.state, pane, &mut this.effects)
                });
            })
            .child(SharedString::from(format!(
                "{mark} {place}{}{more}",
                first.message
            )))
            .into_any_element()
    };
    match (&typst.outcome, &typst.pages) {
        (Outcome::Failed(errors), _) => Some(diagnostics(errors, "✘", hsla(theme.prose.error))),
        (Outcome::Pending | Outcome::Compiled, Some(Compiled { warnings, .. })) => {
            match warnings.as_slice() {
                [] => None,
                warnings => Some(diagnostics(warnings, "⚠", hsla(theme.prose.warning))),
            }
        }
        (Outcome::Pending, None) => Some(
            strip(hsla(theme.muted))
                .child("compiling…")
                .into_any_element(),
        ),
        (Outcome::Compiled, None) => None,
    }
}
