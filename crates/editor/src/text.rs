use std::cell::Cell;
use std::fmt::Write as _;
use std::rc::Rc;

use gpui::{
    App, Bounds, DispatchPhase, Element, ElementId, ElementInputHandler, Entity, FocusHandle,
    GlobalElementId, Hitbox, HitboxBehavior, InspectorElementId, IntoElement, LayoutId,
    MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, Pixels, Point, ScrollDelta,
    ScrollWheelEvent, ShapedLine, SharedString, Size, Style, TextAlign, TextRun, UnderlineStyle,
    Window, fill, point, px, relative, size,
};
use model::mode::Mode;
use model::theme::{self as roles, Modifiers};
use slotmap::Key as _;
use state::buffer::Content;
use state::display::{
    self, CursorShape, DiffSign, Focus, Frame, Gutter, LineMark, Numbering, Rank, RowKind,
};
use state::layout::{PaneId, View, Viewport};
use state::scroll::{self, Click};

use crate::theme::Colors;
use crate::workspace::Workspace;

/// Draws the text view shown in `pane`.
pub struct TextElement {
    pub workspace: Entity<Workspace>,
    pub pane: PaneId,
}

impl IntoElement for TextElement {
    type Element = Self;

    fn into_element(self) -> Self {
        self
    }
}

/// Buffers kept across frames per pane so drawing reuses its allocations.
#[derive(Default)]
pub struct Cache {
    pub frame: Frame,
    pub lines: Vec<ShapedLine>,
    pub numbers: Vec<(u16, ShapedLine)>,
    pub runs: Vec<TextRun>,
    pub label: String,
    pub pointer: Rc<Pointer>,
}

#[derive(Default)]
pub struct Pointer {
    /// Wheel movement not yet amounting to a whole row.
    pub wheel: Cell<Pixels>,
    pub drag: Cell<Drag>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Drag {
    #[default]
    Idle,
    Selecting,
}

pub struct Prepaint {
    pub hitbox: Hitbox,
    pub text_origin: Point<Pixels>,
    pub cell: Size<Pixels>,
    pub gutter: Gutter,
    pub focus: Focus,
    pub handle: FocusHandle,
    pub preedit: Option<(Point<Pixels>, ShapedLine)>,
}

/// Cell under `position`, clamped to the text area's top-left.
fn cell_at(origin: Point<Pixels>, cell: Size<Pixels>, position: Point<Pixels>) -> (u16, u16) {
    let row = ((position.y - origin.y) / cell.height).floor().max(0.0);
    let col = ((position.x - origin.x) / cell.width).floor().max(0.0);
    (row as u16, col as u16)
}

impl Element for TextElement {
    type RequestLayoutState = ();
    type PrepaintState = Prepaint;

    fn id(&self) -> Option<ElementId> {
        Some(ElementId::NamedInteger(
            "text".into(),
            self.pane.data().as_ffi(),
        ))
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, ()) {
        let mut style = Style::default();
        style.size.width = relative(1.).into();
        style.size.height = relative(1.).into();
        (window.request_layout(style, [], cx), ())
    }

