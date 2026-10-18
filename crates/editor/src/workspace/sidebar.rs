//! The file tree and Changes sidebars, and the tree's context menu and drag preview.

use std::ops::Range;

use gpui::{
    ClickEvent, Context, Deferred, Div, Hsla, IntoElement, MouseButton, MouseDownEvent, Pixels,
    Point, Render, SharedString, Stateful, Window, anchored, canvas, deferred, div, prelude::*, px,
    uniform_list,
};
use model::command::ChangesCommand;
use model::git::{GitStatus, Group, Kind};
use model::theme::Theme;
use model::window::Axis;
use state::git::{Entry, Work};
use state::project::path::RelPath;
use state::sidebar::changes::{self, ListRow};
use state::sidebar::tree::{Fold, RowKind, TreeAction, TreeTarget};
use state::{Focus, Repo, Sidebar, SidebarWidth, view::panes};

use super::pane_tree::{DIVIDER, divider};
use super::{Drag, Workspace};
use crate::fonts::{Fonts, hsla};

pub(super) const INDENT: f32 = 14.0;
/// Sidebar widths in ems of the interface font.
pub(super) const SIDEBAR_DEFAULT: f32 = 20.0;
pub(super) const SIDEBAR_MIN: f32 = 8.0;

pub struct TreeMenu {
    pub target: TreeTarget,
    pub at: Point<Pixels>,
}

/// A file tree entry being dragged to another folder.
pub struct DraggedEntry {
    pub path: RelPath,
}

/// What follows the pointer while dragging a tree entry.
pub struct DragPreview {
    pub name: SharedString,
    pub bg: Hsla,
    pub fg: Hsla,
}

impl Render for DragPreview {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .px_2()
            .rounded_sm()
            .bg(self.bg)
            .text_color(self.fg)
            .child(self.name.clone())
    }
}

