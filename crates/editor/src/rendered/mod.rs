//! Rendered views: documents as GPUI lists of blocks, and pages fitted by the shared sheet view;
//! SVGs are rasterised at their drawn size.

use gpui::{
    AnyElement, App, Context, IntoElement, ListAlignment, ListOffset, ListState, Pixels,
    WeakEntity, Window, div, list, prelude::*, px,
};
use gpui::{MouseButton, MouseUpEvent};
use helix_core::SmallVec;
use model::scroll_bar::Thumb;
use model::units::Px;
use slotmap::SecondaryMap;
use state::book::{BookFile, BookId, BookTab};
use state::reading::Press;
use state::render::document::{DocumentScroll, DocumentState, Parsed, Placed, TopItem};
use state::render::flow::{Flow, Origin};
use state::render::markdown::LocalImage;
use state::render::typeset::{Figures, each_figure, each_image};
use state::text::buffer::BufferId;
use state::view::layout::PaneId;
use state::view::tab::{Showing, Tab};

use crate::fonts::PROSE_LINE;
use crate::workspace::Workspace;

pub mod document;

use document::Tints;
mod pages;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Flowing {
    Note(BufferId),
    Book(BookId),
}

pub struct RenderedPane {
    pub list: ListState,
    pub shows: Flowing,
    pub version: u64,
    pub hashes: Vec<u64>,
    pub measured: Measured,
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

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ListTop {
    pub item: usize,
    pub offset: Pixels,
}

/// What the state asks of a list.
#[derive(Clone, Copy)]
enum Want {
    /// A line not yet resolved into a place.
    Wait,
    At(ListTop),
    Moved {
        from: ListTop,
        lines: i32,
        half_pages: i32,
    },
}

/// A place needs no report when the list lands there; a motion always does.
enum Asked {
    At(ListTop),
    Move,
}

/// The scroll the list was last set to or reported, unless items moved under it since.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Applied {
    Stale,
    At(ListTop),
}

impl RenderedPane {
    pub fn thumb(&self) -> Option<Thumb> {
        let count = self.list.item_count() as f32;
        let top = self.list.logical_scroll_top();
        let bottom = self.list.viewport_bounds().bottom();
        let first = self.list.bounds_for_item(top.item_ix)?;
        let past = (top.offset_in_item / first.size.height).clamp(0.0, 1.0);
        let shown: f32 = (top.item_ix..self.list.item_count())
            .map_while(|item| self.list.bounds_for_item(item))
            .take_while(|item| item.top() < bottom)
            .map(|item| ((bottom - item.top()) / item.size.height).min(1.0))
            .sum();
        let top = (top.item_ix as f32 + past) / count;
        Some(Thumb {
            top,
            length: ((shown - past) / count).clamp(0.0, 1.0 - top),
        })
    }

    pub fn at_fraction(&self, top: f32) -> Option<ListTop> {
        let last = self.list.item_count().checked_sub(1)?;
        let along = top.clamp(0.0, 1.0) * self.list.item_count() as f32;
        let item = (along.floor() as usize).min(last);
        let height = (self.list.bounds_for_item(item)).map_or(px(0.), |bounds| bounds.size.height);
        Some(ListTop {
            item,
            offset: height * along.fract(),
        })
    }
}

enum Shown<'a> {
    Note {
        buffer: BufferId,
        view: &'a DocumentState,
        parsed: Option<&'a Parsed>,
    },
    Book {
        book: BookId,
        tab: &'a BookTab,
        file: &'a BookFile,
    },
}

struct Laid<'a, O: Origin> {
    shows: Flowing,
    flow: &'a Flow<O>,
    version: u64,
    toggled: &'a [u32],
    generation: u64,
    want: Want,
}

struct Lists<'a> {
    rendered: &'a mut SecondaryMap<PaneId, RenderedPane>,
    figures: &'a Figures,
    window_height: Pixels,
    line: Pixels,
}