    fn prepaint(
        &mut self,
        id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) -> Prepaint {
        let pane = self.pane;
        let workspace = self.workspace.read(cx);
        let cell = workspace.theme.cell;
        let len_lines = match workspace.state.layout.panes.get(pane).map(|p| &p.view) {
            Some(View::Text(view)) => match &workspace.state.buffers[view.buffer].content {
                Content::Text(doc) => doc.text.len_lines(),
            },
            Some(View::Empty) | None => 1,
        };
        let gutter = Gutter::new(len_lines);
        let text_origin = point(
            bounds.origin.x + cell.width * gutter.cols as f32,
            bounds.origin.y,
        );
        let viewport = Viewport {
            rows: (bounds.size.height / cell.height).floor().max(0.0) as u16,
            cols: ((bounds.size.width / cell.width).floor().max(0.0) as u16)
                .saturating_sub(gutter.cols),
        };
        self.workspace.update(cx, |workspace, _| {
            scroll::set_viewport(&mut workspace.state, pane, viewport)
        });
        let hitbox = window.insert_hitbox(bounds, HitboxBehavior::Normal);
        let id = id.expect("text element has an id");
        window.with_element_state(id, |cache: Option<Cache>, window| {
            let mut cache = cache.unwrap_or_default();
            let workspace = self.workspace.read(cx);
            let theme = &workspace.theme;
            let state = &workspace.state;
            let focus = match state.layout.focus == pane {
                true => Focus::Focused(state.input.mode),
                false => Focus::Unfocused,
            };
            let frame = &mut cache.frame;
            match state.layout.panes.get(pane).map(|p| &p.view) {
                Some(View::Text(view)) => match &state.buffers[view.buffer].content {
                    Content::Text(doc) => {
                        display::layout(frame, doc, view, &state.languages, focus)
                    }
                },
                Some(View::Empty) | None => {
                    frame.rows.clear();
                    frame.selections.clear();
                    frame.cursors.clear();
                }
            }

            let text_system = window.text_system();
            cache.lines.clear();
            for row in &frame.rows {
                cache.runs.clear();
                cache
                    .runs
                    .extend(frame.runs[row.runs.clone()].iter().map(|run| {
                        let fg = run.style.fg.map_or(theme.text, |slot| theme.colors[slot]);
                        TextRun {
                            len: run.len,
                            font: theme.mono_font(run.style.modifiers).clone(),
                            color: fg,
                            background_color: None,
                            underline: run.style.modifiers.contains(Modifiers::UNDERLINED).then(
                                || UnderlineStyle {
                                    color: Some(fg),
                                    thickness: px(1.),
                                    wavy: false,
                                },
                            ),
                            strikethrough: None,
                        }
                    }));
                let text = SharedString::from(frame.text[row.text.clone()].to_owned());
                cache
                    .lines
                    .push(text_system.shape_line(text, theme.mono_size, &cache.runs, None));
            }

            let numbering = match focus {
                Focus::Focused(Mode::Normal | Mode::Select) => Numbering::Relative,
                Focus::Focused(Mode::Insert) | Focus::Unfocused => Numbering::Absolute,
            };
            cache.numbers.clear();
            for (index, row) in frame.rows.iter().enumerate() {
                match row.kind {
                    RowKind::First => {
                        cache.label.clear();
                        let number = display::line_label(row.line, frame.primary_line, numbering);
                        let _ = write!(
                            cache.label,
                            "{number:>width$}",
                            width = gutter.digits as usize
                        );
                        let slot = match row.mark {
                            LineMark::Cursor => roles::LINENR_SELECTED,
                            LineMark::Plain => roles::LINENR,
                        };
                        let run = TextRun {
                            len: cache.label.len(),
                            font: theme.mono[0].clone(),
                            color: theme.colors[slot],
                            background_color: None,
                            underline: None,
                            strikethrough: None,
                        };
                        let line = text_system.shape_line(
                            SharedString::from(cache.label.clone()),
                            theme.mono_size,
                            &[run],
                            None,
                        );
                        cache.numbers.push((index as u16, line));
                    }
                    RowKind::Wrapped => {}
                }
            }

            let primary = frame.cursors.iter().find(|c| c.rank == Rank::Primary);
            let preedit = match (focus, workspace.preedit.is_empty(), primary) {
                (Focus::Focused(_), false, Some(cursor)) => {
                    let run = TextRun {
                        len: workspace.preedit.len(),
                        font: theme.mono[0].clone(),
                        color: theme.text,
                        background_color: Some(theme.background),
                        underline: Some(UnderlineStyle {
                            color: Some(theme.text),
                            thickness: px(1.),
                            wavy: false,
                        }),
                        strikethrough: None,
                    };
                    let line = text_system.shape_line(
                        SharedString::from(workspace.preedit.clone()),
                        theme.mono_size,
                        &[run],
                        None,
                    );
                    let origin = point(
                        text_origin.x + cell.width * cursor.col as f32,
                        text_origin.y + cell.height * cursor.row as f32,
                    );
                    Some((origin, line))
                }
                (Focus::Focused(_) | Focus::Unfocused, ..) => None,
            };
            let prepaint = Prepaint {
                hitbox,
                text_origin,
                cell,
                gutter,
                focus,
                handle: workspace.focus.clone(),
                preedit,
            };
            (prepaint, cache)
        })
    }

