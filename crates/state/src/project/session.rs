use std::collections::HashMap;
use std::io;
use std::path::{Path, PathBuf};

use helix_core::{Range, Selection, SmallVec};
use model::window::Axis;
use serde::{Deserialize, Serialize};

use crate::apply;
use crate::git::diffview::DiffView;
use crate::job::Job;
use crate::project::path::RelPath;
use crate::render::rendered::{Link, RenderedView, Scroll};
use crate::sidebar::tree::{self, Dir, Fold, Listing};
use crate::text::buffer::{BufferId, Content};
use crate::view::layout::{
    Child, CloseError, Flex, InvalidTabs, Item, Layout, Node, Pane, PaneId, TabKey, Tabs, TextView,
    View,
};
use crate::{Effect, Focus, Sidebar, SidebarWidth, State};
use model::git::Group;

/// Workspace layout saved on exit and restored with `session.restore = "last"`.
#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Session {
    /// Panes in reading order; `layout` and `focus` index into it.
    pub panes: Vec<SessionPane>,
    pub layout: SessionNode,
    pub focus: usize,
    pub sidebar: SessionSidebar,
    /// Logical pixels; sessions saved before it existed use the default.
    #[serde(default = "default_sidebar_width")]
    pub sidebar_width: f32,
    /// Expanded tree directories relative to the root.
    pub expanded: Vec<String>,
    /// Collapsed groups of the Changes sidebar.
    #[serde(default)]
    pub collapsed: Vec<Group>,
}

/// `flex` is the node's weight within its parent split; older sessions split evenly.
#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase", deny_unknown_fields)]
pub enum SessionNode {
    Pane {
        pane: usize,
        #[serde(default = "even")]
        flex: f32,
    },
    Split {
        axis: Axis,
        children: Vec<SessionNode>,
        #[serde(default = "even")]
        flex: f32,
    },
}

fn even() -> f32 {
    Flex::EVEN.get()
}

fn default_sidebar_width() -> f32 {
    SidebarWidth::DEFAULT.get()
}

/// `active` and `preview` index into `tabs`.
#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase", deny_unknown_fields)]
pub enum SessionPane {
    Empty,
    Tabs {
        tabs: Vec<SessionItem>,
        active: usize,
        #[serde(default)]
        preview: Option<usize>,
    },
}

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase", deny_unknown_fields)]
pub enum SessionItem {
    Text {
        path: PathBuf,
        /// `[anchor, head]` char offsets.
        ranges: Vec<[usize; 2]>,
        primary: usize,
        scroll: usize,
    },
    /// A diff against HEAD; the selection and scroll are its buffer side's.
    Diff {
        path: PathBuf,
        ranges: Vec<[usize; 2]>,
        primary: usize,
        scroll: usize,
    },
    /// A rendered view; the selection and scroll are the source position it toggles back to.
    Rendered {
        path: PathBuf,
        ranges: Vec<[usize; 2]>,
        primary: usize,
        scroll: usize,
        top: Top,
        /// Reading-order index of the pane it follows; standalone when absent.
        #[serde(default)]
        follows: Option<usize>,
    },
}

/// Where a rendered view stood: a block, or a source line not yet resolved into one.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Top {
    Item(usize),
    Line(usize),
}

/// Which kind of tab a saved item restores.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Saved {
    Text,
    Diff,
    Rendered { top: Top, follows: Option<usize> },
}

