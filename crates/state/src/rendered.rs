//! Rendered views of markdown and typst buffers: their previews, scroll, the source pane they
//! follow, and their commands.

use helix_core::{Selection, SmallVec, Transaction};
use model::command::{Direction, RenderedCommand, ScrollBy};
use model::config::OpenAs;
use model::notice::Notice;
use model::typeset::{Origin, SourceFile};
use model::window::{Axis, Px};
use slotmap::SecondaryMap;

use crate::apply::map_views;
use crate::buffer::{self, Buffer, BufferId, Position, Target, TextDoc};
use crate::changes::Arrive;
use crate::layout::{Pane, PaneId, TabKey, Tabs, TextView, View};
use crate::markdown::{self, Base, BlockKind, Callout, CalloutFold, Format, Markdown};
use crate::path::RelPath;
use crate::typeset::{self, Outcome, Typst};
use crate::vault;
use crate::{Effect, State, scroll, workspace};

pub type Previews = SecondaryMap<BufferId, Preview>;

/// What the rendered tabs of one buffer show.
pub enum Preview {
    Markdown(Parsed),
    Typst(Typst),
}

pub struct Parsed {
    /// `TextDoc::version` the blocks describe.
    pub version: u64,
    /// `vault::Index::generation` the links were resolved against.
    pub generation: u64,
    pub markdown: Markdown,
}

#[derive(Clone)]
pub struct RenderedView {
    /// Source selection and scroll, restored when the tab toggles back to text.
    pub text: TextView,
    pub scroll: Scroll,
    pub link: Link,
    /// Callouts, by `Callout::index`, whose fold the reader flipped from the default.
    pub toggled: Vec<u32>,
    pub fit: Fit,
}

/// How a typst preview scales its pages.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fit {
    /// Pages span the pane width.
    Width,
    /// Each page fits the pane whole.
    Page,
}

/// Where a rendered view stands, or is asked to move for the host to measure.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Scroll {
    /// Source line to bring to the top once the buffer is parsed.
    Line(usize),
    At(BlockScroll),
    /// Keys moved the view from `from` by `by`.
    Moved {
        from: BlockScroll,
        by: Motion,
    },
}

/// The top of a rendered view: `offset` below the top of item `item`, a markdown root or a
/// typst page.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct BlockScroll {
    pub item: usize,
    pub offset: Px,
}

/// A distance in the host's measured lines and viewport heights.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Motion {
    pub lines: i32,
    pub half_pages: i32,
}

impl Scroll {
    pub fn item(item: usize) -> Scroll {
        Scroll::At(BlockScroll {
            item,
            offset: Px(0.0),
        })
    }

    /// The place moves start from, once the line is resolved.
    pub fn anchor(self) -> Option<BlockScroll> {
        match self {
            Scroll::Line(_) => None,
            Scroll::At(at) | Scroll::Moved { from: at, .. } => Some(at),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Link {
    Standalone,
    /// Follows the text view of the same buffer in `pane`; `seen` is `None` until the first sync.
    Follows {
        pane: PaneId,
        seen: Option<Seen>,
    },
}

/// The source position both sides last agreed on; a change on one side moves the other.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Seen {
    pub cursor: usize,
    pub top: usize,
    pub item: usize,
}

impl RenderedView {
    pub(crate) fn new(buffer: BufferId, link: Link, scroll: Scroll) -> RenderedView {
        RenderedView {
            text: TextView::new(buffer),
            scroll,
            link,
            toggled: Vec::new(),
            fit: Fit::Width,
        }
    }

    /// The callout's fold as shown, after the reader's toggles.
    pub fn fold(&self, callout: &Callout) -> CalloutFold {
        match (
            callout.fold,
            self.toggled.binary_search(&callout.index).is_ok(),
        ) {
            (CalloutFold::Static, _) => CalloutFold::Static,
            (fold, false) => fold,
            (CalloutFold::Open, true) => CalloutFold::Closed,
            (CalloutFold::Closed, true) => CalloutFold::Open,
        }
    }
}

/// The parse of `buffer`, if current with `doc`.
pub fn markdown<'a>(
    previews: &'a Previews,
    buffer: BufferId,
    doc: &TextDoc,
) -> Option<&'a Markdown> {
    match previews.get(buffer) {
        Some(Preview::Markdown(parsed)) if parsed.version == doc.version => Some(&parsed.markdown),
        Some(Preview::Markdown(_) | Preview::Typst(_)) | None => None,
    }
}

