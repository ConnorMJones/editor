//! The file tree and Changes sidebars, and the tree's context menu and drag preview.

use std::ops::Range;

use gpui::{
    AnyElement, ClickEvent, Context, Deferred, Div, Hsla, IntoElement, MouseButton, MouseDownEvent,
    Pixels, Point, Render, SharedString, Stateful, Window, anchored, canvas, deferred, div,
    prelude::*, uniform_list,
};
use model::command::ChangesCommand;
use model::config::SidebarSide;
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

/// Sizes in ems of the interface font.
pub const INDENT: f32 = 0.9;
pub const SIDEBAR_DEFAULT: f32 = 20.0;
/// A sidebar dragged narrower than this hides.
pub const SIDEBAR_HIDE: f32 = 4.0;
/// The columns of status glyphs and buttons at a row's right end.
pub const COLUMN: f32 = 1.3;

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

/// Whether a list row is the cursor row, and so how it is filled and outlined.
#[derive(Clone, Copy)]
pub enum Picked {
    /// The cursor row while the sidebar has the keys.
    Focused,
    /// The cursor row while the editor has the keys.
    Resting,
    No,
}

impl Picked {
    pub fn of(row: usize, cursor: Option<usize>, focus: Focus) -> Picked {
        match (cursor, focus) {
            (Some(at), Focus::Sidebar) if at == row => Picked::Focused,
            (Some(at), Focus::Editor) if at == row => Picked::Resting,
            (Some(_) | None, _) => Picked::No,
        }
    }
}

impl Workspace {
    /// The sidebar beside the panes: a header as tall as their tab bars, the list, and a footer
    /// as tall as their status lines, with the divider on the side facing the panes.
    pub fn sidebar(
        &self,
        focus: Focus,
        side: SidebarSide,
        window: Pixels,
        cx: &mut Context<Self>,
    ) -> Div {
        let theme = &self.theme;
        let fonts = &self.fonts;
        let border = match focus {
            Focus::Sidebar => hsla(theme.chrome.accent),
            Focus::Editor => hsla(theme.chrome.border),
        };
        let muted = hsla(theme.muted);
        let areas = self.areas.clone();
        let (title, detail, summary) = match (&self.state.sidebar, &self.state.project.git) {
            (Sidebar::Changes(_), Repo::Found { snapshot, .. }) => {
                let count = |group| {
                    (snapshot.changes.iter())
                        .filter(|changed| changed.entry.group() == group)
                        .count()
                };
                (
                    "CHANGES",
                    snapshot.changes.len().to_string(),
                    format!(
                        "{} staged · {} changed",
                        count(Group::Staged),
                        count(Group::Changes)
                    ),
                )
            }
            (Sidebar::Changes(_), Repo::None | Repo::Unknown) => {
                ("CHANGES", String::new(), "not a repository".to_owned())
            }
            (Sidebar::Files(_) | Sidebar::Hidden, git) => {
                let root = self.state.project.root.path();
                let name = (root.file_name())
                    .map_or(String::new(), |name| name.to_string_lossy().to_uppercase());
                let summary = match git {
                    Repo::Found { snapshot, .. } => format!("{} changed", snapshot.changes.len()),
                    Repo::None | Repo::Unknown => String::new(),
                };
                ("FILES", name, summary)
            }
        };
        let strip = |height: Pixels| {
            div()
                .flex_none()
                .h(height)
                .px(fonts.ui_size * 0.6)
                .flex()
                .flex_row()
                .items_center()
                .gap(fonts.ui_size * 0.5)
                .overflow_hidden()
                .whitespace_nowrap()
                .text_color(muted)
                .border_color(hsla(theme.chrome.border))
        };
        let note = match (&self.state.sidebar, &self.state.project.git) {
            (Sidebar::Changes(_), Repo::Found { snapshot, .. }) => match snapshot.unlisted.len() {
                0 => None,
                1 => Some("1 change is not listed; see the log".to_owned()),
                count => Some(format!("{count} changes are not listed; see the log")),
            },
            (Sidebar::Hidden | Sidebar::Files(_), _)
            | (Sidebar::Changes(_), Repo::None | Repo::Unknown) => None,
        };
        let list = match self.state.sidebar {
            Sidebar::Changes(_) => self.changes_list(focus, cx),
            Sidebar::Files(_) | Sidebar::Hidden => self.file_tree(focus, cx),
        };
        let panel = div()
            .relative()
            .w(match self.state.sidebar_width {
                SidebarWidth::Default => fonts.ui_size * SIDEBAR_DEFAULT,
                SidebarWidth::Dragged(width) => gpui::px(width.get().0),
            }
            .min(window - DIVIDER))
            .h_full()
            .flex()
            .flex_col()
            .overflow_hidden()
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
            .child(
                strip(fonts.bar)
                    .border_b_1()
                    .child(div().flex_none().child(title))
                    .child(div().flex_1().min_w_0().truncate().child(detail)),
            )
            .children(note.map(|note| strip(fonts.ui_line).child(div().truncate().child(note))))
            .child(list)
            .child(
                strip(fonts.ui_line)
                    .border_t_1()
                    .child(div().flex_1().min_w_0().truncate().child(summary)),
            );
        let divider = divider(Axis::Row, border).on_mouse_down(
            MouseButton::Left,
            cx.listener(|this, _, _, cx| {
                this.drag = Drag::Sidebar;
                cx.stop_propagation();
            }),
        );
        let frame = div().flex_none().h_full().flex().flex_row();
        match side {
            SidebarSide::Left => frame.child(panel).child(divider),
            SidebarSide::Right => frame.child(divider).child(panel),
        }
    }

