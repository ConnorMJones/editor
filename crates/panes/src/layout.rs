use model::window::{Axis, Side};
use slotmap::new_key_type;
use smallvec::SmallVec;

new_key_type! {
    pub struct PaneId;
}

#[derive(Clone, Debug, PartialEq)]
pub enum Node {
    Pane(PaneId),
    Split(Split),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Split {
    // private: a split has two or more children and none splits along its own axis; a direct change
    // makes splits that the neighbour search and the divider drags read wrongly.
    axis: Axis,
    children: Vec<Child>,
}

#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum SplitError {
    #[error("a split needs at least two children")]
    TooFew,
    #[error("child {0} splits along its parent's axis")]
    SameAxis(usize),
}

impl Split {
    pub fn new(axis: Axis, children: Vec<Child>) -> Result<Split, SplitError> {
        let same = (children.iter()).position(|child| match &child.node {
            Node::Split(inner) => inner.axis == axis,
            Node::Pane(_) => false,
        });
        match (children.len(), same) {
            (0 | 1, _) => Err(SplitError::TooFew),
            (_, Some(index)) => Err(SplitError::SameAxis(index)),
            (_, None) => Ok(Split { axis, children }),
        }
    }

    pub fn axis(&self) -> Axis {
        self.axis
    }

    pub fn children(&self) -> &[Child] {
        &self.children
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Child {
    pub flex: Flex,
    pub node: Node,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Place {
    Into { pane: PaneId, at: Option<usize> },
    Beside { pane: PaneId, side: Side },
}

#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(try_from = "f32", into = "f32")]
pub struct Flex(f32); // private: a weight of zero or less gives a pane no space or a negative size

#[derive(Debug, thiserror::Error)]
#[error("split weight {0} is not a positive number")]
pub struct InvalidFlex(pub f32);

impl TryFrom<f32> for Flex {
    type Error = InvalidFlex;

    fn try_from(weight: f32) -> Result<Flex, InvalidFlex> {
        Flex::new(weight).ok_or(InvalidFlex(weight))
    }
}

impl From<Flex> for f32 {
    fn from(flex: Flex) -> f32 {
        flex.0
    }
}

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

pub type SplitPath = SmallVec<[usize; 4]>;

pub const MIN_SHARE: f32 = 0.05;
pub const STEP: f32 = 0.05;

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

#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum LayoutError {
    #[error("the layout's panes and the panes given differ")]
    Leaves,
    #[error("the focused pane is not in the layout")]
    Focus,
}

#[derive(Debug, PartialEq, Eq)]
pub enum CloseError {
    LastPane,
    NotInLayout,
}

#[derive(Debug, PartialEq, Eq, thiserror::Error)]
#[error("pane {0:?} is not in the layout")]
pub struct NotInLayout(pub PaneId);

use model::command::PaneResize;
use slotmap::SlotMap;

use crate::tabs::{Item, Pane, Tabbed, Tabs, Tick};

pub struct Layout<T> {
    // private: the tree's leaves are these panes and the focus is one of them; a direct change
    // leaves a pane that is never drawn or a focus on a pane that does not exist.
    panes: SlotMap<PaneId, Pane<T>>,
    root: Node,
    focus: PaneId,
    maximised: Maximised, // private: a maximised pane that closed or lost the focus would hide every other pane
    pub clock: Tick,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Maximised {
    No,
    Pane(PaneId),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Shown<'a> {
    Tree(&'a Node),
    One(PaneId),
}

impl<T: Tabbed> Layout<T> {
    pub fn new() -> Layout<T> {
        let mut panes = SlotMap::with_key();
        let focus = panes.insert(Pane::EMPTY);
        Layout {
            panes,
            root: Node::Pane(focus),
            focus,
            maximised: Maximised::No,
            clock: Tick::default(),
        }
    }

    pub fn restored(
        panes: SlotMap<PaneId, Pane<T>>,
        root: Node,
        focus: PaneId,
        clock: Tick,
    ) -> Result<Layout<T>, LayoutError> {
        let mut leaves = Vec::with_capacity(panes.len());
        collect_leaves(&root, &mut leaves);
        let count = leaves.len();
        leaves.sort();
        leaves.dedup();
        let exact = leaves.len() == count
            && count == panes.len()
            && leaves.iter().all(|leaf| panes.contains_key(*leaf));
        exact.then_some(()).ok_or(LayoutError::Leaves)?;
        (panes.contains_key(focus))
            .then_some(())
            .ok_or(LayoutError::Focus)?;
        Ok(Layout {
            panes,
            root,
            focus,
            maximised: Maximised::No,
            clock,
        })
    }

    pub fn panes(&self) -> &SlotMap<PaneId, Pane<T>> {
        &self.panes
    }

    pub fn root(&self) -> &Node {
        &self.root
    }

    pub fn focus(&self) -> PaneId {
        self.focus
    }

    pub fn focused(&self) -> &Pane<T> {
        &self.panes[self.focus]
    }

    pub fn focused_mut(&mut self) -> &mut Pane<T> {
        &mut self.panes[self.focus]
    }

    pub fn pane(&mut self, pane: PaneId) -> &mut Pane<T> {
        &mut self.panes[pane]
    }

    pub fn find_pane(&mut self, pane: PaneId) -> Option<&mut Pane<T>> {
        self.panes.get_mut(pane)
    }

    pub fn panes_mut(&mut self) -> impl Iterator<Item = (PaneId, &mut Pane<T>)> {
        self.panes.iter_mut()
    }

    pub fn set_focus(&mut self, pane: PaneId) -> Result<(), NotInLayout> {
        match self.panes.contains_key(pane) {
            true => {
                self.focus_on(pane);
                Ok(())
            }
            false => Err(NotInLayout(pane)),
        }
    }

    pub fn focus_side(&mut self, side: Side) -> Option<PaneId> {
        let pane = self.neighbour(side)?;
        self.focus_on(pane);
        Some(pane)
    }

    pub fn focus_next(&mut self) -> PaneId {
        self.focus_on(self.next());
        self.focus
    }

    fn focus_on(&mut self, pane: PaneId) {
        match self.maximised {
            Maximised::Pane(shown) if shown != pane => self.maximised = Maximised::No,
            Maximised::Pane(_) | Maximised::No => {}
        }
        self.focus = pane;
    }

    pub fn tick(&mut self) -> Tick {
        self.clock.0 += 1;
        self.clock
    }

    pub fn keep(&mut self, key: T::Key) {
        for pane in self.panes.values_mut() {
            match &mut pane.tabs {
                Some(tabs)
                    if tabs
                        .preview()
                        .is_some_and(|preview| tabs.items()[preview].tab.key() == key) =>
                {
                    tabs.keep()
                }
                Some(_) | None => {}
            }
        }
    }

    pub fn split(&mut self, target: PaneId, side: Side, tabs: Tabs<T>) -> Option<PaneId> {
        self.panes.get(target)?;
        let id = self.panes.insert(Pane { tabs: Some(tabs) });
        beside(&mut self.root, target, side, id);
        self.maximised = Maximised::No;
        self.focus = id;
        Some(id)
    }

    pub fn split_copy(&mut self, target: PaneId, side: Side) -> Option<PaneId> {
        let now = self.tick();
        let tab = self.panes.get(target)?.active()?.split_copy();
        self.split(target, side, Tabs::split_copy(tab, now))
    }

    pub fn move_tab(&mut self, from: PaneId, index: usize, to: Place) -> Option<PaneId> {
        let now = self.tick();
        let count = self.panes.get(from)?.items().len();
        self.panes.get(from)?.items().get(index)?;
        let landed = match to {
            // A pane's only tab split off beside itself would leave the layout as it was.
            Place::Beside { pane, .. } if pane == from && count == 1 => from,
            Place::Beside { pane, side } => {
                self.panes.get(pane)?;
                let item = self.remove_tab(from, index)?;
                let id = self.panes.insert(Pane {
                    tabs: Some(Tabs::one(item)),
                });
                beside(&mut self.root, pane, side, id);
                self.maximised = Maximised::No;
                id
            }
            Place::Into { pane, at } if pane == from => {
                let tabs = self.panes[from].tabs.as_mut()?;
                tabs.reorder(index, at.unwrap_or(tabs.active()), now);
                from
            }
            Place::Into { pane, at } => {
                self.panes.get(pane)?;
                let item = self.remove_tab(from, index)?;
                let target = &mut self.panes[pane].tabs;
                match target {
                    Some(tabs) => tabs.take(item, at, now),
                    None => *target = Some(Tabs::one(item)),
                }
                pane
            }
        };
        self.focus_on(landed);
        Some(landed)
    }

    pub fn close(&mut self, pane: PaneId) -> Result<(), CloseError> {
        let beside = match &self.root {
            Node::Pane(_) => return Err(CloseError::LastPane),
            Node::Split(_) => remove_node(&mut self.root, pane).ok_or(CloseError::NotInLayout)?,
        };
        self.panes.remove(pane);
        self.maximised = Maximised::No;
        match self.focus == pane {
            true => self.focus = beside,
            false => {}
        }
        Ok(())
    }

    pub fn remove_tab(&mut self, pane: PaneId, index: usize) -> Option<Item<T>> {
        let now = self.tick();
        let target = self.panes.get_mut(pane)?;
        target.tabs.as_ref()?.items().get(index)?;
        let (rest, item) = target.tabs.take()?.remove(index, now);
        match rest {
            Some(tabs) => target.tabs = Some(tabs),
            None => match self.close(pane) {
                Ok(()) | Err(CloseError::LastPane) => {}
                Err(CloseError::NotInLayout) => unreachable!("panes are leaves of the layout"),
            },
        }
        Some(item)
    }

    pub fn only(&mut self) {
        let focus = self.focus;
        self.panes.retain(|id, _| id == focus);
        self.root = Node::Pane(focus);
        self.maximised = Maximised::No;
    }

    pub fn leaves(&self) -> Vec<PaneId> {
        let mut out = Vec::with_capacity(self.panes.len());
        collect_leaves(&self.root, &mut out);
        out
    }

    pub fn next(&self) -> PaneId {
        let order = self.leaves();
        let index =
            (order.iter().position(|&p| p == self.focus)).expect("the focused pane is a leaf");
        order[(index + 1) % order.len()]
    }

    pub fn rects(&self) -> Vec<(PaneId, Rect)> {
        let mut out = Vec::with_capacity(self.panes.len());
        collect_rects(&self.root, Rect::FULL, &mut out);
        out
    }

    pub fn drag_divider(&mut self, path: &[usize], divider: usize, at: f32) {
        let mut area = Rect::FULL;
        let mut node = &mut self.root;
        for &step in path {
            let Node::Split(split) = node else {
                return;
            };
            let Some(rect) = child_rects(split.axis, &split.children, area).nth(step) else {
                return;
            };
            area = rect;
            node = &mut split.children[step].node;
        }
        let Node::Split(Split { axis, children }) = node else {
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

    pub fn neighbour(&self, side: Side) -> Option<PaneId> {
        let rects = self.rects();
        let (_, from) =
            *(rects.iter().find(|(id, _)| *id == self.focus)).expect("the focused pane is a leaf");
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

impl<T: Tabbed> Default for Layout<T> {
    fn default() -> Self {
        Layout::new()
    }
}

fn beside(node: &mut Node, target: PaneId, side: Side, new: PaneId) -> bool {
    let axis = side.axis();
    match node {
        Node::Pane(id) if *id == target => {
            let pair = match side {
                Side::Left | Side::Up => [new, target],
                Side::Right | Side::Down => [target, new],
            };
            *node = Node::Split(Split {
                axis,
                children: pair.map(|pane| Child::even(Node::Pane(pane))).to_vec(),
            });
            true
        }
        Node::Pane(_) => false,
        Node::Split(split) => {
            let direct = (split.children.iter()).position(|child| child.node == Node::Pane(target));
            match (direct, split.axis == axis) {
                // Like vim's equalalways and zed, adding a pane evens out its split.
                (Some(index), true) => {
                    let at = match side {
                        Side::Left | Side::Up => index,
                        Side::Right | Side::Down => index + 1,
                    };
                    split.children.insert(at, Child::even(Node::Pane(new)));
                    for child in split.children.iter_mut() {
                        child.flex = Flex::EVEN;
                    }
                    true
                }
                (Some(_), false) | (None, _) => (split.children.iter_mut())
                    .any(|child| beside(&mut child.node, target, side, new)),
            }
        }
    }
}

fn remove_node(node: &mut Node, target: PaneId) -> Option<PaneId> {
    let Node::Split(Split { children, .. }) = node else {
        return None;
    };
    let count = children.len();
    let beside = match children.iter().position(|c| c.node == Node::Pane(target)) {
        Some(index) => {
            children.remove(index);
            let neighbour = index.saturating_sub(1);
            let mut leaf = &children[neighbour].node;
            loop {
                leaf = match leaf {
                    Node::Pane(id) => break *id,
                    Node::Split(Split { children, .. }) => match index {
                        0 => &children[0].node,
                        _ => &children[children.len() - 1].node,
                    },
                };
            }
        }
        None => children
            .iter_mut()
            .find_map(|child| remove_node(&mut child.node, target))?,
    };
    flatten(node, count);
    Some(beside)
}

fn flatten(node: &mut Node, count: usize) {
    let Node::Split(Split { axis, children }) = node else {
        return;
    };
    let axis = *axis;
    let mut flat: Vec<Child> = Vec::with_capacity(children.len());
    for child in children.drain(..) {
        match child.node {
            Node::Split(Split {
                axis: inner,
                children: grand,
            }) if inner == axis => flat.extend(grand),
            node => flat.push(Child {
                flex: child.flex,
                node,
            }),
        }
    }
    // Like vim's equalalways and zed, a split whose members changed is evened out.
    match flat.len() == count {
        true => {}
        false => {
            for child in flat.iter_mut() {
                child.flex = Flex::EVEN;
            }
        }
    }
    *node = match flat.len() {
        1 => flat.pop().expect("one child").node,
        _ => Node::Split(Split {
            axis,
            children: flat,
        }),
    };
}

fn collect_leaves(node: &Node, out: &mut Vec<PaneId>) {
    match node {
        Node::Pane(id) => out.push(*id),
        Node::Split(split) => (split.children.iter()).for_each(|c| collect_leaves(&c.node, out)),
    }
}

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
        Node::Split(Split { axis, children }) => {
            for (child, rect) in children.iter().zip(child_rects(*axis, children, area)) {
                collect_rects(&child.node, rect, out);
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Reached {
    Missing,
    Below,
    Done,
}

impl<T: Tabbed> Layout<T> {
    pub fn shown(&self) -> Shown<'_> {
        match self.maximised {
            Maximised::Pane(pane) => Shown::One(pane),
            Maximised::No => Shown::Tree(&self.root),
        }
    }

    pub fn maximised(&self) -> Maximised {
        self.maximised
    }

    pub fn toggle_maximise(&mut self) {
        self.maximised = match self.maximised {
            Maximised::No => Maximised::Pane(self.focus),
            Maximised::Pane(_) => Maximised::No,
        };
    }

    pub fn swap(&mut self, side: Side) -> Option<PaneId> {
        let other = self.neighbour(side)?;
        swap_leaves(&mut self.root, self.focus, other);
        self.maximised = Maximised::No;
        Some(other)
    }

    pub fn transpose(&mut self) {
        let Some(split) = holding(&mut self.root, self.focus) else {
            return;
        };
        split.axis = match split.axis {
            Axis::Row => Axis::Column,
            Axis::Column => Axis::Row,
        };
        settle(&mut self.root);
        self.maximised = Maximised::No;
    }

    pub fn resize(&mut self, resize: PaneResize) {
        let focus = self.focus;
        let by = |grow: f32| move |share: f32| share + grow;
        match resize {
            PaneResize::Even => even(&mut self.root),
            PaneResize::Narrower => {
                reshare(&mut self.root, focus, Axis::Row, &by(-STEP));
            }
            PaneResize::Wider => {
                reshare(&mut self.root, focus, Axis::Row, &by(STEP));
            }
            PaneResize::Shorter => {
                reshare(&mut self.root, focus, Axis::Column, &by(-STEP));
            }
            PaneResize::Taller => {
                reshare(&mut self.root, focus, Axis::Column, &by(STEP));
            }
            PaneResize::WidestHere => {
                fill(&mut self.root, focus, Axis::Row);
            }
            PaneResize::TallestHere => {
                fill(&mut self.root, focus, Axis::Column);
            }
        }
    }

    pub fn move_tab_to(&mut self, side: Side) -> Option<PaneId> {
        let from = self.focus;
        let index = self.panes.get(from)?.tabs.as_ref()?.active();
        let to = match self.neighbour(side) {
            Some(pane) => Place::Into { pane, at: None },
            None => Place::Beside { pane: from, side },
        };
        self.move_tab(from, index, to)
    }
}

fn swap_leaves(node: &mut Node, a: PaneId, b: PaneId) {
    match node {
        Node::Pane(id) if *id == a => *id = b,
        Node::Pane(id) if *id == b => *id = a,
        Node::Pane(_) => {}
        Node::Split(split) => {
            for child in split.children.iter_mut() {
                swap_leaves(&mut child.node, a, b)
            }
        }
    }
}

fn holding(node: &mut Node, target: PaneId) -> Option<&mut Split> {
    let Node::Split(split) = node else {
        return None;
    };
    match (split.children.iter()).any(|child| child.node == Node::Pane(target)) {
        true => Some(split),
        false => (split.children.iter_mut()).find_map(|child| holding(&mut child.node, target)),
    }
}

fn settle(node: &mut Node) {
    let Node::Split(split) = node else {
        return;
    };
    for child in split.children.iter_mut() {
        settle(&mut child.node);
    }
    let count = split.children.len();
    flatten(node, count);
}

fn even(node: &mut Node) {
    let Node::Split(split) = node else {
        return;
    };
    for child in split.children.iter_mut() {
        child.flex = Flex::EVEN;
        even(&mut child.node);
    }
}

fn reshare(node: &mut Node, target: PaneId, axis: Axis, share: &dyn Fn(f32) -> f32) -> Reached {
    let split = match node {
        Node::Pane(id) if *id == target => return Reached::Below,
        Node::Pane(_) => return Reached::Missing,
        Node::Split(split) => split,
    };
    let below = (split.children.iter_mut().enumerate())
        .map(|(index, child)| (index, reshare(&mut child.node, target, axis, share)))
        .find(|(_, reached)| *reached != Reached::Missing);
    match below {
        Some((index, Reached::Below)) if split.axis == axis => {
            set_share(&mut split.children, index, share);
            Reached::Done
        }
        Some((_, reached)) => reached,
        None => Reached::Missing,
    }
}

fn fill(node: &mut Node, target: PaneId, axis: Axis) -> Reached {
    let split = match node {
        Node::Pane(id) if *id == target => return Reached::Below,
        Node::Pane(_) => return Reached::Missing,
        Node::Split(split) => split,
    };
    let below = (split.children.iter_mut().enumerate())
        .map(|(index, child)| (index, fill(&mut child.node, target, axis)))
        .find(|(_, reached)| *reached != Reached::Missing);
    match below {
        Some((index, Reached::Below | Reached::Done)) if split.axis == axis => {
            set_share(&mut split.children, index, &|_| 1.0);
            Reached::Below
        }
        Some((_, reached)) => reached,
        None => Reached::Missing,
    }
}

fn set_share(children: &mut [Child], index: usize, share: &dyn Fn(f32) -> f32) {
    let total: f32 = children.iter().map(|child| child.flex.0).sum();
    let others = (children.len() - 1) as f32;
    let old = children[index].flex.0 / total;
    let new = share(old).clamp(MIN_SHARE, 1.0 - MIN_SHARE * others);
    let rest = (1.0 - new) / (1.0 - old).max(f32::EPSILON);
    for (at, child) in children.iter_mut().enumerate() {
        let weight = match at == index {
            true => new,
            false => (child.flex.0 / total * rest).max(MIN_SHARE),
        };
        child.flex = Flex(weight);
    }
}
