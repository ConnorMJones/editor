//! Pages views: typst laid out as fixed pages.

use helix_core::SmallVec;
use model::command::{Direction, PagesCommand, ReadMove, ZoomBy};
use model::config::OpenAs;
use model::notice::Notice;
use model::typeset::{LineColumn, Origin, PagePoint, SourceFile};
use model::units::Px;
use slotmap::SecondaryMap;

use crate::git::navigate::Arrive;
use crate::project::Project;
use crate::render::source::{Link, Source};
use crate::render::step;
use crate::render::typeset::{self, Compiled, Locate, Outcome, Typst};
use crate::text::buffer::{Buffer, BufferId, Position, Target};
use crate::view::layout::{Pane, PaneId, View};
use crate::view::panes;
use crate::{Effect, State};

/// The compile state of every buffer a pages view shows.
pub type Compiles = SecondaryMap<BufferId, Typst>;

#[derive(Clone)]
pub struct PagesView {
    pub source: Source<PagesSeen>,
    pub scroll: PagesScroll,
    pub fit: Fit,
    pub zoom: Zoom,
}

/// The source cursor the view last went to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PagesSeen {
    pub cursor: usize,
}

/// Where a pages view stands, or is asked to move for the host to measure.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PagesScroll {
    /// Source line to bring to the top once the typesetter locates it.
    Line(usize),
    /// A point on a page; the host turns it into a place at its drawn scale.
    Point(PagePoint),
    At(PagesAt),
    /// Keys moved the view from `from` by `by`.
    Moved {
        from: PagesAt,
        by: PagesMotion,
    },
}

/// The top of a pages view: `offset` below the top of `page`, panned `left` to the right.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PagesAt {
    pub page: usize,
    pub offset: Px,
    pub left: Px,
}

/// A distance in the host's measured lines and viewport heights; `columns` move sideways.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PagesMotion {
    pub lines: i32,
    pub half_pages: i32,
    pub columns: i32,
}

/// How pages scale.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fit {
    /// Pages span the pane width.
    Width,
    /// Each page fits the pane whole.
    Page,
}

/// Page scale in steps of 1.25× from the fit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Zoom(i8);

impl Zoom {
    pub const FIT: Zoom = Zoom(0);
    const OUT: i8 = -6;
    const IN: i8 = 8;

    pub fn step(self, by: ZoomBy) -> Zoom {
        match by {
            ZoomBy::In => Zoom((self.0 + 1).min(Zoom::IN)),
            ZoomBy::Out => Zoom((self.0 - 1).max(Zoom::OUT)),
            ZoomBy::Reset => Zoom::FIT,
        }
    }

    pub fn factor(self) -> f32 {
        1.25f32.powi(self.0.into())
    }
}

impl PagesView {
    pub(crate) fn new(source: Source<PagesSeen>, scroll: PagesScroll) -> PagesView {
        PagesView {
            source,
            scroll,
            fit: Fit::Width,
            zoom: Zoom::FIT,
        }
    }
}

impl PagesScroll {
    pub fn page(page: usize) -> PagesScroll {
        PagesScroll::At(PagesAt {
            page,
            offset: Px(0.0),
            left: Px(0.0),
        })
    }

    /// The place moves start from, once the line or point is resolved.
    pub fn anchor(self) -> Option<PagesAt> {
        match self {
            PagesScroll::Line(_) | PagesScroll::Point(_) => None,
            PagesScroll::At(at) | PagesScroll::Moved { from: at, .. } => Some(at),
        }
    }
}