    pub fn changes_list(&self, focus: Focus, cx: &mut Context<Self>) -> AnyElement {
        uniform_list(
            "changes",
            changes::list_len(self.state.project.git.changes(), &self.state.changes.folds),
            cx.processor(move |this, range: Range<usize>, _window, cx| {
                let state = &this.state;
                let (theme, fonts) = (&this.theme, &this.fonts);
                let entries = state.project.git.changes();
                let folds = &state.changes.folds;
                let cursor = changes::cursor_row(entries, folds, state.changes.cursor.as_ref());
                let column = |glyph: &'static str, color: Hsla| {
                    div()
                        .flex_none()
                        .w(fonts.ui_size * COLUMN)
                        .flex()
                        .justify_center()
                        .text_color(color)
                        .child(glyph)
                };
                changes::rows(entries, folds)
                    .skip(range.start)
                    .zip(range)
                    .map(|(list_row, row)| {
                        let base = list_row_base(theme, fonts, row, Picked::of(row, cursor, focus))
                            .group("change-row");
                        match list_row {
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
                                        fonts,
                                        ("unstage-all", row),
                                        "−",
                                        row,
                                        ChangesCommand::Unstage,
                                        cx,
                                    ),
                                    Group::Conflicts | Group::Changes => row_button(
                                        theme,
                                        fonts,
                                        ("stage-all", row),
                                        "+",
                                        row,
                                        ChangesCommand::Stage,
                                        cx,
                                    ),
                                };
                                base.text_color(hsla(theme.muted))
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
                                    .child(column(chevron, hsla(theme.muted)))
                                    .child(div().flex_1().min_w_0().truncate().child(label))
                                    .child(button)
                                    .child(
                                        div()
                                            .flex_none()
                                            .min_w(fonts.ui_size * COLUMN)
                                            .flex()
                                            .justify_end()
                                            .child(count.to_string()),
                                    )
                                    .into_any_element()
                            }
                            ListRow::Entry(index) => {
                                let changed = &entries[index];
                                let (preview_bg, preview_fg) =
                                    (hsla(theme.chrome.title), hsla(theme.text));
                                let (letter, status) = match changed.entry {
                                    Entry::Conflict => ("C", GitStatus::Conflicted),
                                    Entry::Unstaged(Work::Untracked) => ("U", GitStatus::Untracked),
                                    Entry::Staged { kind, .. }
                                    | Entry::Unstaged(Work::Changed { kind, .. }) => match kind {
                                        Kind::Added => ("A", GitStatus::Added),
                                        Kind::Deleted => ("D", GitStatus::Deleted),
                                        Kind::Renamed => ("R", GitStatus::Modified),
                                        Kind::TypeChange => ("T", GitStatus::Modified),
                                        Kind::Modified => ("M", GitStatus::Modified),
                                    },
                                };
                                // Staged in full, in part, or not at all.
                                let marker = match changed.entry {
                                    Entry::Conflict => "",
                                    Entry::Staged { unstaged: None, .. } => "●",
                                    Entry::Staged {
                                        unstaged: Some(_), ..
                                    }
                                    | Entry::Unstaged(Work::Changed {
                                        staged: Some(_), ..
                                    }) => "◐",
                                    Entry::Unstaged(
                                        Work::Untracked | Work::Changed { staged: None, .. },
                                    ) => "○",
                                };
                                let buttons = match changed.entry.group() {
                                    Group::Changes => [
                                        Some(row_button(
                                            theme,
                                            fonts,
                                            ("discard", row),
                                            "↶",
                                            row,
                                            ChangesCommand::Discard,
                                            cx,
                                        )),
                                        Some(row_button(
                                            theme,
                                            fonts,
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
                                            fonts,
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
                                            fonts,
                                            ("stage", row),
                                            "+",
                                            row,
                                            ChangesCommand::Stage,
                                            cx,
                                        )),
                                        None,
                                    ],
                                };
                                let status_color = hsla(theme.chrome[status]);
                                base.pl(fonts.ui_size * (0.4 + COLUMN))
                                    .on_drag(
                                        DraggedEntry {
                                            path: changed.path.clone(),
                                        },
                                        move |dragged, _, _, cx| {
                                            cx.new(|_| DragPreview {
                                                name: dragged.path.name().to_owned().into(),
                                                bg: preview_bg,
                                                fg: preview_fg,
                                            })
                                        },
                                    )
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
                                    // The name gives way only once the folder after it is gone.
                                    .child(
                                        div()
                                            .min_w_0()
                                            .truncate()
                                            .text_color(status_color)
                                            .child(changed.path.name().to_owned()),
                                    )
                                    .child(
                                        div()
                                            .flex_1()
                                            .min_w_0()
                                            .pl(fonts.ui_size * 0.5)
                                            .truncate()
                                            .text_color(hsla(theme.muted))
                                            .child(changed.path.folder_path().to_owned()),
                                    )
                                    .children(buttons.into_iter().flatten())
                                    .child(column(marker, hsla(theme.muted)))
                                    .child(column(letter, status_color))
                                    .into_any_element()
                            }
                        }
                    })
                    .collect()
            }),
        )
        .track_scroll(&self.changes_scroll)
        .flex_1()
        .min_h_0()
        .into_any_element()
    }

    pub fn file_tree(&self, focus: Focus, cx: &mut Context<Self>) -> AnyElement {
        let drop_bg = hsla(self.theme.chrome.selection);
        div()
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
                        let (theme, fonts) = (&this.theme, &this.fonts);
                        let (preview_bg, preview_fg) = (hsla(theme.chrome.title), hsla(theme.text));
                        range
                            .map(|index| {
                                let row = &state.tree.rows[index];
                                let status = match &state.project.git {
                                    Repo::Found { snapshot, .. } => snapshot.get(&row.path),
                                    Repo::None | Repo::Unknown => None,
                                };
                                let color = match status {
                                    Some(status) => hsla(theme.chrome[status]),
                                    None => hsla(theme.text),
                                };
                                let chevron = match row.kind {
                                    RowKind::File => "",
                                    RowKind::Dir(Fold::Collapsed) => "▸",
                                    RowKind::Dir(Fold::Expanded) => "▾",
                                };
                                let unsaved = match state.tree.unsaved.paths.contains(&row.path) {
                                    true => "●",
                                    false => "",
                                };
                                let picked = Picked::of(index, Some(state.tree.cursor), focus);
                                list_row_base(theme, fonts, index, picked)
                                    .pl(fonts.ui_size * (0.4 + INDENT * row.depth as f32))
                                    .text_color(color)
                                    .on_mouse_down(
                                        MouseButton::Right,
                                        cx.listener(move |this, event: &MouseDownEvent, _, cx| {
                                            this.tree_menu = Some(TreeMenu {
                                                target: TreeTarget::Row(index),
                                                at: event.position,
                                            });
                                            cx.stop_propagation();
                                            cx.notify();
                                        }),
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
                                    .child(
                                        div()
                                            .flex_none()
                                            .w(fonts.ui_size * COLUMN)
                                            .text_color(hsla(theme.muted))
                                            .child(chevron),
                                    )
                                    .child(
                                        div()
                                            .flex_1()
                                            .min_w_0()
                                            .truncate()
                                            .child(row.path.name().to_owned()),
                                    )
                                    .child(
                                        div()
                                            .flex_none()
                                            .w(fonts.ui_size * COLUMN)
                                            .flex()
                                            .justify_center()
                                            .text_color(hsla(theme.chrome.unsaved))
                                            .child(unsaved),
                                    )
                                    .into_any_element()
                            })
                            .collect()
                    }),
                )
                .track_scroll(&self.tree_scroll)
                .size_full(),
            )
            .into_any_element()
    }
}

