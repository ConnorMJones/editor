//! The status line under each pane.

use gpui::{Div, div, prelude::*};
use ink::doc::{Drawing, ShapeKind, Visibility};
use ink::tool::{Gather, ToolKind};
use model::command::Shown;
use model::mode::Mode;
use model::sheet::{Fit, Zoom};
use state::State;
use state::draft::{Draft, Edits};
use state::drawing::{InkTabs, PdfMode, PdfTab};
use state::git::conflict;
use state::git::diff::{self, Bases, Removal, Side};
use state::json_canvas::Doing;
use state::reading::{Found, Reading, bookmark_in_view};
use state::text::buffer::{Disk, Highlight};
use state::view::layout::{Pane, PaneId};
use state::view::tab::{Opened, Tab};

use super::Workspace;
use crate::fonts::hsla;

struct Status {
    label: Label,
    file: String,
    flags: String,
    /// Right-aligned, in order.
    parts: [Part; 4],
}

#[derive(Clone, Copy)]
enum Label {
    Mode,
    Ink,
    Read,
    View,
}

struct Part {
    text: String,
    tone: Tone,
}

#[derive(Clone, Copy)]
enum Tone {
    Plain,
    Alert,
}

impl Part {
    fn found(found: &Found) -> Part {
        match found {
            Found::Hits { hits, at, .. } => Part::plain(format!("hit {}/{}", at + 1, hits.len())),
            Found::Searching { .. } => Part::plain("searching".to_owned()),
            Found::Nothing => Part::NONE,
        }
    }

    fn plain(text: String) -> Part {
        Part {
            text,
            tone: Tone::Plain,
        }
    }

    const NONE: Part = Part {
        text: String::new(),
        tone: Tone::Plain,
    };
}

