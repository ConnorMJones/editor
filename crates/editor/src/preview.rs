use std::path::Path;

use gpui::{
    App, Bounds, Element, ElementId, Entity, GlobalElementId, InspectorElementId, IntoElement,
    LayoutId, Pixels, ShapedLine, SharedString, Style, TextAlign, TextRun, Window, fill, point,
    relative, size,
};
use helix_core::Selection;
use helix_core::text_annotations::TextAnnotations;
use model::theme as roles;
use state::Overlay;
use state::buffer::{Content, TextDoc};
use state::display::{self, CursorShape, Focus, Frame};
use state::layout::{Pane, Scroll, Viewport};
use state::picker::{self, Preview, PreviewSource};

use crate::workspace::Workspace;

/// Read-only view of the picker's highlighted item around its target line.
pub struct PreviewElement {
    pub workspace: Entity<Workspace>,
}

impl IntoElement for PreviewElement {
    type Element = Self;

    fn into_element(self) -> Self {
        self
    }
}

/// Buffers kept across frames so drawing reuses its allocations.
#[derive(Default)]
pub struct Cache {
    pub frame: Frame,
    pub lines: Vec<ShapedLine>,
    pub runs: Vec<TextRun>,
}

pub struct Prepaint {
    pub cell: gpui::Size<Pixels>,
    /// Document line drawn highlighted; `None` for messages.
    pub target: Option<usize>,
}