impl Shown<'_> {
    fn of(state: &state::State, pane: PaneId) -> Option<Shown<'_>> {
        match state.layout.panes().get(pane)?.active()? {
            Tab::JsonCanvas(_) => None,
            Tab::Text(text) => match text.showing() {
                Showing::Document(text, view, _) => Some(Shown::Note {
                    buffer: text.buffer(),
                    view,
                    parsed: state.documents.get(text.buffer()),
                }),
                Showing::Source(_) | Showing::Diff(..) | Showing::Pages(..) => None,
            },
            Tab::Book(tab) => Some(Shown::Book {
                book: tab.file(),
                tab,
                file: &state.books[tab.file()],
            }),
            Tab::Ink(_) | Tab::Image(_) => None,
        }
    }

    fn flowing(&self) -> Flowing {
        match self {
            Shown::Note { buffer, .. } => Flowing::Note(*buffer),
            Shown::Book { book, .. } => Flowing::Book(*book),
        }
    }
}

pub fn report(state: &mut state::State, pane: PaneId, shows: Flowing, at: ListTop) {
    let to = TopItem {
        item: at.item,
        offset: Px(at.offset.into()),
    };
    match shows {
        Flowing::Note(_) => state::render::document::scrolled(state, pane, to),
        Flowing::Book(book) => state::book::scrolled(state, pane, book, to),
    }
}

fn want(placed: Placed) -> Want {
    let list_top = |at: TopItem| ListTop {
        item: at.item,
        offset: px(at.offset.0),
    };
    match placed {
        Placed::At(at) => Want::At(list_top(at)),
        Placed::Moved { from, by } => Want::Moved {
            from: list_top(from),
            lines: by.lines,
            half_pages: by.half_pages,
        },
    }
}

