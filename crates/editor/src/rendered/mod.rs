//! Rendered markdown and typst views: GPUI lists of blocks or pages, with typeset SVGs
//! rasterised at their drawn device size.

use gpui::{
    AnyElement, App, Context, IntoElement, ListAlignment, ListOffset, ListState, Pixels,
    WeakEntity, Window, div, list, prelude::*, px,
};
use helix_core::SmallVec;
use model::typeset::{PagePoint, Svg};
use model::units::Px;
use state::State;
use state::render::document::{DocumentAt, DocumentScroll};
use state::render::pages::{Fit, PagesAt, PagesScroll, Zoom};
use state::render::typeset::Typst;
use state::text::buffer::BufferId;
use state::view::layout::{Pane, PaneId, View};

use crate::fonts::PROSE_LINE;
use crate::workspace::Workspace;

mod document;
mod pages;
pub mod raster;

use pages::{PAGE_GAP, page_width};

/// A document or pages pane's list and what its measured items describe.
pub struct RenderedPane {
    pub list: ListState,
    pub buffer: BufferId,
    /// Document version (markdown) or compiled version (typst) the list was spliced to.
    pub version: u64,
    /// Item hashes: block source hashes, or typst page sizes.
    pub hashes: Vec<u64>,
    /// What else the items were measured with.
    pub measured: Measured,
    /// How far the pages are drawn panned left, as last applied.
    pub left: Pixels,
    /// Scroll last exchanged between the list and the state.
    pub applied: Applied,
}

#[derive(Clone, PartialEq)]
pub struct Measured {
    /// `Figures::generation`, which sizes figures.
    pub figures: u64,
    pub shape: Shape,
    /// Viewport size the list was last laid out in; pages are sized from it.
    pub width: Pixels,
    pub height: Pixels,
}

/// The view state items are laid out with, per kind of view.
#[derive(Clone, PartialEq)]
pub enum Shape {
    /// `generation` is the file index's, which resolved the embeds.
    Document {
        toggled: Vec<u32>,
        generation: u64,
    },
    Pages {
        fit: Fit,
        zoom: Zoom,
    },
}

/// Where a list stands: `offset` into item `item`, with its contents panned `left`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ListAt {
    pub item: usize,
    pub offset: Pixels,
    pub left: Pixels,
}

/// What the state asks of a list.
#[derive(Clone, Copy)]
enum Want<'a> {
    /// A line not yet resolved into a place.
    Wait,
    At(ListAt),
    Moved {
        from: ListAt,
        lines: i32,
        half_pages: i32,
        columns: i32,
    },
    /// A point on `page`, drawn at `fit` and `zoom`.
    Point {
        at: PagePoint,
        page: &'a Svg,
        fit: Fit,
        zoom: Zoom,
    },
}

/// What the state holds for a list it asked to move: a place, which needs no report when the list
/// lands there, or a motion, which always does.
enum Asked {
    At(ListAt),
    Move,
}

/// What a list's items are, for how far it can pan.
enum Items<'a> {
    Blocks,
    Pages {
        pages: &'a [Svg],
        fit: Fit,
        zoom: Zoom,
    },
}

/// The scroll the list was last set to or reported, unless items moved under it since.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Applied {
    Stale,
    At(ListAt),
}

/// Tells the state where `pane`'s list stands, in its view's own terms.
fn report(state: &mut State, pane: PaneId, at: ListAt) {
    match state.layout.panes().get(pane).and_then(Pane::active) {
        Some(View::Document(_)) => state::render::document::scrolled(
            state,
            pane,
            DocumentAt {
                item: at.item,
                offset: Px(at.offset.into()),
            },
        ),
        Some(View::Pages(_)) => state::render::pages::scrolled(
            state,
            pane,
            PagesAt {
                page: at.item,
                offset: Px(at.offset.into()),
                left: Px(at.left.into()),
            },
        ),
        Some(View::Text(_) | View::Diff(_)) | None => {}
    }
}

