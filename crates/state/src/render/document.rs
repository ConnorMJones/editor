//! Document views: markdown laid out as a flow of blocks.

use helix_core::{Selection, SmallVec, Transaction};
use model::command::{Direction, DocumentCommand, ReadMove};
use model::config::OpenAs;
use model::notice::Notice;
use model::units::Px;
use slotmap::SecondaryMap;

use crate::apply::map_views;
use crate::project::Project;
use crate::project::files::FileIndex;
use crate::project::path::RelPath;
use crate::render::markdown::{self, Base, BlockKind, Callout, CalloutFold, Markdown};
use crate::render::source::{Link, Source};
use crate::render::{step, typeset};
use crate::text::buffer::{Buffer, BufferId, Content, TextDoc};
use crate::view::layout::{Pane, PaneId, TabKey, View};
use crate::view::panes::{self, Place};
use crate::{Effect, State};

/// The current parse of every buffer a document view shows.
pub type Documents = SecondaryMap<BufferId, Parsed>;

pub struct Parsed {
    /// `TextDoc::version` the blocks describe.
    pub version: u64,
    /// `links::Index::generation` the links were resolved against.
    pub generation: u64,
    pub markdown: Markdown,
}

#[derive(Clone)]
pub struct DocumentView {
    pub source: Source<DocumentSeen>,
    pub scroll: DocumentScroll,
    /// Callouts, by `Callout::index`, whose fold the reader flipped from the default.
    pub toggled: Vec<u32>,
}

/// The source position both sides last agreed on; a change on one side moves the other.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DocumentSeen {
    pub cursor: usize,
    pub top: usize,
    pub item: usize,
}

/// Where a document view stands, or is asked to move for the host to measure.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum DocumentScroll {
    /// Source line to bring to the top once the buffer is parsed.
    Line(usize),
    At(DocumentAt),
    /// Keys moved the view from `from` by `by`.
    Moved {
        from: DocumentAt,
        by: DocumentMotion,
    },
}

/// The top of a document view: `offset` below the top of root block `item`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct DocumentAt {
    pub item: usize,
    pub offset: Px,
}

/// A distance in the host's measured lines and viewport heights.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DocumentMotion {
    pub lines: i32,
    pub half_pages: i32,
}

impl DocumentView {
    pub(crate) fn new(source: Source<DocumentSeen>, scroll: DocumentScroll) -> DocumentView {
        DocumentView {
            source,
            scroll,
            toggled: Vec::new(),
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

impl DocumentScroll {
    pub fn item(item: usize) -> DocumentScroll {
        DocumentScroll::At(DocumentAt {
            item,
            offset: Px(0.0),
        })
    }

    /// The place moves start from, once the line is resolved.
    pub fn anchor(self) -> Option<DocumentAt> {
        match self {
            DocumentScroll::Line(_) => None,
            DocumentScroll::At(at) | DocumentScroll::Moved { from: at, .. } => Some(at),
        }
    }
}

/// The parse of `buffer`, if current with `doc`.
pub fn markdown<'a>(
    documents: &'a Documents,
    buffer: BufferId,
    doc: &TextDoc,
) -> Option<&'a Markdown> {
    match documents.get(buffer) {
        Some(parsed) if parsed.version == doc.version => Some(&parsed.markdown),
        Some(_) | None => None,
    }
}

/// Whether the set of parses figures are drawn from changed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Parses {
    Kept,
    Changed,
}

/// A document pane as `sync` found it, with the source lines its items stood for before a
/// reparse.
struct Shown {
    pane: PaneId,
    buffer: BufferId,
    line: Option<usize>,
    seen: Option<usize>,
}

