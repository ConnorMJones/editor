//! Rendered views: documents as GPUI lists of blocks, and pages fitted by the shared sheet view;
//! SVGs are rasterised at their drawn size.

use gpui::{
    AnyElement, App, Context, IntoElement, ListAlignment, ListOffset, ListState, Pixels,
    WeakEntity, Window, div, list, prelude::*, px,
};
use helix_core::SmallVec;
use model::units::Px;
use state::render::document::{DocumentAt, DocumentScroll};
use state::render::typeset::each_figure;
use state::text::buffer::BufferId;
use state::view::layout::{Pane, PaneId};
use state::view::tab::{Showing, TextTabs};

use crate::fonts::PROSE_LINE;
use crate::workspace::Workspace;

mod document;
mod pages;
pub mod raster;

/// A document pane's list and what its measured items describe.
pub struct RenderedPane {
    pub list: ListState,
    pub buffer: BufferId,
    /// Document version the list was spliced to.
    pub version: u64,
    /// Block source hashes.
    pub hashes: Vec<u64>,
    /// What else the items were measured with.
    pub measured: Measured,
    /// Scroll last exchanged between the list and the state.
    pub applied: Applied,
}

#[derive(Clone, PartialEq)]
pub struct Measured {
    /// `Figures::generation`, which sizes figures.
    pub figures: u64,
    /// Folded callouts.
    pub toggled: Vec<u32>,
    /// The file index's generation, which resolved the embeds.
    pub generation: u64,
}

/// Where a list stands: `offset` into item `item`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ListAt {
    pub item: usize,
    pub offset: Pixels,
}

/// What the state asks of a list.
#[derive(Clone, Copy)]
enum Want {
    /// A line not yet resolved into a place.
    Wait,
    At(ListAt),
    Moved {
        from: ListAt,
        lines: i32,
        half_pages: i32,
    },
}

/// A place needs no report when the list lands there; a motion always does.
enum Asked {
    At(ListAt),
    Move,
}

/// The scroll the list was last set to or reported, unless items moved under it since.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Applied {
    Stale,
    At(ListAt),
}

/// Tells the state where `pane`'s list stands.
fn report(state: &mut state::State, pane: PaneId, at: ListAt) {
    state::render::document::scrolled(
        state,
        pane,
        DocumentAt {
            item: at.item,
            offset: Px(at.offset.into()),
        },
    )
}

