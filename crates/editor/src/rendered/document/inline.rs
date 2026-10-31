//! Text within a block: styled runs, links, and math flowing on a shared baseline.

use gpui::{
    AnyElement, App, ClickEvent, InteractiveText, IntoElement, MouseButton, MouseDownEvent,
    MouseMoveEvent, Pixels, Point, SharedString, StrikethroughStyle, StyledText, TextRun,
    UnderlineStyle, WeakEntity, Window, div, prelude::*, px,
};
use helix_core::SmallVec;
use model::theme::{Modifiers, Theme};
use model::typeset::{Display, Svg};
use model::units::Px;
use state::book::Pressed;
use state::reading::Press;
use state::render::markdown::{Emphasis, Figure, FigureKind, Inline, Span, SpanKind};
use state::render::typeset::Render;
use state::view::layout::PaneId;

use super::super::update;
use super::Ink;
use super::media::figure_element;
use super::{Draw, Drawable};
use crate::fonts::{Fonts, hsla};
use crate::pictures::picture_canvas;
use crate::workspace::Workspace;

fn follow_link<O: Drawable>(
    workspace: &WeakEntity<Workspace>,
    key: O::Key,
    pane: PaneId,
    index: usize,
    link: usize,
    window: &mut Window,
    cx: &mut App,
) {
    update(workspace, window, cx, |this| {
        let target = O::flow(&this.state, key)
            .and_then(|flow| flow.blocks.get(index))
            .and_then(|block| block.inline())
            .and_then(|text| text.links.get(link))
            .map(|link| link.target.clone());
        match target {
            Some(target) => O::follow(this, key, pane, &target),
            None => {}
        }
    });
}

/// Styled text with clickable links; with math it flows as words and formulas on one baseline.
pub fn inline<O: Drawable>(
    draw: &Draw<O>,
    text: &Inline<O::Target>,
    index: usize,
    ink: Ink,
    base: Modifiers,
) -> AnyElement {
    let theme = draw.theme;
    let fonts = draw.fonts;
    let flows = (text.spans.iter()).any(|span| matches!(span.kind, SpanKind::Math(_)));
    match flows {
        false => {
            let tints = (draw.marked)(index);
            let mut runs: Vec<TextRun> = Vec::with_capacity(text.spans.len() + 2 * tints.len());
            for span in text.spans.iter() {
                let whole = run(theme, fonts, span, ink, base);
                let mut cuts: SmallVec<[usize; 8]> = (tints.iter())
                    .flat_map(|(range, _)| [range.start, range.end])
                    .filter(|cut| span.range.start < *cut && *cut < span.range.end)
                    .collect();
                cuts.push(span.range.end);
                cuts.sort_unstable();
                cuts.dedup();
                let mut from = span.range.start;
                for cut in cuts {
                    let tinted = (tints.iter().rev())
                        .find(|(range, _)| range.start <= from && from < range.end)
                        .map(|(_, color)| *color);
                    runs.push(TextRun {
                        len: cut - from,
                        background_color: tinted.or(whole.background_color),
                        ..whole.clone()
                    });
                    from = cut;
                }
            }
            let styled = StyledText::new(SharedString::from(text.text.clone())).with_runs(runs);
            let layout = styled.layout().clone();
            let shown = match text.links.is_empty() {
                true => styled.into_any_element(),
                false => {
                    let ranges = text.links.iter().map(|link| link.range.clone()).collect();
                    let (workspace, key, pane) = (draw.workspace.clone(), draw.key, draw.pane);
                    InteractiveText::new(("links", index), styled)
                        .on_click(ranges, move |link, window, cx| {
                            cx.stop_propagation();
                            follow_link::<O>(&workspace, key, pane, index, link, window, cx);
                        })
                        .into_any_element()
                }
            };
            match O::book(draw.key) {
                Some(book) => {
                    let (workspace, pane) = (draw.workspace.clone(), draw.pane);
                    let pressed = move |press: Press,
                                        at: Point<Pixels>,
                                        window: &mut Window,
                                        cx: &mut App| {
                        let byte = layout
                            .index_for_position(at)
                            .unwrap_or_else(|nearest| nearest);
                        update(&workspace, window, cx, |this| {
                            let at = Pressed { block: index, byte };
                            state::book::press(&mut this.state, pane, book, Some(at), press)
                        });
                    };
                    let moved = pressed.clone();
                    div()
                        .id(("text", index))
                        .on_mouse_down(
                            MouseButton::Left,
                            move |event: &MouseDownEvent, window, cx| {
                                pressed(Press::Down, event.position, window, cx)
                            },
                        )
                        .on_mouse_move(move |event: &MouseMoveEvent, window, cx| {
                            match event.pressed_button {
                                Some(MouseButton::Left) => {
                                    moved(Press::Move, event.position, window, cx)
                                }
                                Some(_) | None => {}
                            }
                        })
                        .child(shown)
                        .into_any_element()
                }
                None => shown,
            }
        }
        true => {
            let depth = |svg: &Svg| Px::from(svg.height - svg.baseline).0;
            let text_depth = fonts.prose_depth / px(1.);
            let inline_ready = |figure: Figure| match (figure.kind, draw.figures.get(figure)) {
                (FigureKind::Math(Display::Inline), Some(Render::Ready(svg))) => Some(svg),
                (FigureKind::Math(_) | FigureKind::Mermaid, _) => None,
            };
            let deepest = (text.spans.iter())
                .filter_map(|span| match span.kind {
                    SpanKind::Math(figure) => inline_ready(figure).map(depth),
                    SpanKind::Text
                    | SpanKind::Code
                    | SpanKind::Html
                    | SpanKind::Footnote
                    | SpanKind::Image => None,
                })
                .fold(text_depth, f32::max);
            let piece = |range: std::ops::Range<usize>, span: &Span| {
                let styled =
                    StyledText::new(SharedString::from(text.text[range.clone()].to_owned()))
                        .with_runs(vec![TextRun {
                            len: range.len(),
                            ..run(theme, fonts, span, ink, base)
                        }]);
                let piece = div().flex_none().pb(px(deepest - text_depth));
                match text
                    .links
                    .iter()
                    .position(|link| link.range.contains(&range.start))
                {
                    None => piece.child(styled).into_any_element(),
                    Some(link) => {
                        let (workspace, key, pane) = (draw.workspace.clone(), draw.key, draw.pane);
                        piece
                            .id(("link", range.start))
                            .cursor_pointer()
                            .on_click(move |_: &ClickEvent, window, cx| {
                                cx.stop_propagation();
                                follow_link::<O>(&workspace, key, pane, index, link, window, cx);
                            })
                            .child(styled)
                            .into_any_element()
                    }
                }
            };
            let mut pieces: Vec<AnyElement> = Vec::with_capacity(text.spans.len() * 4);
            for span in text.spans.iter() {
                match span.kind {
                    SpanKind::Math(figure) => match (figure.kind, inline_ready(figure)) {
                        (_, Some(svg)) => pieces.push(
                            div()
                                .flex_none()
                                .pb(px(deepest - depth(svg)))
                                .child(
                                    picture_canvas(draw.pictures, &draw.workspace, svg.picture())
                                        .w(px(Px::from(svg.width).0))
                                        .h(px(Px::from(svg.height).0)),
                                )
                                .into_any_element(),
                        ),
                        (FigureKind::Math(Display::Block) | FigureKind::Mermaid, _) => pieces.push(
                            div()
                                .w_full()
                                .py_1()
                                .child(figure_element(draw, figure, &text.text[span.range.clone()]))
                                .into_any_element(),
                        ),
                        (FigureKind::Math(Display::Inline), None) => {
                            pieces.push(piece(span.range.clone(), span))
                        }
                    },
                    SpanKind::Text
                    | SpanKind::Code
                    | SpanKind::Html
                    | SpanKind::Footnote
                    | SpanKind::Image => {
                        let mut start = span.range.start;
                        for word in
                            text.text[span.range.clone()].split_inclusive(char::is_whitespace)
                        {
                            let end = start + word.len();
                            match word.strip_suffix('\n') {
                                Some(line) => {
                                    match line.is_empty() {
                                        true => {}
                                        false => {
                                            pieces.push(piece(start..start + line.len(), span))
                                        }
                                    }
                                    pieces.push(div().w_full().into_any_element());
                                }
                                None => pieces.push(piece(start..end, span)),
                            }
                            start = end;
                        }
                    }
                }
            }
            div()
                .flex()
                .flex_row()
                .flex_wrap()
                .items_end()
                .children(pieces)
                .into_any_element()
        }
    }
}

