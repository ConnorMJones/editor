//! Text input from the platform's input method.

use std::ops::Range;

use gpui::{Bounds, Context, EntityInputHandler, Pixels, UTF16Selection, Window, point};
use helix_core::visual_offset_from_anchor;
use state::git::conflict;
use state::git::diffview::{Left, RIGHT};
use state::input;

use state::view::display::{self, Gutter, Indent};
use state::view::scroll;
use state::view::tab::{Beside, Showing};

use super::Workspace;

/// Text arrives here once composed, so insert mode sees text rather than raw key presses.
impl EntityInputHandler for Workspace {
    fn text_for_range(
        &mut self,
        _range: Range<usize>,
        _adjusted_range: &mut Option<Range<usize>>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<String> {
        None
    }

    fn selected_text_range(
        &mut self,
        _ignore_disabled_input: bool,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        None
    }

    fn marked_text_range(
        &self,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Range<usize>> {
        match self.preedit.is_empty() {
            true => None,
            false => Some(0..self.preedit.encode_utf16().count()),
        }
    }

    fn unmark_text(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        self.preedit.clear();
        cx.notify();
    }

    fn replace_text_in_range(
        &mut self,
        _range: Option<Range<usize>>,
        text: &str,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.preedit.clear();
        input::text(&mut self.state, text, &mut self.effects);
        self.flush(cx);
        cx.notify();
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        _range: Option<Range<usize>>,
        text: &str,
        _selected_range: Option<Range<usize>>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.preedit.clear();
        self.preedit.push_str(text);
        cx.notify();
    }

    fn bounds_for_range(
        &mut self,
        _range: Range<usize>,
        element_bounds: Bounds<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        let state = &self.state;
        let shown = state.layout.focused().active_text()?;
        let view = shown.text();
        let doc = &state.buffers[view.buffer()].doc;
        let text = doc.text.slice(..);
        let (padding, gutter_lines) = match shown.showing() {
            Showing::Document(_, _, Beside::Alone) | Showing::Pages(_, _, Beside::Alone) => {
                return None;
            }
            Showing::Source(_)
            | Showing::Document(_, _, Beside::Source(..))
            | Showing::Pages(_, _, Beside::Source(..)) => {
                (conflict::padding(doc), text.len_lines())
            }
            Showing::Diff(_, diff) => (
                diff.align.sides[RIGHT].padding(),
                match &diff.left {
                    Left::Head { doc: head, .. } => text.len_lines().max(head.text.len_lines()),
                    Left::Pending | Left::Absent | Left::Parsing { .. } => text.len_lines(),
                },
            ),
        };
        let viewport = view.scroll.viewport;
        let format = display::text_format(viewport, Indent::of(doc, &state.languages));
        let (pos, _) = visual_offset_from_anchor(
            text,
            view.scroll.anchor,
            view.selection.primary().cursor(text),
            &format,
            &scroll::annotations(text, padding),
            (viewport.rows as isize + view.scroll.rows).max(0) as usize,
        )
        .ok()?;
        let row = pos.row as isize - view.scroll.rows;
        let cell = self.fonts.cell;
        let col = Gutter::new(gutter_lines).cols as usize + pos.col;
        Some(Bounds::new(
            point(
                element_bounds.origin.x + cell.width * col as f32,
                element_bounds.origin.y + cell.height * row as f32,
            ),
            cell,
        ))
    }

    fn character_index_for_point(
        &mut self,
        _point: gpui::Point<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<usize> {
        None
    }

    fn accepts_text_input(&self, _window: &mut Window, _cx: &mut Context<Self>) -> bool {
        input::typed(input::context(&self.state), ' ').is_some()
    }
}
