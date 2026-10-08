use helix_core::Selection;
use model::window::{Axis, Side};
use slotmap::{SlotMap, new_key_type};

use crate::buffer::BufferId;

new_key_type! {
    pub struct PaneId;
}

/// Panes arranged in a split tree; `focus` always names a leaf of `root`.
pub struct Layout {
    pub panes: SlotMap<PaneId, Pane>,
    pub root: Node,
    pub focus: PaneId,
}

/// A split always has at least two children, none of which is a split along the same axis.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Node {
    Pane(PaneId),
    Split { axis: Axis, children: Vec<Node> },
}

/// Fraction of the editor area occupied by a pane.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

#[derive(Debug, PartialEq, Eq)]
pub struct LastPane;

impl Layout {
    pub fn new() -> Layout {
        let mut panes = SlotMap::with_key();
        let focus = panes.insert(Pane {
            view: View::Empty,
            parked: Vec::new(),
        });
        Layout {
            panes,
            root: Node::Pane(focus),
            focus,
        }
    }

    /// Places `pane` next to the focused pane along `axis` and focuses it.
    pub fn split(&mut self, axis: Axis, pane: Pane) -> PaneId {
        let id = self.panes.insert(pane);
        split_node(&mut self.root, self.focus, axis, id);
        self.focus = id;
        id
    }

    /// Removes `pane`, focusing the pane that took its place.
    pub fn close(&mut self, pane: PaneId) -> Result<(), LastPane> {
        match &self.root {
            Node::Pane(_) => Err(LastPane),
            Node::Split { .. } => {
                let order = self.leaves();
                let index = order.iter().position(|&p| p == pane).unwrap_or(0);
                remove_node(&mut self.root, pane);
                self.panes.remove(pane);
                match self.focus == pane {
                    true => {
                        let remaining = self.leaves();
                        self.focus = remaining[index.saturating_sub(1).min(remaining.len() - 1)];
                    }
                    false => {}
                }
                Ok(())
            }
        }
    }

    pub fn only(&mut self) {
        let focus = self.focus;
        self.panes.retain(|id, _| id == focus);
        self.root = Node::Pane(focus);
    }

    /// Panes in reading order.
    pub fn leaves(&self) -> Vec<PaneId> {
        let mut out = Vec::with_capacity(self.panes.len());
        collect_leaves(&self.root, &mut out);
        out
    }

    pub fn next(&self) -> PaneId {
        let order = self.leaves();
        let index = order.iter().position(|&p| p == self.focus).unwrap_or(0);
        order[(index + 1) % order.len()]
    }

    pub fn rects(&self) -> Vec<(PaneId, Rect)> {
        let mut out = Vec::with_capacity(self.panes.len());
        collect_rects(
            &self.root,
            Rect {
                x: 0.0,
                y: 0.0,
                w: 1.0,
                h: 1.0,
            },
            &mut out,
        );
        out
    }

    /// Nearest pane touching the focused pane's `side` that overlaps it across that side.
    pub fn neighbour(&self, side: Side) -> Option<PaneId> {
        let rects = self.rects();
        let (_, from) = *rects.iter().find(|(id, _)| *id == self.focus)?;
        const EPS: f32 = 1e-4;
        let overlap = |a0: f32, a1: f32, b0: f32, b1: f32| a0.max(b0) < a1.min(b1) - EPS;
        rects
            .iter()
            .filter(|(id, _)| *id != self.focus)
            .filter(|(_, r)| match side {
                Side::Left => {
                    (r.x + r.w - from.x).abs() < EPS
                        && overlap(r.y, r.y + r.h, from.y, from.y + from.h)
                }
                Side::Right => {
                    (from.x + from.w - r.x).abs() < EPS
                        && overlap(r.y, r.y + r.h, from.y, from.y + from.h)
                }
                Side::Up => {
                    (r.y + r.h - from.y).abs() < EPS
                        && overlap(r.x, r.x + r.w, from.x, from.x + from.w)
                }
                Side::Down => {
                    (from.y + from.h - r.y).abs() < EPS
                        && overlap(r.x, r.x + r.w, from.x, from.x + from.w)
                }
            })
            .min_by(|(_, a), (_, b)| {
                let distance = |r: &Rect| match side {
                    Side::Left | Side::Right => (r.y - from.y).abs(),
                    Side::Up | Side::Down => (r.x - from.x).abs(),
                };
                distance(a).total_cmp(&distance(b))
            })
            .map(|(id, _)| *id)
    }
}

