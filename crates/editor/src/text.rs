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
use helix_core::Selection;
use model::mode::Mode;
use model::theme::{Modifiers, Theme, Tone};
use slotmap::Key as _;
use state::git::conflict;
use state::git::diff::{self, Stage};
use state::git::diffview;
use state::view::display::{
    self, CursorShape, DiffSign, Focus, Frame, Gutter, LineMark, Numbering, Rank, RowKind,
};
use state::view::draws::{self, Drawing, Draws, Part, Rows};
use state::view::layout::{PaneId, Viewport};
use state::view::scroll::{self, Gesture, OnTrack, Press, Scrollbar, Unit};

use crate::fonts::{Fonts, hsla};
use crate::workspace::Workspace;

/// Right-edge strip for the scrollbar.
const SCROLLBAR_WIDTH: Pixels = px(8.);
const MIN_THUMB: f32 = 16.;

/// Draws one side of the tab shown in `pane`: its source, or either side of its diff.
pub struct TextElement {
    pub workspace: Entity<Workspace>,
    pub pane: PaneId,
    pub part: Part,
}

impl IntoElement for TextElement {
    type Element = Self;

    fn into_element(self) -> Self {
        self
    }
}

/// Index 0 is this frame and 1 the previous; unchanged rows reuse their shaped lines.
#[derive(Default)]
pub struct Cache {
    pub frames: [Frame; 2],
    pub lines: [Vec<Rc<ShapedLine>>; 2],
    pub numbers: [Vec<Number>; 2],
    pub runs: Vec<TextRun>,
    /// Gutter git sign of each row of `frames[0]`.
    pub signs: Vec<Option<(DiffSign, Stage)>>,
    pub label: String,
    pub pointer: Rc<Pointer>,
}

/// A shaped gutter label; `label`, `mark` and `digits` determine its text and colour.
pub struct Number {
    pub row: u16,
    pub label: usize,
    pub mark: LineMark,
    pub digits: u16,
    pub line: Rc<ShapedLine>,
}

#[derive(Default)]
pub struct Pointer {
    /// Wheel movement not yet amounting to a whole row.
    pub wheel: Cell<Pixels>,
    pub drag: Cell<Drag>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum Drag {
    #[default]
    Idle,
    Selecting(Gesture),
    /// `grab` is the pointer's offset from the thumb's top.
    Thumb {
        grab: Pixels,
    },
}

/// Scrollbar geometry of one frame; thumb drags map back to lines through it.
pub struct Bar {
    pub hitbox: Hitbox,
    pub scrollbar: Scrollbar,
    pub len_lines: usize,
    /// Document lines on screen.
    pub shown: usize,
    pub page: isize,
}

pub struct Prepaint {
    pub hitbox: Hitbox,
    pub text_origin: Point<Pixels>,
    pub cell: Size<Pixels>,
    pub gutter: Gutter,
    pub focus: Focus,
    pub handle: FocusHandle,
    pub preedit: Option<(Point<Pixels>, ShapedLine)>,
    pub bar: Bar,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Over {
    Text,
    Track,
    Outside,
}

fn over(text: &Hitbox, track: &Hitbox, window: &Window) -> Over {
    match (text.is_hovered(window), track.is_hovered(window)) {
        (true, _) => Over::Text,
        (false, true) => Over::Track,
        (false, false) => Over::Outside,
    }
}
/// Cell under `position`, clamped to the text area's top-left.
fn cell_at(origin: Point<Pixels>, cell: Size<Pixels>, position: Point<Pixels>) -> (u16, u16) {
    let row = ((position.y - origin.y) / cell.height).floor().max(0.0);
    let col = ((position.x - origin.x) / cell.width).floor().max(0.0);
    (row as u16, col as u16)
}

/// How a row reuses the previous frame's shaped lines, by index.
enum Reuse {
    Same(usize),
    /// New colours or decorations over `Glyphs`.
    Restyle(Glyphs),
}

enum Glyphs {
    /// The text and fonts match; only colours differ.
    Kept(usize),
    Shape,
}

/// Byte lengths per font; runs that differ only in colour shape the same glyphs.
fn font_spans(runs: &[display::Run]) -> impl Iterator<Item = (usize, Modifiers)> + '_ {
    let font = |run: &display::Run| run.style.modifiers & (Modifiers::BOLD | Modifiers::ITALIC);
    runs.chunk_by(move |a, b| font(a) == font(b))
        .map(move |chunk| (chunk.iter().map(|run| run.len).sum(), font(&chunk[0])))
}

pub fn text_run(theme: &Theme, fonts: &Fonts, run: &display::Run) -> TextRun {
    let fg = run
        .style
        .fg
        .map_or(hsla(theme.text), |slot| hsla(theme.syntax[slot]));
    TextRun {
        len: run.len,
        font: fonts.mono_font(run.style.modifiers).clone(),
        color: fg,
        background_color: None,
        underline: run
            .style
            .modifiers
            .contains(Modifiers::UNDERLINED)
            .then(|| UnderlineStyle {
                color: Some(fg),
                thickness: px(1.),
                wavy: false,
            }),
        strikethrough: None,
    }
}

impl Element for TextElement {
    type RequestLayoutState = ();
    type PrepaintState = Prepaint;

