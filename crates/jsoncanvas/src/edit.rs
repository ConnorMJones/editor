use std::collections::{HashMap, HashSet, VecDeque};
use std::hash::{BuildHasher, RandomState};

use crate::{Area, EdgeId, JsonCanvas, JsonCanvasColor, Node, NodeId};

pub const KEPT: usize = 100;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Picked {
    // private: a pick holds at least one id and each id one time; an empty pick makes a
    // command change nothing and still add an undo step.
    nodes: Vec<NodeId>,
    edges: Vec<EdgeId>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Offset {
    pub dx: i64,
    pub dy: i64,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Change {
    Move {
        picked: Picked,
        by: Offset,
    },
    Resize {
        node: NodeId,
        area: Area,
    },
    Remove(Picked),
    Recolor {
        picked: Picked,
        color: Option<JsonCanvasColor>,
    },
    Text {
        node: NodeId,
        text: String,
    },
    Label {
        labelled: Labelled,
        label: Option<String>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Labelled {
    Group(NodeId),
    Edge(EdgeId),
}

#[derive(Debug, thiserror::Error, PartialEq)]
pub enum ChangeError {
    #[error("node \"{}\" is not in the canvas", .0.as_str())]
    UnknownNode(NodeId),
    #[error("edge \"{}\" is not in the canvas", .0.as_str())]
    UnknownEdge(EdgeId),
    #[error("node \"{}\" is not a text card", .0.as_str())]
    NotText(NodeId),
    #[error("node \"{}\" is not a group", .0.as_str())]
    NotGroup(NodeId),
    #[error("an edge can't join node \"{}\" to itself", .0.as_str())]
    SameNode(NodeId),
}

#[derive(Debug, thiserror::Error, PartialEq)]
pub enum HistoryError {
    #[error("nothing to undo")]
    NothingToUndo,
    #[error("nothing to redo")]
    NothingToRedo,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NewNode {
    pub area: Area,
    pub color: Option<JsonCanvasColor>,
    pub kind: crate::NodeKind,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NewEdge {
    pub from: crate::End,
    pub to: crate::End,
}

pub struct Copies {
    pub picked: Picked,
    pub from: HashMap<NodeId, NodeId>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Revision(u64); // private: each state gets a new number; a made-up number can match the saved state by mistake

#[derive(Clone, Debug)]
pub struct Edited {
    // private: undo and the saved check use these fields together; a change to one field only
    // restores the wrong state or shows a changed file as saved.
    canvas: JsonCanvas,
    revision: Revision,
    next: u64,
    past: VecDeque<(JsonCanvas, Revision)>,
    future: Vec<(JsonCanvas, Revision)>,
}

impl Picked {
    pub fn new(nodes: Vec<NodeId>, edges: Vec<EdgeId>) -> Option<Picked> {
        let mut seen = HashSet::new();
        let nodes: Vec<NodeId> = nodes
            .into_iter()
            .filter(|id| seen.insert(id.clone()))
            .collect();
        let mut seen = HashSet::new();
        let edges: Vec<EdgeId> = edges
            .into_iter()
            .filter(|id| seen.insert(id.clone()))
            .collect();
        match (nodes.as_slice(), edges.as_slice()) {
            ([], []) => None,
            _ => Some(Picked { nodes, edges }),
        }
    }

    pub fn node(id: NodeId) -> Picked {
        Picked {
            nodes: vec![id],
            edges: Vec::new(),
        }
    }

    pub fn edge(id: EdgeId) -> Picked {
        Picked {
            nodes: Vec::new(),
            edges: vec![id],
        }
    }

    pub fn nodes(&self) -> &[NodeId] {
        &self.nodes
    }

    pub fn edges(&self) -> &[EdgeId] {
        &self.edges
    }

    pub fn holds_node(&self, id: &NodeId) -> bool {
        self.nodes.contains(id)
    }

    pub fn holds_edge(&self, id: &EdgeId) -> bool {
        self.edges.contains(id)
    }

    pub fn holds(&self, other: &Picked) -> bool {
        other.nodes.iter().all(|id| self.holds_node(id))
            && other.edges.iter().all(|id| self.holds_edge(id))
    }

    pub fn toggled(self, other: Picked) -> Option<Picked> {
        fn flip<T: PartialEq>(mine: Vec<T>, theirs: Vec<T>) -> Vec<T> {
            let (dropped, added): (Vec<T>, Vec<T>) =
                theirs.into_iter().partition(|id| mine.contains(id));
            (mine.into_iter())
                .filter(|id| !dropped.contains(id))
                .chain(added)
                .collect()
        }
        Picked::new(flip(self.nodes, other.nodes), flip(self.edges, other.edges))
    }
}

impl Area {
    pub fn moved(self, by: Offset) -> Area {
        Area {
            x: self.x.saturating_add(by.dx),
            y: self.y.saturating_add(by.dy),
            ..self
        }
    }

    pub fn right(self) -> i64 {
        self.x.saturating_add(i64::from(self.width.get()))
    }

    pub fn bottom(self) -> i64 {
        self.y.saturating_add(i64::from(self.height.get()))
    }

    pub fn holds(self, other: Area) -> bool {
        other.x >= self.x
            && other.y >= self.y
            && other.right() <= self.right()
            && other.bottom() <= self.bottom()
    }
}

impl JsonCanvas {
    pub fn nodes(&self) -> &[Node] {
        &self.nodes
    }

    pub fn edges(&self) -> &[crate::Edge] {
        &self.edges
    }

    pub fn node(&self, id: &NodeId) -> Option<&Node> {
        self.nodes.iter().find(|node| node.id == *id)
    }

    pub fn kept(&self, picked: Picked) -> Option<Picked> {
        let Picked { nodes, edges } = picked;
        Picked::new(
            (nodes.into_iter())
                .filter(|id| self.nodes.iter().any(|node| node.id == *id))
                .collect(),
            (edges.into_iter())
                .filter(|id| self.edges.iter().any(|edge| edge.id == *id))
                .collect(),
        )
    }

    fn check(&self, picked: &Picked) -> Result<(), ChangeError> {
        let nodes: HashSet<&NodeId> = self.nodes.iter().map(|node| &node.id).collect();
        let edges: HashSet<&EdgeId> = self.edges.iter().map(|edge| &edge.id).collect();
        match (
            picked.nodes.iter().find(|id| !nodes.contains(id)),
            picked.edges.iter().find(|id| !edges.contains(id)),
        ) {
            (Some(node), _) => Err(ChangeError::UnknownNode(node.clone())),
            (None, Some(edge)) => Err(ChangeError::UnknownEdge(edge.clone())),
            (None, None) => Ok(()),
        }
    }

    pub fn carried(&self, picked: &Picked) -> Vec<NodeId> {
        let groups: Vec<Area> = (self.nodes.iter())
            .filter(|node| picked.holds_node(&node.id))
            .filter(|node| matches!(node.kind, crate::NodeKind::Group { .. }))
            .map(|node| node.area)
            .collect();
        (self.nodes.iter())
            .filter(|node| {
                picked.holds_node(&node.id) || groups.iter().any(|group| group.holds(node.area))
            })
            .map(|node| node.id.clone())
            .collect()
    }

    pub fn apply(&mut self, change: &Change) -> Result<(), ChangeError> {
        match change {
            Change::Move { picked, by } => {
                self.check(picked)?;
                let carried = self.carried(picked);
                for node in self
                    .nodes
                    .iter_mut()
                    .filter(|node| carried.contains(&node.id))
                {
                    node.area = node.area.moved(*by);
                }
            }
            Change::Resize { node, area } => {
                let found = (self.nodes.iter_mut())
                    .find(|kept| kept.id == *node)
                    .ok_or_else(|| ChangeError::UnknownNode(node.clone()))?;
                found.area = *area;
            }
            Change::Remove(picked) => {
                self.check(picked)?;
                self.nodes.retain(|node| !picked.holds_node(&node.id));
                self.edges.retain(|edge| {
                    !picked.holds_edge(&edge.id)
                        && !picked.holds_node(&edge.from.node)
                        && !picked.holds_node(&edge.to.node)
                });
            }
            Change::Text { node, text } => {
                let found = (self.nodes.iter_mut())
                    .find(|kept| kept.id == *node)
                    .ok_or_else(|| ChangeError::UnknownNode(node.clone()))?;
                match &mut found.kind {
                    crate::NodeKind::Text(kept) => kept.clone_from(text),
                    crate::NodeKind::File { .. }
                    | crate::NodeKind::Link(_)
                    | crate::NodeKind::Group { .. } => {
                        return Err(ChangeError::NotText(node.clone()));
                    }
                }
            }
            Change::Label {
                labelled: Labelled::Group(node),
                label,
            } => {
                let found = (self.nodes.iter_mut())
                    .find(|kept| kept.id == *node)
                    .ok_or_else(|| ChangeError::UnknownNode(node.clone()))?;
                match &mut found.kind {
                    crate::NodeKind::Group { label: kept, .. } => kept.clone_from(label),
                    crate::NodeKind::Text(_)
                    | crate::NodeKind::File { .. }
                    | crate::NodeKind::Link(_) => {
                        return Err(ChangeError::NotGroup(node.clone()));
                    }
                }
            }
            Change::Label {
                labelled: Labelled::Edge(edge),
                label,
            } => {
                let found = (self.edges.iter_mut())
                    .find(|kept| kept.id == *edge)
                    .ok_or_else(|| ChangeError::UnknownEdge(edge.clone()))?;
                found.label.clone_from(label);
            }
            Change::Recolor { picked, color } => {
                self.check(picked)?;
                for node in self
                    .nodes
                    .iter_mut()
                    .filter(|node| picked.holds_node(&node.id))
                {
                    node.color = *color;
                }
                for edge in self
                    .edges
                    .iter_mut()
                    .filter(|edge| picked.holds_edge(&edge.id))
                {
                    edge.color = *color;
                }
            }
        }
        Ok(())
    }

    fn taken(&self) -> HashSet<String> {
        (self.nodes.iter().map(|node| node.id.0.clone()))
            .chain(self.edges.iter().map(|edge| edge.id.0.clone()))
            .collect()
    }

    pub fn add_node(&mut self, new: NewNode) -> NodeId {
        let id = NodeId(fresh(&mut self.taken()));
        let NewNode { area, color, kind } = new;
        let at = match kind {
            crate::NodeKind::Group { .. } => 0,
            crate::NodeKind::Text(_) | crate::NodeKind::File { .. } | crate::NodeKind::Link(_) => {
                self.nodes.len()
            }
        };
        self.nodes.insert(
            at,
            Node {
                id: id.clone(),
                area,
                color,
                kind,
                extra: crate::Fields::new(),
            },
        );
        id
    }

    pub fn add_edge(&mut self, new: NewEdge) -> Result<EdgeId, ChangeError> {
        let NewEdge { from, to } = new;
        self.check(&Picked {
            nodes: vec![from.node.clone(), to.node.clone()],
            edges: Vec::new(),
        })?;
        match from.node == to.node {
            true => Err(ChangeError::SameNode(from.node)),
            false => {
                let id = EdgeId(fresh(&mut self.taken()));
                self.edges.push(crate::Edge {
                    id: id.clone(),
                    from,
                    to,
                    color: None,
                    label: None,
                    extra: crate::Fields::new(),
                });
                Ok(id)
            }
        }
    }

    pub fn duplicate(&mut self, picked: &Picked, by: Offset) -> Result<Copies, ChangeError> {
        self.check(picked)?;
        let carried = self.carried(picked);
        let mut taken = self.taken();
        let mut fresh = || fresh(&mut taken);
        let renamed: HashMap<NodeId, NodeId> = (carried.iter())
            .map(|id| (id.clone(), NodeId(fresh())))
            .collect();
        let nodes: Vec<Node> = (self.nodes.iter())
            .filter_map(|node| {
                renamed.get(&node.id).map(|id| Node {
                    id: id.clone(),
                    area: node.area.moved(by),
                    ..node.clone()
                })
            })
            .collect();
        let edges: Vec<crate::Edge> = (self.edges.iter())
            .filter_map(
                |edge| match (renamed.get(&edge.from.node), renamed.get(&edge.to.node)) {
                    (Some(from), Some(to)) => {
                        let mut copy = edge.clone();
                        copy.id = EdgeId(fresh());
                        copy.from.node = from.clone();
                        copy.to.node = to.clone();
                        Some(copy)
                    }
                    _ => None,
                },
            )
            .collect();
        let copies = Copies {
            picked: Picked {
                nodes: nodes.iter().map(|node| node.id.clone()).collect(),
                edges: Vec::new(),
            },
            from: renamed.into_iter().map(|(from, to)| (to, from)).collect(),
        };
        self.nodes.extend(nodes);
        self.edges.extend(edges);
        Ok(copies)
    }
}

impl Edited {
    pub fn new(canvas: JsonCanvas) -> Edited {
        Edited {
            canvas,
            revision: Revision(0),
            next: 1,
            past: VecDeque::new(),
            future: Vec::new(),
        }
    }

    pub fn canvas(&self) -> &JsonCanvas {
        &self.canvas
    }

    pub fn revision(&self) -> Revision {
        self.revision
    }

    pub fn change(&mut self, change: &Change) -> Result<Revision, ChangeError> {
        self.step(|canvas| canvas.apply(change))?;
        Ok(self.revision)
    }

    pub fn duplicate(&mut self, picked: &Picked, by: Offset) -> Result<Copies, ChangeError> {
        self.step(|canvas| canvas.duplicate(picked, by))
    }

    pub fn add_node(&mut self, new: NewNode) -> NodeId {
        let before = self.canvas.clone();
        let id = self.canvas.add_node(new);
        self.advance(before);
        id
    }

    pub fn add_edge(&mut self, new: NewEdge) -> Result<EdgeId, ChangeError> {
        self.step(|canvas| canvas.add_edge(new))
    }

    fn step<R>(
        &mut self,
        change: impl FnOnce(&mut JsonCanvas) -> Result<R, ChangeError>,
    ) -> Result<R, ChangeError> {
        let before = self.canvas.clone();
        match change(&mut self.canvas) {
            Ok(done) => {
                self.advance(before);
                Ok(done)
            }
            Err(error) => {
                self.canvas = before;
                Err(error)
            }
        }
    }

    pub fn replace(&mut self, canvas: JsonCanvas) -> Revision {
        let before = std::mem::replace(&mut self.canvas, canvas);
        self.advance(before)
    }

    fn advance(&mut self, before: JsonCanvas) -> Revision {
        self.past.push_back((before, self.revision));
        while self.past.len() > KEPT {
            self.past.pop_front();
        }
        self.future.clear();
        self.revision = Revision(self.next);
        self.next += 1;
        self.revision
    }

    pub fn undo(&mut self) -> Result<Revision, HistoryError> {
        let (canvas, revision) = self.past.pop_back().ok_or(HistoryError::NothingToUndo)?;
        let current = std::mem::replace(&mut self.canvas, canvas);
        self.future.push((current, self.revision));
        self.revision = revision;
        Ok(revision)
    }

    pub fn redo(&mut self) -> Result<Revision, HistoryError> {
        let (canvas, revision) = self.future.pop().ok_or(HistoryError::NothingToRedo)?;
        let current = std::mem::replace(&mut self.canvas, canvas);
        self.past.push_back((current, self.revision));
        self.revision = revision;
        Ok(revision)
    }
}

fn fresh(taken: &mut HashSet<String>) -> String {
    loop {
        let id = format!("{:016x}", RandomState::new().hash_one(taken.len()));
        match taken.insert(id.clone()) {
            true => return id,
            false => continue,
        }
    }
}