/// Keeps parses for open document tabs, parses the shown ones and moves their scroll and links.
pub fn sync(state: &mut State, out: &mut Vec<Effect>) {
    let State {
        layout,
        buffers,
        languages,
        project: Project {
            root,
            index: file_index,
            ..
        },
        figures,
        documents,
        ..
    } = state;
    let before = documents.len();
    documents.retain(|buffer, _| {
        (layout.panes().values().flat_map(Pane::items)).any(|item| match &item.view {
            View::Document(view) => view.source.text.buffer() == buffer,
            View::Text(_) | View::Diff(_) | View::Pages(_) => false,
        })
    });
    let mut parses = match documents.len() == before {
        true => Parses::Kept,
        false => Parses::Changed,
    };
    let mut shown: SmallVec<[Shown; 4]> = SmallVec::new();
    for (pane, target) in layout.panes().iter() {
        let Some(View::Document(view)) = target.active() else {
            continue;
        };
        let buffer = view.source.text.buffer();
        // The last parse, current or not, maps items back to the lines they stood for.
        let parsed = documents.get(buffer).map(|parsed| &parsed.markdown);
        let line_of = |item: usize| parsed.and_then(|markdown| markdown.item_line(item));
        shown.push(Shown {
            pane,
            buffer,
            line: view.scroll.anchor().and_then(|at| line_of(at.item)),
            seen: match view.source.link {
                Link::Follows {
                    seen: Some(seen), ..
                } => line_of(seen.item),
                Link::Follows { seen: None, .. } | Link::Standalone => None,
            },
        });
    }
    let mut reparsed: SmallVec<[BufferId; 4]> = SmallVec::new();
    for entry in &shown {
        // Links resolve against the file index, so documents wait for the first walk.
        let FileIndex::Ready(indexed) = &mut *file_index else {
            break;
        };
        let index = indexed.links().0;
        let Buffer {
            path,
            content: Content::Text { doc, .. },
        } = &buffers[entry.buffer]
        else {
            continue;
        };
        match documents.get(entry.buffer) {
            Some(parsed)
                if parsed.version == doc.version && parsed.generation == index.generation => {}
            Some(_) | None => {
                let reuse = documents.remove(entry.buffer).map(|parsed| parsed.markdown);
                let root = root.path();
                let note = RelPath::new(root, path).ok();
                let base = Base {
                    dir: path.parent().unwrap_or(root),
                    root,
                    folder: note.as_ref().map_or("", RelPath::folder_path),
                    vault: index,
                };
                let mut markdown = markdown::parse(doc.text.slice(..), base, reuse);
                markdown::highlight(&mut markdown, languages);
                documents.insert(
                    entry.buffer,
                    Parsed {
                        version: doc.version,
                        generation: index.generation,
                        markdown,
                    },
                );
                parses = Parses::Changed;
                reparsed.push(entry.buffer);
            }
        }
    }
    match parses {
        Parses::Kept => {}
        Parses::Changed => typeset::mark(figures, documents, out),
    }
    for entry in &shown {
        let doc = buffers[entry.buffer].doc();
        let Some(view) = layout.document(entry.pane) else {
            continue;
        };
        let source = match view.source.link {
            Link::Follows { pane, .. } => match layout.panes()[pane].active() {
                Some(View::Text(text)) if text.buffer() == entry.buffer => doc.map(|doc| {
                    let rope = doc.text.slice(..);
                    let at = text.selection.primary().cursor(rope);
                    let top = rope.char_to_line(text.scroll.anchor.min(rope.len_chars()));
                    (rope.char_to_line(at), top)
                }),
                Some(View::Text(_) | View::Diff(_) | View::Document(_) | View::Pages(_)) | None => {
                    None
                }
            },
            Link::Standalone => None,
        };
        let Some(view) = layout.document_mut(entry.pane) else {
            continue;
        };
        let markdown = match (doc, documents.get(entry.buffer)) {
            (Some(doc), Some(parsed)) if parsed.version == doc.version => &parsed.markdown,
            (_, Some(_) | None) => continue,
        };
        let last = markdown.roots.len().saturating_sub(1);
        let remap = reparsed.contains(&entry.buffer);
        match (&mut view.scroll, remap, entry.line) {
            (DocumentScroll::Line(line), _, _) => {
                view.scroll = DocumentScroll::item(markdown.item_at_line(*line).min(last))
            }
            (DocumentScroll::At(at) | DocumentScroll::Moved { from: at, .. }, true, Some(line)) => {
                at.item = markdown.item_at_line(line).min(last)
            }
            (DocumentScroll::At(at) | DocumentScroll::Moved { from: at, .. }, _, _) => {
                at.item = at.item.min(last)
            }
        }
        match (remap, &mut view.source.link, entry.seen) {
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
            (&mut view.source.link, source, view.scroll.anchor())
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
                view.scroll = DocumentScroll::item(item);
                item
            }
            Some(_) | None => at.item,
        };
        *seen = Some(DocumentSeen { cursor, top, item });
    }
}

/// The host measured `pane`'s document view at `to`; a move to another item scrolls the
/// source it follows, leaving the source cursor where it is.
pub fn scrolled(state: &mut State, pane: PaneId, to: DocumentAt) {
    let State {
        layout,
        buffers,
        documents,
        ..
    } = state;
    let Some(view) = layout.document_mut(pane) else {
        return;
    };
    view.scroll = DocumentScroll::At(to);
    let buffer = view.source.text.buffer();
    let (source, seen) = match view.source.link {
        Link::Follows {
            pane: source,
            seen: Some(seen),
        } if seen.item != to.item => (source, seen),
        Link::Follows { .. } | Link::Standalone => return,
    };
    let Some(doc) = buffers[buffer].doc() else {
        return;
    };
    let Some(line) =
        markdown(documents, buffer, doc).and_then(|markdown| markdown.item_line(to.item))
    else {
        return;
    };
    match layout.pane(source).active_mut() {
        Some(View::Text(text)) if text.buffer() == buffer => {
            text.scroll.anchor = doc.text.line_to_char(line.min(doc.text.len_lines() - 1));
            text.scroll.rows = 0;
        }
        Some(View::Text(_) | View::Diff(_) | View::Document(_) | View::Pages(_)) | None => {}
    }
    let view = (layout.document_mut(pane)).expect("scrolling the source leaves this view");
    view.source.link = Link::Follows {
        pane: source,
        seen: Some(DocumentSeen {
            top: line,
            item: to.item,
            ..seen
        }),
    };
}

