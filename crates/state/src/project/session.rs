use std::collections::HashMap;
use std::io;
use std::path::{Path, PathBuf};

use helix_core::{Range, Selection, SmallVec};
use model::window::Axis;
use serde::{Deserialize, Serialize};

use crate::apply;
use crate::job::Job;
use crate::project::path::{Folder, RelPath};
use crate::render::Opening;
use crate::render::document::DocumentScroll;
use crate::render::pages::PagesScroll;
use crate::sidebar::tree::{self, Dir, Fold, Listing};
use crate::text::buffer::Content;
use crate::view::layout::{
    Child, CloseError, Flex, InvalidTabs, Item, Layout, Node, Pane, PaneId, Tabs,
};
use crate::view::tab::{Beside, NoMode, Showing, Tab};
use crate::{Effect, Files, Focus, Sidebar, SidebarWidth, State};
use model::command::Shown;
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
    pub sidebar_width: SidebarWidth,
    /// Expanded tree directories relative to the root.
    pub expanded: Vec<RelPath>,
    /// Collapsed groups of the Changes sidebar.
    pub collapsed: Vec<Group>,
}

/// `flex` is the node's weight within its parent split.
#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase", deny_unknown_fields)]
pub enum SessionNode {
    Pane {
        pane: usize,
        flex: Flex,
    },
    Split {
        axis: Axis,
        children: Vec<SessionNode>,
        flex: Flex,
    },
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

/// A tab: its file, the mode it shows, and the selection and scroll its text keeps.
#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SessionItem {
    pub path: PathBuf,
    pub mode: SessionMode,
    /// `[anchor, head]` char offsets.
    pub ranges: Vec<[usize; 2]>,
    pub primary: usize,
    pub scroll: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase", deny_unknown_fields)]
pub enum SessionMode {
    Source,
    Diff,
    Render {
        top: Opening,
    },
    /// The source beside its rendering.
    Split {
        top: Opening,
    },
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
    #[error("pane {pane}, tab {tab}: the file has no rendering")]
    NoRendering { pane: usize, tab: usize },
    #[error("focus names pane {0}, which does not exist")]
    MissingFocus(usize),
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
        .map(|&pane| match &state.layout.panes()[pane].tabs {
            None => SessionPane::Empty,
            Some(tabs) => SessionPane::Tabs {
                tabs: tabs
                    .items()
                    .iter()
                    .map(|item| {
                        let view = item.tab.text();
                        let path = state.buffers[view.buffer()].path.clone();
                        let ranges = view
                            .selection
                            .ranges()
                            .iter()
                            .map(|range| [range.anchor, range.head])
                            .collect();
                        let mode = match item.tab.showing() {
                            Showing::Source(_) => SessionMode::Source,
                            Showing::Diff(..) => SessionMode::Diff,
                            Showing::Document(_, view, beside) => saved(
                                match view.scroll {
                                    DocumentScroll::Line(line) => Opening::Line(line),
                                    DocumentScroll::At(at)
                                    | DocumentScroll::Moved { from: at, .. } => {
                                        Opening::Item(at.item)
                                    }
                                },
                                beside,
                            ),
                            Showing::Pages(_, view, beside) => saved(
                                match view.scroll {
                                    PagesScroll::Line(line) => Opening::Line(line),
                                    PagesScroll::Point(at) => Opening::Item(at.page),
                                    PagesScroll::At(at) | PagesScroll::Moved { from: at, .. } => {
                                        Opening::Item(at.page)
                                    }
                                },
                                beside,
                            ),
                        };
                        SessionItem {
                            path,
                            mode,
                            ranges,
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
    let mut expanded: Vec<RelPath> = (state.tree.dirs.iter())
        .filter_map(|(folder, dir)| match (folder, dir.fold) {
            (Folder::Sub(path), Fold::Expanded) => Some(path.clone()),
            (Folder::Root, _) | (Folder::Sub(_), Fold::Collapsed) => None,
        })
        .collect();
    expanded.sort_unstable();
    Session {
        panes,
        layout: snapshot_node(&state.layout.root(), Flex::EVEN, &index),
        focus: index[&state.layout.focus()],
        sidebar: match &state.sidebar {
            Sidebar::Hidden => SessionSidebar::Hidden,
            Sidebar::Files(_) => SessionSidebar::Shown,
            Sidebar::Changes(_) => SessionSidebar::Changes,
        },
        sidebar_width: state.sidebar_width,
        expanded,
        collapsed: Group::ALL
            .into_iter()
            .filter(|group| state.changes.folds[*group as usize] == Fold::Collapsed)
            .collect(),
    }
}

fn snapshot_node(node: &Node, flex: Flex, index: &HashMap<PaneId, usize>) -> SessionNode {
    match node {
        Node::Pane(pane) => SessionNode::Pane {
            pane: index[pane],
            flex,
        },
        Node::Split { axis, children } => SessionNode::Split {
            axis: *axis,
            children: children
                .iter()
                .map(|c| snapshot_node(&c.node, c.flex, index))
                .collect(),
            flex,
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
    let mut panes = slotmap::SlotMap::with_key();
    let ids: Vec<PaneId> = (session.panes.iter())
        .map(|_| panes.insert(Pane::EMPTY))
        .collect();
    let mut slots: Vec<Option<PaneId>> = ids.iter().copied().map(Some).collect();
    let root = parse_node(session.layout, &mut slots)?.node;
    match slots.iter().position(Option::is_some) {
        Some(pane) => return Err(SessionError::UnusedPane(pane)),
        None => {}
    }
    let focus = *(ids.get(session.focus)).ok_or(SessionError::MissingFocus(session.focus))?;
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
                .any(|earlier| earlier.path == item.path)
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

    for (pane, saved) in session.panes.into_iter().enumerate() {
        let SessionPane::Tabs {
            tabs,
            active,
            preview,
        } = saved
        else {
            continue;
        };
        // Saved index of each kept item; files deleted since are dropped, not recreated.
        let mut kept: Vec<usize> = Vec::with_capacity(tabs.len());
        let mut items: Vec<Item> = Vec::with_capacity(tabs.len());
        for (saved_index, item) in tabs.into_iter().enumerate() {
            let SessionItem {
                path,
                mode,
                ranges,
                primary,
                scroll,
            } = item;
            // A diff also shows a file deleted from the worktree.
            let buffer = match (mode, path.is_file(), path.exists()) {
                (_, true, _) | (SessionMode::Diff, false, false) => apply::load(state, path, out),
                (
                    SessionMode::Source | SessionMode::Render { .. } | SessionMode::Split { .. },
                    false,
                    _,
                )
                | (SessionMode::Diff, false, true) => None,
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
            let mut tab = Tab::open(buffer, &state.buffers[buffer].path);
            let view = tab.text_mut();
            view.selection = match ranges.is_empty() {
                true => Selection::point(0),
                false => {
                    let primary = primary.min(ranges.len() - 1);
                    Selection::new(ranges, primary)
                }
            };
            view.scroll.anchor = scroll.min(len);
            // Only a rendering uses where it opens.
            let (shown, at) = match mode {
                SessionMode::Source => (Shown::Source, Opening::Line(0)),
                SessionMode::Diff => (Shown::Diff, Opening::Line(0)),
                SessionMode::Render { top } => (Shown::Render, top),
                SessionMode::Split { top } => (Shown::Split, top),
            };
            match tab.enter(shown, at) {
                Ok(()) => {}
                Err(NoMode) => {
                    return Err(SessionError::NoRendering {
                        pane,
                        tab: saved_index,
                    });
                }
            }
            items.push(Item {
                tab,
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
        panes[ids[pane]].tabs = tabs;
    }
    state.layout = Layout::restored(panes, root, focus, state.layout.clock);
    // Panes whose files were all deleted close, as an emptied pane does.
    let empty: SmallVec<[PaneId; 4]> = (state.layout.panes().iter())
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
        SessionSidebar::Shown => Sidebar::Files(Files::Editor),
        SessionSidebar::Changes => Sidebar::Changes(Focus::Editor),
    };
    state.sidebar_width = session.sidebar_width;
    for group in session.collapsed {
        state.changes.folds[group as usize] = Fold::Collapsed;
    }
    for path in session.expanded {
        let dir = Dir {
            fold: Fold::Expanded,
            listing: Listing::Loading,
        };
        state.tree.dirs.insert(Folder::Sub(path.clone()), dir);
        out.push(Effect::Job(Job::ListDir {
            root: state.project.root.path().to_owned(),
            dir: Folder::Sub(path),
        }));
    }
    tree::rebuild(&mut state.tree);
    Ok(())
}

/// Builds the layout tree, taking each pane's id from `slots` as the tree places it.
fn parse_node(node: SessionNode, slots: &mut [Option<PaneId>]) -> Result<Child, SessionError> {
    match node {
        SessionNode::Pane { pane, flex } => match slots.get_mut(pane) {
            None => Err(SessionError::MissingPane(pane)),
            Some(slot) => match slot.take() {
                Some(id) => Ok(Child {
                    flex,
                    node: Node::Pane(id),
                }),
                None => Err(SessionError::DuplicatePane(pane)),
            },
        },
        SessionNode::Split {
            axis,
            children,
            flex,
        } => match children.len() {
            0 | 1 => Err(SessionError::EmptySplit),
            _ => Ok(Child {
                flex,
                node: Node::Split {
                    axis,
                    children: (children.into_iter())
                        .map(|child| parse_node(child, slots))
                        .collect::<Result<_, _>>()?,
                },
            }),
        },
    }
}

/// How a rendering at `top` is saved: alone in its tab, or beside the source.
fn saved<S>(top: Opening, beside: Beside<S>) -> SessionMode {
    match beside {
        Beside::Alone => SessionMode::Render { top },
        Beside::Source(..) => SessionMode::Split { top },
    }
}
