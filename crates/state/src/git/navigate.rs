//! `]g` navigation through hunks and on to the next changed file.

use helix_core::{Range, RopeSlice, Selection};
use model::command::{Direction, HunkTarget};
use model::config::OpenAs;
use model::git::Group;
use model::mode::Mode;
use model::notice::Notice;

use crate::apply::count_of;
use crate::git::diff::{Bases, Hunk, Side, next_hunk, prev_hunk};
use crate::project::path::RelPath;
use crate::text::buffer::{BufferId, Content, Target};
use crate::view::layout::{PaneId, TabKey, View};
use crate::view::scroll;
use crate::{Effect, Repo, State};

/// Which view of a file a tab shows; `]g` keeps it when it moves on to the next changed file.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Arrive {
    Text,
    Diff,
    Document,
    Pages,
}

impl Arrive {
    pub fn key(self, buffer: BufferId) -> TabKey {
        match self {
            Arrive::Text => TabKey::Text(buffer),
            Arrive::Diff => TabKey::Diff(buffer),
            Arrive::Document => TabKey::Document(buffer),
            Arrive::Pages => TabKey::Pages(buffer),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Landing {
    pub pane: PaneId,
    pub edge: Edge,
    pub arrive: Arrive,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Edge {
    First,
    Last,
}

/// Selection covering a hunk's buffer lines; a removal is the character after it.
fn hunk_range(hunk: &Hunk, text: RopeSlice) -> Range {
    let anchor = text.line_to_char(hunk.after.start as usize);
    let head = match hunk.after.is_empty() {
        true => (anchor + 1).min(text.len_chars()),
        false => text.line_to_char(hunk.after.end as usize),
    };
    Range::new(anchor, head)
}

/// Moves a pending `]g` landing onto the buffer's first or last hunk once its diff is fresh.
pub fn land(state: &mut State, buffer: BufferId) {
    let State {
        buffers, layout, ..
    } = state;
    let Some(Content::Text { doc, diff, .. }) = buffers.get_mut(buffer).map(|b| &mut b.content)
    else {
        return;
    };
    let tracked = match &mut diff.bases {
        Bases::Tracked(tracked) if tracked.is_fresh(doc.version) => tracked,
        Bases::Loading { .. } | Bases::Tracked(_) | Bases::NoRepo | Bases::Untracked => return,
    };
    let Some(landing) = tracked.landing.take() else {
        return;
    };
    let hunk = match landing.edge {
        Edge::First => tracked.hunks.first(),
        Edge::Last => tracked.hunks.last(),
    };
    let view = (layout.find_pane(landing.pane)).and_then(|pane| {
        let index = pane.tabs.as_ref()?.find(landing.arrive.key(buffer))?;
        Some(pane.items_mut()[index].view.text_mut())
    });
    match (hunk, view) {
        (Some(hunk), Some(view)) => {
            let range = hunk_range(hunk, doc.text.slice(..));
            view.selection = Selection::single(range.anchor, range.head);
        }
        (None, _) | (_, None) => return,
    }
    scroll::follow_cursor(state, landing.pane);
}

/// Each range moved to (select mode: extended to) the count-th item from it; `None` when the
/// primary range has no item that way.
pub fn step<T>(
    selection: &Selection,
    mode: Mode,
    direction: Direction,
    count: usize,
    items: &[T],
    nearest: impl Fn(&Range) -> Option<usize>,
    range: impl Fn(&T) -> Range,
) -> Option<Selection> {
    let pick = |current: &Range| {
        nearest(current).map(|index| match direction {
            Direction::Forward => (index + count - 1).min(items.len() - 1),
            Direction::Backward => index.saturating_sub(count - 1),
        })
    };
    pick(&selection.primary())?;
    Some(selection.clone().transform(|current| {
        let Some(index) = pick(&current) else {
            return current;
        };
        let target = range(&items[index]);
        match (mode, direction) {
            (Mode::Select, _) => Range::new(
                current.anchor,
                match target.head <= current.anchor {
                    true => target.anchor,
                    false => target.head,
                },
            ),
            (Mode::Normal | Mode::Insert, Direction::Forward) => target,
            (Mode::Normal | Mode::Insert, Direction::Backward) => target.flip(),
        }
    }))
}

/// `]g [g ]G [G`, after helix's `goto_next_change`; past the ends it opens the next changed file.
pub fn goto_hunk(state: &mut State, target: HunkTarget, out: &mut Vec<Effect>) {
    let count = count_of(state);
    let mode = state.input.mode;
    let State {
        layout,
        buffers,
        notice,
        ..
    } = &mut *state;
    let Some(view) = layout.focused_mut().active_mut() else {
        *notice = Some(Notice::NoBuffer);
        return;
    };
    let arrive = match view {
        View::Text(_) | View::Document(_) | View::Pages(_) => Arrive::Text,
        View::Diff(_) => Arrive::Diff,
    };
    let view = view.text_mut();
    let buffer = view.buffer();
    let Some(Content::Text { doc, diff, .. }) = buffers.get(buffer).map(|b| &b.content) else {
        return;
    };
    let hunks: &[Hunk] = match &diff.bases {
        Bases::Tracked(tracked) => &tracked.hunks,
        Bases::NoRepo | Bases::Loading { .. } | Bases::Untracked => &[],
    };
    let text = doc.text.slice(..);
    let mut only = |hunk: Option<&Hunk>| match hunk {
        Some(hunk) => {
            let range = hunk_range(hunk, text);
            view.selection = Selection::single(range.anchor, range.head);
        }
        None => *notice = Some(Notice::NoHunks),
    };
    let direction = match target {
        HunkTarget::Next => Direction::Forward,
        HunkTarget::Previous => Direction::Backward,
        HunkTarget::First => return only(hunks.first()),
        HunkTarget::Last => return only(hunks.last()),
    };
    let nearest = |range: &Range| {
        let line = range.cursor_line(text) as u32;
        match direction {
            Direction::Forward => next_hunk(hunks, Side::After, line),
            Direction::Backward => prev_hunk(hunks, Side::After, line),
        }
    };
    let range = |hunk: &Hunk| hunk_range(hunk, text);
    match step(
        &view.selection,
        mode,
        direction,
        count,
        hunks,
        nearest,
        range,
    ) {
        Some(selection) => view.selection = selection,
        // Past the ends: the next or previous changed file in sidebar order.
        None => {
            let root = state.project.root.path();
            let current = RelPath::new(root, &state.buffers[buffer].path).ok();
            let neighbour = match &state.project.git {
                Repo::Found { snapshot, .. } => snapshot.neighbour(current.as_ref(), direction),
                Repo::Unknown | Repo::None => None,
            };
            let Some(changed) = neighbour else {
                state.notice = Some(Notice::NoChangedFiles);
                return;
            };
            let path = changed.path.to_path(root);
            // Conflicts are resolved in the text, so they never open as a diff.
            let arrive = match changed.group {
                Group::Conflicts => Arrive::Text,
                Group::Staged | Group::Changes => arrive,
            };
            let edge = match direction {
                Direction::Forward => Edge::First,
                Direction::Backward => Edge::Last,
            };
            crate::apply::open(
                state,
                path,
                arrive,
                Some(Target::Hunk(edge)),
                OpenAs::Preview,
                out,
            )
        }
    }
}

/// Lands on `edge` of a text buffer once its diff allows; called when `]g` opens a file.
pub(crate) fn expect_landing(state: &mut State, buffer: BufferId, landing: Landing) {
    match &mut state.buffers[buffer].content {
        Content::Text { diff, .. } => match &mut diff.bases {
            Bases::Loading { landing: waiting } => *waiting = Some(landing),
            Bases::Tracked(tracked) => tracked.landing = Some(landing),
            // An untracked file is all new; it opens at its start.
            Bases::NoRepo | Bases::Untracked => {}
        },
        Content::Loading { .. } => {}
    }
    land(state, buffer);
}