/// A scroll key on the focused document view; moves wait for the line to resolve into a place.
pub fn moved(state: &mut State, motion: ReadMove) {
    let focus = state.layout.focus();
    let State {
        layout, documents, ..
    } = state;
    let Some(view) = layout.document_mut(focus) else {
        return;
    };
    let Some(from) = view.scroll.anchor() else {
        return;
    };
    view.scroll = match motion {
        ReadMove::Scroll(direction, by) => {
            let (lines, half_pages) = step(direction, by);
            let before = match view.scroll {
                DocumentScroll::Moved { by, .. } => by,
                DocumentScroll::At(_) | DocumentScroll::Line(_) => DocumentMotion::default(),
            };
            DocumentScroll::Moved {
                from,
                by: DocumentMotion {
                    lines: before.lines + lines,
                    half_pages: before.half_pages + half_pages,
                },
            }
        }
        ReadMove::Top => DocumentScroll::item(0),
        ReadMove::Bottom => {
            let items = (documents.get(view.source.text.buffer()))
                .map_or(0, |parsed| parsed.markdown.roots.len());
            DocumentScroll::item(items.saturating_sub(1))
        }
    };
}

/// Keys only document views have.
pub fn command(state: &mut State, command: DocumentCommand) {
    let focus = state.layout.focus();
    let State {
        layout,
        documents,
        notice,
        ..
    } = state;
    let Some(view) = layout.document_mut(focus) else {
        return;
    };
    let DocumentCommand::Heading(direction) = command;
    let (Some(at), Some(Parsed { markdown, .. })) = (
        view.scroll.anchor(),
        documents.get(view.source.text.buffer()),
    ) else {
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
        Some(item) => view.scroll = DocumentScroll::item(item),
        None => *notice = Some(Notice::NoHeading),
    }
}

/// A drawn block's first source line, and the text version it was drawn from.
#[derive(Clone, Copy, Debug)]
pub struct BlockClick {
    pub line: usize,
    pub version: u64,
}

/// A click on a drawn block: the source it follows gets its cursor on the block's first line
/// and the focus.
pub fn click(state: &mut State, pane: PaneId, click: BlockClick) {
    let Some(view) = state.layout.document_mut(pane) else {
        return;
    };
    let buffer = view.source.text.buffer();
    let Some(doc) = state.buffers[buffer]
        .doc()
        .filter(|doc| doc.version == click.version)
    else {
        return;
    };
    let rope = doc.text.clone();
    let at = rope.line_to_char(click.line);
    match view.source.link {
        Link::Standalone => {
            view.source.text.selection = Selection::point(at);
            panes::focus_pane(state, pane);
        }
        Link::Follows { pane: source, seen } => {
            let item = view.scroll.anchor().map(|at| at.item);
            let place = Place {
                selection: Selection::point(at),
                anchor: None,
            };
            let key = TabKey::Text(buffer);
            let shown = panes::show_at(state, source, key, OpenAs::Permanent, place);
            let top = rope.char_to_line(shown.scroll.anchor);
            (state.layout.set_focus(source)).expect("a link names a live pane");
            // The view stays where it is; a link not yet synced is aligned by the next sync.
            let seen = match (seen, item) {
                (Some(_), Some(item)) => Some(DocumentSeen {
                    cursor: click.line,
                    top,
                    item,
                }),
                (Some(_), None) | (None, _) => None,
            };
            let view =
                (state.layout.document_mut(pane)).expect("showing the source leaves this view");
            view.source.link = Link::Follows { pane: source, seen };
        }
    }
}

/// Flips the task checkbox whose `[` sits at byte `marker` of `buffer` at `version`, as one
/// undoable edit; ignored once the text has moved past `version`.
pub fn toggle_task(state: &mut State, buffer: BufferId, version: u64, marker: usize) {
    let State {
        layout,
        buffers,
        languages,
        ..
    } = state;
    let Some(doc) =
        (buffers.get_mut(buffer).and_then(Buffer::doc_mut)).filter(|doc| doc.version == version)
    else {
        return;
    };
    let rope = doc.text.slice(..);
    let at = rope.byte_to_char(marker);
    let flipped = match (rope.char(at), rope.char(at + 1), rope.char(at + 2)) {
        ('[', ' ', ']') => "x",
        ('[', 'x' | 'X', ']') => " ",
        _ => return,
    };
    let selection = layout.selection_of(buffer);
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

/// Flips the fold of callout `index` in `pane`'s document view; the source is untouched.
pub fn toggle_fold(state: &mut State, pane: PaneId, index: u32) {
    let Some(view) = state.layout.document_mut(pane) else {
        return;
    };
    match view.toggled.binary_search(&index) {
        Ok(at) => {
            view.toggled.remove(at);
        }
        Err(at) => view.toggled.insert(at, index),
    }
}