/// Whether the set of parses figures are drawn from changed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Parses {
    Kept,
    Changed,
}

/// A rendered pane as `sync` found it, with the source lines its items stood for before a
/// reparse.
pub struct Shown {
    pub pane: PaneId,
    pub buffer: BufferId,
    pub line: Option<usize>,
    pub seen: Option<usize>,
}

/// Keeps previews for open rendered tabs, parses the shown ones and moves their scroll and links.
pub fn sync(state: &mut State, out: &mut Vec<Effect>) {
    let State {
        layout,
        buffers,
        languages,
        root,
        vault,
        files,
        figures,
        previews,
        ..
    } = state;
    let before = previews.len();
    previews.retain(|buffer, _| {
        (layout.panes.values().flat_map(Pane::items)).any(|item| match &item.view {
            View::Rendered(view) => view.text.buffer() == buffer,
            View::Text(_) | View::Diff(_) => false,
        })
    });
    let mut parses = match previews.len() == before {
        true => Parses::Kept,
        false => Parses::Changed,
    };
    let mut shown: SmallVec<[Shown; 4]> = SmallVec::new();
    for (pane, target) in layout.panes.iter() {
        let Some(View::Rendered(view)) = target.active() else {
            continue;
        };
        let buffer = view.text.buffer();
        // The last parse, current or not, maps items back to the lines they stood for.
        let parsed = match previews.get(buffer) {
            Some(Preview::Markdown(parsed)) => Some(&parsed.markdown),
            Some(Preview::Typst(_)) | None => None,
        };
        let line_of = |item: usize| parsed.and_then(|markdown| markdown.item_line(item));
        shown.push(Shown {
            pane,
            buffer,
            line: view.scroll.anchor().and_then(|at| line_of(at.item)),
            seen: match view.link {
                Link::Follows {
                    seen: Some(seen), ..
                } => line_of(seen.item),
                Link::Follows { seen: None, .. } | Link::Standalone => None,
            },
        });
    }
    let mut reparsed: SmallVec<[BufferId; 4]> = SmallVec::new();
    let mut compiled: SmallVec<[BufferId; 4]> = SmallVec::new();
    for entry in &shown {
        // Built on first use only, so nothing is indexed while no rendered view shows.
        let index = vault::index(vault, files);
        let Some(Buffer {
            path,
            content: crate::buffer::Content::Text { doc, .. },
        }) = buffers.get(entry.buffer)
        else {
            continue;
        };
        let buffer = entry.buffer;
        match (Format::of(path), previews.get(buffer)) {
            (Some(Format::Markdown), Some(Preview::Markdown(parsed)))
                if parsed.version == doc.version && parsed.generation == index.generation => {}
            (Some(Format::Typst), Some(Preview::Typst(_))) => compiled.push(buffer),
            (None, None) => {}
            (Some(Format::Typst), Some(Preview::Markdown(_)) | None) => {
                previews.insert(buffer, Preview::Typst(Typst::new()));
                compiled.push(buffer);
                parses = Parses::Changed;
            }
            (None, Some(_)) => {
                previews.remove(buffer);
                parses = Parses::Changed;
            }
            (Some(Format::Markdown), Some(_) | None) => {
                let reuse = match previews.remove(buffer) {
                    Some(Preview::Markdown(parsed)) => Some(parsed.markdown),
                    Some(Preview::Typst(_)) | None => None,
                };
                let root = root.path();
                let note = RelPath::new(root, path).ok();
                let base = Base {
                    dir: path.parent().unwrap_or(root),
                    root,
                    folder: note.as_ref().and_then(RelPath::parent).unwrap_or(""),
                    vault: index,
                };
                let mut markdown = markdown::parse(doc.text.slice(..), base, reuse);
                markdown::highlight(&mut markdown, languages);
                previews.insert(
                    buffer,
                    Preview::Markdown(Parsed {
                        version: doc.version,
                        generation: index.generation,
                        markdown,
                    }),
                );
                parses = Parses::Changed;
                reparsed.push(buffer);
            }
        }
    }
    match parses {
        Parses::Kept => {}
        Parses::Changed => typeset::mark(figures, previews, out),
    }
    typeset::request_pages(previews, buffers, root.path(), &compiled, out);
    for entry in &shown {
        let doc = buffer::doc(buffers, entry.buffer);
        let Some(view) = layout.rendered(entry.pane) else {
            continue;
        };
        let (link, source) = match view.link {
            Link::Follows { pane, .. } => match layout.panes.get(pane).map(Pane::active) {
                None => (Link::Standalone, None),
                Some(Some(View::Text(text))) if text.buffer() == entry.buffer => {
                    let source = doc.map(|doc| {
                        let rope = doc.text.slice(..);
                        let cursor = rope.char_to_line(text.selection.primary().cursor(rope));
                        let top = rope.char_to_line(text.scroll.anchor.min(rope.len_chars()));
                        (cursor, top)
                    });
                    (view.link, source)
                }
                Some(Some(View::Text(_) | View::Diff(_) | View::Rendered(_)) | None) => {
                    (view.link, None)
                }
            },
            Link::Standalone => (view.link, None),
        };
        let Some(view) = layout.rendered_mut(entry.pane) else {
            continue;
        };
        view.link = link;
        let markdown = match (doc, previews.get(entry.buffer)) {
            (Some(doc), Some(Preview::Markdown(parsed))) if parsed.version == doc.version => {
                &parsed.markdown
            }
            (_, Some(Preview::Typst(_))) => {
                // Until typst maps source lines to pages, a wanted line shows the first page.
                match view.scroll {
                    Scroll::Line(_) => view.scroll = Scroll::item(0),
                    Scroll::At(_) | Scroll::Moved { .. } => {}
                }
                continue;
            }
            (_, Some(Preview::Markdown(_)) | None) => continue,
        };
        let last = markdown.roots.len().saturating_sub(1);
        let remap = reparsed.contains(&entry.buffer);
        match (&mut view.scroll, remap, entry.line) {
            (Scroll::Line(line), _, _) => {
                view.scroll = Scroll::item(markdown.item_at_line(*line).min(last))
            }
            (Scroll::At(at) | Scroll::Moved { from: at, .. }, true, Some(line)) => {
                at.item = markdown.item_at_line(line).min(last)
            }
            (Scroll::At(at) | Scroll::Moved { from: at, .. }, _, _) => at.item = at.item.min(last),
        }
        match (remap, &mut view.link, entry.seen) {
            (
                true,
                Link::Follows {
                    seen: Some(seen), ..
                },
                Some(line),
            ) => seen.item = markdown.item_at_line(line),
            (true | false, Link::Follows { .. } | Link::Standalone, _) => {}
        }
        let (Link::Follows { seen, .. }, Some((cursor, top)), Some(at)) =
            (&mut view.link, source, view.scroll.anchor())
        else {
            continue;
        };
        let target = match *seen {
            None => None,
            Some(seen) if seen.cursor != cursor => Some(cursor),
            Some(seen) if seen.top != top => Some(top),
            Some(_) => None,
        };
        let item = match target.map(|line| markdown.item_at_line(line)) {
            Some(item) if item != at.item => {
                view.scroll = Scroll::item(item);
                item
            }
            Some(_) | None => at.item,
        };
        *seen = Some(Seen { cursor, top, item });
    }
}