/// Brings each document and pages pane's list in line with the state and carries out its scroll.
pub fn sync(workspace: &mut Workspace, cx: &mut Context<Workspace>) {
    let Workspace {
        state,
        rendered,
        fonts,
        window_height,
        ..
    } = workspace;
    rendered.retain(
        |pane, cache| match state.layout.panes().get(pane).and_then(Pane::active) {
            Some(view @ (View::Document(_) | View::Pages(_))) => view.buffer() == cache.buffer,
            Some(View::Text(_) | View::Diff(_)) | None => false,
        },
    );
    let shown: SmallVec<[PaneId; 4]> = (state.layout.panes().iter())
        .filter(|(_, target)| matches!(target.active(), Some(View::Document(_) | View::Pages(_))))
        .map(|(pane, _)| pane)
        .collect();
    let line = fonts.prose_size * PROSE_LINE;
    let list_at = |item: usize, offset: Px, left: Px| ListAt {
        item,
        offset: px(offset.0),
        left: px(left.0),
    };
    for pane in shown {
        let (buffer, version, hashes, shape, want, items) =
            match state.layout.panes()[pane].active() {
                Some(View::Document(view)) => {
                    let buffer = view.source.text.buffer();
                    let Some(parsed) = state.documents.get(buffer) else {
                        continue;
                    };
                    let want = match view.scroll {
                        DocumentScroll::Line(_) => Want::Wait,
                        DocumentScroll::At(at) => Want::At(list_at(at.item, at.offset, Px(0.0))),
                        DocumentScroll::Moved { from, by } => Want::Moved {
                            from: list_at(from.item, from.offset, Px(0.0)),
                            lines: by.lines,
                            half_pages: by.half_pages,
                            columns: 0,
                        },
                    };
                    let shape = Shape::Document {
                        toggled: view.toggled.clone(),
                        generation: parsed.generation,
                    };
                    (
                        buffer,
                        parsed.version,
                        &parsed.markdown.hashes,
                        shape,
                        want,
                        Items::Blocks,
                    )
                }
                Some(View::Pages(view)) => {
                    let buffer = view.source.text.buffer();
                    let Some(Typst {
                        pages: Some(compiled),
                        ..
                    }) = state.compiles.get(buffer)
                    else {
                        continue;
                    };
                    let want = match view.scroll {
                        PagesScroll::Line(_) => Want::Wait,
                        PagesScroll::Point(at) => match compiled.pages.get(at.page) {
                            Some(page) => Want::Point {
                                at,
                                page,
                                fit: view.fit,
                                zoom: view.zoom,
                            },
                            None => Want::Wait,
                        },
                        PagesScroll::At(at) => Want::At(list_at(at.page, at.offset, at.left)),
                        PagesScroll::Moved { from, by } => Want::Moved {
                            from: list_at(from.page, from.offset, from.left),
                            lines: by.lines,
                            half_pages: by.half_pages,
                            columns: by.columns,
                        },
                    };
                    let shape = Shape::Pages {
                        fit: view.fit,
                        zoom: view.zoom,
                    };
                    let items = Items::Pages {
                        pages: &compiled.pages,
                        fit: view.fit,
                        zoom: view.zoom,
                    };
                    (
                        buffer,
                        compiled.version,
                        &compiled.sizes,
                        shape,
                        want,
                        items,
                    )
                }
                Some(View::Text(_) | View::Diff(_)) | None => continue,
            };
        let cache = match rendered.get_mut(pane) {
            Some(cache) => cache,
            None => {
                let list = ListState::new(hashes.len(), ListAlignment::Top, *window_height);
                let workspace = cx.weak_entity();
                // The list scrolls itself on the wheel; the state hears of it once the list lets go.
                list.set_scroll_handler(move |_, window, cx| {
                    let workspace = workspace.clone();
                    window.defer(cx, move |_, cx| {
                        let _ = workspace.update(cx, |this, cx| {
                            let Some(cache) = this.rendered.get_mut(pane) else {
                                return;
                            };
                            let top = cache.list.logical_scroll_top();
                            let at = ListAt {
                                item: top.item_ix,
                                offset: top.offset_in_item,
                                left: cache.left,
                            };
                            cache.applied = Applied::At(at);
                            report(&mut this.state, pane, at);
                            this.flush(cx);
                        });
                    });
                });
                rendered.insert(
                    pane,
                    RenderedPane {
                        list,
                        buffer,
                        version,
                        hashes: hashes.clone(),
                        measured: Measured {
                            figures: state.figures.generation,
                            shape: shape.clone(),
                            width: px(0.),
                            height: px(0.),
                        },
                        left: px(0.),
                        applied: Applied::Stale,
                    },
                );
                &mut rendered[pane]
            }
        };
        match cache.version == version {
            true => {}
            false => {
                // Only the run between the unchanged ends is measured again.
                let old = &cache.hashes;
                let prefix = old.iter().zip(hashes).take_while(|(a, b)| a == b).count();
                let suffix = (old[prefix..].iter().rev())
                    .zip(hashes[prefix..].iter().rev())
                    .take_while(|(a, b)| a == b)
                    .count();
                cache
                    .list
                    .splice(prefix..old.len() - suffix, hashes.len() - prefix - suffix);
                cache.hashes.clone_from(hashes);
                cache.version = version;
                cache.applied = Applied::Stale;
            }
        }
        let measured = &mut cache.measured;
        match (measured.figures, &measured.shape) == (state.figures.generation, &shape) {
            true => {}
            false => {
                measured.figures = state.figures.generation;
                measured.shape = shape;
                cache.list.remeasure();
            }
        }
        let viewport = cache.list.viewport_bounds().size;
        let (from, down, across, asked) = match (want, cache.applied) {
            (Want::Wait, _) => continue,
            (Want::At(at), Applied::At(applied)) if at == applied => continue,
            (Want::At(at), Applied::At(_) | Applied::Stale) => (at, px(0.), px(0.), Asked::At(at)),
            (
                Want::Moved {
                    from,
                    lines,
                    half_pages,
                    columns,
                },
                _,
            ) => (
                from,
                line * lines as f32 + viewport.height * (half_pages as f32 / 2.),
                line * columns as f32,
                Asked::Move,
            ),
            (
                Want::Point {
                    at,
                    page,
                    fit,
                    zoom,
                },
                _,
            ) => {
                let scale = page_width(&cache.measured, fit, zoom, page) / px(page.width.0);
                let gap = match at.page {
                    0 => PAGE_GAP,
                    _ => px(0.),
                };
                // The point lands a third of the way down, centred across.
                (
                    list_at(at.page, Px(0.0), Px(0.0)),
                    gap + px(at.y.0 * scale) - viewport.height / 3.,
                    PAGE_GAP + px(at.x.0 * scale) - viewport.width / 2.,
                    Asked::Move,
                )
            }
        };
        cache.list.scroll_to(ListOffset {
            item_ix: from.item,
            offset_in_item: px(0.),
        });
        cache.list.scroll_by(from.offset + down);
        let widest = match items {
            Items::Blocks => px(0.),
            Items::Pages { pages, fit, zoom } => (pages.iter())
                .map(|page| page_width(&cache.measured, fit, zoom, page))
                .fold(px(0.), |widest, width| widest.max(width)),
        };
        let room = (widest + PAGE_GAP * 2. - viewport.width).max(px(0.));
        cache.left = (from.left + across).max(px(0.)).min(room);
        let top = cache.list.logical_scroll_top();
        let landed = ListAt {
            item: top.item_ix,
            offset: top.offset_in_item,
            left: cache.left,
        };
        cache.applied = Applied::At(landed);
        match asked {
            Asked::At(at) if at == landed => {}
            Asked::At(_) | Asked::Move => report(state, pane, landed),
        }
    }
}