/// Keeps compiles for open pages tabs, compiles the shown ones and finds their source lines on
/// the pages.
pub fn sync(state: &mut State, out: &mut Vec<Effect>) {
    let State {
        layout,
        buffers,
        project: Project { root, .. },
        compiles,
        ..
    } = state;
    compiles.retain(|buffer, _| {
        (layout.panes().values().flat_map(Pane::items)).any(|item| match &item.view {
            View::Pages(view) => view.source.text.buffer() == buffer,
            View::Text(_) | View::Diff(_) | View::Document(_) => false,
        })
    });
    let shown: SmallVec<[(PaneId, BufferId); 4]> = (layout.panes().iter())
        .filter_map(|(pane, target)| match target.active() {
            Some(View::Pages(view)) => Some((pane, view.source.text.buffer())),
            Some(View::Text(_) | View::Diff(_) | View::Document(_)) | None => None,
        })
        .collect();
    let mut wanted: SmallVec<[BufferId; 4]> = SmallVec::new();
    for &(_, buffer) in &shown {
        match compiles.get(buffer) {
            Some(_) => {}
            None => {
                compiles.insert(buffer, Typst::new());
            }
        }
        wanted.push(buffer);
    }
    typeset::request_pages(compiles, buffers, root.path(), &wanted, out);
    for &(pane, buffer) in &shown {
        let Some(doc) = buffers[buffer].doc() else {
            continue;
        };
        let source = match layout.pages(pane).map(|view| view.source.link) {
            Some(Link::Follows { pane: source, .. }) => match layout.panes()[source].active() {
                Some(View::Text(text)) if text.buffer() == buffer => {
                    let rope = doc.text.slice(..);
                    let at = text.selection.primary().cursor(rope);
                    Some((rope.char_to_line(at), rope.char_to_byte(at)))
                }
                Some(View::Text(_) | View::Diff(_) | View::Document(_) | View::Pages(_)) | None => {
                    None
                }
            },
            Some(Link::Standalone) | None => None,
        };
        let (Some(view), Some(typst)) = (layout.pages_mut(pane), compiles.get_mut(buffer)) else {
            continue;
        };
        // Lines are found on the pages by the typesetter, once there are pages.
        let wanted = match (&typst.pages, view.scroll, &mut view.source.link, source) {
            (None, _, _, _) => None,
            (Some(_), PagesScroll::Line(line), _, _) => {
                Some(doc.text.line_to_byte(line.min(doc.text.len_lines() - 1)))
            }
            (
                Some(_),
                _,
                Link::Follows {
                    seen: Some(seen), ..
                },
                Some((cursor, _)),
            ) if seen.cursor == cursor => None,
            (Some(_), _, Link::Follows { seen, .. }, Some((cursor, byte))) => {
                *seen = Some(PagesSeen { cursor });
                Some(byte)
            }
            (Some(_), _, Link::Follows { .. } | Link::Standalone, _) => None,
        };
        let main = &buffers[buffer].path;
        match (wanted, typst.locate) {
            (None, _) => {}
            (Some(cursor), Locate::Idle) => {
                typst.locate = Locate::Running;
                out.push(Effect::Typeset(typeset::Request::Locate {
                    buffer,
                    main: main.clone(),
                    cursor,
                }));
            }
            (Some(cursor), Locate::Running | Locate::Queued(_)) => {
                typst.locate = Locate::Queued(cursor)
            }
        }
    }
}

/// The host measured `pane`'s pages view at `to`.
pub fn scrolled(state: &mut State, pane: PaneId, to: PagesAt) {
    match state.layout.pages_mut(pane) {
        Some(view) => view.scroll = PagesScroll::At(to),
        None => {}
    }
}

/// A scroll key on the focused pages view; moves wait for the line or point to resolve into a
/// place.
pub fn moved(state: &mut State, motion: ReadMove) {
    let focus = state.layout.focus();
    let State {
        layout, compiles, ..
    } = state;
    let Some(view) = layout.pages_mut(focus) else {
        return;
    };
    let Some(from) = view.scroll.anchor() else {
        return;
    };
    view.scroll = match motion {
        ReadMove::Scroll(direction, by) => {
            let (lines, half_pages) = step(direction, by);
            let step = PagesMotion {
                lines,
                half_pages,
                columns: 0,
            };
            moved_by(view.scroll, from, step)
        }
        ReadMove::Top => PagesScroll::page(0),
        ReadMove::Bottom => {
            let pages = (compiles.get(view.source.text.buffer()))
                .and_then(|typst| typst.pages.as_ref())
                .map_or(0, |compiled| compiled.pages.len());
            PagesScroll::page(pages.saturating_sub(1))
        }
    };
}