impl SessionItem {
    fn key(&self) -> (std::mem::Discriminant<SessionItem>, &Path) {
        match self {
            SessionItem::Text { path, .. }
            | SessionItem::Diff { path, .. }
            | SessionItem::Rendered { path, .. } => (std::mem::discriminant(self), path),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SessionSidebar {
    Hidden,
    /// The file tree.
    Shown,
    Changes,
}

#[derive(Debug, thiserror::Error)]
pub enum SessionError {
    #[error("{0}")]
    Read(io::Error),
    #[error("{0}")]
    Parse(toml::de::Error),
    #[error("layout refers to pane {0}, which does not exist")]
    MissingPane(usize),
    #[error("layout uses pane {0} more than once")]
    DuplicatePane(usize),
    #[error("a split needs at least two children")]
    EmptySplit,
    #[error("pane {0} is not placed in the layout")]
    UnusedPane(usize),
    #[error("split weight {0} is not a positive number")]
    InvalidFlex(f32),
    #[error("pane {pane}: {reason}")]
    Tabs { pane: usize, reason: InvalidTabs },
}

/// Session file for `root` under `dir`, named after the escaped root path.
pub fn file(dir: &Path, root: &Path) -> PathBuf {
    let name = root
        .to_string_lossy()
        .replace('%', "%25")
        .replace('/', "%2F");
    dir.join(format!("{name}.toml"))
}

/// `None` when no session was saved for this root yet.
pub fn load(path: &Path) -> Result<Option<Session>, SessionError> {
    match std::fs::read_to_string(path) {
        Ok(text) => toml::from_str(&text).map(Some).map_err(SessionError::Parse),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(SessionError::Read(error)),
    }
}

pub fn save(path: &Path, session: &Session) -> io::Result<()> {
    let text = toml::to_string(session).map_err(io::Error::other)?;
    match path.parent() {
        Some(dir) => std::fs::create_dir_all(dir)?,
        None => {}
    }
    std::fs::write(path, text)
}

pub fn snapshot(state: &State) -> Session {
    let leaves = state.layout.leaves();
    let index: HashMap<PaneId, usize> = leaves.iter().enumerate().map(|(i, &p)| (p, i)).collect();
    let panes = leaves
        .iter()
        .map(|&pane| match &state.layout.panes[pane].tabs {
            None => SessionPane::Empty,
            Some(tabs) => SessionPane::Tabs {
                tabs: tabs
                    .items()
                    .iter()
                    .map(|item| {
                        let view = item.view.text();
                        let path = state.buffers[view.buffer()].path.clone();
                        let ranges = view
                            .selection
                            .ranges()
                            .iter()
                            .map(|range| [range.anchor, range.head])
                            .collect();
                        let (primary, scroll) =
                            (view.selection.primary_index(), view.scroll.anchor);
                        match &item.view {
                            View::Text(_) => SessionItem::Text {
                                path,
                                ranges,
                                primary,
                                scroll,
                            },
                            View::Diff(_) => SessionItem::Diff {
                                path,
                                ranges,
                                primary,
                                scroll,
                            },
                            View::Rendered(rendered) => SessionItem::Rendered {
                                path,
                                ranges,
                                primary,
                                scroll,
                                top: match rendered.scroll {
                                    Scroll::Line(line) => Top::Line(line),
                                    Scroll::Point(at) => Top::Item(at.page),
                                    Scroll::At(at) | Scroll::Moved { from: at, .. } => {
                                        Top::Item(at.item)
                                    }
                                },
                                follows: match rendered.link {
                                    Link::Follows { pane, .. } => index.get(&pane).copied(),
                                    Link::Standalone => None,
                                },
                            },
                        }
                    })
                    .collect(),
                active: tabs.active(),
                preview: tabs.preview(),
            },
        })
        .collect();
    let mut expanded: Vec<String> = state
        .tree
        .dirs
        .iter()
        .filter(|(path, dir)| !path.is_root() && dir.fold == Fold::Expanded)
        .map(|(path, _)| path.as_str().to_owned())
        .collect();
    expanded.sort_unstable();
    Session {
        panes,
        layout: snapshot_node(&state.layout.root, Flex::EVEN, &index),
        focus: index[&state.layout.focus],
        sidebar: match state.sidebar {
            Sidebar::Hidden => SessionSidebar::Hidden,
            Sidebar::Files(_) => SessionSidebar::Shown,
            Sidebar::Changes(_) => SessionSidebar::Changes,
        },
        sidebar_width: state.sidebar_width.get(),
        expanded,
        collapsed: Group::ALL
            .into_iter()
            .filter(|group| state.git.folds[*group as usize] == Fold::Collapsed)
            .collect(),
    }
}

fn snapshot_node(node: &Node, flex: Flex, index: &HashMap<PaneId, usize>) -> SessionNode {
    match node {
        Node::Pane(pane) => SessionNode::Pane {
            pane: index[pane],
            flex: flex.get(),
        },
        Node::Split { axis, children } => SessionNode::Split {
            axis: *axis,
            children: children
                .iter()
                .map(|c| snapshot_node(&c.node, c.flex, index))
                .collect(),
            flex: flex.get(),
        },
    }
}

/// Reopens the session's buffers and rebuilds its layout; files that no longer open are
/// skipped, their panes left empty when no tab remains. Call after `boot`.
pub fn restore(
    state: &mut State,
    session: Session,
    out: &mut Vec<Effect>,
) -> Result<(), SessionError> {
    let mut used = vec![false; session.panes.len()];
    check_node(&session.layout, &mut used)?;
    match used.iter().position(|placed| !placed) {
        Some(pane) => return Err(SessionError::UnusedPane(pane)),
        None => {}
    }
    for (pane, saved) in session.panes.iter().enumerate() {
        let SessionPane::Tabs {
            tabs,
            active,
            preview,
        } = saved
        else {
            continue;
        };
        let duplicate = tabs.iter().enumerate().position(|(index, item)| {
            tabs[..index]
                .iter()
                .any(|earlier| earlier.key() == item.key())
        });
        let reason = match (tabs.len(), duplicate, preview) {
            (0, _, _) => InvalidTabs::Empty,
            (_, Some(index), _) => InvalidTabs::Duplicate(index),
            (len, None, _) if *active >= len => InvalidTabs::Active(*active),
            (len, None, Some(preview)) if *preview >= len => InvalidTabs::Preview(*preview),
            (_, None, _) => continue,
        };
        return Err(SessionError::Tabs { pane, reason });
    }
    let focus_slot = session.focus.min(session.panes.len().saturating_sub(1));

    let mut panes = slotmap::SlotMap::with_key();
    let mut ids: Vec<PaneId> = Vec::with_capacity(session.panes.len());
    // Rendered views and the pane slot each follows, linked once every pane exists.
    let mut follows: Vec<(usize, BufferId, usize)> = Vec::new();
    for (pane, saved) in session.panes.into_iter().enumerate() {
        let SessionPane::Tabs {
            tabs,
            active,
            preview,
        } = saved
        else {
            ids.push(panes.insert(Pane::EMPTY));
            continue;
        };
        // Saved index of each kept item; files deleted since are dropped, not recreated.
        let mut kept: Vec<usize> = Vec::with_capacity(tabs.len());
        let mut items: Vec<Item> = Vec::with_capacity(tabs.len());
        for (saved_index, item) in tabs.into_iter().enumerate() {
            let (saved, path, ranges, primary, scroll) = match item {
                SessionItem::Text {
                    path,
                    ranges,
                    primary,
                    scroll,
                } => (Saved::Text, path, ranges, primary, scroll),
                SessionItem::Diff {
                    path,
                    ranges,
                    primary,
                    scroll,
                } => (Saved::Diff, path, ranges, primary, scroll),
                SessionItem::Rendered {
                    path,
                    ranges,
                    primary,
                    scroll,
                    top,
                    follows,
                } => (
                    Saved::Rendered { top, follows },
                    path,
                    ranges,
                    primary,
                    scroll,
                ),
            };
            // A diff also shows a file deleted from the worktree.
            let buffer = match (saved, path.is_file(), path.exists()) {
                (_, true, _) | (Saved::Diff, false, false) => apply::load(state, path, out),
                (Saved::Text | Saved::Rendered { .. }, false, _) | (Saved::Diff, false, true) => {
                    None
                }
            };
            let Some(buffer) = buffer else {
                continue;
            };
            // Loading buffers clamp their views when the text arrives.
            let len = match &state.buffers[buffer].content {
                Content::Text { doc, .. } => doc.text.len_chars(),
                Content::Loading { .. } => usize::MAX,
            };
            let ranges: SmallVec<[Range; 1]> = ranges
                .iter()
                .map(|&[anchor, head]| Range::new(anchor.min(len), head.min(len)))
                .collect();
            let mut view = TextView::new(buffer);
            view.selection = match ranges.is_empty() {
                true => Selection::point(0),
                false => {
                    let primary = primary.min(ranges.len() - 1);
                    Selection::new(ranges, primary)
                }
            };
            view.scroll.anchor = scroll.min(len);
            let view = match saved {
                Saved::Text => View::Text(view),
                Saved::Diff => View::Diff(DiffView {
                    text: view,
                    ..DiffView::new(buffer)
                }),
                Saved::Rendered { top, follows: link } => {
                    match link {
                        Some(slot) => follows.push((pane, buffer, slot)),
                        None => {}
                    }
                    let scroll = match top {
                        Top::Item(item) => Scroll::item(item),
                        Top::Line(line) => Scroll::Line(line),
                    };
                    View::Rendered(RenderedView {
                        text: view,
                        ..RenderedView::new(buffer, Link::Standalone, scroll)
                    })
                }
            };
            items.push(Item {
                view,
                used: state.layout.tick(),
            });
            kept.push(saved_index);
        }
        let active = (kept.iter().enumerate())
            .min_by_key(|(_, saved)| saved.abs_diff(active))
            .map(|(index, _)| index);
        let tabs = match active {
            None => None,
            Some(active) => {
                let preview = preview.and_then(|p| kept.iter().position(|&saved| saved == p));
                let now = state.layout.tick();
                let mut tabs = Tabs::from_parts(items, active, preview)
                    .map_err(|reason| SessionError::Tabs { pane, reason })?;
                tabs.activate(active, now);
                Some(tabs)
            }
        };
        ids.push(panes.insert(Pane { tabs }));
    }
    for (slot, buffer, source) in follows {
        let Some(&source) = ids.get(source).filter(|id| panes[**id].tabs.is_some()) else {
            continue;
        };
        let tabs = panes[ids[slot]].tabs.as_mut();
        match tabs.and_then(|tabs| {
            let index = tabs.find(TabKey::Rendered(buffer))?;
            Some(&mut tabs.items_mut()[index].view)
        }) {
            Some(View::Rendered(view)) => {
                view.link = Link::Follows {
                    pane: source,
                    seen: None,
                }
            }
            Some(View::Text(_) | View::Diff(_)) | None => {}
        }
    }
    state.layout = match ids.get(focus_slot) {
        Some(&focus) => Layout {
            root: restore_node(session.layout, &ids).node,
            panes,
            focus,
            clock: state.layout.clock,
        },
        None => Layout::new(),
    };
    // Panes whose files were all deleted close, as an emptied pane does.
    let empty: SmallVec<[PaneId; 4]> = (state.layout.panes.iter())
        .filter(|(_, pane)| pane.tabs.is_none())
        .map(|(id, _)| id)
        .collect();
    for pane in empty {
        match state.layout.close(pane) {
            Ok(()) | Err(CloseError::LastPane) => {}
            Err(CloseError::NotInLayout) => unreachable!("restored panes are leaves"),
        }
    }

    state.sidebar = match session.sidebar {
        SessionSidebar::Hidden => Sidebar::Hidden,
        SessionSidebar::Shown => Sidebar::Files(Focus::Editor),
        SessionSidebar::Changes => Sidebar::Changes(Focus::Editor),
    };
    state.sidebar_width = SidebarWidth::new(session.sidebar_width);
    for group in session.collapsed {
        state.git.folds[group as usize] = Fold::Collapsed;
    }
    for dir in session.expanded {
        match RelPath::parse(&dir) {
            Ok(path) if !path.is_root() => {
                state.tree.dirs.insert(
                    path.clone(),
                    Dir {
                        fold: Fold::Expanded,
                        listing: Listing::Loading,
                    },
                );
                out.push(Effect::Job(Job::ListDir {
                    root: state.root.path().to_owned(),
                    dir: path,
                }));
            }
            Ok(_) | Err(_) => {}
        }
    }
    tree::rebuild(&mut state.tree);
    Ok(())
}

/// Every pane index must exist and appear exactly once; splits need two or more children and
/// every weight must be positive.
fn check_node(node: &SessionNode, used: &mut [bool]) -> Result<(), SessionError> {
    match node {
        SessionNode::Pane { flex, .. } | SessionNode::Split { flex, .. }
            if Flex::new(*flex).is_none() =>
        {
            Err(SessionError::InvalidFlex(*flex))
        }
        SessionNode::Pane { pane, .. } => match used.get_mut(*pane) {
            None => Err(SessionError::MissingPane(*pane)),
            Some(true) => Err(SessionError::DuplicatePane(*pane)),
            Some(slot) => {
                *slot = true;
                Ok(())
            }
        },
        SessionNode::Split { children, .. } => match children.len() {
            0 | 1 => Err(SessionError::EmptySplit),
            _ => children
                .iter()
                .try_for_each(|child| check_node(child, used)),
        },
    }
}

/// Builds the layout tree from a node that passed `check_node`.
fn restore_node(node: SessionNode, ids: &[PaneId]) -> Child {
    let (flex, node) = match node {
        SessionNode::Pane { pane, flex } => (flex, Node::Pane(ids[pane])),
        SessionNode::Split {
            axis,
            children,
            flex,
        } => (
            flex,
            Node::Split {
                axis,
                children: children
                    .into_iter()
                    .map(|child| restore_node(child, ids))
                    .collect(),
            },
        ),
    };
    Child {
        flex: Flex::new(flex).expect("checked by check_node"),
        node,
    }
}