/// The list drawing `pane`'s document or pages view, under a pages view's diagnostics strip.
pub fn body(workspace: &Workspace, pane: PaneId, cx: &mut Context<Workspace>) -> AnyElement {
    let Some(cache) = workspace.rendered.get(pane) else {
        return div().flex_1().into_any_element();
    };
    let header = match workspace.state.layout.pages(pane) {
        Some(view) => (workspace.state.compiles.get(view.source.text.buffer()))
            .and_then(|typst| pages::header(workspace, pane, typst, cx)),
        None => None,
    };
    div()
        .flex_1()
        .min_h_0()
        .flex()
        .flex_col()
        .children(header)
        .child(
            list(
                cache.list.clone(),
                cx.processor(move |this, index: usize, _window, cx| item(this, pane, index, cx)),
            )
            .flex_1()
            .min_h_0(),
        )
        .into_any_element()
}

fn item(
    this: &mut Workspace,
    pane: PaneId,
    index: usize,
    cx: &mut Context<Workspace>,
) -> AnyElement {
    match this.state.layout.panes().get(pane).and_then(Pane::active) {
        Some(View::Document(view)) => document::item(this, pane, view, index, cx),
        Some(View::Pages(view)) => pages::item(this, pane, view, index, cx),
        Some(View::Text(_) | View::Diff(_)) | None => div().into_any_element(),
    }
}

/// Runs the workspace with `f`, then redraws and keeps keys on the workspace.
fn update(
    workspace: &WeakEntity<Workspace>,
    window: &mut Window,
    cx: &mut App,
    f: impl FnOnce(&mut Workspace),
) {
    let _ = workspace.update(cx, |this, cx| {
        f(this);
        window.focus(&this.focus, cx);
        this.changed(cx);
    });
}