impl Default for Layout {
    fn default() -> Self {
        Layout::new()
    }
}

fn split_node(node: &mut Node, target: PaneId, axis: Axis, new: PaneId) -> bool {
    match node {
        Node::Pane(id) if *id == target => {
            *node = Node::Split {
                axis,
                children: vec![Node::Pane(target), Node::Pane(new)],
            };
            true
        }
        Node::Pane(_) => false,
        Node::Split {
            axis: own,
            children,
        } => {
            let direct = children
                .iter()
                .position(|child| *child == Node::Pane(target));
            match (direct, *own == axis) {
                (Some(i), true) => {
                    children.insert(i + 1, Node::Pane(new));
                    true
                }
                (Some(_), false) | (None, _) => children
                    .iter_mut()
                    .any(|child| split_node(child, target, axis, new)),
            }
        }
    }
}

fn remove_node(node: &mut Node, target: PaneId) -> bool {
    match node {
        Node::Pane(_) => false,
        Node::Split { axis, children } => {
            let removed = match children.iter().position(|c| *c == Node::Pane(target)) {
                Some(i) => {
                    children.remove(i);
                    true
                }
                None => children.iter_mut().any(|child| remove_node(child, target)),
            };
            // Re-establish the invariants: no single-child splits, no same-axis nesting.
            let axis = *axis;
            let mut flat = Vec::with_capacity(children.len());
            for child in children.drain(..) {
                match child {
                    Node::Split {
                        axis: inner,
                        children: grand,
                    } if inner == axis => flat.extend(grand),
                    other => flat.push(other),
                }
            }
            *node = match flat.len() {
                1 => flat.pop().expect("one child"),
                _ => Node::Split {
                    axis,
                    children: flat,
                },
            };
            removed
        }
    }
}

fn collect_leaves(node: &Node, out: &mut Vec<PaneId>) {
    match node {
        Node::Pane(id) => out.push(*id),
        Node::Split { children, .. } => children.iter().for_each(|c| collect_leaves(c, out)),
    }
}

fn collect_rects(node: &Node, area: Rect, out: &mut Vec<(PaneId, Rect)>) {
    match node {
        Node::Pane(id) => out.push((*id, area)),
        Node::Split { axis, children } => {
            let share = 1.0 / children.len() as f32;
            for (i, child) in children.iter().enumerate() {
                let offset = i as f32 * share;
                let rect = match axis {
                    Axis::Row => Rect {
                        x: area.x + offset * area.w,
                        w: share * area.w,
                        ..area
                    },
                    Axis::Column => Rect {
                        y: area.y + offset * area.h,
                        h: share * area.h,
                        ..area
                    },
                };
                collect_rects(child, rect, out);
            }
        }
    }
}

pub struct Pane {
    pub view: View,
    /// Views of buffers this pane showed before, restored when switching back.
    pub parked: Vec<TextView>,
}

pub enum View {
    Empty,
    Text(TextView),
}

#[derive(Clone)]
pub struct TextView {
    pub buffer: BufferId,
    pub selection: Selection,
    pub scroll: Scroll,
}

/// Visible window of a text view; `anchor` is the first char of the top visual row.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Scroll {
    pub anchor: usize,
    pub viewport: Viewport,
}

/// Text area size in cells as last drawn; zero before the first frame.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Viewport {
    pub rows: u16,
    pub cols: u16,
}