fn run(theme: &Theme, fonts: &Fonts, span: &Span, ink: Ink, base: Modifiers) -> TextRun {
    let style = span.style;
    let missing = style.contains(Emphasis::MISSING);
    let modifiers =
        base | match style.contains(Emphasis::STRONG) {
            true => Modifiers::BOLD,
            false => Modifiers::empty(),
        } | match style.contains(Emphasis::ITALIC) || missing {
            true => Modifiers::ITALIC,
            false => Modifiers::empty(),
        };
    let prose = &fonts.prose[(modifiers & (Modifiers::BOLD | Modifiers::ITALIC)).bits() as usize];
    let link = match missing {
        true => hsla(theme.prose.missing_link),
        false => hsla(theme.prose.link),
    };
    let (font, color, background) = match span.kind {
        SpanKind::Text | SpanKind::Image => (prose.clone(), hsla(theme.text), None),
        SpanKind::Code | SpanKind::Html => (
            fonts.mono_font(modifiers).clone(),
            hsla(theme.prose.code),
            Some(hsla(theme.surface)),
        ),
        SpanKind::Math(_) => (
            fonts.mono_font(modifiers).clone(),
            hsla(theme.prose.math),
            Some(hsla(theme.surface)),
        ),
        SpanKind::Footnote => (prose.clone(), link, None),
    };
    let color = match (style.contains(Emphasis::LINK), span.kind, ink) {
        (true, _, _) => link,
        (false, SpanKind::Image, _) => hsla(theme.muted),
        (false, _, Ink::Done) => hsla(theme.muted),
        (false, _, Ink::Plain) => color,
    };
    let struck = style.contains(Emphasis::STRIKE) || ink == Ink::Done;
    TextRun {
        len: span.range.len(),
        font,
        color,
        background_color: background,
        underline: match (
            style.contains(Emphasis::LINK),
            style.contains(Emphasis::UNDERLINE),
        ) {
            (true, _) => Some(UnderlineStyle {
                thickness: px(1.),
                color: Some(link),
                wavy: false,
            }),
            (false, true) => Some(UnderlineStyle {
                thickness: px(1.),
                color: Some(color),
                wavy: false,
            }),
            (false, false) => None,
        },
        strikethrough: match struck {
            true => Some(StrikethroughStyle {
                thickness: px(1.),
                color: Some(color),
            }),
            false => None,
        },
    }
}
