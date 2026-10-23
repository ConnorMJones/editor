//! Typst rendered as fixed pages.

use helix_core::SmallVec;
use model::command::{Direction, PagesCommand, ReadMove, Shown, ZoomBy};
use model::config::OpenAs;
use model::geom::{self, Point, Vector};
use model::job::Busy;
use model::notice::Notice;
use model::sheet::{Fit, PageIndex, SheetView, Spot, Viewport, Zoom};
use model::typeset::{LineColumn, Origin, PagePoint, SourceFile};
use model::units::{Pt, Px};
use slotmap::SecondaryMap;

use crate::project::Project;
use crate::render::typeset::{self, Compiled, Outcome, Typst};
use crate::render::{Opening, Rendering, step};
use crate::text::buffer::{BufferId, Position, Target};
use crate::view::layout::{Pane, PaneId};
use crate::view::panes::{self, Arrive};
use crate::view::tab::{BesideMut, Showing, TextTabs};
use crate::{Effect, State};

/// The compile state of every buffer a pages view shows.
pub type Compiles = SecondaryMap<BufferId, Typst>;

/// Where a rendered typst tab looks, and how it scales its pages.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PagesState {
    pub view: SheetView,
    pub locating: Locating,
    /// The pane as last drawn, for keys that move by its lines or part of it.
    pub last: Option<Measure>,
}

/// Whether the view waits for the typesetter to place a source line.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Locating {
    Settled,
    Line(usize),
}

/// A pane's size and its line height, as the host drew it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Measure {
    pub viewport: Viewport,
    pub line: Px,
}

/// The source cursor a split's pages last went to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PagesSeen {
    pub cursor: usize,
}

impl Rendering for PagesState {
    type Seen = PagesSeen;

    fn open(at: Opening) -> PagesState {
        let (page, locating) = match at {
            Opening::Line(line) => (0, Locating::Line(line)),
            Opening::Item(page) => (page, Locating::Settled),
        };
        PagesState {
            view: SheetView::on(PageIndex(page)),
            locating,
            last: None,
        }
    }
}

impl PagesState {
    /// Centres the view on `at`, which settles a wait for a line.
    pub fn show(&mut self, at: PagePoint) {
        self.view.centre = Spot {
            page: PageIndex(at.page),
            at: Point::new(at.x.0, at.y.0),
        };
        self.locating = Locating::Settled;
    }
}

