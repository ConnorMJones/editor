use helix_core::{Selection, SmallVec};
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
#[derive(Clone, Debug, PartialEq)]
pub enum Node {
    Pane(PaneId),
    Split { axis: Axis, children: Vec<Child> },
}

/// A split member; it spans `flex` over the sum of its siblings' flexes.
#[derive(Clone, Debug, PartialEq)]
pub struct Child {
    pub flex: Flex,
    pub node: Node,
}

/// A positive, finite share weight.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Flex(f32);

impl Flex {
    pub const EVEN: Flex = Flex(1.0);

    pub fn new(weight: f32) -> Option<Flex> {
        match weight.is_finite() && weight > 0.0 {
            true => Some(Flex(weight)),
            false => None,
        }
    }

    pub fn get(self) -> f32 {
        self.0
    }
}

impl Child {
    pub fn even(node: Node) -> Child {
        Child {
            flex: Flex::EVEN,
            node,
        }
    }
}

/// Child indices leading from the root to a split.
pub type SplitPath = SmallVec<[usize; 4]>;

/// Smallest share of a split a dragged divider leaves either neighbour.
pub const MIN_SHARE: f32 = 0.05;

/// Fraction of the editor area occupied by a pane.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Rect {
    pub const FULL: Rect = Rect {
        x: 0.0,
        y: 0.0,
        w: 1.0,
        h: 1.0,
    };
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

    /// Places `pane` next to the focused pane along `axis`, halving its space, and focuses it.
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
        collect_rects(&self.root, Rect::FULL, &mut out);
        out
    }

    /// Moves the divider after child `divider` of the split at `path` to `at`, a fraction of
    /// the editor area along the split's axis; stale paths are ignored.
    pub fn drag_divider(&mut self, path: &[usize], divider: usize, at: f32) {
        let mut area = Rect::FULL;
        let mut node = &mut self.root;
        for &step in path {
            let Node::Split { axis, children } = node else {
                return;
            };
            let Some(rect) = child_rects(*axis, children, area).nth(step) else {
                return;
            };
            area = rect;
            node = &mut children[step].node;
        }
        let Node::Split { axis, children } = node else {
            return;
        };
        let (start, extent) = match axis {
            Axis::Row => (area.x, area.w),
            Axis::Column => (area.y, area.h),
        };
        match (divider + 1 < children.len(), at.is_finite()) {
            (true, true) => {}
            (false, _) | (_, false) => return,
        }
        let total: f32 = children.iter().map(|child| child.flex.0).sum();
        let before: f32 = children[..divider]
            .iter()
            .map(|child| child.flex.0)
            .sum::<f32>()
            / total;
        let pair = (children[divider].flex.0 + children[divider + 1].flex.0) / total;
        let margin = MIN_SHARE.min(pair / 2.0);
        let first = ((at - start) / extent - before).clamp(margin, pair - margin);
        children[divider].flex = Flex(first * total);
        children[divider + 1].flex = Flex((pair - first) * total);
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
                children: vec![
                    Child::even(Node::Pane(target)),
                    Child::even(Node::Pane(new)),
                ],
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
                .position(|child| child.node == Node::Pane(target));
            match (direct, *own == axis) {
                (Some(i), true) => {
                    let half = Flex(children[i].flex.0 / 2.0);
                    children[i].flex = half;
                    children.insert(
                        i + 1,
                        Child {
                            flex: half,
                            node: Node::Pane(new),
                        },
                    );
                    true
                }
                (Some(_), false) | (None, _) => children
                    .iter_mut()
                    .any(|child| split_node(&mut child.node, target, axis, new)),
            }
        }
    }
}

fn remove_node(node: &mut Node, target: PaneId) -> bool {
    match node {
        Node::Pane(_) => false,
        Node::Split { axis, children } => {
            let removed = match children.iter().position(|c| c.node == Node::Pane(target)) {
                Some(i) => {
                    children.remove(i);
                    true
                }
                None => children
                    .iter_mut()
                    .any(|child| remove_node(&mut child.node, target)),
            };
            // Re-establish the invariants: no single-child splits, no same-axis nesting.
            let axis = *axis;
            let mut flat = Vec::with_capacity(children.len());
            for child in children.drain(..) {
                match child.node {
                    Node::Split {
                        axis: inner,
                        children: grand,
                    } if inner == axis => {
                        let total: f32 = grand.iter().map(|g| g.flex.0).sum();
                        flat.extend(grand.into_iter().map(|g| Child {
                            flex: Flex(child.flex.0 * g.flex.0 / total),
                            node: g.node,
                        }));
                    }
                    node => flat.push(Child {
                        flex: child.flex,
                        node,
                    }),
                }
            }
            *node = match flat.len() {
                1 => flat.pop().expect("one child").node,
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
        Node::Split { children, .. } => children.iter().for_each(|c| collect_leaves(&c.node, out)),
    }
}

/// Areas of a split's children, in order.
pub fn child_rects(axis: Axis, children: &[Child], area: Rect) -> impl Iterator<Item = Rect> {
    let total: f32 = children.iter().map(|child| child.flex.0).sum();
    children.iter().scan(0.0f32, move |offset, child| {
        let share = child.flex.0 / total;
        let rect = match axis {
            Axis::Row => Rect {
                x: area.x + *offset * area.w,
                w: share * area.w,
                ..area
            },
            Axis::Column => Rect {
                y: area.y + *offset * area.h,
                h: share * area.h,
                ..area
            },
        };
        *offset += share;
        Some(rect)
    })
}

fn collect_rects(node: &Node, area: Rect, out: &mut Vec<(PaneId, Rect)>) {
    match node {
        Node::Pane(id) => out.push((*id, area)),
        Node::Split { axis, children } => {
            for (child, rect) in children.iter().zip(child_rects(*axis, children, area)) {
                collect_rects(&child.node, rect, out);
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