impl Workspace {
    /// Helix statusline: mode, file name, modification; selections, position, encoding.
    pub fn statusline(&self, pane: PaneId) -> Div {
        let theme = &self.theme;
        let fonts = &self.fonts;
        let state = &self.state;
        let focused = pane == state.layout.focus();
        let active = state.layout.panes().get(pane).and_then(Pane::active);
        let marked = (active.and_then(|tab| bookmark_in_view(tab, &state.pdfs, &state.books)))
            .map_or("", |_| " \u{25cf}");
        let status = match active {
            Some(Tab::Ink(ink)) => {
                let opened = ink.opened();
                let (tip, layer, layers, disk, place) = match ink {
                    InkTabs::Paged(tab) => {
                        let file = &state.pageds[tab.file()];
                        let pages = file.edited.drawing().stack().sizes().len();
                        let place = format!(
                            "page {}/{pages}  {}",
                            tab.view.centre.page.0 + 1,
                            scale(tab.view.fit, tab.view.zoom)
                        );
                        let layers = file.edited.drawing().layers();
                        (tab.hand.tip, tab.hand.layer, layers, file.disk, place)
                    }
                    InkTabs::Pdf(tab) => {
                        let pdf = &state.pdfs[tab.file()];
                        let file = &pdf.ink;
                        let pages = file.edited.drawing().stack().sizes().len();
                        let focus = tab.view.centre.page;
                        let place = format!(
                            "page {}/{pages}{marked}  {}",
                            focus.0 + 1,
                            scale(tab.view.fit, tab.view.zoom)
                        );
                        let layers = file.edited.drawing().layers();
                        (tab.hand.tip, tab.hand.layer, layers, file.disk, place)
                    }
                    InkTabs::Board(tab) => {
                        let file = &state.boards[tab.file()];
                        let layers = file.edited.drawing().layers();
                        let place = percent(tab.view.zoom);
                        (tab.hand.tip, tab.hand.layer, layers, file.disk, place)
                    }
                };
                let tool = match tip {
                    ToolKind::Brush => "draw",
                    ToolKind::Eraser => "erase",
                    ToolKind::Select(Gather::Box) => "select",
                    ToolKind::Select(Gather::Lasso) => "lasso",
                    ToolKind::Shape(ShapeKind::Line) => "line",
                    ToolKind::Shape(ShapeKind::Arrow) => "arrow",
                    ToolKind::Shape(ShapeKind::Rectangle) => "rectangle",
                    ToolKind::Shape(ShapeKind::Ellipse) => "ellipse",
                    ToolKind::Text => "text",
                };
                let label = match ink {
                    InkTabs::Pdf(PdfTab {
                        mode: PdfMode::Read(Reading::Writing(_)),
                        ..
                    }) => Label::Mode,
                    InkTabs::Pdf(PdfTab {
                        mode: PdfMode::Read(_),
                        ..
                    }) => Label::Read,
                    InkTabs::Pdf(PdfTab {
                        mode: PdfMode::Markup,
                        ..
                    })
                    | InkTabs::Paged(_)
                    | InkTabs::Board(_) => Label::Ink,
                };
                let tool = match label {
                    Label::Read => Part::NONE,
                    Label::Ink | Label::Mode | Label::View => Part::plain(tool.to_owned()),
                };
                Status {
                    label,
                    file: name(state, opened),
                    flags: format!(
                        "{}{}{}",
                        modified_flag(state, opened),
                        active
                            .and_then(Tab::drafting)
                            .map_or("", |(draft, _)| draft_flag(draft)),
                        disk_flag(disk)
                    ),
                    parts: [
                        tool,
                        Part::plain({
                            match layers.get(layer) {
                                Some(shown) => {
                                    let hidden = match shown.visibility {
                                        Visibility::Shown => "",
                                        Visibility::Hidden => " (hidden)",
                                    };
                                    format!(
                                        "{} {}/{}{hidden}",
                                        shown.name.as_str(),
                                        layer.0 + 1,
                                        layers.len()
                                    )
                                }
                                None => String::new(),
                            }
                        }),
                        Part::plain(place),
                        match ink {
                            InkTabs::Pdf(PdfTab { found, .. }) => Part::found(found),
                            InkTabs::Paged(_) | InkTabs::Board(_) => Part::NONE,
                        },
                    ],
                }
            }
            Some(Tab::Text(shown)) => {
                let view = shown.text();
                let opened = Opened::Text(view.buffer());
                let state::text::buffer::Buffer {
                    doc, disk, diff, ..
                } = &state.buffers[view.buffer()];
                let cursor = view.selection.primary().cursor(doc.text.slice(..));
                let line = doc.text.char_to_line(cursor);
                let column = cursor - doc.text.line_to_char(line);
                let highlight = match doc.highlight {
                    Highlight::Failed { .. } => " [parse failed]",
                    Highlight::Plain | Highlight::Parsing { .. } | Highlight::Parsed(_) => "",
                };
                let selections = match view.selection.len() {
                    1 => "1 sel".to_owned(),
                    n => format!("{n} sels"),
                };
                let hunks = match (shown.shown(), &diff.bases) {
                    (Shown::Source, _) => String::new(),
                    (Shown::Render, _) => "rendered".to_owned(),
                    (Shown::Split, _) => "split".to_owned(),
                    (Shown::Diff, Bases::Tracked(tracked)) => {
                        let at = diff::hunk_at(
                            &tracked.hunks,
                            Side::After,
                            line as u32,
                            Removal::Include,
                        );
                        match (at, tracked.hunks.len()) {
                            (_, 0) => "diff: no changes".to_owned(),
                            (Some(at), count) => format!("diff: hunk {}/{count}", at + 1),
                            (None, count) => format!("diff: {count} hunks"),
                        }
                    }
                    (Shown::Diff, Bases::Untracked) => "diff: untracked".to_owned(),
                    (Shown::Diff, Bases::NoRepo) => "diff: not in a repository".to_owned(),
                    (Shown::Diff, Bases::Loading { .. }) => "diff".to_owned(),
                };
                let list = &conflict::current(doc).list;
                let at = list.partition_point(|conflict| conflict.close < line);
                let conflicts = match (list.len(), list.get(at)) {
                    (0, _) => String::new(),
                    (count, Some(inside)) if inside.open <= line => {
                        format!("conflict {}/{count}", at + 1)
                    }
                    (1, _) => "1 conflict".to_owned(),
                    (count, _) => format!("{count} conflicts"),
                };
                Status {
                    label: Label::Mode,
                    file: name(state, opened),
                    flags: format!(
                        "{}{}{highlight}",
                        modified_flag(state, opened),
                        disk_flag(*disk)
                    ),
                    parts: [
                        Part::plain(hunks),
                        Part {
                            text: conflicts,
                            tone: Tone::Alert,
                        },
                        Part::plain(selections),
                        Part::plain(format!("{}:{}", line + 1, column + 1)),
                    ],
                }
            }
            Some(Tab::Image(tab)) => {
                let opened = Opened::Image(tab.file());
                let file = &state.images[tab.file()];
                let size = file.stack.sizes()[0];
                Status {
                    label: Label::View,
                    file: name(state, opened),
                    flags: disk_flag(file.disk).to_owned(),
                    parts: [
                        Part::plain(format!(
                            "{}×{}",
                            size.width.get().round(),
                            size.height.get().round()
                        )),
                        Part::plain(scale(tab.view.fit, tab.view.zoom)),
                        Part::NONE,
                        Part::NONE,
                    ],
                }
            }
            Some(Tab::JsonCanvas(tab)) => {
                let file = &state.json_canvases[tab.file()];
                let (label, draft) = match &tab.doing {
                    Doing::Writing(writing) => (Label::Mode, draft_flag(&writing.draft)),
                    Doing::Arranging(_) => (Label::View, ""),
                };
                let opened = Opened::JsonCanvas(tab.file());
                Status {
                    label,
                    file: name(state, opened),
                    flags: format!(
                        "{}{}{}",
                        modified_flag(state, opened),
                        draft,
                        disk_flag(file.disk)
                    ),
                    parts: [
                        Part::plain(format!("{} cards", file.canvas().nodes().len())),
                        Part::plain(percent(tab.view.zoom)),
                        Part::NONE,
                        Part::NONE,
                    ],
                }
            }
            Some(Tab::Book(tab)) => {
                let file = &state.books[tab.file()];
                let items = file.flow.flow.roots.len();
                let read = (tab.scroll.at().item + 1) * 100 / items;
                let (label, flags) = match &tab.reading {
                    Reading::Writing(writing) => (Label::Mode, draft_flag(&writing.draft)),
                    Reading::Idle
                    | Reading::Selecting { .. }
                    | Reading::Selected(_)
                    | Reading::OnHighlight(_) => (Label::Read, ""),
                };
                Status {
                    label,
                    file: name(state, Opened::Book(tab.file())),
                    flags: format!("{flags}{}", disk_flag(file.disk)),
                    parts: [
                        Part::plain(file.title.clone()),
                        Part::plain(format!("{read}%{marked}")),
                        Part::found(&tab.found),
                        Part::NONE,
                    ],
                }
            }
            None => Status {
                label: Label::Mode,
                file: "[scratch]".to_owned(),
                flags: String::new(),
                parts: [Part::NONE, Part::NONE, Part::NONE, Part::NONE],
            },
        };
        let mode = state.input.mode;
        let (label, label_bg) = match status.label {
            Label::Mode => (mode.label(), theme.mode[mode]),
            Label::Ink => ("INK", theme.mode[Mode::Normal]),
            Label::Read => ("READ", theme.mode[Mode::Normal]),
            Label::View => ("VIEW", theme.mode[Mode::Normal]),
        };
        let (label, label_bg, label_fg) = match focused {
            true => (label, hsla(label_bg), hsla(theme.mode.fg)),
            false => ("   ", hsla(theme.surface), hsla(theme.muted)),
        };
        let pending: String = match focused {
            true => state
                .input
                .pending
                .as_slice()
                .iter()
                .map(|key| key.to_string())
                .collect(),
            false => String::new(),
        };
        let text = match focused {
            true => hsla(theme.text),
            false => hsla(theme.muted),
        };
        let parts = status.parts.into_iter().map(|part| {
            let tone = match part.tone {
                Tone::Plain => text,
                Tone::Alert => hsla(theme.chrome.close_hover),
            };
            div().px_2().text_color(tone).child(part.text)
        });
        div()
            .flex_none()
            .h(fonts.ui_line)
            .flex()
            .flex_row()
            .items_center()
            .bg(hsla(theme.surface))
            .text_color(text)
            .font(fonts.ui.clone())
            .text_size(fonts.ui_size)
            .child(div().px_2().bg(label_bg).text_color(label_fg).child(label))
            .child(div().pl_2().child(status.file))
            .child(
                div()
                    .pl_1()
                    .text_color(hsla(theme.chrome.unsaved))
                    .child(status.flags),
            )
            .child(div().flex_1())
            .child(div().px_2().text_color(hsla(theme.muted)).child(pending))
            .children(parts)
    }
}

/// The file's name under the project root.
fn name(state: &State, opened: Opened) -> String {
    state.project.root.label(state.path_of(opened))
}

fn disk_flag(disk: Disk) -> &'static str {
    match disk {
        Disk::Synced { .. } | Disk::New => "",
        Disk::ChangedExternally { .. } => " [changed on disk]",
        Disk::Deleted => " [deleted]",
    }
}

fn draft_flag(draft: &Draft) -> &'static str {
    match draft.edits() {
        Edits::Unsaved => "[+]",
        Edits::Saved => "",
    }
}

fn modified_flag(state: &State, opened: Opened) -> &'static str {
    match state.is_modified(opened) {
        true => "[+]",
        false => "",
    }
}

fn percent(zoom: Zoom) -> String {
    format!("{:.0}%", zoom.get() * 100.0)
}

fn scale(fit: Fit, zoom: Zoom) -> String {
    let fit = match fit {
        Fit::Width => "width",
        Fit::Height => "height",
        Fit::Page => "page",
    };
    format!("{fit} {}", percent(zoom))
}
