//! Figures, images and boxes standing in for content: code, and what cannot show yet.

use gpui::{AnyElement, Div, IntoElement, SharedString, StyledText, TextRun, div, prelude::*, px};
use model::picture::{Picture, Source};
use model::theme::Theme;
use model::units::Px;
use state::render::markdown::{Figure, FigureKind, ImageFile};
use state::render::typeset::{Probe, Render};
use state::viewer::Probed;

use super::Draw;
use crate::fonts::{Fonts, hsla};
use crate::pictures::picture_canvas;

/// A monospace box with an optional label, highlighted when `runs` are given.
pub fn code_box(
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
pub fn placeholder(
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
pub fn figure_element(draw: &Draw, figure: Figure, source: &str) -> AnyElement {
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
                    picture_canvas(draw.pictures, &draw.workspace, svg.picture())
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

/// An image file at its pixel size, narrowed to the column; a box until its size is read.
pub fn picture(draw: &Draw, file: &ImageFile, src: &str) -> AnyElement {
    let (theme, fonts) = (draw.theme, draw.fonts);
    let boxed =
        |label, text: &str| placeholder(theme, fonts, label, SharedString::from(text.to_owned()));
    match file {
        ImageFile::Local(local) => match draw.figures.image(local) {
            Some(Probe::Read(Probed { size, modified })) => {
                let (width, height) = (size.width.get(), size.height.get());
                let picture = Picture {
                    source: Source::File {
                        path: local.path.clone(),
                        modified: *modified,
                    },
                    encoding: local.encoding,
                };
                div()
                    .w_full()
                    .child(
                        picture_canvas(draw.pictures, &draw.workspace, picture)
                            .w_full()
                            .max_w(px(width))
                            .aspect_ratio(width / height),
                    )
                    .into_any_element()
            }
            Some(Probe::Failed(error)) => boxed("image unreadable", &format!("{src}\n{error}")),
            Some(Probe::Pending) | None => boxed("image", src),
        },
        ImageFile::Remote(url) => boxed("remote image", url),
        ImageFile::Missing => boxed("image not found", src),
        ImageFile::NotPicture => boxed("not an image", src),
    }
}