impl Workspace {
    pub(super) fn sidebar(&self, focus: Focus, window: Pixels, cx: &mut Context<Self>) -> Div {
        let theme = &self.theme;
        let fonts = &self.fonts;
        let drop_bg = hsla(theme.chrome.selection);
        let border = match focus {
            Focus::Sidebar => hsla(theme.chrome.accent),
            Focus::Editor => hsla(theme.chrome.border),
        };
        let areas = self.areas.clone();
        div()
            .flex_none()
            .h_full()
            .flex()
            .flex_row()
            .child(
                div()
                    .relative()
                    .w(fit_sidebar(
                        match self.state.sidebar_width {
                            SidebarWidth::Default => fonts.ui_size * SIDEBAR_DEFAULT,
                            SidebarWidth::Dragged(width) => px(width.get().0),
                        },
                        fonts,
                        window,
                    ))
                    .h_full()
                    .flex()
                    .flex_col()
                    .bg(hsla(theme.surface))
                    .font(fonts.ui.clone())
                    .text_size(fonts.ui_size)
                    .child(
                        canvas(
                            move |bounds, _, _| areas.sidebar.set(bounds),
                            |_, _, _, _| {},
                        )
                        .absolute()
                        .size_full(),
                    )
                    .children(match (&self.state.sidebar, &self.state.project.git) {
                        (Sidebar::Changes(_), Repo::Found { snapshot, .. }) => {
                            let note = match snapshot.unlisted.len() {
                                0 => None,
                                1 => Some("1 change is not listed; see the log".to_owned()),
                                count => {
                                    Some(format!("{count} changes are not listed; see the log"))
                                }
                            };
                            note.map(|note| {
                                div()
                                    .px_2()
                                    .py_1()
                                    .text_color(hsla(theme.muted))
                                    .child(note)
                            })
                        }
                        (Sidebar::Hidden | Sidebar::Files(_), _)
                        | (Sidebar::Changes(_), Repo::None | Repo::Unknown) => None,
                    })
                    .child(match self.state.sidebar {
                        Sidebar::Changes(_) => uniform_list(
                            "changes",
                            changes::list_len(
                                self.state.project.git.changes(),
                                &self.state.changes.folds,
                            ),
                            cx.processor(move |this, range: Range<usize>, _window, cx| {
                                let state = &this.state;
                                let theme = &this.theme;
                                let entries = state.project.git.changes();
                                let folds = &state.changes.folds;
                                let cursor = changes::cursor_row(
                                    entries,
                                    folds,
                                    state.changes.cursor.as_ref(),
                                );
                                changes::rows(entries, folds)
                                    .skip(range.start)
                                    .zip(range)
                                    .map(|(list_row, row)| match list_row {
                                        ListRow::Header { group, count, fold } => {
                                            let label = match group {
                                                Group::Conflicts => "Conflicts",
                                                Group::Staged => "Staged Changes",
                                                Group::Changes => "Changes",
                                            };
                                            let chevron = match fold {
                                                Fold::Expanded => "▾",
                                                Fold::Collapsed => "▸",
                                            };
                                            let button = match group {
                                                Group::Staged => row_button(
                                                    theme,
                                                    ("unstage-all", row),
                                                    "−",
                                                    row,
                                                    ChangesCommand::Unstage,
                                                    cx,
                                                ),
                                                Group::Conflicts | Group::Changes => row_button(
                                                    theme,
                                                    ("stage-all", row),
                                                    "+",
                                                    row,
                                                    ChangesCommand::Stage,
                                                    cx,
                                                ),
                                            };
                                            div()
                                                .id(row)
                                                .group("change-row")
                                                .w_full()
                                                .flex()
                                                .flex_row()
                                                .gap_1()
                                                .px_2()
                                                .bg(row_bg(theme, row, cursor, focus))
                                                .hover(|style| style.bg(hsla(theme.chrome.hover)))
                                                .text_color(hsla(theme.muted))
                                                .on_click(cx.listener(
                                                    move |this, _: &ClickEvent, window, cx| {
                                                        panes::click_changes(
                                                            &mut this.state,
                                                            row,
                                                            panes::Click::Single,
                                                            &mut this.effects,
                                                        );
                                                        window.focus(&this.focus, cx);
                                                        this.changed(cx);
                                                    },
                                                ))
                                                .child(div().flex_none().w(px(12.)).child(chevron))
                                                .child(div().flex_1().child(label))
                                                .child(button)
                                                .child(div().flex_none().child(count.to_string()))
                                        }
                                        ListRow::Entry(index) => {
                                            let changed = &entries[index];
                                            let (letter, status) = match changed.entry {
                                                Entry::Conflict => ("C", GitStatus::Conflicted),
                                                Entry::Unstaged(Work::Untracked) => {
                                                    ("U", GitStatus::Untracked)
                                                }
                                                Entry::Staged { kind, .. }
                                                | Entry::Unstaged(Work::Changed { kind, .. }) => {
                                                    match kind {
                                                        Kind::Added => ("A", GitStatus::Added),
                                                        Kind::Deleted => ("D", GitStatus::Deleted),
                                                        Kind::Renamed => ("R", GitStatus::Modified),
                                                        Kind::TypeChange => {
                                                            ("T", GitStatus::Modified)
                                                        }
                                                        Kind::Modified => {
                                                            ("M", GitStatus::Modified)
                                                        }
                                                    }
                                                }
                                            };
                                            // Staged in full, in part, or not at all.
                                            let marker = match changed.entry {
                                                Entry::Conflict => "",
                                                Entry::Staged { unstaged: None, .. } => "●",
                                                Entry::Staged {
                                                    unstaged: Some(_), ..
                                                }
                                                | Entry::Unstaged(Work::Changed {
                                                    staged: Some(_),
                                                    ..
                                                }) => "◐",
                                                Entry::Unstaged(
                                                    Work::Untracked
                                                    | Work::Changed { staged: None, .. },
                                                ) => "○",
                                            };
                                            let buttons = match changed.entry.group() {
                                                Group::Changes => [
                                                    Some(row_button(
                                                        theme,
                                                        ("discard", row),
                                                        "↶",
                                                        row,
                                                        ChangesCommand::Discard,
                                                        cx,
                                                    )),
                                                    Some(row_button(
                                                        theme,
                                                        ("stage", row),
                                                        "+",
                                                        row,
                                                        ChangesCommand::Stage,
                                                        cx,
                                                    )),
                                                ],
                                                Group::Staged => [
                                                    Some(row_button(
                                                        theme,
                                                        ("unstage", row),
                                                        "−",
                                                        row,
                                                        ChangesCommand::Unstage,
                                                        cx,
                                                    )),
                                                    None,
                                                ],
                                                Group::Conflicts => [
                                                    Some(row_button(
                                                        theme,
                                                        ("stage", row),
                                                        "+",
                                                        row,
                                                        ChangesCommand::Stage,
                                                        cx,
                                                    )),
                                                    None,
                                                ],
                                            };
                                            let dir = changed.path.folder_path();
                                            div()
                                                .id(row)
                                                .group("change-row")
                                                .w_full()
                                                .flex()
                                                .flex_row()
                                                .gap_2()
                                                .pl_4()
                                                .pr_2()
                                                .bg(row_bg(theme, row, cursor, focus))
                                                .hover(|style| style.bg(hsla(theme.chrome.hover)))
                                                .on_click(cx.listener(
                                                    move |this, event: &ClickEvent, window, cx| {
                                                        panes::click_changes(
                                                            &mut this.state,
                                                            row,
                                                            panes::Click::of(event.click_count()),
                                                            &mut this.effects,
                                                        );
                                                        window.focus(&this.focus, cx);
                                                        this.changed(cx);
                                                    },
                                                ))
                                                .child(
                                                    div()
                                                        .flex_none()
                                                        .text_color(hsla(theme.chrome[status]))
                                                        .child(changed.path.name().to_owned()),
                                                )
                                                .child(
                                                    div()
                                                        .flex_1()
                                                        .min_w_0()
                                                        .overflow_hidden()
                                                        .whitespace_nowrap()
                                                        .text_color(hsla(theme.muted))
                                                        .child(dir.to_owned()),
                                                )
                                                .children(buttons.into_iter().flatten())
                                                .child(
                                                    div()
                                                        .flex_none()
                                                        .text_color(hsla(theme.muted))
                                                        .child(marker),
                                                )
                                                .child(
                                                    div()
                                                        .flex_none()
                                                        .w(px(12.))
                                                        .text_color(hsla(theme.chrome[status]))
                                                        .child(letter),
                                                )
                                        }
                                    })
                                    .collect()
                            }),
                        )
                        .track_scroll(&self.changes_scroll)
                        .flex_1()
                        .into_any_element(),
                        Sidebar::Files(_) | Sidebar::Hidden => div()
                            .id("file-tree-area")
                            .flex_1()
                            .min_h_0()
                            .on_mouse_down(
                                MouseButton::Right,
                                cx.listener(|this, event: &MouseDownEvent, _, cx| {
                                    this.tree_menu = Some(TreeMenu {
                                        target: TreeTarget::Root,
                                        at: event.position,
                                    });
                                    cx.notify();
                                }),
                            )
                            .drag_over::<DraggedEntry>(move |style, _, _, _| style.bg(drop_bg))
                            .on_drop(cx.listener(|this, dragged: &DraggedEntry, window, cx| {
                                panes::move_entry(
                                    &mut this.state,
                                    dragged.path.clone(),
                                    TreeTarget::Root,
                                    &mut this.effects,
                                );
                                window.focus(&this.focus, cx);
                                this.changed(cx);
                            }))
                            .child(
                                uniform_list(
                                    "file-tree",
                                    self.state.tree.rows.len(),
                                    cx.processor(move |this, range: Range<usize>, _window, cx| {
                                        let state = &this.state;
                                        let theme = &this.theme;
                                        let (drop_bg, preview_bg, preview_fg) = (
                                            hsla(theme.chrome.selection),
                                            hsla(theme.chrome.title),
                                            hsla(theme.text),
                                        );
                                        range
                                            .map(|index| {
                                                let row = &state.tree.rows[index];
                                                let status = match &state.project.git {
                                                    Repo::Found { snapshot, .. } => {
                                                        snapshot.get(&row.path)
                                                    }
                                                    Repo::None | Repo::Unknown => None,
                                                };
                                                let color = match status {
                                                    Some(status) => hsla(theme.chrome[status]),
                                                    None => hsla(theme.text),
                                                };
                                                let icon = match row.kind {
                                                    RowKind::File => "  ",
                                                    RowKind::Dir(Fold::Collapsed) => "▸ ",
                                                    RowKind::Dir(Fold::Expanded) => "▾ ",
                                                };
                                                let unsaved = match state
                                                    .tree
                                                    .unsaved
                                                    .paths
                                                    .contains(&row.path)
                                                {
                                                    true => "●",
                                                    false => "",
                                                };
                                                let bg = row_bg(theme, index, Some(state.tree.cursor), focus);
                                                div()
                                            .id(index)
                                            .flex()
                                            .flex_row()
                                            .bg(bg)
                                            .hover(|style| style.bg(hsla(theme.chrome.hover)))
                                            .pl(px(4.0 + INDENT * row.depth as f32))
                                            .pr_2()
                                            .text_color(color)
                                            .on_mouse_down(
                                                MouseButton::Right,
                                                cx.listener(
                                                    move |this, event: &MouseDownEvent, _, cx| {
                                                        this.tree_menu = Some(TreeMenu {
                                                            target: TreeTarget::Row(index),
                                                            at: event.position,
                                                        });
                                                        cx.stop_propagation();
                                                        cx.notify();
                                                    },
                                                ),
                                            )
                                            .on_drag(
                                                DraggedEntry {
                                                    path: row.path.clone(),
                                                },
                                                move |dragged, _, _, cx| {
                                                    cx.new(|_| DragPreview {
                                                        name: dragged.path.name().to_owned().into(),
                                                        bg: preview_bg,
                                                        fg: preview_fg,
                                                    })
                                                },
                                            )
                                            .drag_over::<DraggedEntry>(move |style, _, _, _| {
                                                style.bg(drop_bg)
                                            })
                                            .on_drop(cx.listener(
                                                move |this, dragged: &DraggedEntry, window, cx| {
                                                    panes::move_entry(
                                                        &mut this.state,
                                                        dragged.path.clone(),
                                                        TreeTarget::Row(index),
                                                        &mut this.effects,
                                                    );
                                                    window.focus(&this.focus, cx);
                                                    this.changed(cx);
                                                },
                                            ))
                                            .on_click(cx.listener(
                                                move |this, event: &ClickEvent, window, cx| {
                                                    let click = panes::Click::of(event.click_count());
                                                    panes::click_tree(
                                                        &mut this.state,
                                                        index,
                                                        click,
                                                        &mut this.effects,
                                                    );
                                                    window.focus(&this.focus, cx);
                                                    this.changed(cx);
                                                },
                                            ))
                                            .child(div().text_color(hsla(theme.muted)).child(icon))
                                            .child(
                                                div()
                                                    .flex_1()
                                                    .overflow_hidden()
                                                    .child(row.path.name().to_owned()),
                                            )
                                            .child(
                                                div()
                                                    .text_color(hsla(theme.chrome.unsaved))
                                                    .child(unsaved),
                                            )
                                            })
                                            .collect()
                                    }),
                                )
                                .track_scroll(&self.tree_scroll)
                                .size_full(),
                            )
                            .into_any_element(),
                    }),
            )
            .child(divider(Axis::Row, border).on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    this.drag = Drag::Sidebar;
                    cx.stop_propagation();
                }),
            ))
    }

    pub(super) fn tree_menu(&self, cx: &mut Context<Self>) -> Option<Deferred> {
        let menu = self.tree_menu.as_ref()?;
        let theme = &self.theme;
        let fonts = &self.fonts;
        let target = menu.target;
        const ITEMS: [(TreeAction, &str); 4] = [
            (TreeAction::NewFile, "New File"),
            (TreeAction::NewFolder, "New Folder"),
            (TreeAction::Rename, "Rename"),
            (TreeAction::Delete, "Delete"),
        ];
        let shown = match target {
            TreeTarget::Row(_) => ITEMS.len(),
            TreeTarget::Root => 2,
        };
        let hover = hsla(theme.chrome.hover);
        let items = ITEMS[..shown].iter().map(|&(action, label)| {
            div()
                .id(label)
                .px_3()
                .py_0p5()
                .rounded_sm()
                .hover(move |style| style.bg(hover))
                .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                    this.tree_menu = None;
                    panes::tree_action(&mut this.state, target, action, &mut this.effects);
                    window.focus(&this.focus, cx);
                    this.changed(cx);
                }))
                .child(label)
        });
        Some(
            deferred(
                anchored().position(menu.at).snap_to_window().child(
                    div()
                        .id("tree-menu")
                        .min_w(px(140.))
                        .p_1()
                        .flex()
                        .flex_col()
                        .bg(hsla(theme.surface))
                        .border_1()
                        .border_color(hsla(theme.chrome.border))
                        .rounded_md()
                        .font(fonts.ui.clone())
                        .text_size(fonts.ui_size)
                        .text_color(hsla(theme.text))
                        .on_mouse_down_out(cx.listener(|this, _, _, cx| {
                            this.tree_menu = None;
                            cx.notify();
                        }))
                        .children(items),
                ),
            )
            .with_priority(1),
        )
    }
}