    fn id(&self) -> Option<ElementId> {
        let name = match self.part {
            Part::Text => "text",
            Part::Head => "diff-head",
            Part::Buffer => "diff-buffer",
        };
        Some(ElementId::NamedInteger(
            name.into(),
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
        let _span = tracing::trace_span!("text prepaint").entered();
        let (pane, part) = (self.pane, self.part);
        let workspace = self.workspace.read(cx);
        let (theme, fonts, state) = (&workspace.theme, &workspace.fonts, &workspace.state);
        let cell = fonts.cell;
        let Drawing {
            draws,
            gutter,
            lines: len_lines,
        } = draws::drawing(state, pane, part);
        let text_origin = point(
            bounds.origin.x + cell.width * gutter.cols as f32,
            bounds.origin.y,
        );
        let text_width = bounds.size.width - SCROLLBAR_WIDTH;
        let viewport = Viewport {
            rows: (bounds.size.height / cell.height).floor().max(0.0) as u16,
            cols: ((text_width / cell.width).floor().max(0.0) as u16).saturating_sub(gutter.cols),
        };
        let measured = match draws {
            Draws::Editable { view, .. } | Draws::Deleted { view } => Some(view.scroll.viewport),
            Draws::Head { .. } | Draws::NoHead | Draws::Absent => None,
        };
        let focus = match (draws, state.layout.focus() == pane) {
            (Draws::Head { .. } | Draws::NoHead, _) => Focus::ReadOnly,
            (Draws::Editable { .. } | Draws::Deleted { .. } | Draws::Absent, true) => {
                Focus::Focused(state.input.mode)
            }
            (Draws::Editable { .. } | Draws::Deleted { .. } | Draws::Absent, false) => {
                Focus::Unfocused
            }
        };
        let hitbox = window.insert_hitbox(
            Bounds::new(bounds.origin, size(text_width, bounds.size.height)),
            HitboxBehavior::Normal,
        );
        let track = Bounds::new(
            point(bounds.right() - SCROLLBAR_WIDTH, bounds.origin.y),
            size(SCROLLBAR_WIDTH, bounds.size.height),
        );
        let bar_hitbox = window.insert_hitbox(track, HitboxBehavior::Normal);
        let id = id.expect("text element has an id");
        let prepaint = window.with_element_state(id, |cache: Option<Cache>, window| {
            let mut cache = cache.unwrap_or_default();
            cache.frames.swap(0, 1);
            cache.lines.swap(0, 1);
            cache.numbers.swap(0, 1);
            let [frame, previous] = &mut cache.frames;
            let languages = &state.languages;
            let tracked = match draws {
                Draws::Editable {
                    view,
                    doc,
                    rows,
                    tracked,
                } => {
                    let text = doc.text.slice(..);
                    let drawn = scroll::fit(
                        doc,
                        &view.selection,
                        view.scroll,
                        viewport,
                        rows.padding(),
                        languages,
                    );
                    display::layout(
                        frame,
                        doc,
                        &view.selection,
                        drawn,
                        languages,
                        focus,
                        &scroll::annotations(text, rows.padding()),
                        rows.shades(),
                    );
                    match rows {
                        Rows::Conflicts(conflicts) => {
                            conflict::label(frame, text, conflicts, viewport.rows)
                        }
                        Rows::Diff(_) => {}
                    }
                    tracked
                }
                Draws::Head { doc, top, side } => {
                    display::layout(
                        frame,
                        doc,
                        &Selection::point(0),
                        top,
                        languages,
                        focus,
                        &scroll::annotations(doc.text.slice(..), side.padding()),
                        &side.shades,
                    );
                    None
                }
                Draws::Deleted { .. } | Draws::NoHead => {
                    display::blank(frame, viewport.rows);
                    None
                }
                Draws::Absent => {
                    display::blank(frame, 0);
                    None
                }
            };

            let span = tracing::trace_span!("shape rows").entered();
            let text_system = window.text_system();
            let [lines, old_lines] = &mut cache.lines;
            lines.clear();
            // Rows usually shift together, so the match after the last one is tried first.
            let mut hint = 0;
            for row in &frame.rows {
                let text = &frame.text[row.text.clone()];
                let runs = &frame.runs[row.runs.clone()];
                let same = |old: &display::Row| {
                    &previous.text[old.text.clone()] == text
                        && &previous.runs[old.runs.clone()] == runs
                };
                let same_glyphs = |old: &display::Row| {
                    &previous.text[old.text.clone()] == text
                        && font_spans(&previous.runs[old.runs.clone()]).eq(font_spans(runs))
                };
                let reuse = match previous.rows.get(hint) {
                    Some(old) if same(old) => Reuse::Same(hint),
                    Some(_) | None => match previous.rows.iter().position(same) {
                        Some(at) => Reuse::Same(at),
                        None => Reuse::Restyle(
                            previous
                                .rows
                                .iter()
                                .position(same_glyphs)
                                .map_or(Glyphs::Shape, Glyphs::Kept),
                        ),
                    },
                };
                let line = match reuse {
                    Reuse::Same(at) => {
                        hint = at + 1;
                        old_lines[at].clone()
                    }
                    Reuse::Restyle(glyphs) => {
                        cache.runs.clear();
                        cache
                            .runs
                            .extend(runs.iter().map(|run| text_run(theme, fonts, run)));
                        match glyphs {
                            Glyphs::Kept(at) => {
                                hint = at + 1;
                                Rc::new(old_lines[at].with_runs(&cache.runs))
                            }
                            Glyphs::Shape => Rc::new(text_system.shape_line(
                                SharedString::from(text.to_owned()),
                                fonts.mono_size,
                                &cache.runs,
                                None,
                            )),
                        }
                    }
                };
                lines.push(line);
            }

            let numbering = match focus {
                Focus::Focused(Mode::Normal | Mode::Select) => Numbering::Relative,
                Focus::Focused(Mode::Insert) | Focus::Unfocused | Focus::ReadOnly => {
                    Numbering::Absolute
                }
            };
            let [numbers, old_numbers] = &mut cache.numbers;
            numbers.clear();
            for (index, row) in frame.rows.iter().enumerate() {
                match row.kind {
                    RowKind::First => {
                        let label = display::line_label(row.line, frame.primary_line, numbering);
                        let old = old_numbers.iter().find(|old| {
                            old.label == label
                                && old.mark == row.mark
                                && old.digits == gutter.digits
                        });
                        let line = match old {
                            Some(old) => old.line.clone(),
                            None => {
                                cache.label.clear();
                                let _ = write!(
                                    cache.label,
                                    "{label:>width$}",
                                    width = gutter.digits as usize
                                );
                                let number = match row.mark {
                                    LineMark::Cursor => theme.editor.line_number_cursor,
                                    LineMark::Plain => theme.editor.line_number,
                                };
                                let run = TextRun {
                                    len: cache.label.len(),
                                    font: fonts.mono[0].clone(),
                                    color: hsla(number),
                                    background_color: None,
                                    underline: None,
                                    strikethrough: None,
                                };
                                Rc::new(text_system.shape_line(
                                    SharedString::from(cache.label.clone()),
                                    fonts.mono_size,
                                    &[run],
                                    None,
                                ))
                            }
                        };
                        numbers.push(Number {
                            row: index as u16,
                            label,
                            mark: row.mark,
                            digits: gutter.digits,
                            line,
                        });
                    }
                    RowKind::Wrapped | RowKind::Virtual(_) => {}
                }
            }
            drop(span);

            cache.signs.clear();
            cache.signs.extend(frame.rows.iter().map(|row| {
                match (
                    tracked.and_then(|tracked| diff::sign_at(tracked, row.line)),
                    row.kind,
                ) {
                    (Some((DiffSign::Removed, _)), RowKind::Wrapped)
                    | (_, RowKind::Virtual(_))
                    | (None, _) => None,
                    (Some(sign), RowKind::First | RowKind::Wrapped) => Some(sign),
                }
            }));

            let visible = match (frame.rows.first(), frame.rows.last()) {
                (Some(first), Some(last)) => first.line..last.line + 1,
                (None, _) | (_, None) => 0..len_lines,
            };
            let bar = Bar {
                hitbox: bar_hitbox,
                scrollbar: match draws {
                    Draws::Head { .. } | Draws::NoHead => Scrollbar::Hidden,
                    Draws::Editable { .. } | Draws::Deleted { .. } | Draws::Absent => {
                        scroll::scrollbar(
                            len_lines,
                            visible.clone(),
                            f32::from(track.size.height),
                            MIN_THUMB,
                        )
                    }
                },
                len_lines,
                shown: visible.len(),
                page: viewport.rows as isize,
            };

            let primary = frame.cursors.iter().find(|c| c.rank == Rank::Primary);
            let preedit = match (focus, workspace.preedit.is_empty(), primary) {
                (Focus::Focused(_), false, Some(cursor)) => {
                    let run = TextRun {
                        len: workspace.preedit.len(),
                        font: fonts.mono[0].clone(),
                        color: hsla(theme.text),
                        background_color: Some(hsla(theme.background)),
                        underline: Some(UnderlineStyle {
                            color: Some(hsla(theme.text)),
                            thickness: px(1.),
                            wavy: false,
                        }),
                        strikethrough: None,
                    };
                    let line = text_system.shape_line(
                        SharedString::from(workspace.preedit.clone()),
                        fonts.mono_size,
                        &[run],
                        None,
                    );
                    let origin = point(
                        text_origin.x + cell.width * cursor.col as f32,
                        text_origin.y + cell.height * cursor.row as f32,
                    );
                    Some((origin, line))
                }
                (Focus::Focused(_) | Focus::Unfocused | Focus::ReadOnly, ..) => None,
            };
            let prepaint = Prepaint {
                hitbox,
                text_origin,
                cell,
                gutter,
                focus,
                handle: workspace.focus.clone(),
                preedit,
                bar,
            };
            (prepaint, cache)
        });
        // This frame drew at the new size through `scroll::fit`; the state hears of it after.
        match measured {
            Some(drawn) if drawn != viewport => {
                let workspace = self.workspace.clone();
                window.defer(cx, move |_, cx| {
                    workspace.update(cx, |workspace, cx| {
                        scroll::measured(&mut workspace.state, pane, viewport);
                        workspace.flush(cx);
                        cx.notify();
                    })
                });
            }
            Some(_) | None => {}
        }
        prepaint
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
        let _span = tracing::trace_span!("text paint").entered();
        let Prepaint {
            hitbox,
            text_origin,
            cell,
            gutter,
            focus,
            handle,
            preedit,
            bar,
        } = prepaint;
        let (origin, cell, gutter, focus) = (*text_origin, *cell, *gutter, *focus);
        let theme = &self.workspace.read(cx).theme;
        let (editor, rows, tones, staged) = (theme.editor, theme.rows, theme.tones, theme.staged);
        let pane = self.pane;
        let id = id.expect("text element has an id");
        let pointer = window.with_element_state(id, |cache: Option<Cache>, window| {
            let cache = cache.unwrap_or_default();
            let frame = &cache.frames[0];
            let y = |row: u16| origin.y + cell.height * row as f32;
            let x = |col: u16| origin.x + cell.width * col as f32;
            let text_width = hitbox.bounds.right() - origin.x;

            match focus {
                Focus::Focused(_) => {
                    for (index, row) in frame.rows.iter().enumerate() {
                        match (row.line == frame.primary_line, row.kind) {
                            (_, RowKind::Virtual(_)) | (false, _) => {}
                            (true, RowKind::First | RowKind::Wrapped) => window.paint_quad(fill(
                                Bounds::new(
                                    point(origin.x, y(index as u16)),
                                    size(text_width, cell.height),
                                ),
                                hsla(editor.cursorline),
                            )),
                        }
                    }
                }
                Focus::Unfocused | Focus::ReadOnly => {}
            }
            for (index, row) in frame.rows.iter().enumerate() {
                match row.background {
                    Some(role) => window.paint_quad(fill(
                        Bounds::new(
                            point(origin.x, y(index as u16)),
                            size(text_width, cell.height),
                        ),
                        hsla(rows[role]),
                    )),
                    None => {}
                }
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
                    hsla(editor.selection),
                ));
            }
            let cursor_bg = |rank: Rank| match rank {
                Rank::Primary => hsla(editor.cursor_primary),
                Rank::Secondary => hsla(editor.cursor_secondary),
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
            for (index, line) in cache.lines[0].iter().enumerate() {
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
            for number in &cache.numbers[0] {
                let _ = number.line.paint(
                    point(number_x, y(number.row)),
                    cell.height,
                    TextAlign::Left,
                    None,
                    window,
                    cx,
                );
            }
            let diff_x = bounds.origin.x + cell.width * (gutter.cols - 1) as f32;
            for (index, sign) in cache.signs.iter().enumerate() {
                let top = y(index as u16);
                let (sign, stage) = match sign {
                    Some(sign) => *sign,
                    None => continue,
                };
                let (tone, extent) = match sign {
                    DiffSign::Added => (Tone::Added, size(cell.width / 4., cell.height)),
                    DiffSign::Modified => (Tone::Modified, size(cell.width / 4., cell.height)),
                    DiffSign::Removed => (Tone::Removed, size(cell.width, px(2.))),
                };
                // Staged changes are dimmed; partly staged ones still need attention.
                let color = hsla(match stage {
                    Stage::Staged => staged[tone],
                    Stage::Unstaged | Stage::Partial => tones[tone],
                });
                window.paint_quad(fill(Bounds::new(point(diff_x, top), extent), color));
            }

            let track = bar.hitbox.bounds;
            match bar.scrollbar {
                Scrollbar::Hidden => {}
                Scrollbar::Shown(thumb) => {
                    window.paint_quad(fill(track, hsla(editor.scrollbar_track)));
                    let thumb_color = match cache.pointer.drag.get() {
                        Drag::Thumb { .. } => editor.scrollbar_thumb_dragged,
                        Drag::Idle | Drag::Selecting(_) => editor.scrollbar_thumb,
                    };
                    window.paint_quad(fill(
                        Bounds::new(
                            point(track.origin.x, track.origin.y + px(thumb.top)),
                            size(track.size.width, px(thumb.height)),
                        ),
                        hsla(thumb_color),
                    ));
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
            Focus::Unfocused | Focus::ReadOnly => {}
        }

        let track = bar.hitbox.bounds;
        let (scrollbar, len_lines, shown, page) =
            (bar.scrollbar, bar.len_lines, bar.shown, bar.page);
        let part = self.part;
        window.on_mouse_event({
            let workspace = self.workspace.clone();
            let hitbox = hitbox.clone();
            let bar_hitbox = bar.hitbox.clone();
            let pointer = pointer.clone();
            move |event: &MouseDownEvent, phase, window, cx| match (
                phase,
                event.button,
                over(&hitbox, &bar_hitbox, window),
                part,
            ) {
                // The HEAD side has no cursor; a click moves the buffer side's to the facing line.
                (DispatchPhase::Bubble, MouseButton::Left, Over::Text, Part::Head) => {
                    let (row, col) = cell_at(origin, cell, event.position);
                    workspace.update(cx, |workspace, cx| {
                        diffview::click_left(&mut workspace.state, pane, row, col);
                        window.focus(&workspace.focus, cx);
                        cx.notify();
                    });
                }
                (
                    DispatchPhase::Bubble,
                    MouseButton::Left,
                    Over::Text,
                    Part::Text | Part::Buffer,
                ) => {
                    let (row, col) = cell_at(origin, cell, event.position);
                    let press = match event.modifiers.alt {
                        true => Press::AddCursor,
                        false => Press::Select(Unit::from_clicks(event.click_count)),
                    };
                    workspace.update(cx, |workspace, cx| {
                        let gesture = scroll::press(&mut workspace.state, pane, row, col, press);
                        pointer
                            .drag
                            .set(gesture.map_or(Drag::Idle, Drag::Selecting));
                        window.focus(&workspace.focus, cx);
                        cx.notify();
                    });
                }
                (
                    DispatchPhase::Bubble,
                    MouseButton::Middle,
                    Over::Text,
                    Part::Text | Part::Buffer,
                ) => {
                    let (row, col) = cell_at(origin, cell, event.position);
                    workspace.update(cx, |workspace, cx| {
                        scroll::middle_click(
                            &mut workspace.state,
                            pane,
                            row,
                            col,
                            &mut workspace.effects,
                        );
                        workspace.flush(cx);
                        window.focus(&workspace.focus, cx);
                        cx.notify();
                    });
                }
                (DispatchPhase::Bubble, MouseButton::Left, Over::Track, _) => match scrollbar {
                    Scrollbar::Shown(thumb) => {
                        match thumb.hit(f32::from(event.position.y - track.origin.y)) {
                            OnTrack::Thumb { grab } => {
                                pointer.drag.set(Drag::Thumb { grab: px(grab) })
                            }
                            OnTrack::Above => workspace.update(cx, |workspace, cx| {
                                scroll::wheel(&mut workspace.state, pane, -page);
                                cx.notify();
                            }),
                            OnTrack::Below => workspace.update(cx, |workspace, cx| {
                                scroll::wheel(&mut workspace.state, pane, page);
                                cx.notify();
                            }),
                        }
                    }
                    Scrollbar::Hidden => {}
                },
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
                scrollbar,
            ) {
                (DispatchPhase::Bubble, Some(MouseButton::Left), Drag::Selecting(gesture), _) => {
                    let (row, col) = cell_at(origin, cell, event.position);
                    workspace.update(cx, |workspace, cx| {
                        scroll::drag(&mut workspace.state, pane, row, col, gesture);
                        cx.notify();
                    });
                }
                (
                    DispatchPhase::Bubble,
                    Some(MouseButton::Left),
                    Drag::Thumb { grab },
                    Scrollbar::Shown(thumb),
                ) => {
                    let top = event.position.y - track.origin.y - grab;
                    let line = scroll::line_at_thumb(
                        len_lines,
                        shown,
                        thumb.height,
                        f32::from(track.size.height),
                        f32::from(top),
                    );
                    workspace.update(cx, |workspace, cx| {
                        scroll::scroll_to_line(&mut workspace.state, pane, line);
                        cx.notify();
                    });
                }
                _ => {}
            }
        });
        window.on_mouse_event({
            let workspace = self.workspace.clone();
            let pointer = pointer.clone();
            move |event: &MouseUpEvent, phase, _window, cx| match (
                phase,
                event.button,
                pointer.drag.get(),
            ) {
                (DispatchPhase::Bubble, MouseButton::Left, Drag::Selecting(_)) => {
                    pointer.drag.set(Drag::Idle);
                    workspace.update(cx, |workspace, cx| {
                        scroll::release(&mut workspace.state, pane, &mut workspace.effects);
                        workspace.flush(cx);
                    })
                }
                (DispatchPhase::Bubble, MouseButton::Left, Drag::Thumb { .. }) => {
                    pointer.drag.set(Drag::Idle);
                    workspace.update(cx, |_, cx| cx.notify())
                }
                _ => {}
            }
        });
        window.on_mouse_event({
            let workspace = self.workspace.clone();
            let hitbox = hitbox.clone();
            let bar_hitbox = bar.hitbox.clone();
            move |event: &ScrollWheelEvent, phase, window, cx| match (
                phase,
                over(&hitbox, &bar_hitbox, window),
            ) {
                (DispatchPhase::Bubble, Over::Text | Over::Track) => {
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