    fn paint(
        &mut self,
        id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut (),
        prepaint: &mut Prepaint,
        window: &mut Window,
        cx: &mut App,
    ) {
        let Prepaint {
            hitbox,
            text_origin,
            cell,
            gutter,
            focus,
            handle,
            preedit,
        } = prepaint;
        let (origin, cell, gutter, focus) = (*text_origin, *cell, *gutter, *focus);
        let colors = Colors(self.workspace.read(cx).theme.colors.0);
        let pane = self.pane;
        // Git status is not tracked yet; signs are indexed by document line.
        let diff: &[DiffSign] = &[];
        let id = id.expect("text element has an id");
        let pointer = window.with_element_state(id, |cache: Option<Cache>, window| {
            let cache = cache.unwrap_or_default();
            let frame = &cache.frame;
            let y = |row: u16| origin.y + cell.height * row as f32;
            let x = |col: u16| origin.x + cell.width * col as f32;
            let text_width = bounds.right() - origin.x;

            match focus {
                Focus::Focused(_) => {
                    for (index, row) in frame.rows.iter().enumerate() {
                        match row.line == frame.primary_line {
                            true => window.paint_quad(fill(
                                Bounds::new(
                                    point(origin.x, y(index as u16)),
                                    size(text_width, cell.height),
                                ),
                                colors[roles::CURSORLINE],
                            )),
                            false => {}
                        }
                    }
                }
                Focus::Unfocused => {}
            }
            for span in &frame.selections {
                window.paint_quad(fill(
                    Bounds::new(
                        point(x(span.cols.start), y(span.row)),
                        size(
                            cell.width * (span.cols.end - span.cols.start) as f32,
                            cell.height,
                        ),
                    ),
                    colors[roles::SELECTION],
                ));
            }
            let cursor_bg = |rank: Rank| match rank {
                Rank::Primary => colors[roles::CURSOR_PRIMARY.bg],
                Rank::Secondary => colors[roles::CURSOR_SECONDARY.bg],
            };
            for cursor in &frame.cursors {
                match cursor.shape {
                    CursorShape::Block => window.paint_quad(fill(
                        Bounds::new(
                            point(x(cursor.col), y(cursor.row)),
                            size(cell.width * cursor.width as f32, cell.height),
                        ),
                        cursor_bg(cursor.rank),
                    )),
                    CursorShape::Bar | CursorShape::Underline => {}
                }
            }
            for (index, line) in cache.lines.iter().enumerate() {
                let _ = line.paint(
                    point(origin.x, y(index as u16)),
                    cell.height,
                    TextAlign::Left,
                    None,
                    window,
                    cx,
                );
            }
            for cursor in &frame.cursors {
                let (at, extent) = match cursor.shape {
                    CursorShape::Block => continue,
                    CursorShape::Bar => (
                        point(x(cursor.col), y(cursor.row)),
                        size(px(2.), cell.height),
                    ),
                    CursorShape::Underline => (
                        point(x(cursor.col), y(cursor.row) + cell.height - px(2.)),
                        size(cell.width * cursor.width as f32, px(2.)),
                    ),
                };
                window.paint_quad(fill(Bounds::new(at, extent), cursor_bg(cursor.rank)));
            }

            let number_x = bounds.origin.x + cell.width * Gutter::NUMBER_COL as f32;
            for (row, line) in &cache.numbers {
                let _ = line.paint(
                    point(number_x, y(*row)),
                    cell.height,
                    TextAlign::Left,
                    None,
                    window,
                    cx,
                );
            }
            let diff_x = bounds.origin.x + cell.width * (gutter.cols - 1) as f32;
            for (index, row) in frame.rows.iter().enumerate() {
                let top = y(index as u16);
                let sign = match diff.get(row.line).copied().unwrap_or_default() {
                    DiffSign::Unchanged => None,
                    DiffSign::Added => Some((roles::DIFF_PLUS, size(cell.width / 4., cell.height))),
                    DiffSign::Modified => {
                        Some((roles::DIFF_DELTA, size(cell.width / 4., cell.height)))
                    }
                    DiffSign::Removed => Some((roles::DIFF_MINUS, size(cell.width, px(2.)))),
                };
                match sign {
                    Some((slot, extent)) => window
                        .paint_quad(fill(Bounds::new(point(diff_x, top), extent), colors[slot])),
                    None => {}
                }
            }

            match preedit.take() {
                Some((at, line)) => {
                    let _ = line.paint(at, cell.height, TextAlign::Left, None, window, cx);
                }
                None => {}
            }
            (cache.pointer.clone(), cache)
        });

        match focus {
            Focus::Focused(_) => window.handle_input(
                handle,
                ElementInputHandler::new(bounds, self.workspace.clone()),
                cx,
            ),
            Focus::Unfocused => {}
        }

        window.on_mouse_event({
            let workspace = self.workspace.clone();
            let hitbox = hitbox.clone();
            let pointer = pointer.clone();
            move |event: &MouseDownEvent, phase, window, cx| match (
                phase,
                event.button,
                hitbox.is_hovered(window),
            ) {
                (DispatchPhase::Bubble, MouseButton::Left, true) => {
                    let (row, col) = cell_at(origin, cell, event.position);
                    pointer.drag.set(Drag::Selecting);
                    workspace.update(cx, |workspace, cx| {
                        scroll::click(&mut workspace.state, pane, row, col, Click::Place);
                        window.focus(&workspace.focus, cx);
                        cx.notify();
                    });
                }
                _ => {}
            }
        });
        window.on_mouse_event({
            let workspace = self.workspace.clone();
            let pointer = pointer.clone();
            move |event: &MouseMoveEvent, phase, _window, cx| match (
                phase,
                event.pressed_button,
                pointer.drag.get(),
            ) {
                (DispatchPhase::Bubble, Some(MouseButton::Left), Drag::Selecting) => {
                    let (row, col) = cell_at(origin, cell, event.position);
                    workspace.update(cx, |workspace, cx| {
                        scroll::click(&mut workspace.state, pane, row, col, Click::Extend);
                        cx.notify();
                    });
                }
                _ => {}
            }
        });
        window.on_mouse_event({
            let pointer = pointer.clone();
            move |event: &MouseUpEvent, phase, _window, _cx| match (phase, event.button) {
                (DispatchPhase::Bubble, MouseButton::Left) => pointer.drag.set(Drag::Idle),
                _ => {}
            }
        });
        window.on_mouse_event({
            let workspace = self.workspace.clone();
            let hitbox = hitbox.clone();
            move |event: &ScrollWheelEvent, phase, window, cx| match (
                phase,
                hitbox.is_hovered(window),
            ) {
                (DispatchPhase::Bubble, true) => {
                    let delta = match event.delta {
                        ScrollDelta::Lines(lines) => {
                            cell.height * (lines.y * scroll::WHEEL_LINES as f32)
                        }
                        ScrollDelta::Pixels(pixels) => pixels.y,
                    };
                    let total = pointer.wheel.get() - delta;
                    let rows = (total / cell.height).trunc();
                    pointer.wheel.set(total - cell.height * rows);
                    match rows as isize {
                        0 => {}
                        rows => workspace.update(cx, |workspace, cx| {
                            scroll::wheel(&mut workspace.state, pane, rows);
                            cx.notify();
                        }),
                    }
                }
                _ => {}
            }
        });
    }
}
