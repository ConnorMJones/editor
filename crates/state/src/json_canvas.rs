use std::collections::HashMap;
use std::num::NonZeroU32;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use helix_core::Rope;
use ink::tool::Corner;
use jsoncanvas::{
    Area, Change, EdgeId, Edited, End, HistoryError, JsonCanvas, JsonCanvasColor, NewEdge, NewNode,
    NodeId, NodeKind, Offset, Picked, Revision, Side,
};
use model::command::{DraftCommand, JsonCanvasCommand};
use model::geom::{Pane, Point, Rect, Surface, Vector, segment_distance};
use model::mode::Mode;
use model::notice::Notice;
use model::picture::{Picture, Source};
use model::sheet::{Measure, PlaneView, Viewport, Zoom};
use model::units::Px;
use slotmap::{SlotMap, new_key_type};

use crate::draft::{Beside, Draft, Edits, Start};
use crate::drawing::command::Step;
use crate::job::Job;
use crate::project::files::FileIndex;
use crate::project::path::{FileKind, RelPath, TextKind};
use crate::reading::Taps;
use crate::render::markdown::{self, Base, Markdown};
use crate::text::buffer::Disk;
use crate::text::buffer::Target;
use crate::view::layout::PaneId;
use crate::view::panes::{Arrive, PanePoint};
use crate::view::tab::Tab;
use crate::{Confirm, Effect, Overlay, State};
use model::config::OpenAs;

pub const GRID: i64 = 20;
pub const LEAST_SIDE: i64 = 40;
pub const REACH: Px = Px(8.0);
pub const COPY_OFFSET: Offset = Offset {
    dx: 2 * GRID,
    dy: 2 * GRID,
};
pub const TEXT_CARD: (i64, i64) = (260, 60);
pub const FILE_CARD: (i64, i64) = (400, 400);
pub const GROUP_MARGIN: i64 = 40;
pub const BEND: f32 = 80.0;
pub const EDGE_SAMPLES: usize = 24;
pub const SIDES: [Side; 4] = [Side::Top, Side::Right, Side::Bottom, Side::Left];

new_key_type! {
    pub struct JsonCanvasId;
}

pub type JsonCanvasFiles = SlotMap<JsonCanvasId, JsonCanvasFile>;

pub struct JsonCanvasFile {
    pub path: Arc<Path>,
    pub edited: Edited,
    pub saved: Revision,
    pub files: HashMap<NodeId, Embedded>,
    pub parsed: Parsed,
    pub disk: Disk,
}

pub struct JsonCanvasRead {
    pub canvas: JsonCanvas,
    pub files: HashMap<NodeId, Embedded>,
}

#[derive(Clone)]
pub enum Embedded {
    Note { path: Arc<Path>, text: Arc<str> },
    Picture(Picture),
    Other { path: Arc<Path>, kind: FileKind },
    Missing(Arc<Path>),
}

pub enum Parsed {
    Stale,
    At {
        generation: u64,
        cards: HashMap<NodeId, Markdown>,
    },
}

pub struct JsonCanvasTab {
    file: JsonCanvasId, // private: tabs are found by their file; a changed file shows the wrong tab as open
    pub view: PlaneView,
    pub last: Option<Measure>,
    pub arrival: Arrival,
    pub doing: Doing,
}

