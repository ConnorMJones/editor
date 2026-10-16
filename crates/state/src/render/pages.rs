//! Typst rendered as fixed pages.

use helix_core::SmallVec;
use model::command::{Direction, PagesCommand, ReadMove, Shown, ZoomBy};
use model::config::OpenAs;
use model::notice::Notice;
use model::typeset::{LineColumn, Origin, PagePoint, SourceFile};
use model::units::Px;
use slotmap::SecondaryMap;

use crate::project::Project;
use crate::render::typeset::{self, Compiled, Locate, Outcome, Typst};
use crate::render::{Opening, Rendering, step};
use crate::text::buffer::{Buffer, BufferId, Position, Target};
use crate::view::layout::{Pane, PaneId};
use crate::view::panes::{self, Arrive};
use crate::view::tab::{BesideMut, Showing, Tab};
use crate::{Effect, State};

/// The compile state of every buffer a pages view shows.
pub type Compiles = SecondaryMap<BufferId, Typst>;

/// Where a rendered typst tab stands, and how its pages scale.
#[derive(Clone)]
pub struct PagesState {
    pub scroll: PagesScroll,
    pub fit: Fit,
    pub zoom: Zoom,
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

/// The source cursor a split's pages last went to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PagesSeen {
    pub cursor: usize,
}

impl Rendering for PagesState {
    type Seen = PagesSeen;

    fn open(at: Opening) -> PagesState {
        PagesState {
            scroll: match at {
                Opening::Line(line) => PagesScroll::Line(line),
                Opening::Item(page) => PagesScroll::page(page),
            },
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
        (layout.panes().values().flat_map(Pane::items)).any(|item| match item.tab.showing() {
            Showing::Pages(text, ..) => text.buffer() == buffer,
            Showing::Source(_) | Showing::Diff(..) | Showing::Document(..) => false,
        })
    });
    let shown: SmallVec<[(PaneId, BufferId); 4]> = (layout.panes().iter())
        .filter_map(
            |(pane, target)| match target.active().map(|tab| tab.showing()) {
                Some(Showing::Pages(text, ..)) => Some((pane, text.buffer())),
                Some(Showing::Source(_) | Showing::Diff(..) | Showing::Document(..)) | None => None,
            },
        )
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
        let (Some((text, view, beside)), Some(typst)) =
            (layout.pages_mut(pane), compiles.get_mut(buffer))
        else {
            continue;
        };
        let rope = doc.text.slice(..);
        let at = text.selection.primary().cursor(rope);
        let cursor = rope.char_to_line(at);
        // Lines are found on the pages by the typesetter, once there are pages; beside the source
        // the pages follow its cursor.
        let wanted = match (&typst.pages, view.scroll, beside) {
            (None, _, _) => None,
            (Some(_), PagesScroll::Line(line), _) => {
                Some(doc.text.line_to_byte(line.min(doc.text.len_lines() - 1)))
            }
            (Some(_), _, BesideMut::Source(Some(seen), _)) if seen.cursor == cursor => None,
            (Some(_), _, BesideMut::Source(seen, _)) => {
                *seen = Some(PagesSeen { cursor });
                Some(rope.char_to_byte(at))
            }
            (Some(_), _, BesideMut::Alone) => None,
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
        Some((_, view, _)) => view.scroll = PagesScroll::At(to),
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
    let Some((text, view, _)) = layout.pages_mut(focus) else {
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
            let pages = (compiles.get(text.buffer()))
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
    let Some((_, view, _)) = state.layout.pages_mut(focus) else {
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
    let Some((text, _)) = state.layout.pages(pane) else {
        return;
    };
    let Buffer { path, .. } = &state.buffers[text.buffer()];
    out.push(Effect::Typeset(typeset::Request::Click {
        pane,
        main: path.clone(),
        at,
    }));
}

/// A click on the diagnostic strip of `pane`'s pages view: the first error, else the first
/// warning, opens at its place.
pub fn goto_diagnostic(state: &mut State, pane: PaneId, out: &mut Vec<Effect>) {
    let Some((text, _)) = state.layout.pages(pane) else {
        return;
    };
    let Some(typst) = state.compiles.get(text.buffer()) else {
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

/// Opens `path` in `pane` with the cursor at `at`: as source, or beside its rendering in a split.
pub fn goto_source(
    state: &mut State,
    pane: PaneId,
    path: std::path::PathBuf,
    at: LineColumn,
    out: &mut Vec<Effect>,
) {
    let arrive = match state
        .layout
        .panes()
        .get(pane)
        .and_then(Pane::active)
        .map(Tab::shown)
    {
        Some(Shown::Split) => Arrive::In(Shown::Split),
        Some(Shown::Source | Shown::Diff | Shown::Render) | None => Arrive::In(Shown::Source),
    };
    panes::focus_pane(state, pane);
    crate::apply::open(
        state,
        path,
        arrive,
        Some(Target::Position(Position {
            line: at.line,
            column: at.column,
        })),
        OpenAs::Permanent,
        out,
    );
}
