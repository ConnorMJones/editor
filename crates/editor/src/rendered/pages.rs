//! Pages views: typst pages drawn as a list, under a strip of its diagnostics.

use std::cell::Cell;
use std::rc::Rc;

use gpui::{
    AnyElement, Bounds, ClickEvent, Context, IntoElement, MouseButton, Pixels, SharedString,
    canvas, div, prelude::*, px, white,
};
use model::typeset::{Diagnostic, Origin, PagePoint, SourceFile, Svg};
use model::units::Pt;
use state::render::pages::{Fit, PagesState, Zoom};
use state::render::typeset::{Compiled, Outcome, Typst};
use state::text::buffer::BufferId;
use state::view::layout::PaneId;

use super::raster::svg_canvas;
use super::{Measured, update};
use crate::fonts::hsla;
use crate::workspace::Workspace;

/// Space between typst pages.
pub(super) const PAGE_GAP: Pixels = px(16.);

/// The strip above the pages: the first error or warning, or that a first compile runs.
pub(super) fn header(
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

/// Page `index` of `pane`'s pages view, panned with the list.
pub(super) fn item(
    this: &Workspace,
    pane: PaneId,
    buffer: BufferId,
    view: &PagesState,
    index: usize,
    cx: &mut Context<Workspace>,
) -> AnyElement {
    let typst = this.state.compiles.get(buffer);
    let (Some(typst), Some(cache)) = (typst, this.rendered.get(pane)) else {
        return div().into_any_element();
    };
    let Some(page) = (typst.pages.as_ref()).and_then(|compiled| compiled.pages.get(index)) else {
        return div().into_any_element();
    };
    let width = page_width(&cache.measured, view.fit, view.zoom, page);
    let scale = width / px(page.width.0);
    // Recorded each frame so a click turns into a point on the page.
    let bounds = Rc::new(Cell::new(Bounds::default()));
    let (recorded, workspace) = (bounds.clone(), cx.weak_entity());
    let sheet = div()
        .relative()
        .flex_none()
        .mx_auto()
        .w(width)
        .h(width * (page.height.0 / page.width.0))
        .bg(white())
        .shadow_md()
        .child(svg_canvas(&this.rasters, &cx.weak_entity(), page).size_full())
        .child(
            canvas(move |area, _, _| recorded.set(area), |_, _, _, _| {})
                .absolute()
                .size_full(),
        )
        .on_mouse_down(MouseButton::Left, move |event, window, cx| {
            cx.stop_propagation();
            let origin = bounds.get().origin;
            let at = PagePoint {
                page: index,
                x: Pt((event.position.x - origin.x) / px(1.) / scale),
                y: Pt((event.position.y - origin.y) / px(1.) / scale),
            };
            update(&workspace, window, cx, |this| {
                state::render::pages::click_page(&mut this.state, pane, at, &mut this.effects)
            });
        });
    let sheet = match typst.outcome {
        Outcome::Failed(_) => sheet.opacity(0.45),
        Outcome::Pending | Outcome::Compiled => sheet,
    };
    div()
        .w_full()
        .overflow_hidden()
        .px(PAGE_GAP)
        .pt(match index {
            0 => PAGE_GAP,
            _ => px(0.),
        })
        .pb(PAGE_GAP)
        .child(div().relative().left(-cache.left).flex().child(sheet))
        .into_any_element()
}

/// Drawn width of typst page `page` under `fit` and `zoom`, in a viewport measured at `measured`.
pub(super) fn page_width(measured: &Measured, fit: Fit, zoom: Zoom, page: &Svg) -> Pixels {
    let room = (measured.width - PAGE_GAP * 2.).max(px(1.));
    let fitted = match fit {
        Fit::Width => room,
        Fit::Page => {
            room.min((measured.height - PAGE_GAP * 2.).max(px(1.)) * (page.width.0 / page.height.0))
        }
    };
    fitted * zoom.factor()
}
