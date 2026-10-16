//! The text buffer a document or pages view is rendered from: following it and toggling to it.

use helix_core::Selection;
use model::config::OpenAs;
use model::notice::Notice;
use model::window::Axis;

use crate::State;
use crate::render::document::{self, DocumentScroll, DocumentView};
use crate::render::markdown::Format;
use crate::render::pages::{PagesScroll, PagesView};
use crate::text::buffer::{Buffer, BufferId, Content, TextDoc};
use crate::view::layout::{Pane, PaneId, TabKey, Tabs, TextView, View};
use crate::view::panes::{self, Place};

/// The text buffer a view is rendered from: toggled back to, and possibly followed.
#[derive(Clone)]
pub struct Source<S> {
    /// Selection and scroll restored when the tab toggles back to text.
    pub text: TextView,
    pub link: Link<S>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Link<S> {
    Standalone,
    /// Follows the text view of the same buffer in `pane`; `seen` is `None` until the first sync.
    Follows {
        pane: PaneId,
        seen: Option<S>,
    },
}

impl<S> Link<S> {
    /// Becomes standalone when the followed pane is `gone`.
    pub fn unlink_if(&mut self, gone: impl Fn(PaneId) -> bool) {
        match self {
            Link::Follows { pane, .. } if gone(*pane) => *self = Link::Standalone,
            Link::Follows { .. } | Link::Standalone => {}
        }
    }
}

/// `space v`: opens a rendered view of the focused text in a split to the right that
/// follows it, keeping focus on the text; on the text it follows, or on itself, closes it.
pub fn split(state: &mut State) {
    let focus = state.layout.focus();
    let Some(view) = state.layout.focused().active() else {
        state.notice = Some(Notice::NoBuffer);
        return;
    };
    let text = view.text().clone();
    let source = match view {
        View::Document(DocumentView {
            source:
                Source {
                    link: Link::Follows { pane, .. },
                    ..
                },
            ..
        })
        | View::Pages(PagesView {
            source:
                Source {
                    link: Link::Follows { pane, .. },
                    ..
                },
            ..
        }) => *pane,
        View::Document(_) | View::Pages(_) => return toggle(state),
        View::Text(_) | View::Diff(_) => focus,
    };
    let buffer = text.buffer();
    let follower = state.layout.panes().iter().find_map(|(pane, target)| {
        let follows = |item: &crate::view::layout::Item| match &item.view {
            View::Document(DocumentView {
                source:
                    Source {
                        link: Link::Follows { pane, .. },
                        ..
                    },
                ..
            })
            | View::Pages(PagesView {
                source:
                    Source {
                        link: Link::Follows { pane, .. },
                        ..
                    },
                ..
            }) => item.view.buffer() == buffer && *pane == source,
            View::Document(_) | View::Pages(_) | View::Text(_) | View::Diff(_) => false,
        };
        (target.items().iter().position(follows)).map(|index| (pane, index))
    });
    match follower {
        Some((pane, index)) => {
            panes::close_tab(state, pane, index, crate::apply::Force::No);
            (state.layout.set_focus(source)).expect("a link names a live pane");
        }
        None => {
            let Some((format, doc)) = renderable(state, buffer) else {
                return;
            };
            let rope = doc.text.slice(..);
            let line = rope.char_to_line(text.selection.primary().cursor(rope));
            let view = match format {
                Format::Markdown => View::Document(DocumentView::new(
                    Source {
                        text,
                        link: Link::Follows {
                            pane: focus,
                            seen: None,
                        },
                    },
                    DocumentScroll::Line(line),
                )),
                Format::Typst => View::Pages(PagesView::new(
                    Source {
                        text,
                        link: Link::Follows {
                            pane: focus,
                            seen: None,
                        },
                    },
                    PagesScroll::Line(line),
                )),
            };
            let now = state.layout.tick();
            let tabs = Tabs::new(view, OpenAs::Permanent, now);
            state.layout.split(Axis::Row, Pane { tabs: Some(tabs) });
            (state.layout.set_focus(focus)).expect("the split keeps the source pane");
        }
    }
}

/// The buffer's format and loaded text, or `None` with a notice when it is loading or has no
/// rendered view.
pub fn renderable(state: &mut State, buffer: BufferId) -> Option<(Format, &TextDoc)> {
    let State {
        buffers, notice, ..
    } = state;
    let Buffer { path, content } = &buffers[buffer];
    match (Format::of(path), content) {
        (Some(format), Content::Text { doc, .. }) => Some((format, doc)),
        (Some(_), Content::Loading { .. }) => {
            *notice = Some(Notice::StillLoading { path: path.clone() });
            None
        }
        (None, _) => {
            *notice = Some(Notice::NotRenderable { path: path.clone() });
            None
        }
    }
}

/// `space V`: shows the focused tab's file rendered, or a rendered view as text, in the same
/// pane and at the same place; an existing tab of the other kind is activated.
pub fn toggle(state: &mut State) {
    let focus = state.layout.focus();
    let Some(tabs) = state.layout.focused().tabs.as_ref() else {
        state.notice = Some(Notice::NoBuffer);
        return;
    };
    let view = tabs.items()[tabs.active()].view.clone();
    let open = match tabs.preview() == Some(tabs.active()) {
        true => OpenAs::Preview,
        false => OpenAs::Permanent,
    };
    let buffer = view.buffer();
    match view {
        View::Text(_) | View::Diff(_) => {
            let text = view.text().clone();
            let Some((format, doc)) = renderable(state, buffer) else {
                return;
            };
            let rope = doc.text.slice(..);
            let top = rope.char_to_line(text.scroll.anchor.min(rope.len_chars()));
            let cursor = rope.char_to_line(text.selection.primary().cursor(rope));
            // The cursor marks the place while it is in view; an undrawn view counts as showing it.
            let line = match (text.scroll.viewport.rows as usize, cursor >= top) {
                (0, _) => cursor,
                (rows, true) if cursor < top + rows => cursor,
                (_, _) => top,
            };
            // A rendered tab already in the pane is replaced, so it opens at the text's place.
            let (key, view) = match format {
                Format::Markdown => (
                    TabKey::Document(buffer),
                    View::Document(DocumentView::new(
                        Source {
                            text,
                            link: Link::Standalone,
                        },
                        DocumentScroll::Line(line),
                    )),
                ),
                Format::Typst => (
                    TabKey::Pages(buffer),
                    View::Pages(PagesView::new(
                        Source {
                            text,
                            link: Link::Standalone,
                        },
                        PagesScroll::Line(line),
                    )),
                ),
            };
            *panes::show(state, focus, key, open) = view;
        }
        View::Document(view) => {
            let top = (view.scroll.anchor()).and_then(|at| {
                let doc = state.buffers[buffer].doc()?;
                document::markdown(&state.documents, buffer, doc)?.item_line(at.item)
            });
            to_text(state, focus, view.source.text, top, open);
        }
        View::Pages(view) => to_text(state, focus, view.source.text, None, open),
    }
}

/// Shows `text` again in `pane` with `top` at the top, else where it was.
fn to_text(state: &mut State, pane: PaneId, text: TextView, top: Option<usize>, open: OpenAs) {
    let buffer = text.buffer();
    // A restored tab may still be loading; it shows without being placed.
    let place = state.buffers[buffer].doc().map(|doc| {
        let rope = doc.text.slice(..);
        let top =
            top.unwrap_or_else(|| rope.char_to_line(text.scroll.anchor.min(rope.len_chars())));
        let rows = text.scroll.viewport.rows as usize;
        let cursor = rope.char_to_line(text.selection.primary().cursor(rope));
        let anchor = rope.line_to_char(top);
        let selection = match (top..top + rows.max(1)).contains(&cursor) {
            true => text.selection,
            false => Selection::point(anchor),
        };
        Place {
            selection,
            anchor: Some(anchor),
        }
    });
    match place {
        Some(place) => {
            panes::show_at(state, pane, TabKey::Text(buffer), open, place);
        }
        None => {
            panes::show(state, pane, TabKey::Text(buffer), open);
        }
    }
}

/// Back to the pane a view follows, or to the text of a standalone view.
pub fn leave<S>(state: &mut State, link: &Link<S>) {
    match link {
        Link::Follows { pane, .. } => {
            (state.layout.set_focus(*pane)).expect("a link names a live pane")
        }
        Link::Standalone => toggle(state),
    }
}