/// The host measured `pane`'s rendered view at `to`; a move to another item scrolls the
/// source it follows, leaving the source cursor where it is.
pub fn scrolled(state: &mut State, pane: PaneId, to: BlockScroll) {
    let State {
        layout,
        buffers,
        previews,
        ..
    } = state;
    let Some(view) = layout.rendered_mut(pane) else {
        return;
    };
    view.scroll = Scroll::At(to);
    let buffer = view.text.buffer();
    let (source, seen) = match view.link {
        Link::Follows {
            pane: source,
            seen: Some(seen),
        } if seen.item != to.item => (source, seen),
        Link::Follows { .. } | Link::Standalone => return,
    };
    let Some(doc) = buffer::doc(buffers, buffer) else {
        return;
    };
    let Some(line) =
        markdown(previews, buffer, doc).and_then(|markdown| markdown.item_line(to.item))
    else {
        return;
    };
    match layout.panes.get_mut(source).and_then(Pane::active_mut) {
        Some(View::Text(text)) if text.buffer() == buffer => {
            text.scroll.anchor = doc.text.line_to_char(line.min(doc.text.len_lines() - 1));
            text.scroll.rows = 0;
        }
        Some(View::Text(_) | View::Diff(_) | View::Rendered(_)) | None => {}
    }
    match layout.rendered_mut(pane) {
        Some(view) => {
            view.link = Link::Follows {
                pane: source,
                seen: Some(Seen {
                    top: line,
                    item: to.item,
                    ..seen
                }),
            }
        }
        None => {}
    }
}

