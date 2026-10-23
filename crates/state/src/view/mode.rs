//! Switching a tab between the modes its kind of file offers.

use helix_core::Selection;
use model::command::{Direction, Resize, Shown};
use model::notice::Notice;

use crate::State;
use crate::render::{Opening, document};
use crate::view::layout::{Pane, PaneId};
use crate::view::scroll;
use crate::view::tab::{Beside, BesideMut, NoMode, Share, Showing, ShowingMut, TextTabs};

/// `space d`, `space V`: the focused tab in `shown`, or back to its source from there.
pub fn toggle(state: &mut State, shown: Shown) {
    match state.layout.focused().active_text().map(TextTabs::shown) {
        Some(current) if current == shown => show(state, Shown::Source),
        Some(_) => show(state, shown),
        None => state.notice = Some(Notice::NoBuffer),
    }
}

/// `space tab`: the focused tab in the next mode its kind of file has.
pub fn cycle(state: &mut State, direction: Direction) {
    match state
        .layout
        .focused()
        .active_text()
        .map(|tab| tab.next(direction))
    {
        Some(next) => show(state, next),
        None => state.notice = Some(Notice::NoBuffer),
    }
}

/// Shows the focused tab in `shown` at the same place: a rendering opens at the text's cursor
/// while it is in view, else at the text's top, and the text takes a rendering's top when it
/// leaves one.
pub fn show(state: &mut State, shown: Shown) {
    let focus = state.layout.focus();
    let State {
        layout,
        buffers,
        documents,
        notice,
        ..
    } = state;
    let Some(tab) = layout.focused_mut().active_text_mut() else {
        return *notice = Some(Notice::NoBuffer);
    };
    let buffer = tab.buffer();
    let doc = &buffers[buffer].doc;
    let rope = doc.text.slice(..);
    let rendered_top = match tab.showing() {
        Showing::Document(_, view, Beside::Alone) => (view.scroll.anchor()).and_then(|at| {
            document::markdown(documents, buffer, doc).and_then(|parsed| parsed.item_line(at.item))
        }),
        // Beside a rendering the source was in view all along.
        Showing::Source(_)
        | Showing::Diff(..)
        | Showing::Document(_, _, Beside::Source(..))
        | Showing::Pages(..) => None,
    };
    let text = tab.text();
    let rows = text.scroll.viewport.rows as usize;
    let cursor = rope.char_to_line(text.selection.primary().cursor(rope));
    let top =
        rendered_top.unwrap_or_else(|| rope.char_to_line(text.scroll.anchor.min(rope.len_chars())));
    // The cursor marks the place while it is in view; an undrawn view counts as showing it.
    let line = match (rows, cursor >= top) {
        (0, _) => cursor,
        (rows, true) if cursor < top + rows => cursor,
        (_, _) => top,
    };
    match tab.enter(shown, Opening::Line(line)) {
        Ok(()) => {}
        Err(NoMode) => {
            return *notice = Some(Notice::NotRenderable {
                path: buffers[buffer].path.clone(),
            });
        }
    }
    match rendered_top {
        Some(top) => {
            let text = tab.text_mut();
            let anchor = rope.line_to_char(top);
            text.scroll.anchor = anchor;
            text.scroll.rows = 0;
            match (top..top + rows.max(1)).contains(&cursor) {
                true => {}
                false => text.selection = Selection::point(anchor),
            }
        }
        None => {}
    }
    scroll::follow_cursor(state, focus);
}

/// `C-w <`, `C-w >`, `C-w =`: the focused split's source side narrower, wider or even.
pub fn resize(state: &mut State, resize: Resize) {
    match (state.layout.focused_mut().active_text_mut()).map(TextTabs::showing_mut) {
        Some(
            ShowingMut::Document(_, _, BesideMut::Source(_, share))
            | ShowingMut::Pages(_, _, BesideMut::Source(_, share)),
        ) => *share = share.resized(resize),
        Some(
            ShowingMut::Source(_)
            | ShowingMut::Diff(..)
            | ShowingMut::Document(_, _, BesideMut::Alone)
            | ShowingMut::Pages(_, _, BesideMut::Alone),
        )
        | None => state.notice = Some(Notice::NotSplit),
    }
}

/// The host dragged `pane`'s split divider to `share`; ignored once the tab has left the split.
pub fn drag(state: &mut State, pane: PaneId, share: Share) {
    match (state.layout.find_pane(pane))
        .and_then(Pane::active_text_mut)
        .map(TextTabs::showing_mut)
    {
        Some(
            ShowingMut::Document(_, _, BesideMut::Source(_, held))
            | ShowingMut::Pages(_, _, BesideMut::Source(_, held)),
        ) => *held = share,
        Some(
            ShowingMut::Source(_)
            | ShowingMut::Diff(..)
            | ShowingMut::Document(_, _, BesideMut::Alone)
            | ShowingMut::Pages(_, _, BesideMut::Alone),
        )
        | None => {}
    }
}