/// A Changes row button, shown while its row is hovered.
pub(super) fn row_button(
    theme: &Theme,
    id: (&'static str, usize),
    glyph: &'static str,
    row: usize,
    command: ChangesCommand,
    cx: &mut Context<Workspace>,
) -> Stateful<Div> {
    let hover = hsla(theme.chrome.hover);
    let text = hsla(theme.text);
    div()
        .id(id)
        .flex_none()
        .w(px(16.))
        .flex()
        .justify_center()
        .rounded_sm()
        .text_color(hsla(theme.muted))
        .invisible()
        .group_hover("change-row", |style| style.visible())
        .hover(move |style| style.bg(hover).text_color(text))
        .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
            cx.stop_propagation();
            panes::changes_action(&mut this.state, row, command, &mut this.effects);
            window.focus(&this.focus, cx);
            this.changed(cx);
        }))
        .child(glyph)
}

/// Fits a sidebar width between its minimum and the window, as zed fits its docks.
pub(super) fn fit_sidebar(width: Pixels, fonts: &Fonts, window: Pixels) -> Pixels {
    width.min(window - DIVIDER).max(fonts.ui_size * SIDEBAR_MIN)
}

/// A sidebar row's background; the cursor row shows whether the sidebar has the keys.
fn row_bg(theme: &Theme, row: usize, cursor: Option<usize>, focus: Focus) -> Hsla {
    match (cursor, focus) {
        (Some(at), Focus::Sidebar) if at == row => hsla(theme.chrome.selection),
        (Some(at), Focus::Editor) if at == row => hsla(theme.background),
        (Some(_) | None, _) => hsla(theme.surface),
    }
}