/// Brings each document pane's list in line with the state and carries out its scroll.
pub fn sync(workspace: &mut Workspace, cx: &mut Context<Workspace>) {
    let _span = tracing::trace_span!("rendered sync").entered();
    let Workspace {
        state,
        rendered,
        fonts,
        window_height,
        ..
    } = workspace;
    rendered.retain(|pane, cache| {
        match (state.layout.panes().get(pane))
            .and_then(Pane::active_text)
            .map(TextTabs::showing)
        {
            Some(Showing::Document(text, ..)) => text.buffer() == cache.buffer,
            Some(Showing::Source(_) | Showing::Diff(..) | Showing::Pages(..)) | None => false,
        }
    });
    let shown: SmallVec<[PaneId; 4]> = (state.layout.panes().iter())
        .filter(|(_, target)| {
            matches!(
                target.active_text().map(TextTabs::showing),
                Some(Showing::Document(..))
            )
        })
        .map(|(pane, _)| pane)
        .collect();
    let line = fonts.prose_size * PROSE_LINE;
    let list_at = |item: usize, offset: Px| ListAt {
        item,
        offset: px(offset.0),
    };
    for pane in shown {
        let Some(Showing::Document(text, view, _)) = state.layout.panes()[pane]
            .active_text()
            .map(TextTabs::showing)
        else {
            continue;
        };
        let buffer = text.buffer();
        let Some(parsed) = state.documents.get(buffer) else {
            continue;
        };
        let want = match view.scroll {
            DocumentScroll::Line(_) => Want::Wait,
            DocumentScroll::At(at) => Want::At(list_at(at.item, at.offset)),
            DocumentScroll::Moved { from, by } => Want::Moved {
                from: list_at(from.item, from.offset),
                lines: by.lines,
                half_pages: by.half_pages,
            },
        };
        let (version, hashes, markdown) =
            (parsed.version, &parsed.markdown.hashes, &parsed.markdown);
        let toggled = view.toggled.clone();
        let generation = parsed.generation;
        let cache = match rendered.get_mut(pane) {
            Some(cache) => cache,
            None => {
                let list = ListState::new(hashes.len(), ListAlignment::Top, *window_height);
                let workspace = cx.weak_entity();
                // The list scrolls itself on the wheel; the state hears once the list lets go.
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
                            toggled: toggled.clone(),
                            generation,
                        },
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
        let since = std::mem::replace(&mut measured.figures, state.figures.generation);
        let arrived = (since != state.figures.generation).then_some(since);
        let shaped = measured.toggled == toggled && measured.generation == generation;
        match (shaped, arrived) {
            (false, _) => {
                measured.toggled = toggled;
                measured.generation = generation;
                cache.list.remeasure();
            }
            // Only the items holding a figure filled since the last measure change height.
            (true, Some(since)) => {
                let filled = |figure| (state.figures.entries.get(&figure)).map_or(0, |e| e.filled);
                for (item, &root) in markdown.roots.iter().enumerate() {
                    let mut newest = 0;
                    let blocks = &markdown.blocks[root..markdown.blocks[root].end];
                    each_figure(blocks, |figure, _| newest = newest.max(filled(figure)));
                    match newest > since {
                        true => cache.list.remeasure_items(item..item + 1),
                        false => {}
                    }
                }
            }
            (true, None) => {}
        }
        let viewport = cache.list.viewport_bounds().size;
        let (from, down, asked) = match (want, cache.applied) {
            (Want::Wait, _) => continue,
            (Want::At(at), Applied::At(applied)) if at == applied => continue,
            (Want::At(at), Applied::At(_) | Applied::Stale) => (at, px(0.), Asked::At(at)),
            (
                Want::Moved {
                    from,
                    lines,
                    half_pages,
                },
                _,
            ) => (
                from,
                line * lines as f32 + viewport.height * (half_pages as f32 / 2.),
                Asked::Move,
            ),
        };
        cache.list.scroll_to(ListOffset {
            item_ix: from.item,
            offset_in_item: px(0.),
        });
        cache.list.scroll_by(from.offset + down);
        let top = cache.list.logical_scroll_top();
        let landed = ListAt {
            item: top.item_ix,
            offset: top.offset_in_item,
        };
        cache.applied = Applied::At(landed);
        match asked {
            Asked::At(at) if at == landed => {}
            Asked::At(_) | Asked::Move => report(state, pane, landed),
        }
    }
}

/// The body of `pane`'s document or pages view: a list of blocks, or the pages under their
/// diagnostics strip.
pub fn body(workspace: &Workspace, pane: PaneId, cx: &mut Context<Workspace>) -> AnyElement {
    match workspace.state.layout.pages(pane) {
        Some((text, _)) => {
            let header = (workspace.state.compiles.get(text.buffer()))
                .and_then(|typst| pages::header(workspace, pane, typst, cx));
            div()
                .flex_1()
                .min_h_0()
                .flex()
                .flex_col()
                .children(header)
                .child(div().flex_1().min_h_0().child(pages::PagesElement {
                    workspace: cx.entity(),
                    pane,
                }))
                .into_any_element()
        }
        None => match workspace.rendered.get(pane) {
            Some(cache) => list(
                cache.list.clone(),
                cx.processor(move |this, index: usize, _window, cx| item(this, pane, index, cx)),
            )
            .flex_1()
            .min_h_0()
            .into_any_element(),
            None => div().flex_1().into_any_element(),
        },
    }
}

fn item(
    this: &mut Workspace,
    pane: PaneId,
    index: usize,
    cx: &mut Context<Workspace>,
) -> AnyElement {
    match (this.state.layout.panes().get(pane))
        .and_then(Pane::active_text)
        .map(TextTabs::showing)
    {
        Some(Showing::Document(text, view, _)) => {
            document::item(this, pane, text.buffer(), view, index, cx)
        }
        // The list closes with the document view, so these show only for a frame.
        Some(Showing::Pages(..) | Showing::Source(_) | Showing::Diff(..)) | None => {
            div().into_any_element()
        }
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