/// What the preview area shows this frame.
enum Shown<'a> {
    Doc { doc: &'a TextDoc, line: usize },
    Message(&'static str),
}

impl Element for PreviewElement {
    type RequestLayoutState = ();
    type PrepaintState = Prepaint;

    fn id(&self) -> Option<ElementId> {
        Some("picker-preview".into())
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
        let id = id.expect("preview element has an id");
        window.with_element_state(id, |cache: Option<Cache>, window| {
            let mut cache = cache.unwrap_or_default();
            let workspace = self.workspace.read(cx);
            let theme = &workspace.theme;
            let state = &workspace.state;
            let cell = theme.cell;
            let viewport = Viewport {
                rows: (bounds.size.height / cell.height).floor().max(0.0) as u16,
                cols: (bounds.size.width / cell.width).floor().max(0.0) as u16,
            };
            let root = state.root.path();
            let shown = match &state.overlay {
                Overlay::Picker(open) => match picker::preview_source(open) {
                    None => Shown::Message(""),
                    Some(PreviewSource::Buffer(buffer)) => {
                        match state.buffers.get(buffer).map(|b| &b.content) {
                            Some(Content::Text { doc, .. }) => {
                                let cursor = (state.layout.panes.values())
                                    .flat_map(Pane::items)
                                    .find_map(|item| {
                                        let view = item.view.text();
                                        (view.buffer() == buffer)
                                            .then(|| view.selection.primary().head)
                                    });
                                Shown::Doc {
                                    doc,
                                    line: doc.text.char_to_line(
                                        cursor.unwrap_or(0).min(doc.text.len_chars()),
                                    ),
                                }
                            }
                            Some(Content::Loading { .. }) => Shown::Message("loading…"),
                            None => Shown::Message(""),
                        }
                    }
                    Some(PreviewSource::File { path, line }) => {
                        // An open buffer shows its unsaved edits.
                        let buffered = state.buffers.iter().find(|(_, open_buffer)| {
                            open_buffer.path.strip_prefix(root).ok()
                                == Some(Path::new(path.as_str()))
                        });
                        match (buffered, open.previews.get(path)) {
                            (
                                Some((
                                    _,
                                    state::buffer::Buffer {
                                        content: Content::Text { doc, .. },
                                        ..
                                    },
                                )),
                                _,
                            ) => Shown::Doc { doc, line },
                            (Some(_), _) | (None, None | Some(Preview::Loading)) => {
                                Shown::Message("loading…")
                            }
                            (None, Some(Preview::Text(doc))) => Shown::Doc { doc, line },
                            (None, Some(Preview::Binary)) => Shown::Message("<binary file>"),
                            (None, Some(Preview::TooLarge)) => {
                                Shown::Message("<file too large to preview>")
                            }
                            (None, Some(Preview::Failed(_))) => {
                                Shown::Message("<file cannot be read>")
                            }
                        }
                    }
                },
                Overlay::None | Overlay::Prompt(_) => Shown::Message(""),
            };

            let text_system = window.text_system();
            cache.lines.clear();
            let target = match shown {
                Shown::Doc { doc, line } => {
                    let line = line.min(doc.text.len_lines().saturating_sub(1));
                    let selection = Selection::point(doc.text.line_to_char(line));
                    let scroll = Scroll {
                        anchor: doc
                            .text
                            .line_to_char(line.saturating_sub(viewport.rows as usize / 3)),
                        rows: 0,
                        viewport,
                    };
                    let frame = &mut cache.frame;
                    display::layout(
                        frame,
                        doc,
                        &selection,
                        scroll,
                        &state.languages,
                        Focus::Unfocused,
                        &TextAnnotations::default(),
                        &[],
                    );
                    for row in &frame.rows {
                        cache.runs.clear();
                        cache.runs.extend(
                            frame.runs[row.runs.clone()]
                                .iter()
                                .map(|run| crate::text::text_run(theme, run)),
                        );
                        let text = SharedString::from(frame.text[row.text.clone()].to_owned());
                        cache.lines.push(text_system.shape_line(
                            text,
                            theme.mono_size,
                            &cache.runs,
                            None,
                        ));
                    }
                    Some(line)
                }
                Shown::Message(message) => {
                    cache.frame.rows.clear();
                    cache.frame.cursors.clear();
                    let run = TextRun {
                        len: message.len(),
                        font: theme.mono[0].clone(),
                        color: theme.text_muted,
                        background_color: None,
                        underline: None,
                        strikethrough: None,
                    };
                    cache.lines.push(text_system.shape_line(
                        SharedString::new_static(message),
                        theme.mono_size,
                        &[run],
                        None,
                    ));
                    None
                }
            };
            (Prepaint { cell, target }, cache)
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
        let Prepaint { cell, target } = *prepaint;
        let colors = &self.workspace.read(cx).theme.colors;
        let (highlight, cursor_bg) = (colors[roles::CURSORLINE], colors[roles::CURSOR_PRIMARY.bg]);
        let id = id.expect("preview element has an id");
        window.with_element_state(id, |cache: Option<Cache>, window| {
            let cache = cache.unwrap_or_default();
            let y = |row: usize| bounds.origin.y + cell.height * row as f32;
            for (index, row) in cache.frame.rows.iter().enumerate() {
                match Some(row.line) == target {
                    true => window.paint_quad(fill(
                        Bounds::new(
                            point(bounds.origin.x, y(index)),
                            size(bounds.size.width, cell.height),
                        ),
                        highlight,
                    )),
                    false => {}
                }
            }
            // Block cursors swap their cell's text colour, so they are drawn under the text.
            for cursor in &cache.frame.cursors {
                match cursor.shape {
                    CursorShape::Block => window.paint_quad(fill(
                        Bounds::new(
                            point(
                                bounds.origin.x + cell.width * cursor.col as f32,
                                y(cursor.row as usize),
                            ),
                            size(cell.width * cursor.width as f32, cell.height),
                        ),
                        cursor_bg,
                    )),
                    CursorShape::Bar | CursorShape::Underline => {}
                }
            }
            for (index, line) in cache.lines.iter().enumerate() {
                let _ = line.paint(
                    point(bounds.origin.x, y(index)),
                    cell.height,
                    TextAlign::Left,
                    None,
                    window,
                    cx,
                );
            }
            ((), cache)
        });
    }
}
