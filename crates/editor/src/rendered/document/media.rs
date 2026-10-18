//! Figures, images and boxes standing in for content: code, and what cannot show yet.

use gpui::{
    AnyElement, Div, IntoElement, ObjectFit, SharedString, StyledText, TextRun, div, img,
    prelude::*, px,
};
use model::theme::Theme;
use model::units::Px;
use state::render::markdown::{Figure, FigureKind, ImageFile};
use state::render::typeset::Render;

use super::super::raster::svg_canvas;
use super::Draw;
use crate::fonts::{Fonts, hsla};

/// A monospace box with an optional label, highlighted when `runs` are given.
pub(super) fn code_box(
    theme: &Theme,
    fonts: &Fonts,
    label: Option<SharedString>,
    text: SharedString,
    runs: Option<Vec<TextRun>>,
) -> Div {
    let styled = match runs {
        Some(runs) => StyledText::new(text).with_runs(runs),
        None => StyledText::new(text),
    };
    div()
        .relative()
        .w_full()
        .px_3()
        .py_2()
        .rounded_md()
        .bg(hsla(theme.surface))
        .font(fonts.mono[0].clone())
        .text_size(fonts.mono_size * 0.9)
        .line_height(fonts.mono_size * 1.3)
        .whitespace_normal()
        .overflow_hidden()
        .child(styled)
        .children(label.map(|label| {
            div()
                .absolute()
                .top_1()
                .right_2()
                .text_size(fonts.ui_size * 0.85)
                .text_color(hsla(theme.muted))
                .child(label)
        }))
}

/// A muted box standing in for content that cannot be shown yet.
pub(super) fn placeholder(
    theme: &Theme,
    fonts: &Fonts,
    label: &'static str,
    text: SharedString,
) -> AnyElement {
    div()
        .w_full()
        .px_3()
        .py_2()
        .rounded_md()
        .border_1()
        .border_dashed()
        .border_color(hsla(theme.chrome.border))
        .text_color(hsla(theme.muted))
        .flex()
        .flex_col()
        .child(
            div()
                .text_size(fonts.ui_size * 0.85)
                .font(fonts.ui.clone())
                .child(label),
        )
        .child(
            div()
                .font(fonts.mono[0].clone())
                .text_size(fonts.mono_size * 0.9)
                .child(text),
        )
        .into_any_element()
}

/// A typeset figure: its image once ready, else its source (and error) in a muted box.
pub(super) fn figure_element(draw: &Draw, figure: Figure, source: &str) -> AnyElement {
    let theme = draw.theme;
    let fonts = draw.fonts;
    let label = match figure.kind {
        FigureKind::Math(_) => "math",
        FigureKind::Mermaid => "mermaid",
    };
    match draw.figures.get(figure) {
        Some(Render::Ready(svg)) => {
            let (Px(width), Px(height)) = (Px::from(svg.width), Px::from(svg.height));
            div()
                .w_full()
                .flex()
                .justify_center()
                .child(
                    svg_canvas(draw.rasters, &draw.workspace, svg)
                        .w_full()
                        .max_w(px(width))
                        .aspect_ratio(width / height),
                )
                .into_any_element()
        }
        Some(Render::Failed(error)) => placeholder(
            theme,
            fonts,
            label,
            SharedString::from(format!("{error}\n{source}")),
        ),
        Some(Render::Pending) | None => {
            placeholder(theme, fonts, label, SharedString::from(source.to_owned()))
        }
    }
}

pub(super) fn picture(theme: &Theme, fonts: &Fonts, file: &ImageFile, src: &str) -> AnyElement {
    match file {
        ImageFile::Local(path) => {
            let (missing, theme_muted) =
                (SharedString::from(format!("🖼 {src}")), hsla(theme.muted));
            img(path.clone())
                .max_w_full()
                .object_fit(ObjectFit::ScaleDown)
                .with_fallback(move || {
                    div()
                        .text_color(theme_muted)
                        .child(missing.clone())
                        .into_any_element()
                })
                .into_any_element()
        }
        ImageFile::Remote(url) => placeholder(
            theme,
            fonts,
            "remote image",
            SharedString::from(url.clone()),
        ),
        ImageFile::Missing => placeholder(
            theme,
            fonts,
            "image not found",
            SharedString::from(src.to_owned()),
        ),
    }
}