/// A list row across the sidebar's whole width: the cursor row filled and, while the sidebar has
/// the keys, outlined; other rows show the pointer's hover.
pub fn list_row_base(theme: &Theme, fonts: &Fonts, row: usize, picked: Picked) -> Stateful<Div> {
    let row = div()
        .id(row)
        .w_full()
        .h(fonts.ui_line)
        .flex()
        .flex_row()
        .items_center()
        .pr(fonts.ui_size * 0.3)
        .overflow_hidden()
        .whitespace_nowrap()
        .border_1();
    let hover = hsla(theme.chrome.hover);
    match picked {
        Picked::Focused => row
            .bg(hsla(theme.chrome.picked))
            .border_color(hsla(theme.chrome.accent)),
        Picked::Resting => row
            .bg(hsla(theme.chrome.picked))
            .border_color(gpui::transparent_black()),
        Picked::No => row
            .border_color(gpui::transparent_black())
            .hover(move |style| style.bg(hover)),
    }
}

impl Workspace {
    pub fn tree_menu(&self, cx: &mut Context<Self>) -> Option<Deferred> {
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
                        .min_w(fonts.ui_size * 10.0)
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
pub fn row_button(
    theme: &Theme,
    fonts: &Fonts,
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
        .w(fonts.ui_size * COLUMN)
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
