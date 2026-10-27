use std::collections::HashMap;
use std::io;
use std::path::{Path, PathBuf};

use helix_core::SmallVec;
use model::geom::Point;
use model::sheet::{Fit, PageIndex, PlaneView, SheetView, Spot, Zoom};
use model::window::Axis;
use serde::{Deserialize, Serialize};

use crate::apply;
use crate::drawing::{self, InkTabs, PdfMode, ResumeIn};
use crate::job::Job;
use crate::project::path::{Folder, RelPath};
use crate::render::Opening;
use crate::render::document::DocumentScroll;
use crate::render::pages::Locating;
use crate::sidebar::tree::{self, Dir, Fold, Listing};
use crate::text::buffer::{BufferId, Saved, Target};
use crate::view::layout::{
    Child, CloseError, Flex, InvalidTabs, Item, Layout, LayoutError, Node, Pane, PaneId, Split,
    SplitError, Tabs,
};
use crate::view::tab::{Beside, NoMode, Opened, Showing, Tab, TextTabs};
use crate::viewer::ImageTab;
use crate::{Effect, Files, Focus, Sidebar, SidebarWidth, State};
use ink::doc::LayerIndex;
use model::command::Shown;
use model::git::Group;

#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Session {
    pub panes: Vec<SessionPane>,
    pub layout: SessionNode,
    pub focus: usize,
    pub sidebar: SessionSidebar,
    pub sidebar_width: SidebarWidth,
    pub expanded: Vec<RelPath>,
    pub collapsed: Vec<Group>,
}

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

enum Restoring {
    Text(SessionMode, Vec<[usize; 2]>, usize, usize),
    Reopen(Reopen),
}