/// Keeps compiles for open pages tabs, compiles the shown ones and locates source lines.
pub fn sync(state: &mut State, out: &mut Vec<Effect>) {
    let State {
        layout,
        buffers,
        project: Project { root, .. },
        compiles,
        ..
    } = state;
    compiles.retain(|buffer, _| {
        (layout.panes().values().flat_map(Pane::items)).any(|item| {
            match item.tab.as_text().map(TextTabs::showing) {
                Some(Showing::Pages(text, ..)) => text.buffer() == buffer,
                Some(Showing::Source(_) | Showing::Diff(..) | Showing::Document(..)) | None => {
                    false
                }
            }
        })
    });
    let shown: SmallVec<[(PaneId, BufferId); 4]> = (layout.panes().iter())
        .filter_map(
            |(pane, target)| match target.active_text().map(|tab| tab.showing()) {
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
        let doc = &buffers[buffer].doc;
        let (Some((text, view, beside)), Some(typst)) =
            (layout.pages_mut(pane), compiles.get_mut(buffer))
        else {
            continue;
        };
        let rope = doc.text.slice(..);
        let at = text.selection.primary().cursor(rope);
        let cursor = rope.char_to_line(at);
        // Beside the source the pages follow its cursor, seen once its lookup starts.
        let wanted = match (&typst.pages, view.locating, beside) {
            (None, _, _) => None,
            (Some(_), Locating::Line(line), _) => Some((
                doc.text.line_to_byte(line.min(doc.text.len_lines() - 1)),
                None,
            )),
            (Some(_), _, BesideMut::Source(Some(seen), _)) if seen.cursor == cursor => None,
            (Some(_), _, BesideMut::Source(seen, _)) => Some((rope.char_to_byte(at), Some(seen))),
            (Some(_), _, BesideMut::Alone) => None,
        };
        match (wanted, typst.locate) {
            (Some((byte, seen)), Busy::Idle) => {
                typst.locate = Busy::Running;
                match seen {
                    Some(seen) => *seen = Some(PagesSeen { cursor }),
                    None => {}
                }
                out.push(Effect::Typeset(typeset::Request::Locate {
                    buffer,
                    main: buffers[buffer].path.clone(),
                    cursor: byte,
                }));
            }
            (Some(_), Busy::Running) | (None, _) => {}
        }
    }
}

/// The host drew `pane`'s pages view at `measure`.
pub fn measured(state: &mut State, pane: PaneId, measure: Measure) {
    match state.layout.pages_mut(pane) {
        Some((_, view, _)) => view.last = Some(measure),
        None => {}
    }
}

/// The wheel moved `pane`'s pages by `by`, or zoomed them by `factor` about a point.
pub fn wheeled(state: &mut State, pane: PaneId, wheel: Wheel, viewport: Viewport) {
    let State {
        layout, compiles, ..
    } = state;
    let Some((text, view, _)) = layout.pages_mut(pane) else {
        return;
    };
    let Some(Compiled { stack, .. }) = compiles.get(text.buffer()).and_then(|t| t.pages.as_ref())
    else {
        return;
    };
    match wheel {
        Wheel::Pan(by) => view.view.pan(stack, viewport, by),
        Wheel::Zoom { at, factor } => view.view.zoom_about(stack, viewport, at, factor),
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Wheel {
    Pan(Vector<geom::Pane>),
    Zoom { at: Point<geom::Pane>, factor: f32 },
}

/// Keys move by the host's lines and viewports; before the first draw or compile they wait.
pub fn moved(view: &mut PagesState, typst: Option<&Typst>, motion: ReadMove) {
    let Some(Compiled { stack, .. }) = typst.and_then(|typst| typst.pages.as_ref()) else {
        return;
    };
    match motion {
        ReadMove::Scroll(direction, by) => match view.last {
            Some(Measure { viewport, line }) => {
                let (lines, half_pages) = step(direction, by);
                let down = line.0 * lines as f32 + viewport.height.get() * 0.5 * half_pages as f32;
                view.view.pan(stack, viewport, Vector::new(0.0, down))
            }
            None => {}
        },
        ReadMove::Top => view.view.go_to(stack, PageIndex(0), viewport(view)),
        ReadMove::Bottom => view.view.go_to(stack, stack.last(), viewport(view)),
    }
}

fn viewport(view: &PagesState) -> Option<Viewport> {
    view.last.map(|measure| measure.viewport)
}

pub fn command(state: &mut State, command: PagesCommand) {
    let focus = state.layout.focus();
    let State {
        layout, compiles, ..
    } = state;
    let Some((text, view, _)) = layout.pages_mut(focus) else {
        return;
    };
    match command {
        PagesCommand::Fit => {
            view.view.fit = match view.view.fit {
                Fit::Width => Fit::Height,
                Fit::Height => Fit::Width,
            }
        }
        PagesCommand::Zoom(by) => {
            view.view.zoom = match by {
                ZoomBy::In => view.view.zoom.by(Zoom::STEP),
                ZoomBy::Out => view.view.zoom.by(1.0 / Zoom::STEP),
                ZoomBy::Reset => Zoom::ONE,
            }
        }
        PagesCommand::Pan(direction) => {
            let pages = compiles.get(text.buffer()).and_then(|t| t.pages.as_ref());
            match (pages, view.last) {
                (Some(Compiled { stack, .. }), Some(Measure { viewport, line })) => {
                    let across = match direction {
                        Direction::Forward => line.0,
                        Direction::Backward => -line.0,
                    };
                    view.view.pan(stack, viewport, Vector::new(across, 0.0))
                }
                // Not compiled or drawn yet.
                (None, _) | (_, None) => {}
            }
        }
    }
}

/// A click at `at` on `pane`'s pages view drawn at `viewport`; the typesetter says where a
/// click on a page leads.
pub fn click(
    state: &mut State,
    pane: PaneId,
    at: Point<geom::Pane>,
    viewport: Viewport,
    out: &mut Vec<Effect>,
) {
    let Some((text, view)) = state.layout.pages(pane) else {
        return;
    };
    let buffer = text.buffer();
    let Some(Compiled { stack, .. }) = state.compiles.get(buffer).and_then(|t| t.pages.as_ref())
    else {
        return;
    };
    // Between pages there is nothing to follow.
    let Some((page, on)) = view.view.frame(stack, viewport).hit(stack, at) else {
        return;
    };
    out.push(Effect::Typeset(typeset::Request::Click {
        pane,
        main: state.buffers[buffer].path.clone(),
        at: PagePoint {
            page: page.index.0,
            x: Pt(on.x),
            y: Pt(on.y),
        },
    }));
}

/// The first error, else the first warning, opens at its place.
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
        .and_then(Pane::active_text)
        .map(TextTabs::shown)
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
