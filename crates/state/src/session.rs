use std::collections::HashMap;
use std::io;
use std::path::{Path, PathBuf};

use helix_core::{Range, Selection, SmallVec};
use model::window::Axis;
use serde::{Deserialize, Serialize};

use crate::apply;
use crate::buffer::Content;
use crate::job::Job;
use crate::layout::{
    Child, Flex, InvalidTabs, Item, Layout, Node, Pane, PaneId, Tabs, TextView, View,
};
use crate::path::RelPath;
use crate::tree::{self, Dir, Fold, Listing};
use crate::{Effect, Focus, Sidebar, SidebarWidth, State};

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
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SessionSidebar {
    Hidden,
    Shown,
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
                        let View::Text(view) = &item.view;
                        SessionItem::Text {
                            path: state.buffers[view.buffer()].path.clone(),
                            ranges: view
                                .selection
                                .ranges()
                                .iter()
                                .map(|range| [range.anchor, range.head])
                                .collect(),
                            primary: view.selection.primary_index(),
                            scroll: view.scroll.anchor,
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
        },
        sidebar_width: state.sidebar_width.get(),
        expanded,
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
            let SessionItem::Text { path, .. } = item;
            tabs[..index]
                .iter()
                .any(|SessionItem::Text { path: earlier, .. }| earlier == path)
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
            let SessionItem::Text {
                path,
                ranges,
                primary,
                scroll,
            } = item;
            let buffer = match path.is_file() {
                true => apply::load(state, path, out),
                false => None,
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
            items.push(Item {
                view: View::Text(view),
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
    state.layout = match ids.get(focus_slot) {
        Some(&focus) => Layout {
            root: restore_node(session.layout, &ids).node,
            panes,
            focus,
            clock: state.layout.clock,
        },
        None => Layout::new(),
    };

    state.sidebar = match session.sidebar {
        SessionSidebar::Hidden => Sidebar::Hidden,
        SessionSidebar::Shown => Sidebar::Files(Focus::Editor),
    };
    state.sidebar_width = SidebarWidth::new(session.sidebar_width);
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