enum Reopen {
    Drawing(drawing::Saved),
    Image(SheetView),
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
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

#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SessionItem {
    pub path: PathBuf,
    pub view: SessionView,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase", deny_unknown_fields)]
pub enum SessionView {
    Text {
        mode: SessionMode,
        ranges: Vec<[usize; 2]>,
        primary: usize,
        scroll: usize,
    },
    Paged {
        layer: usize,
        page: usize,
        x: f32,
        y: f32,
        fit: Fit,
        zoom: f32,
    },
    Pdf {
        layer: usize,
        page: usize,
        x: f32,
        y: f32,
        fit: Fit,
        zoom: f32,
        mode: ResumeIn,
    },
    // `x` and `y` are the board point at the pane's centre.
    Board {
        layer: usize,
        x: f32,
        y: f32,
        zoom: f32,
    },
    // `x` and `y` are the point on the image at the pane's centre.
    Image {
        x: f32,
        y: f32,
        fit: Fit,
        zoom: f32,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase", deny_unknown_fields)]
pub enum SessionMode {
    Source,
    Diff,
    Render { top: Opening },
    Split { top: Opening },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SessionSidebar {
    Hidden,
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
    #[error(transparent)]
    Split(#[from] SplitError),
    #[error(transparent)]
    Layout(#[from] LayoutError),
    #[error("pane {0} is not placed in the layout")]
    UnusedPane(usize),
    #[error("pane {pane}, tab {tab}: the file has no rendering")]
    NoRendering { pane: usize, tab: usize },
    #[error("pane {pane}, tab {tab}: the primary selection does not exist")]
    Selection { pane: usize, tab: usize },
    #[error("pane {pane}, tab {tab}: the file is not the kind of file the tab was")]
    Kind { pane: usize, tab: usize },
    #[error("focus names pane {0}, which does not exist")]
    MissingFocus(usize),
    #[error("pane {pane}: {reason}")]
    Tabs { pane: usize, reason: InvalidTabs },
}

pub fn file(dir: &Path, root: &Path) -> PathBuf {
    let name = root
        .to_string_lossy()
        .replace('%', "%25")
        .replace('/', "%2F");
    dir.join(format!("{name}.toml"))
}

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
                    .map(|item| SessionItem {
                        path: state.path_of(item.tab.opened()).to_owned(),
                        view: match &item.tab {
                            Tab::Text(text) => text_view(text),
                            Tab::Ink(InkTabs::Paged(tab)) => SessionView::Paged {
                                layer: tab.hand.layer.0,
                                page: tab.view.centre.page.0,
                                x: tab.view.centre.at.x,
                                y: tab.view.centre.at.y,
                                fit: tab.view.fit,
                                zoom: tab.view.zoom.get(),
                            },
                            Tab::Ink(InkTabs::Pdf(tab)) => SessionView::Pdf {
                                layer: tab.hand.layer.0,
                                page: tab.view.centre.page.0,
                                x: tab.view.centre.at.x,
                                y: tab.view.centre.at.y,
                                fit: tab.view.fit,
                                zoom: tab.view.zoom.get(),
                                mode: match tab.mode {
                                    PdfMode::Read(_) => ResumeIn::Read,
                                    PdfMode::Markup => ResumeIn::Markup,
                                },
                            },
                            Tab::Ink(InkTabs::Board(tab)) => SessionView::Board {
                                layer: tab.hand.layer.0,
                                x: tab.view.centre.x,
                                y: tab.view.centre.y,
                                zoom: tab.view.zoom.get(),
                            },
                            Tab::Image(tab) => SessionView::Image {
                                x: tab.view.centre.at.x,
                                y: tab.view.centre.at.y,
                                fit: tab.view.fit,
                                zoom: tab.view.zoom.get(),
                            },
                        },
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
        Node::Split(split) => SessionNode::Split {
            axis: split.axis(),
            children: (split.children().iter())
                .map(|c| snapshot_node(&c.node, c.flex, index))
                .collect(),
            flex,
        },
    }
}

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

    let mut places: Vec<(PaneId, BufferId, Saved)> = Vec::new();
    for (pane, saved) in session.panes.into_iter().enumerate() {
        let SessionPane::Tabs {
            tabs,
            active,
            preview,
        } = saved
        else {
            continue;
        };
        let mut kept: Vec<usize> = Vec::with_capacity(tabs.len());
        let mut items: Vec<Item> = Vec::with_capacity(tabs.len());
        for (saved_index, item) in tabs.into_iter().enumerate() {
            let SessionItem { path, view } = item;
            let wrong_kind = SessionError::Kind {
                pane,
                tab: saved_index,
            };
            let restoring = match view {
                SessionView::Text {
                    mode,
                    ranges,
                    primary,
                    scroll,
                } => Restoring::Text(mode, ranges, primary, scroll),
                SessionView::Paged {
                    layer,
                    page,
                    x,
                    y,
                    fit,
                    zoom,
                } => Restoring::Reopen(Reopen::Drawing(drawing::Saved::Paged {
                    layer: LayerIndex(layer),
                    view: SheetView {
                        centre: Spot {
                            page: PageIndex(page),
                            at: Point::new(x, y),
                        },
                        fit,
                        zoom: Zoom::clamped(zoom),
                    },
                })),
                SessionView::Pdf {
                    layer,
                    page,
                    x,
                    y,
                    fit,
                    zoom,
                    mode,
                } => Restoring::Reopen(Reopen::Drawing(drawing::Saved::Pdf {
                    layer: LayerIndex(layer),
                    view: SheetView {
                        centre: Spot {
                            page: PageIndex(page),
                            at: Point::new(x, y),
                        },
                        fit,
                        zoom: Zoom::clamped(zoom),
                    },
                    mode,
                })),
                SessionView::Board { layer, x, y, zoom } => {
                    Restoring::Reopen(Reopen::Drawing(drawing::Saved::Board {
                        layer: LayerIndex(layer),
                        view: PlaneView {
                            centre: Point::new(x, y),
                            zoom: Zoom::clamped(zoom),
                        },
                    }))
                }
                SessionView::Image { x, y, fit, zoom } => {
                    Restoring::Reopen(Reopen::Image(SheetView {
                        centre: Spot {
                            page: PageIndex(0),
                            at: Point::new(x, y),
                        },
                        fit,
                        zoom: Zoom::clamped(zoom),
                    }))
                }
            };
            let (mode, ranges, primary, scroll) = match restoring {
                Restoring::Text(mode, ranges, primary, scroll) => (mode, ranges, primary, scroll),
                Restoring::Reopen(reopen) => {
                    match path.is_file() {
                        true => {}
                        false => continue,
                    }
                    let tab = match apply::read_now(state, &path, out) {
                        Ok(opened) => match (reopen, opened) {
                            (Reopen::Drawing(saved), opened) => {
                                drawing::reopen(state, opened, saved)
                            }
                            (Reopen::Image(view), Opened::Image(id)) => {
                                Some(Tab::Image(ImageTab::new(id, view)))
                            }
                            (
                                Reopen::Image(_),
                                Opened::Text(_)
                                | Opened::Paged(_)
                                | Opened::Board(_)
                                | Opened::Pdf(_),
                            ) => None,
                        }
                        .ok_or(wrong_kind)?,
                        Err(notice) => {
                            state.notice = Some(notice);
                            continue;
                        }
                    };
                    items.push(Item {
                        tab,
                        used: state.layout.tick(),
                    });
                    kept.push(saved_index);
                    continue;
                }
            };
            let saved = Saved::new(&ranges, primary, scroll).ok_or(SessionError::Selection {
                pane,
                tab: saved_index,
            })?;
            let buffer = match (mode, path.is_file(), path.exists()) {
                (_, true, _) | (SessionMode::Diff, false, false) => {
                    match apply::read_now(state, &path, out) {
                        Ok(Opened::Text(buffer)) => buffer,
                        Ok(
                            Opened::Paged(_) | Opened::Board(_) | Opened::Pdf(_) | Opened::Image(_),
                        ) => {
                            return Err(wrong_kind);
                        }
                        Err(notice) => {
                            state.notice = Some(notice);
                            continue;
                        }
                    }
                }
                (
                    SessionMode::Source | SessionMode::Render { .. } | SessionMode::Split { .. },
                    false,
                    _,
                )
                | (SessionMode::Diff, false, true) => continue,
            };
            let mut tab = TextTabs::open(buffer, state.buffers[buffer].kind);
            places.push((ids[pane], buffer, saved));
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
                tab: Tab::Text(tab),
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
    state.layout = Layout::restored(panes, root, focus, state.layout.clock)?;
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
    for (pane, buffer, saved) in places {
        apply::place(state, pane, buffer, Target::Saved(saved));
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
        } => {
            let children = (children.into_iter())
                .map(|child| parse_node(child, slots))
                .collect::<Result<_, _>>()?;
            Ok(Child {
                flex,
                node: Node::Split(Split::new(axis, children)?),
            })
        }
    }
}

fn saved<S>(top: Opening, beside: Beside<S>) -> SessionMode {
    match beside {
        Beside::Alone => SessionMode::Render { top },
        Beside::Source(..) => SessionMode::Split { top },
    }
}

fn text_view(text: &TextTabs) -> SessionView {
    let view = text.text();
    let mode = match text.showing() {
        Showing::Source(_) => SessionMode::Source,
        Showing::Diff(..) => SessionMode::Diff,
        Showing::Document(_, view, beside) => saved(
            match view.scroll {
                DocumentScroll::Line(line) => Opening::Line(line),
                DocumentScroll::At(at) | DocumentScroll::Moved { from: at, .. } => {
                    Opening::Item(at.item)
                }
            },
            beside,
        ),
        Showing::Pages(_, view, beside) => saved(
            match view.locating {
                Locating::Line(line) => Opening::Line(line),
                Locating::Settled => Opening::Item(view.view.centre.page.0),
            },
            beside,
        ),
    };
    SessionView::Text {
        mode,
        ranges: (view.selection.ranges().iter())
            .map(|range| [range.anchor, range.head])
            .collect(),
        primary: view.selection.primary_index(),
        scroll: view.scroll.anchor,
    }
}