/// `space v`: opens a rendered view of the focused text in a split to the right that
/// follows it, keeping focus on the text; on the text it follows, or on itself, closes it.
pub fn split(state: &mut State) {
    let focus = state.layout.focus;
    let Some(view) = state.layout.panes.get(focus).and_then(Pane::active) else {
        state.notice = Some(Notice::NoBuffer);
        return;
    };
    let text = view.text().clone();
    let source = match view {
        View::Rendered(rendered) => match rendered.link {
            Link::Follows { pane, .. } => pane,
            Link::Standalone => return toggle(state),
        },
        View::Text(_) | View::Diff(_) => focus,
    };
    let buffer = text.buffer();
    let follower = state.layout.panes.iter().find_map(|(pane, target)| {
        target
            .items()
            .iter()
            .position(|item| match &item.view {
                View::Rendered(rendered) => {
                    rendered.text.buffer() == buffer
                        && matches!(rendered.link, Link::Follows { pane, .. } if pane == source)
                }
                View::Text(_) | View::Diff(_) => false,
            })
            .map(|index| (pane, index))
    });
    match follower {
        Some((pane, index)) => {
            workspace::close_tab(state, pane, index, crate::apply::Force::No);
            match state.layout.panes.contains_key(source) {
                true => state.layout.focus = source,
                false => {}
            }
        }
        None => {
            let Some(doc) = renderable(state, buffer) else {
                return;
            };
            let rope = doc.text.slice(..);
            let line = rope.char_to_line(text.selection.primary().cursor(rope));
            let rendered = RenderedView {
                text,
                ..RenderedView::new(
                    buffer,
                    Link::Follows {
                        pane: focus,
                        seen: None,
                    },
                    Scroll::Line(line),
                )
            };
            let now = state.layout.tick();
            let tabs = Tabs::new(View::Rendered(rendered), OpenAs::Permanent, now);
            state.layout.split(Axis::Row, Pane { tabs: Some(tabs) });
            state.layout.focus = focus;
        }
    }
}