/// Adds `step` to the keys' motion so far from `from`.
fn moved_by(scroll: PagesScroll, from: PagesAt, step: PagesMotion) -> PagesScroll {
    let before = match scroll {
        PagesScroll::Moved { by, .. } => by,
        PagesScroll::At(_) | PagesScroll::Line(_) | PagesScroll::Point(_) => PagesMotion::default(),
    };
    PagesScroll::Moved {
        from,
        by: PagesMotion {
            lines: before.lines + step.lines,
            half_pages: before.half_pages + step.half_pages,
            columns: before.columns + step.columns,
        },
    }
}

/// Keys only pages views have.
pub fn command(state: &mut State, command: PagesCommand) {
    let focus = state.layout.focus();
    let Some(view) = state.layout.pages_mut(focus) else {
        return;
    };
    match command {
        PagesCommand::Fit => {
            view.fit = match view.fit {
                Fit::Width => Fit::Page,
                Fit::Page => Fit::Width,
            }
        }
        PagesCommand::Zoom(by) => view.zoom = view.zoom.step(by),
        PagesCommand::Pan(direction) => match view.scroll.anchor() {
            // Moves wait for the line or point to resolve into a place.
            None => {}
            Some(from) => {
                let columns = match direction {
                    Direction::Forward => 1,
                    Direction::Backward => -1,
                };
                view.scroll = moved_by(
                    view.scroll,
                    from,
                    PagesMotion {
                        columns,
                        ..PagesMotion::default()
                    },
                );
            }
        },
    }
}

/// A click at `at` on a page of `pane`'s pages view; the typesetter says where it leads.
pub fn click_page(state: &mut State, pane: PaneId, at: PagePoint, out: &mut Vec<Effect>) {
    let Some(view) = state.layout.pages(pane) else {
        return;
    };
    let Buffer { path, .. } = &state.buffers[view.source.text.buffer()];
    out.push(Effect::Typeset(typeset::Request::Click {
        pane,
        main: path.clone(),
        at,
    }));
}

/// A click on the diagnostic strip of `pane`'s pages view: the first error, else the first
/// warning, opens at its place.
pub fn goto_diagnostic(state: &mut State, pane: PaneId, out: &mut Vec<Effect>) {
    let Some(view) = state.layout.pages(pane) else {
        return;
    };
    let Some(typst) = state.compiles.get(view.source.text.buffer()) else {
        return;
    };
    let first = match (&typst.outcome, &typst.pages) {
        (Outcome::Failed(errors), _) => &errors[0],
        (Outcome::Pending | Outcome::Compiled, Some(Compiled { warnings, .. })) => {
            match warnings.as_slice() {
                [first, ..] => first,
                [] => return,
            }
        }
        (Outcome::Pending | Outcome::Compiled, None) => return,
    };
    let (path, at) = match &first.origin {
        Origin::Span {
            file: SourceFile::Project(path),
            start,
            ..
        } => (path.clone(), *start),
        Origin::Span {
            file: file @ (SourceFile::Embedded(_) | SourceFile::Package(_)),
            ..
        } => {
            return state.notice = Some(Notice::OutsideProject { file: file.clone() });
        }
        Origin::Detached => return,
    };
    goto_source(state, pane, path, at, out);
}

/// Opens `path` as text with the cursor at `at`, in the pane `pane` follows, else in `pane`.
pub fn goto_source(
    state: &mut State,
    pane: PaneId,
    path: std::path::PathBuf,
    at: LineColumn,
    out: &mut Vec<Effect>,
) {
    let target = match state.layout.pages(pane).map(|view| view.source.link) {
        Some(Link::Follows { pane: source, .. }) => source,
        Some(Link::Standalone) | None => pane,
    };
    panes::focus_pane(state, target);
    crate::apply::open(
        state,
        path,
        Arrive::Text,
        Some(Target::Position(Position {
            line: at.line,
            column: at.column,
        })),
        OpenAs::Permanent,
        out,
    );
}
