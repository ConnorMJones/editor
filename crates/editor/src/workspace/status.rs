//! The status line under each pane.

use gpui::{Div, div, prelude::*};
use model::command::Shown;
use state::git::conflict;
use state::git::diff::{self, Bases, Removal, Side};
use state::text::buffer::{Disk, Highlight};
use state::view::layout::{Pane, PaneId};

use super::Workspace;
use crate::fonts::hsla;

impl Workspace {
    /// Helix statusline: mode, file name, modification; selections, position, encoding.
    pub(super) fn statusline(&self, pane: PaneId) -> Div {
        let theme = &self.theme;
        let fonts = &self.fonts;
        let state = &self.state;
        let focused = pane == state.layout.focus();
        let mode = state.input.mode;
        let (file, flags, hunks, conflicts, selections, position) =
            match state.layout.panes().get(pane).and_then(Pane::active) {
                Some(shown) => {
                    let view = shown.text();
                    let buffer = &state.buffers[view.buffer()];
                    let name = buffer
                        .path
                        .strip_prefix(state.project.root.path())
                        .unwrap_or(&buffer.path)
                        .display()
                        .to_string();
                    let state::text::buffer::Buffer {
                        doc, disk, diff, ..
                    } = buffer;
                    let cursor = view.selection.primary().cursor(doc.text.slice(..));
                    let line = doc.text.char_to_line(cursor);
                    let column = cursor - doc.text.line_to_char(line);
                    let marker = match doc.is_modified() {
                        true => "[+]",
                        false => "",
                    };
                    let disk = match disk {
                        Disk::Synced { .. } | Disk::New => "",
                        Disk::ChangedExternally { .. } => " [changed on disk]",
                        Disk::Deleted => " [deleted]",
                    };
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
                                (Some(at), count) => {
                                    format!("diff: hunk {}/{count}", at + 1)
                                }
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
                    (
                        name,
                        format!("{marker}{disk}{highlight}"),
                        hunks,
                        conflicts,
                        selections,
                        format!("{}:{}", line + 1, column + 1),
                    )
                }
                None => (
                    "[scratch]".to_owned(),
                    String::new(),
                    String::new(),
                    String::new(),
                    String::new(),
                    String::new(),
                ),
            };
        // Buffers are always UTF-8; helix shows the encoding only when it differs.
        let (mode_label, mode_bg, mode_fg) = match focused {
            true => (mode.label(), hsla(theme.mode[mode]), hsla(theme.mode.fg)),
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
        div()
            .flex()
            .flex_row()
            .bg(hsla(theme.surface))
            .text_color(text)
            .font(fonts.ui.clone())
            .text_size(fonts.ui_size)
            .child(
                div()
                    .px_2()
                    .bg(mode_bg)
                    .text_color(mode_fg)
                    .child(mode_label),
            )
            .child(div().pl_2().child(file))
            .child(
                div()
                    .pl_1()
                    .text_color(hsla(theme.chrome.unsaved))
                    .child(flags),
            )
            .child(div().flex_1())
            .child(div().px_2().text_color(hsla(theme.muted)).child(pending))
            .child(div().px_2().child(hunks))
            .child(
                div()
                    .px_2()
                    .text_color(hsla(theme.chrome.close_hover))
                    .child(conflicts),
            )
            .child(div().px_2().child(selections))
            .child(div().px_2().child(position))
    }
}