/// The buffer's loaded text, or `None` with a notice when it is loading or has no rendered view.
pub fn renderable(state: &mut State, buffer: BufferId) -> Option<&TextDoc> {
    let State {
        buffers, notice, ..
    } = state;
    let Buffer { path, content } = buffers.get(buffer)?;
    match (Format::of(path), content) {
        (Some(_), crate::buffer::Content::Text { doc, .. }) => Some(doc),
        (Some(_), crate::buffer::Content::Loading { .. }) => {
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
    let focus = state.layout.focus;
    let Some(tabs) = (state.layout.panes.get(focus)).and_then(|pane| pane.tabs.as_ref()) else {
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
            let Some(doc) = renderable(state, buffer) else {
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
            workspace::show(state, focus, TabKey::Rendered(buffer), open);
            match state.layout.rendered_mut(focus) {
                Some(rendered) if rendered.text.buffer() == buffer => {
                    rendered.text = text;
                    rendered.scroll = Scroll::Line(line);
                }
                Some(_) | None => {}
            }
        }
        View::Rendered(rendered) => {
            workspace::show(state, focus, TabKey::Text(buffer), open);
            let State {
                layout,
                buffers,
                previews,
                ..
            } = state;
            let Some(doc) = buffer::doc(buffers, buffer) else {
                return;
            };
            let rope = doc.text.slice(..);
            let top = (rendered.scroll.anchor())
                .and_then(|at| markdown(previews, buffer, doc)?.item_line(at.item))
                .unwrap_or_else(|| {
                    rope.char_to_line(rendered.text.scroll.anchor.min(rope.len_chars()))
                });
            let rows = rendered.text.scroll.viewport.rows as usize;
            let cursor = rope.char_to_line(rendered.text.selection.primary().cursor(rope));
            match layout.panes.get_mut(focus).and_then(Pane::active_mut) {
                Some(View::Text(text)) if text.buffer() == buffer => {
                    text.selection = match (top..top + rows.max(1)).contains(&cursor) {
                        true => rendered.text.selection,
                        false => Selection::point(rope.line_to_char(top)),
                    };
                    text.scroll.anchor = rope.line_to_char(top);
                    text.scroll.rows = 0;
                }
                Some(View::Text(_) | View::Diff(_) | View::Rendered(_)) | None => {}
            }
            scroll::follow_cursor(state, focus);
        }
    }
}

/// Keys of the focused rendered view.
pub fn command(state: &mut State, command: RenderedCommand) {
    let focus = state.layout.focus;
    let State {
        layout,
        previews,
        notice,
        ..
    } = state;
    let Some(view) = layout.rendered(focus) else {
        return;
    };
    let scroll = match (command, view.scroll.anchor()) {
        (RenderedCommand::Leave, _) => {
            return match view.link {
                Link::Follows { pane, .. } if layout.panes.contains_key(pane) => {
                    layout.focus = pane
                }
                Link::Follows { .. } | Link::Standalone => toggle(state),
            };
        }
        (RenderedCommand::Fit, _) => {
            let fit = match view.fit {
                Fit::Width => Fit::Page,
                Fit::Page => Fit::Width,
            };
            match layout.rendered_mut(focus) {
                Some(view) => view.fit = fit,
                None => {}
            }
            return;
        }
        // Moves wait for the line to resolve into a place.
        (_, None) => return,
        (RenderedCommand::Scroll(direction, by), Some(from)) => {
            let sign = match direction {
                Direction::Forward => 1,
                Direction::Backward => -1,
            };
            let (lines, half_pages) = match by {
                ScrollBy::Lines => (sign, 0),
                ScrollBy::Page => (0, 2 * sign),
                ScrollBy::HalfPageCursor => (0, sign),
            };
            let moved = match view.scroll {
                Scroll::Moved { by, .. } => by,
                Scroll::At(_) | Scroll::Line(_) => Motion::default(),
            };
            Scroll::Moved {
                from,
                by: Motion {
                    lines: moved.lines + lines,
                    half_pages: moved.half_pages + half_pages,
                },
            }
        }
        (RenderedCommand::Top, Some(_)) => Scroll::item(0),
        (RenderedCommand::Bottom, Some(_)) => {
            let items = match previews.get(view.text.buffer()) {
                Some(Preview::Markdown(parsed)) => parsed.markdown.roots.len(),
                Some(Preview::Typst(typst)) => typst
                    .pages
                    .as_ref()
                    .map_or(0, |compiled| compiled.pages.len()),
                None => 0,
            };
            Scroll::item(items.saturating_sub(1))
        }
        (RenderedCommand::Heading(direction), Some(at)) => {
            let Some(Preview::Markdown(Parsed { markdown, .. })) = previews.get(view.text.buffer())
            else {
                return;
            };
            let heading = |item: &usize| {
                matches!(
                    markdown.blocks[markdown.roots[*item]].kind,
                    BlockKind::Heading { .. }
                )
            };
            let found = match direction {
                Direction::Forward => (at.item + 1..markdown.roots.len()).find(heading),
                Direction::Backward => {
                    let before = match at.offset > Px(0.0) {
                        true => at.item + 1,
                        false => at.item,
                    };
                    (0..before.min(markdown.roots.len())).rev().find(heading)
                }
            };
            match found {
                Some(item) => Scroll::item(item),
                None => return *notice = Some(Notice::NoHeading),
            }
        }
    };
    match layout.rendered_mut(focus) {
        Some(view) => view.scroll = scroll,
        None => {}
    }
}

/// A click on block `block` of `pane`'s rendered view: the source it follows gets its cursor
/// on the block's first line and the focus.
pub fn click(state: &mut State, pane: PaneId, block: usize) {
    let Some(view) = state.layout.rendered(pane) else {
        return;
    };
    let (buffer, link) = (view.text.buffer(), view.link);
    let Some(item) = view.scroll.anchor().map(|at| at.item) else {
        return;
    };
    let Some(doc) = buffer::doc(&state.buffers, buffer) else {
        return;
    };
    let Some(line) = markdown(&state.previews, buffer, doc)
        .and_then(|markdown| markdown.blocks.get(block))
        .map(|b| b.lines.start)
    else {
        return;
    };
    let rope = doc.text.slice(..);
    let at = rope.line_to_char(line.min(rope.len_lines() - 1));
    match link {
        Link::Follows { pane: source, seen } if state.layout.panes.contains_key(source) => {
            workspace::show(state, source, TabKey::Text(buffer), OpenAs::Permanent);
            match state
                .layout
                .panes
                .get_mut(source)
                .and_then(Pane::active_mut)
            {
                Some(View::Text(text)) if text.buffer() == buffer => {
                    text.selection = Selection::point(at)
                }
                Some(View::Text(_) | View::Diff(_) | View::Rendered(_)) | None => {}
            }
            state.layout.focus = source;
            scroll::follow_cursor(state, source);
            let top = match (
                state.layout.panes.get(source).and_then(Pane::active),
                buffer::doc(&state.buffers, buffer),
            ) {
                (Some(View::Text(text)), Some(doc)) => {
                    let rope = doc.text.slice(..);
                    rope.char_to_line(text.scroll.anchor.min(rope.len_chars()))
                }
                (Some(_) | None, _) => line,
            };
            match state.layout.rendered_mut(pane) {
                Some(view) => {
                    view.link = Link::Follows {
                        pane: source,
                        seen: seen.map(|_| Seen {
                            cursor: line,
                            top,
                            item,
                        }),
                    }
                }
                None => {}
            }
        }
        Link::Follows { .. } | Link::Standalone => {
            match state.layout.rendered_mut(pane) {
                Some(view) => view.text.selection = Selection::point(at),
                None => {}
            }
            workspace::focus_pane(state, pane);
        }
    }
}

/// Flips the task checkbox whose `[` sits at byte `marker` of `buffer` at `version`, as one
/// undoable edit; a stale version or a marker that moved is ignored.
pub fn toggle_task(state: &mut State, buffer: BufferId, version: u64, marker: usize) {
    let State {
        layout,
        buffers,
        languages,
        ..
    } = state;
    let Some(doc) = buffer::doc_mut(buffers, buffer) else {
        return;
    };
    let rope = doc.text.slice(..);
    match doc.version == version && marker + 2 < rope.len_bytes() {
        true => {}
        false => return,
    }
    let at = rope.byte_to_char(marker);
    let flipped = match (rope.char(at), rope.char(at + 1), rope.char(at + 2)) {
        ('[', ' ', ']') => "x",
        ('[', 'x' | 'X', ']') => " ",
        _ => return,
    };
    let selection = layout
        .panes
        .values()
        .flat_map(Pane::items)
        .map(|item| item.view.text())
        .find(|view| view.buffer() == buffer)
        .map_or_else(|| Selection::point(0), |view| view.selection.clone());
    doc.commit(&selection);
    let tx = Transaction::change(
        &doc.text,
        [(at + 1, at + 2, Some(flipped.into()))].into_iter(),
    );
    doc.apply(&tx, &selection, languages);
    map_views(layout, buffer, tx.changes(), doc.text.slice(..));
    layout.keep(buffer);
    let after = selection.map(tx.changes());
    doc.commit(&after);
}

/// Flips the fold of callout `index` in `pane`'s rendered view; the source is untouched.
pub fn toggle_fold(state: &mut State, pane: PaneId, index: u32) {
    match state.layout.rendered_mut(pane) {
        Some(view) => match view.toggled.binary_search(&index) {
            Ok(at) => {
                view.toggled.remove(at);
            }
            Err(at) => view.toggled.insert(at, index),
        },
        None => {}
    }
}

/// A click on the error strip of `pane`'s typst preview: the file of the first error opens as
/// text with the cursor on it, in the pane the preview follows, else in the preview's pane.
pub fn goto_error(state: &mut State, pane: PaneId, out: &mut Vec<Effect>) {
    let Some(view) = state.layout.rendered(pane) else {
        return;
    };
    let (buffer, link) = (view.text.buffer(), view.link);
    let Some(Preview::Typst(Typst {
        outcome: Outcome::Failed(errors),
        ..
    })) = state.previews.get(buffer)
    else {
        return;
    };
    let (path, start) = match &errors[0].origin {
        Origin::Span {
            file: SourceFile::Project(path),
            start,
            ..
        } => (path.clone(), *start),
        Origin::Span {
            file: SourceFile::Embedded(_),
            ..
        }
        | Origin::Detached => return,
    };
    let target = match link {
        Link::Follows { pane: source, .. } if state.layout.panes.contains_key(source) => source,
        Link::Follows { .. } | Link::Standalone => pane,
    };
    workspace::focus_pane(state, target);
    crate::apply::open(
        state,
        path,
        Arrive::Text,
        Some(Target::Position(Position {
            line: start.line,
            column: start.column,
        })),
        OpenAs::Permanent,
        out,
    );
}