impl Lists<'_> {
    fn follow<O: Origin>(
        self,
        pane: PaneId,
        laid: Laid<O>,
        cx: &mut Context<Workspace>,
    ) -> Option<ListTop> {
        let Lists {
            rendered,
            figures,
            window_height,
            line,
        } = self;
        let Laid {
            shows,
            flow,
            version,
            toggled,
            generation,
            want,
        } = laid;
        let hashes = &flow.hashes;
        let cache = match rendered.get_mut(pane) {
            Some(cache) => cache,
            None => {
                let list = ListState::new(hashes.len(), ListAlignment::Top, window_height);
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
                            let at = ListTop {
                                item: top.item_ix,
                                offset: top.offset_in_item,
                            };
                            cache.applied = Applied::At(at);
                            report(&mut this.state, pane, cache.shows, at);
                            this.flush(cx);
                        });
                    });
                });
                rendered.insert(
                    pane,
                    RenderedPane {
                        list,
                        shows,
                        version,
                        hashes: hashes.clone(),
                        measured: Measured {
                            figures: figures.generation,
                            toggled: toggled.to_vec(),
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
        let since = std::mem::replace(&mut measured.figures, figures.generation);
        let arrived = (since != figures.generation).then_some(since);
        let shaped = measured.toggled == toggled && measured.generation == generation;
        match (shaped, arrived) {
            (false, _) => {
                measured.toggled = toggled.to_vec();
                measured.generation = generation;
                cache.list.remeasure();
            }
            // Only the items holding a figure or image filled since the last measure change height.
            (true, Some(since)) => {
                let Figures {
                    entries, images, ..
                } = figures;
                let filled = |figure| entries.get(&figure).map_or(0, |e| e.filled);
                let sized = |image: &LocalImage| images.get(image).map_or(0, |e| e.filled);
                for (item, &root) in flow.roots.iter().enumerate() {
                    let mut newest = 0;
                    let blocks = &flow.blocks[root..flow.blocks[root].end];
                    each_figure(blocks, |figure, _| newest = newest.max(filled(figure)));
                    each_image(blocks, |image| newest = newest.max(sized(image)));
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
            (Want::Wait, _) => return None,
            (Want::At(at), Applied::At(applied)) if at == applied => return None,
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
        let landed = ListTop {
            item: top.item_ix,
            offset: top.offset_in_item,
        };
        cache.applied = Applied::At(landed);
        match asked {
            Asked::At(at) if at == landed => None,
            Asked::At(_) | Asked::Move => Some(landed),
        }
    }
}

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
        Shown::of(state, pane).map(|shown| shown.flowing()) == Some(cache.shows)
    });
    let panes: SmallVec<[PaneId; 4]> = (state.layout.panes().keys())
        .filter(|pane| Shown::of(state, *pane).is_some())
        .collect();
    let line = fonts.prose_size * PROSE_LINE;
    for pane in panes {
        let (shows, landed) = match Shown::of(state, pane) {
            Some(Shown::Note {
                buffer,
                view,
                parsed: Some(parsed),
            }) => {
                let want = match view.scroll {
                    DocumentScroll::Line(_) => Want::Wait,
                    DocumentScroll::Placed(placed) => want(placed),
                };
                let laid = Laid {
                    shows: Flowing::Note(buffer),
                    flow: &parsed.markdown.flow,
                    version: parsed.version,
                    toggled: &view.toggled,
                    generation: parsed.generation,
                    want,
                };
                let lists = Lists {
                    rendered: &mut *rendered,
                    figures: &state.figures,
                    window_height: *window_height,
                    line,
                };
                (laid.shows, lists.follow(pane, laid, cx))
            }
            Some(Shown::Book { book, tab, file }) => {
                let laid = Laid {
                    shows: Flowing::Book(book),
                    flow: &file.flow.flow,
                    version: file.version,
                    toggled: &[],
                    generation: 0,
                    want: want(tab.scroll),
                };
                let lists = Lists {
                    rendered: &mut *rendered,
                    figures: &state.figures,
                    window_height: *window_height,
                    line,
                };
                (laid.shows, lists.follow(pane, laid, cx))
            }
            Some(Shown::Note { parsed: None, .. }) | None => continue,
        };
        match landed {
            Some(at) => report(state, pane, shows, at),
            None => {}
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
                .child(div().flex_1().min_h_0().child(crate::sheet::SheetElement {
                    workspace: cx.entity(),
                    pane,
                }))
                .into_any_element()
        }
        None => match workspace.rendered.get(pane) {
            Some(cache) => {
                let shown = list(
                    cache.list.clone(),
                    cx.processor(move |this, index: usize, _window, cx| {
                        item(this, pane, index, cx)
                    }),
                )
                .flex_1()
                .min_h_0();
                match cache.shows {
                    Flowing::Book(book) => div()
                        .flex_1()
                        .min_h_0()
                        .flex()
                        .flex_col()
                        .on_mouse_up(
                            MouseButton::Left,
                            cx.listener(move |this, _: &MouseUpEvent, _, cx| {
                                state::book::press(&mut this.state, pane, book, None, Press::Up);
                                this.changed(cx);
                            }),
                        )
                        .child(shown)
                        .into_any_element(),
                    Flowing::Note(_) => shown.into_any_element(),
                }
            }
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
    match Shown::of(&this.state, pane) {
        Some(Shown::Note {
            buffer,
            view,
            parsed: Some(parsed),
        }) => {
            let from = document::From {
                margins: document::Margins::Page,
                fonts: &this.fonts,
                flow: &parsed.markdown.flow,
                key: document::NoteKey::Buffer(buffer),
                version: parsed.version,
                toggled: &view.toggled,
                marked: &|_| Tints::new(),
            };
            document::item(this, pane, from, index, cx)
        }
        Some(Shown::Book { book, tab, file }) => {
            let marks = crate::reading::marks(
                &this.theme.reader,
                &file.notes,
                &tab.found,
                &tab.reading,
                &*file.flow.texts,
            );
            let marked = |block: usize| -> Tints {
                (marks.iter())
                    .filter_map(|(span, color)| Some((file.flow.within(block, span)?, *color)))
                    .collect()
            };
            let from = document::From {
                margins: document::Margins::Page,
                fonts: &this.fonts,
                flow: &file.flow.flow,
                key: book,
                version: file.version,
                toggled: &[],
                marked: &marked,
            };
            document::item(this, pane, from, index, cx)
        }
        // The list closes with the document view, so this shows only for a frame.
        Some(Shown::Note { parsed: None, .. }) | None => div().into_any_element(),
    }
}

/// Runs the workspace with `f`, then redraws and keeps keys on the workspace.
pub fn update(
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