pub enum Doing {
    Arranging(Arranging),
    Writing(Box<CardWriting>),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Arranging {
    pub picked: Option<Picked>,
    pub gesture: Gesture,
}

pub struct CardWriting {
    pub node: NodeId,
    pub draft: Draft,
    pub place: Place,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Place {
    InCard,
    Beside(Beside),
}

pub enum Ended {
    Arranging,
    Finished,
    Refused(jsoncanvas::ChangeError),
}

enum Next {
    Arrange,
    Write(NodeId),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Arrival {
    Fit,
    AsSaved,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Gesture {
    Idle,
    Pressed {
        at: Point<Surface>,
        on: Pressing,
        adding: Adding,
        reach: f32,
    },
    Moving(Moving),
    Resizing(Resizing),
    Boxing {
        from: Point<Surface>,
        to: Point<Surface>,
        adding: Adding,
    },
    Connecting {
        from: NodeId,
        side: Side,
        area: Area,
        to: Point<Surface>,
    },
}

pub enum Commit {
    Change(Change),
    Edge(NewEdge),
    Card(NewNode),
    Write(NodeId),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Moving {
    pub picked: Picked,
    pub carried: Vec<NodeId>,
    pub anchor: Area,
    pub from: Point<Surface>,
    pub to: Point<Surface>,
    pub snap: Snap,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Resizing {
    pub node: NodeId,
    pub corner: Corner,
    pub from: Area,
    pub to: Point<Surface>,
    pub snap: Snap,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Adding {
    Replace,
    Toggle,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Snap {
    Grid,
    Free,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Down,
    Move,
    Up,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pointer {
    pub at: Point<Pane>,
    pub viewport: Viewport,
    pub adding: Adding,
    pub snap: Snap,
    pub taps: Taps,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Hit {
    Handle {
        node: NodeId,
        corner: Corner,
        area: Area,
    },
    Side {
        node: NodeId,
        side: Side,
        area: Area,
    },
    Press(Pressing),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Pressing {
    Node { node: NodeId, area: Area },
    Edge(EdgeId),
    Empty,
}

impl Embedded {
    pub fn path(&self) -> &Path {
        match self {
            Embedded::Note { path, .. }
            | Embedded::Other { path, .. }
            | Embedded::Missing(path) => path,
            Embedded::Picture(picture) => match &picture.source {
                Source::File { path, .. } => path,
                Source::Bytes(_) | Source::Zipped { .. } => Path::new(""),
            },
        }
    }
}

impl Arranging {
    pub const IDLE: Arranging = Arranging {
        picked: None,
        gesture: Gesture::Idle,
    };
}

impl Doing {
    pub fn take(&mut self, file: &mut JsonCanvasFile) -> (Arranging, Ended) {
        match std::mem::replace(self, Doing::Arranging(Arranging::IDLE)) {
            Doing::Arranging(arranging) => (arranging, Ended::Arranging),
            Doing::Writing(mut writing) => {
                let ended = match file.keep(&mut writing.draft, &writing.node) {
                    Ok(()) => Ended::Finished,
                    Err(refused) => Ended::Refused(refused),
                };
                let picked = file.canvas().kept(Picked::node(writing.node));
                (
                    Arranging {
                        picked,
                        gesture: Gesture::Idle,
                    },
                    ended,
                )
            }
        }
    }
}

impl CardWriting {
    pub fn open(
        file: &JsonCanvasFile,
        node: NodeId,
    ) -> Result<CardWriting, jsoncanvas::ChangeError> {
        let found = (file.canvas().node(&node))
            .ok_or_else(|| jsoncanvas::ChangeError::UnknownNode(node.clone()))?;
        let text = match &found.kind {
            NodeKind::Text(text) => text,
            NodeKind::File { .. } | NodeKind::Link(_) | NodeKind::Group { .. } => {
                return Err(jsoncanvas::ChangeError::NotText(node));
            }
        };
        Ok(CardWriting {
            draft: Draft::new(text, Start::AsKept),
            node,
            place: Place::InCard,
        })
    }
}

impl JsonCanvasTab {
    pub fn new(file: JsonCanvasId, view: PlaneView, arrival: Arrival) -> JsonCanvasTab {
        JsonCanvasTab {
            file,
            view,
            last: None,
            arrival,
            doing: Doing::Arranging(Arranging::IDLE),
        }
    }

    pub fn file(&self) -> JsonCanvasId {
        self.file
    }

    pub fn open(file: JsonCanvasId, canvas: &JsonCanvasFile) -> JsonCanvasTab {
        let centre = canvas.bounds().map_or(Point::new(0.0, 0.0), |bounds| {
            Point::new(
                (bounds.min().x + bounds.max().x) / 2.0,
                (bounds.min().y + bounds.max().y) / 2.0,
            )
        });
        JsonCanvasTab::new(
            file,
            PlaneView {
                centre,
                zoom: Zoom::ONE,
            },
            Arrival::Fit,
        )
    }

    pub fn arranging(&self) -> Option<&Arranging> {
        match &self.doing {
            Doing::Arranging(arranging) => Some(arranging),
            Doing::Writing(_) => None,
        }
    }

    pub fn shown(&self, node: &jsoncanvas::Node) -> Area {
        let gesture = match &self.doing {
            Doing::Arranging(arranging) => &arranging.gesture,
            Doing::Writing(_) => return node.area,
        };
        match gesture {
            Gesture::Moving(moving) if moving.carried.contains(&node.id) => {
                node.area.moved(moving.offset())
            }
            Gesture::Resizing(resizing) if resizing.node == node.id => resizing.area(),
            Gesture::Idle
            | Gesture::Pressed { .. }
            | Gesture::Moving(_)
            | Gesture::Resizing(_)
            | Gesture::Boxing { .. }
            | Gesture::Connecting { .. } => node.area,
        }
    }
}

impl Moving {
    pub fn offset(&self) -> Offset {
        let x = self.anchor.x as f32 + self.to.x - self.from.x;
        let y = self.anchor.y as f32 + self.to.y - self.from.y;
        Offset {
            dx: snapped(x, self.snap) - self.anchor.x,
            dy: snapped(y, self.snap) - self.anchor.y,
        }
    }
}

impl Resizing {
    pub fn area(&self) -> Area {
        let from = self.from;
        let (x, y) = (snapped(self.to.x, self.snap), snapped(self.to.y, self.snap));
        let (left, right) = match self.corner {
            Corner::TopLeft | Corner::BottomLeft => {
                (x.min(from.right() - LEAST_SIDE), from.right())
            }
            Corner::TopRight | Corner::BottomRight => (from.x, x.max(from.x + LEAST_SIDE)),
        };
        let (top, bottom) = match self.corner {
            Corner::TopLeft | Corner::TopRight => {
                (y.min(from.bottom() - LEAST_SIDE), from.bottom())
            }
            Corner::BottomLeft | Corner::BottomRight => (from.y, y.max(from.y + LEAST_SIDE)),
        };
        area_at(left, top, right - left, bottom - top).unwrap_or(from)
    }
}

fn snapped(value: f32, snap: Snap) -> i64 {
    match snap {
        Snap::Grid => (value / GRID as f32).round() as i64 * GRID,
        Snap::Free => value.round() as i64,
    }
}

pub fn surface(area: Area) -> Rect<Surface> {
    Rect::spanning(
        Point::new(area.x as f32, area.y as f32),
        Point::new(area.right() as f32, area.bottom() as f32),
    )
}

impl JsonCanvasFile {
    pub fn new(path: &Path, read: JsonCanvasRead, disk: Disk) -> JsonCanvasFile {
        let edited = Edited::new(read.canvas);
        JsonCanvasFile {
            path: path.into(),
            saved: edited.revision(),
            edited,
            files: read.files,
            parsed: Parsed::Stale,
            disk,
        }
    }

    pub fn canvas(&self) -> &JsonCanvas {
        self.edited.canvas()
    }

    pub fn keep(
        &mut self,
        draft: &mut Draft,
        node: &NodeId,
    ) -> Result<(), jsoncanvas::ChangeError> {
        let text = draft.doc.text.to_string();
        let kept = match &self.canvas().node(node).map(|node| &node.kind) {
            Some(NodeKind::Text(kept)) => *kept == text,
            Some(NodeKind::File { .. } | NodeKind::Link(_) | NodeKind::Group { .. }) | None => {
                false
            }
        };
        match kept {
            true => {}
            false => {
                self.edited.change(&Change::Text {
                    node: node.clone(),
                    text,
                })?;
                self.parsed = Parsed::Stale;
            }
        }
        draft.kept();
        Ok(())
    }

    pub fn is_modified(&self) -> bool {
        self.edited.revision() != self.saved
    }

    pub fn replace(&mut self, read: JsonCanvasRead, disk: Disk) {
        self.saved = self.edited.replace(read.canvas);
        self.files.extend(read.files);
        self.parsed = Parsed::Stale;
        self.disk = disk;
    }

    pub fn bounds(&self) -> Option<Rect<Surface>> {
        (self.canvas().nodes().iter())
            .map(|node| surface(node.area))
            .reduce(|a, b| a.union(&b))
    }

    pub fn cards(&self) -> impl Iterator<Item = &Markdown> {
        let cards = match &self.parsed {
            Parsed::At { cards, .. } => Some(cards.values()),
            Parsed::Stale => None,
        };
        cards.into_iter().flatten()
    }

    pub fn card(&self, node: &NodeId) -> Option<&Markdown> {
        match &self.parsed {
            Parsed::At { cards, .. } => cards.get(node),
            Parsed::Stale => None,
        }
    }

    pub fn hit(&self, picked: Option<&Picked>, at: Point<Surface>, reach: f32) -> Hit {
        let canvas = self.canvas();
        let handle = (picked.into_iter())
            .flat_map(Picked::nodes)
            .filter_map(|id| canvas.node(id))
            .find_map(|node| {
                (Corner::AROUND.into_iter())
                    .find(|corner| corner.of(&surface(node.area)).distance(at) <= reach)
                    .map(|corner| Hit::Handle {
                        node: node.id.clone(),
                        corner,
                        area: node.area,
                    })
            });
        let side = || {
            (picked.into_iter())
                .flat_map(Picked::nodes)
                .filter_map(|id| canvas.node(id))
                .find_map(|node| {
                    (SIDES.into_iter())
                        .find(|side| anchor(node.area, *side).0.distance(at) <= reach)
                        .map(|side| Hit::Side {
                            node: node.id.clone(),
                            side,
                            area: node.area,
                        })
                })
        };
        let under = |groups: bool| {
            (canvas.nodes().iter().rev())
                .filter(move |node| matches!(node.kind, NodeKind::Group { .. }) == groups)
                .find(|node| surface(node.area).contains(at))
                .map(|node| Pressing::Node {
                    node: node.id.clone(),
                    area: node.area,
                })
        };
        let edge = || {
            (canvas.edges().iter().rev()).find_map(|edge| {
                let from = canvas.node(&edge.from.node)?;
                let to = canvas.node(&edge.to.node)?;
                let curve = curve(from.area, edge.from.side, to.area, edge.to.side);
                let points: Vec<Point<Surface>> = (0..=EDGE_SAMPLES)
                    .map(|step| along(curve, step as f32 / EDGE_SAMPLES as f32))
                    .collect();
                (points.windows(2))
                    .any(|pair| segment_distance(at, pair[0], pair[1]) <= reach)
                    .then(|| Pressing::Edge(edge.id.clone()))
            })
        };
        let pressed =
            (under(false).or_else(edge).or_else(|| under(true))).unwrap_or(Pressing::Empty);
        handle.or_else(side).unwrap_or(Hit::Press(pressed))
    }
}

pub fn parse(
    text: &str,
    root: &Path,
    beside: &Path,
) -> Result<JsonCanvasRead, jsoncanvas::JsonCanvasError> {
    let canvas = jsoncanvas::read(text)?;
    let files = (canvas.nodes().iter())
        .filter_map(|node| match &node.kind {
            NodeKind::File { file, .. } => {
                let at = ([root, beside].iter().map(|dir| dir.join(file)))
                    .find(|at| at.exists())
                    .unwrap_or_else(|| root.join(file));
                Some((node.id.clone(), embedded(&at)))
            }
            NodeKind::Text(_) | NodeKind::Link(_) | NodeKind::Group { .. } => None,
        })
        .collect();
    Ok(JsonCanvasRead { canvas, files })
}

pub fn embedded(path: &Path) -> Embedded {
    let modified = std::fs::metadata(path).and_then(|meta| meta.modified());
    let path: Arc<Path> = path.into();
    match (FileKind::of(&path), modified) {
        (_, Err(_)) => Embedded::Missing(path),
        (FileKind::Text(TextKind::Markdown), Ok(_)) => match std::fs::read_to_string(&path) {
            Ok(text) => Embedded::Note {
                path,
                text: text.into(),
            },
            Err(_) => Embedded::Missing(path),
        },
        (FileKind::Image(encoding), Ok(modified)) => Embedded::Picture(Picture {
            source: Source::File { path, modified },
            encoding,
        }),
        (kind, Ok(_)) => Embedded::Other { path, kind },
    }
}

pub fn embeds_read(state: &mut State, file: JsonCanvasId, files: Vec<(NodeId, Embedded)>) {
    match state.json_canvases.get_mut(file) {
        Some(canvas) => {
            canvas.files.extend(files);
            canvas.parsed = Parsed::Stale;
        }
        None => {}
    }
}

pub fn pointer(state: &mut State, pane: PaneId, phase: Phase, pointer: Pointer) {
    let State {
        layout,
        json_canvases,
        notice,
        input,
        ..
    } = state;
    match phase {
        Phase::Down => {
            let _ = layout.set_focus(pane);
        }
        Phase::Move | Phase::Up => {}
    }
    let Some(Tab::JsonCanvas(tab)) = layout.find_pane(pane).and_then(|pane| pane.active_mut())
    else {
        return;
    };
    let Some(file) = json_canvases.get_mut(tab.file) else {
        return;
    };
    let frame = tab.view.frame(pointer.viewport);
    let at = frame.from_pane.apply(pointer.at);
    let reach = REACH.0 / frame.scale;
    let snap = pointer.snap;
    match (&tab.doing, file.hit(None, at, reach)) {
        (Doing::Writing(writing), Hit::Press(Pressing::Node { node, .. }))
            if writing.place == Place::InCard && node == writing.node =>
        {
            return;
        }
        (Doing::Writing(_) | Doing::Arranging(_), _) => {}
    }
    let (mut tab_arranging, ended) = tab.doing.take(file);
    let arranging = &mut tab_arranging;
    match ended {
        Ended::Arranging => {}
        Ended::Finished => input.mode = Mode::Normal,
        Ended::Refused(refused) => {
            input.mode = Mode::Normal;
            *notice = Some(Notice::JsonCanvasRefused {
                reason: refused.to_string(),
            });
        }
    }
    let gesture = std::mem::replace(&mut arranging.gesture, Gesture::Idle);
    let (gesture, commit) = match (phase, gesture) {
        (Phase::Down, _) => match (
            file.hit(arranging.picked.as_ref(), at, reach),
            pointer.taps,
            pointer.adding,
        ) {
            (Hit::Handle { node, corner, area }, _, _) => (
                Gesture::Resizing(Resizing {
                    node,
                    corner,
                    from: area,
                    to: at,
                    snap,
                }),
                None,
            ),
            (Hit::Side { node, side, area }, _, _) => (
                Gesture::Connecting {
                    from: node,
                    side,
                    area,
                    to: at,
                },
                None,
            ),
            (Hit::Press(Pressing::Empty), Taps::Two, Adding::Replace) => {
                (Gesture::Idle, text_card(at).map(Commit::Card))
            }
            (Hit::Press(Pressing::Node { node, .. }), Taps::Two, Adding::Replace)
                if matches!(
                    file.canvas().node(&node).map(|found| &found.kind),
                    Some(NodeKind::Text(_))
                ) =>
            {
                arranging.picked = Some(Picked::node(node.clone()));
                (Gesture::Idle, Some(Commit::Write(node)))
            }
            (Hit::Press(on), _, adding) => {
                let one = match &on {
                    Pressing::Node { node, .. } => Some(Picked::node(node.clone())),
                    Pressing::Edge(edge) => Some(Picked::edge(edge.clone())),
                    Pressing::Empty => None,
                };
                arranging.picked = match (one, adding, arranging.picked.take()) {
                    (None, _, kept) => kept,
                    (Some(one), Adding::Toggle, Some(picked)) => picked.toggled(one),
                    (Some(one), Adding::Toggle, None) => Some(one),
                    (Some(one), Adding::Replace, Some(picked)) if picked.holds(&one) => {
                        Some(picked)
                    }
                    (Some(one), Adding::Replace, _) => Some(one),
                };
                (
                    Gesture::Pressed {
                        at,
                        on,
                        adding,
                        reach,
                    },
                    None,
                )
            }
        },
        (
            Phase::Move,
            Gesture::Pressed {
                at: from,
                on,
                adding,
                reach,
            },
        ) if from.distance(at) > reach => {
            let held = match &on {
                Pressing::Node { node, area } => (arranging.picked.clone())
                    .filter(|picked| picked.holds_node(node))
                    .map(|picked| (picked, *area)),
                Pressing::Edge(_) | Pressing::Empty => None,
            };
            let gesture = match (held, on) {
                (Some((picked, anchor)), _) => Gesture::Moving(Moving {
                    carried: file.canvas().carried(&picked),
                    picked,
                    anchor,
                    from,
                    to: at,
                    snap,
                }),
                (None, Pressing::Empty) => Gesture::Boxing {
                    from,
                    to: at,
                    adding,
                },
                (None, on @ (Pressing::Node { .. } | Pressing::Edge(_))) => Gesture::Pressed {
                    at: from,
                    on,
                    adding,
                    reach,
                },
            };
            (gesture, None)
        }
        (Phase::Move, Gesture::Moving(moving)) => (
            Gesture::Moving(Moving {
                to: at,
                snap,
                ..moving
            }),
            None,
        ),
        (Phase::Move, Gesture::Resizing(resizing)) => (
            Gesture::Resizing(Resizing {
                to: at,
                snap,
                ..resizing
            }),
            None,
        ),
        (Phase::Move, Gesture::Boxing { from, adding, .. }) => (
            Gesture::Boxing {
                from,
                to: at,
                adding,
            },
            None,
        ),
        (
            Phase::Move,
            Gesture::Connecting {
                from, side, area, ..
            },
        ) => (
            Gesture::Connecting {
                from,
                side,
                area,
                to: at,
            },
            None,
        ),
        (Phase::Move, gesture @ (Gesture::Idle | Gesture::Pressed { .. })) => (gesture, None),
        (Phase::Up, Gesture::Pressed { on, adding, .. }) => {
            match (on, adding) {
                (Pressing::Node { node, .. }, Adding::Replace) => {
                    arranging.picked = Some(Picked::node(node))
                }
                (Pressing::Edge(edge), Adding::Replace) => {
                    arranging.picked = Some(Picked::edge(edge))
                }
                (Pressing::Empty, Adding::Replace) => arranging.picked = None,
                (Pressing::Node { .. } | Pressing::Edge(_) | Pressing::Empty, Adding::Toggle) => {}
            }
            (Gesture::Idle, None)
        }
        (Phase::Up, Gesture::Moving(moving)) => {
            let commit = match moving.offset() {
                Offset { dx: 0, dy: 0 } => None,
                by => Some(Commit::Change(Change::Move {
                    picked: moving.picked,
                    by,
                })),
            };
            (Gesture::Idle, commit)
        }
        (Phase::Up, Gesture::Resizing(resizing)) => {
            let area = resizing.area();
            let commit = (area != resizing.from).then_some(Commit::Change(Change::Resize {
                node: resizing.node,
                area,
            }));
            (Gesture::Idle, commit)
        }
        (Phase::Up, Gesture::Boxing { from, to, adding }) => {
            let rect = Rect::spanning(from, to);
            let inside = (file.canvas().nodes().iter())
                .filter(|node| {
                    let area = surface(node.area);
                    rect.contains(area.min()) && rect.contains(area.max())
                })
                .map(|node| node.id.clone())
                .collect();
            let boxed = Picked::new(inside, Vec::new());
            arranging.picked = match (adding, arranging.picked.take(), boxed) {
                (Adding::Replace, _, boxed) => boxed,
                (Adding::Toggle, Some(picked), Some(boxed)) => picked.toggled(boxed),
                (Adding::Toggle, picked, None) => picked,
                (Adding::Toggle, None, boxed) => boxed,
            };
            (Gesture::Idle, None)
        }
        (Phase::Up, Gesture::Connecting { from, side, .. }) => {
            let commit = match file.hit(None, at, reach) {
                Hit::Press(Pressing::Node { node, area }) if node != from => {
                    Some(Commit::Edge(NewEdge {
                        from: End {
                            node: from,
                            side: Some(side),
                            cap: None,
                        },
                        to: End {
                            node,
                            side: Some(nearest_side(area, at)),
                            cap: None,
                        },
                    }))
                }
                Hit::Press(Pressing::Node { .. } | Pressing::Edge(_) | Pressing::Empty)
                | Hit::Handle { .. }
                | Hit::Side { .. } => None,
            };
            (Gesture::Idle, commit)
        }
        (Phase::Up, Gesture::Idle) => (Gesture::Idle, None),
    };
    arranging.gesture = gesture;
    let next = match commit {
        None => Ok(Next::Arrange),
        Some(Commit::Change(change)) => file.edited.change(&change).map(|_| Next::Arrange),
        Some(Commit::Edge(edge)) => (file.edited.add_edge(edge)).map(|id| {
            arranging.picked = Some(Picked::edge(id));
            Next::Arrange
        }),
        Some(Commit::Card(card)) => Ok(Next::Write(file.edited.add_node(card))),
        Some(Commit::Write(node)) => Ok(Next::Write(node)),
    };
    tab.doing = doing_after(next, tab_arranging, file, &mut input.mode, notice);
}

fn doing_after(
    next: Result<Next, jsoncanvas::ChangeError>,
    arranging: Arranging,
    file: &mut JsonCanvasFile,
    mode: &mut Mode,
    notice: &mut Option<Notice>,
) -> Doing {
    match next.and_then(|next| match next {
        Next::Arrange => Ok(None),
        Next::Write(node) => CardWriting::open(file, node).map(Some),
    }) {
        Ok(None) => Doing::Arranging(arranging),
        Ok(Some(writing)) => {
            file.parsed = Parsed::Stale;
            *mode = Mode::Insert;
            Doing::Writing(Box::new(writing))
        }
        Err(refused) => {
            *notice = Some(Notice::JsonCanvasRefused {
                reason: refused.to_string(),
            });
            Doing::Arranging(arranging)
        }
    }
}

pub fn command(state: &mut State, command: JsonCanvasCommand) {
    let State {
        layout,
        json_canvases,
        notice,
        input,
        ..
    } = state;
    let Some(Tab::JsonCanvas(tab)) = layout.focused_mut().active_mut() else {
        return *notice = Some(Notice::NotAJsonCanvas);
    };
    let file = &mut json_canvases[tab.file];
    let (mut arranging, ended) = tab.doing.take(file);
    match ended {
        Ended::Arranging => {}
        Ended::Finished => input.mode = Mode::Normal,
        Ended::Refused(refused) => {
            input.mode = Mode::Normal;
            *notice = Some(Notice::JsonCanvasRefused {
                reason: refused.to_string(),
            });
        }
    }
    arranging.gesture = Gesture::Idle;
    let picked = (arranging.picked.take()).and_then(|picked| file.canvas().kept(picked));
    let refused = |error: jsoncanvas::ChangeError| Notice::JsonCanvasRefused {
        reason: error.to_string(),
    };
    let next = match (command, picked) {
        (JsonCanvasCommand::Unpick, _) => Ok(Next::Arrange),
        (JsonCanvasCommand::Focus, picked) => {
            let canvas = file.canvas();
            let around = (picked.iter())
                .flat_map(Picked::nodes)
                .filter_map(|id| canvas.node(id))
                .map(|node| surface(node.area))
                .reduce(|a, b| a.union(&b));
            arranging.picked = picked;
            match (around, tab.last) {
                (Some(around), Some(measure)) => {
                    tab.view = PlaneView::fitted(around, measure.viewport);
                    Ok(Next::Arrange)
                }
                (None, _) => Err(Notice::NothingPicked),
                (Some(_), None) => Ok(Next::Arrange),
            }
        }
        (JsonCanvasCommand::AddText, picked) => match text_card(tab.view.centre) {
            Some(card) => Ok(Next::Write(file.edited.add_node(card))),
            None => {
                arranging.picked = picked;
                Err(Notice::JsonCanvasRefused {
                    reason: "the view is too far out for a card".to_owned(),
                })
            }
        },
        (
            JsonCanvasCommand::Delete
            | JsonCanvasCommand::Duplicate
            | JsonCanvasCommand::Recolor(_)
            | JsonCanvasCommand::Group
            | JsonCanvasCommand::Write,
            None,
        ) => Err(Notice::NothingPicked),
        (JsonCanvasCommand::Write, Some(picked)) => match picked.nodes() {
            [node] => Ok(Next::Write(node.clone())),
            _ => {
                arranging.picked = Some(picked);
                Err(Notice::JsonCanvasRefused {
                    reason: "pick one text card to write in".to_owned(),
                })
            }
        },
        (JsonCanvasCommand::Delete, Some(picked)) => (file.edited.change(&Change::Remove(picked)))
            .map(|_| Next::Arrange)
            .map_err(refused),
        (JsonCanvasCommand::Recolor(preset), Some(picked)) => {
            let change = Change::Recolor {
                picked: picked.clone(),
                color: preset.map(JsonCanvasColor::Preset),
            };
            arranging.picked = Some(picked);
            (file.edited.change(&change))
                .map(|_| Next::Arrange)
                .map_err(refused)
        }
        (JsonCanvasCommand::Duplicate, Some(picked)) => {
            match file.edited.duplicate(&picked, COPY_OFFSET) {
                Ok(copies) => {
                    let embeds: Vec<(NodeId, Embedded)> = (copies.from.iter())
                        .filter_map(|(copy, from)| {
                            Some((copy.clone(), file.files.get(from)?.clone()))
                        })
                        .collect();
                    file.files.extend(embeds);
                    file.parsed = Parsed::Stale;
                    arranging.picked = Some(copies.picked);
                    Ok(Next::Arrange)
                }
                Err(error) => {
                    arranging.picked = Some(picked);
                    Err(refused(error))
                }
            }
        }
        (JsonCanvasCommand::Group, Some(picked)) => {
            let canvas = file.canvas();
            let around = (picked.nodes().iter())
                .filter_map(|id| canvas.node(id))
                .map(|node| node.area)
                .reduce(|a, b| {
                    let (x, y) = (a.x.min(b.x), a.y.min(b.y));
                    let (right, bottom) = (a.right().max(b.right()), a.bottom().max(b.bottom()));
                    area_at(x, y, right - x, bottom - y).unwrap_or(a)
                });
            let group = around.and_then(|around| {
                let (x, y) = (around.x - GROUP_MARGIN, around.y - GROUP_MARGIN);
                area_at(
                    x,
                    y,
                    around.right() + GROUP_MARGIN - x,
                    around.bottom() + GROUP_MARGIN - y,
                )
            });
            match group {
                Some(area) => {
                    let id = file.edited.add_node(NewNode {
                        area,
                        color: None,
                        kind: NodeKind::Group {
                            label: None,
                            background: None,
                        },
                    });
                    arranging.picked = Some(Picked::node(id));
                    Ok(Next::Arrange)
                }
                None => {
                    arranging.picked = Some(picked);
                    Err(Notice::NothingPicked)
                }
            }
        }
    };
    tab.doing = match next {
        Ok(next) => doing_after(Ok(next), arranging, file, &mut input.mode, notice),
        Err(refused) => {
            *notice = Some(refused);
            Doing::Arranging(arranging)
        }
    };
}

pub fn drop_file(
    state: &mut State,
    pane: PaneId,
    path: PathBuf,
    point: PanePoint,
    out: &mut Vec<Effect>,
) -> Result<(), Notice> {
    let State {
        layout,
        json_canvases,
        project,
        input,
        ..
    } = state;
    let rel = RelPath::new(project.root.path(), &path)
        .map_err(|_| Notice::FileOutsideProject { path: path.clone() })?;
    let Some(Tab::JsonCanvas(tab)) = layout.find_pane(pane).and_then(|pane| pane.active_mut())
    else {
        return Err(Notice::NotAJsonCanvas);
    };
    let file = &mut json_canvases[tab.file];
    let at = tab.view.frame(point.viewport).from_pane.apply(point.at);
    let (width, height) = FILE_CARD;
    let area = area_at(
        snapped(at.x, Snap::Grid),
        snapped(at.y, Snap::Grid),
        width,
        height,
    )
    .ok_or(Notice::FileOutsideProject { path: path.clone() })?;
    let node = file.edited.add_node(NewNode {
        area,
        color: None,
        kind: NodeKind::File {
            file: rel.as_str().to_owned(),
            subpath: None,
        },
    });
    let (_, ended) = tab.doing.take(file);
    match ended {
        Ended::Arranging => {}
        Ended::Finished | Ended::Refused(_) => input.mode = Mode::Normal,
    }
    tab.doing = Doing::Arranging(Arranging {
        picked: Some(Picked::node(node.clone())),
        gesture: Gesture::Idle,
    });
    out.push(Effect::Job(Job::JsonCanvasFiles {
        file: tab.file,
        wanted: vec![(node, path)],
    }));
    Ok(())
}

pub fn area_at(x: i64, y: i64, width: i64, height: i64) -> Option<Area> {
    let side = |length: i64| u32::try_from(length).ok().and_then(NonZeroU32::new);
    Some(Area {
        x,
        y,
        width: side(width)?,
        height: side(height)?,
    })
}

fn text_card(centre: Point<Surface>) -> Option<NewNode> {
    let (width, height) = TEXT_CARD;
    let area = area_at(
        snapped(centre.x - width as f32 / 2.0, Snap::Grid),
        snapped(centre.y - height as f32 / 2.0, Snap::Grid),
        width,
        height,
    )?;
    Some(NewNode {
        area,
        color: None,
        kind: NodeKind::Text(String::new()),
    })
}

pub fn centre(area: Area) -> Point<Surface> {
    let bounds = surface(area);
    let (min, max) = (bounds.min(), bounds.max());
    Point::new((min.x + max.x) / 2.0, (min.y + max.y) / 2.0)
}

pub fn facing(from: Area, to: Area) -> Side {
    let (a, b) = (centre(from), centre(to));
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    use std::cmp::Ordering::{Equal, Greater, Less};
    match (
        dx.abs().total_cmp(&dy.abs()),
        dx.total_cmp(&0.0),
        dy.total_cmp(&0.0),
    ) {
        (Greater | Equal, Greater | Equal, _) => Side::Right,
        (Greater | Equal, Less, _) => Side::Left,
        (Less, _, Greater | Equal) => Side::Bottom,
        (Less, _, Less) => Side::Top,
    }
}

pub fn anchor(area: Area, side: Side) -> (Point<Surface>, Vector<Surface>) {
    let bounds = surface(area);
    let (min, max) = (bounds.min(), bounds.max());
    let middle = centre(area);
    match side {
        Side::Top => (Point::new(middle.x, min.y), Vector::new(0.0, -1.0)),
        Side::Right => (Point::new(max.x, middle.y), Vector::new(1.0, 0.0)),
        Side::Bottom => (Point::new(middle.x, max.y), Vector::new(0.0, 1.0)),
        Side::Left => (Point::new(min.x, middle.y), Vector::new(-1.0, 0.0)),
    }
}

pub fn nearest_side(area: Area, at: Point<Surface>) -> Side {
    (SIDES.into_iter())
        .min_by(|a, b| {
            let distance = |side: &Side| anchor(area, *side).0.distance(at);
            distance(a).total_cmp(&distance(b))
        })
        .unwrap_or(Side::Top)
}

pub fn curve(
    from: Area,
    from_side: Option<Side>,
    to: Area,
    to_side: Option<Side>,
) -> [Point<Surface>; 4] {
    let (start, out) = anchor(from, from_side.unwrap_or_else(|| facing(from, to)));
    let (end, into) = anchor(to, to_side.unwrap_or_else(|| facing(to, from)));
    let bend = (start.distance(end) * 0.4).clamp(BEND * 0.5, BEND * 3.0);
    [
        start,
        start + Vector::new(out.x * bend, out.y * bend),
        end + Vector::new(into.x * bend, into.y * bend),
        end,
    ]
}

pub fn along([a, b, c, d]: [Point<Surface>; 4], t: f32) -> Point<Surface> {
    let s = 1.0 - t;
    let (wa, wb, wc, wd) = (s * s * s, 3.0 * s * s * t, 3.0 * s * t * t, t * t * t);
    Point::new(
        wa * a.x + wb * b.x + wc * c.x + wd * d.x,
        wa * a.y + wb * b.y + wc * c.y + wd * d.y,
    )
}

pub fn history(
    tab: &mut JsonCanvasTab,
    file: &mut JsonCanvasFile,
    step: Step,
) -> Result<(), Notice> {
    let (arranging, _) = tab.doing.take(file);
    let stepped = match step {
        Step::Undo => file.edited.undo(),
        Step::Redo => file.edited.redo(),
    };
    file.parsed = Parsed::Stale;
    tab.doing = Doing::Arranging(Arranging {
        picked: (arranging.picked).and_then(|picked| file.canvas().kept(picked)),
        gesture: Gesture::Idle,
    });
    match stepped {
        Ok(_) => Ok(()),
        Err(HistoryError::NothingToUndo) => Err(Notice::AlreadyOldest),
        Err(HistoryError::NothingToRedo) => Err(Notice::AlreadyNewest),
    }
}

pub fn sync(state: &mut State, out: &mut Vec<Effect>) {
    let State {
        json_canvases,
        project,
        figures,
        documents,
        books,
        ..
    } = state;
    let FileIndex::Ready(indexed) = &mut project.index else {
        return;
    };
    let generation = indexed.generation;
    let root = project.root.path();
    let stale = |file: &&mut JsonCanvasFile| match file.parsed {
        Parsed::At { generation: at, .. } => at != generation,
        Parsed::Stale => true,
    };
    let (index, _) = indexed.links();
    let mut parsed = 0;
    for file in json_canvases.values_mut().filter(stale) {
        parsed += 1;
        let parse = |text: &str, at: &Path| {
            let dir = at.parent().unwrap_or(root);
            let folder =
                RelPath::new(root, dir).map_or(String::new(), |rel| rel.as_str().to_owned());
            let base = Base {
                dir,
                root,
                folder: &folder,
                vault: index,
            };
            markdown::parse(Rope::from(text).slice(..), base, None)
        };
        let cards = (file.canvas().nodes().iter())
            .filter_map(|node| match (&node.kind, file.files.get(&node.id)) {
                (NodeKind::Text(text), _) => Some((node.id.clone(), parse(text, &file.path))),
                (NodeKind::File { .. }, Some(Embedded::Note { path, text })) => {
                    Some((node.id.clone(), parse(text, path)))
                }
                (NodeKind::File { .. }, _) | (NodeKind::Link(_) | NodeKind::Group { .. }, _) => {
                    None
                }
            })
            .collect();
        file.parsed = Parsed::At { generation, cards };
    }
    match parsed {
        0 => {}
        _ => crate::render::typeset::mark(figures, documents, books, json_canvases, out),
    }
}

pub fn open_node(
    state: &mut State,
    pane: PaneId,
    file: JsonCanvasId,
    index: usize,
    out: &mut Vec<Effect>,
) {
    let Some(canvas) = state.json_canvases.get(file) else {
        return;
    };
    let Some(node) = canvas.canvas().nodes().get(index) else {
        return;
    };
    match &node.kind {
        NodeKind::File { file, subpath } => {
            let path = (canvas.files.get(&node.id)).map_or_else(
                || state.project.root.path().join(file),
                |at| at.path().to_owned(),
            );
            let to = (subpath.as_ref())
                .map(|subpath| subpath.as_str().trim_start_matches('#'))
                .filter(|heading| !heading.is_empty() && !heading.starts_with('^'))
                .map(|heading| Target::Heading(heading.to_owned()));
            match state.layout.set_focus(pane) {
                Ok(()) => crate::apply::open(state, path, Arrive::AsIs, to, OpenAs::Permanent, out),
                Err(_) => {}
            }
        }
        NodeKind::Link(url) => out.push(Effect::OpenUrl(url.clone())),
        NodeKind::Text(_) | NodeKind::Group { .. } => {}
    }
}

impl Place {
    pub fn beside(self) -> Option<Beside> {
        match self {
            Place::InCard => None,
            Place::Beside(beside) => Some(beside),
        }
    }
}

pub fn draft(state: &mut State, command: DraftCommand) {
    let State {
        layout,
        json_canvases,
        notice,
        overlay,
        input,
        ..
    } = state;
    let Some(Tab::JsonCanvas(tab)) = layout.focused_mut().active_mut() else {
        return;
    };
    let file = &mut json_canvases[tab.file];
    let Doing::Writing(writing) = &mut tab.doing else {
        return *notice = Some(Notice::NotWriting);
    };
    match (command, writing.draft.edits()) {
        (DraftCommand::Swap, _) => {
            writing.place = Place::Beside(match writing.place {
                Place::InCard => Beside::SPLIT,
                Place::Beside(beside) => beside.swapped(),
            })
        }
        (DraftCommand::Expand, _) => {
            writing.place = match writing.place {
                Place::InCard => Place::Beside(Beside::SPLIT),
                Place::Beside(_) => Place::InCard,
            }
        }
        (DraftCommand::Drop, Edits::Unsaved) => *overlay = Overlay::Confirm(Confirm::DropDraft),
        (DraftCommand::Drop, Edits::Saved) | (DraftCommand::Finish, _) => {
            let (arranging, ended) = tab.doing.take(file);
            input.mode = Mode::Normal;
            match ended {
                Ended::Arranging | Ended::Finished => {}
                Ended::Refused(refused) => {
                    *notice = Some(Notice::JsonCanvasRefused {
                        reason: refused.to_string(),
                    })
                }
            }
            tab.doing = Doing::Arranging(arranging);
        }
    }
}

pub fn drop_draft(state: &mut State) {
    let State {
        layout,
        json_canvases,
        input,
        ..
    } = state;
    let Some(Tab::JsonCanvas(tab)) = layout.focused_mut().active_mut() else {
        return;
    };
    let node = match &tab.doing {
        Doing::Writing(writing) => writing.node.clone(),
        Doing::Arranging(_) => return,
    };
    tab.doing = Doing::Arranging(Arranging {
        picked: json_canvases[tab.file].canvas().kept(Picked::node(node)),
        gesture: Gesture::Idle,
    });
    input.mode = Mode::Normal;
}

pub fn keep_drafts(
    layout: &mut crate::view::layout::Layout,
    json_canvases: &mut JsonCanvasFiles,
    which: impl Fn(JsonCanvasId) -> bool,
) -> Result<(), Notice> {
    let writing = (layout.panes_mut())
        .flat_map(|(_, pane)| pane.items_mut())
        .filter_map(|item| match &mut item.tab {
            Tab::JsonCanvas(tab) => match &mut tab.doing {
                Doing::Writing(writing) => Some((tab.file, writing)),
                Doing::Arranging(_) => None,
            },
            Tab::Text(_) | Tab::Ink(_) | Tab::Image(_) | Tab::Book(_) => None,
        })
        .filter(|(file, _)| which(*file));
    for (file, writing) in writing {
        json_canvases[file]
            .keep(&mut writing.draft, &writing.node)
            .map_err(|refused| Notice::JsonCanvasRefused {
                reason: refused.to_string(),
            })?;
    }
    Ok(())
}
