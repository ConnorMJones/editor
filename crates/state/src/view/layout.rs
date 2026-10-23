use std::cmp::Ordering;

use helix_core::{Selection, SmallVec};
use model::config::OpenAs;
use model::window::{Axis, Side};
use slotmap::{SlotMap, new_key_type};

use crate::render::document::{DocumentSeen, DocumentState};
use crate::render::pages::{PagesSeen, PagesState};
use crate::text::buffer::BufferId;
use crate::view::tab::{BesideMut, Opened, Showing, ShowingMut, Tab, TextTabs};

new_key_type! {
    pub struct PaneId;
}

pub struct Layout {
    panes: SlotMap<PaneId, Pane>, // private: the tree's leaves are exactly the panes, focus is one of them
    root: Node, // private: the tree's leaves are exactly the panes, focus is one of them
    focus: PaneId, // private: the tree's leaves are exactly the panes, focus is one of them
    pub clock: Tick,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Node {
    Pane(PaneId),
    Split(Split),
}

/// Panes side by side or stacked.
#[derive(Clone, Debug, PartialEq)]
pub struct Split {
    axis: Axis,           // private: two or more children, none split along `axis`
    children: Vec<Child>, // private: two or more children, none split along `axis`
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

/// A split member; it spans `flex` over the sum of its siblings' flexes.
#[derive(Clone, Debug, PartialEq)]
pub struct Child {
    pub flex: Flex,
    pub node: Node,
}

/// Where a moved tab goes: into a pane's tabs, before `at` or after the active one, or into a new
/// pane on `side` of `pane`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Place {
    Into { pane: PaneId, at: Option<usize> },
    Beside { pane: PaneId, side: Side },
}

/// A positive, finite share weight.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(try_from = "f32", into = "f32")]
pub struct Flex(f32); // private: checked by its constructor

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

impl Layout {
    pub fn new() -> Layout {
        let mut panes = SlotMap::with_key();
        let focus = panes.insert(Pane::EMPTY);
        Layout {
            panes,
            root: Node::Pane(focus),
            focus,
            clock: Tick::default(),
        }
    }

    /// A layout from its parts, refused unless `root`'s leaves are the keys of `panes`, each once,
    /// and `focus` is one of them.
    pub fn restored(
        panes: SlotMap<PaneId, Pane>,
        root: Node,
        focus: PaneId,
        clock: Tick,
    ) -> Result<Layout, LayoutError> {
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
            clock,
        })
    }

    pub fn panes(&self) -> &SlotMap<PaneId, Pane> {
        &self.panes
    }

    pub fn root(&self) -> &Node {
        &self.root
    }

    pub fn focus(&self) -> PaneId {
        self.focus
    }

    pub fn focused(&self) -> &Pane {
        &self.panes[self.focus]
    }

    pub fn focused_mut(&mut self) -> &mut Pane {
        &mut self.panes[self.focus]
    }

    /// A pane the caller knows is live, such as the focus.
    pub fn pane(&mut self, pane: PaneId) -> &mut Pane {
        &mut self.panes[pane]
    }

    /// A pane id kept from earlier, such as the host's last frame; `None` once it closed.
    pub fn find_pane(&mut self, pane: PaneId) -> Option<&mut Pane> {
        self.panes.get_mut(pane)
    }

    /// The tab of `buffer` in `pane`, while both are open.
    pub fn tab_of(&mut self, pane: PaneId, buffer: BufferId) -> Option<&mut TextTabs> {
        let pane = self.panes.get_mut(pane)?;
        let index = pane.tabs.as_ref()?.find(Opened::Text(buffer))?;
        pane.items_mut()[index].tab.as_text_mut()
    }

    /// Every pane, for edits that leave the tree as it is.
    pub fn panes_mut(&mut self) -> impl Iterator<Item = (PaneId, &mut Pane)> {
        self.panes.iter_mut()
    }

    pub fn set_focus(&mut self, pane: PaneId) -> Result<(), NotInLayout> {
        match self.panes.contains_key(pane) {
            true => {
                self.focus = pane;
                Ok(())
            }
            false => Err(NotInLayout(pane)),
        }
    }

    /// Focuses the neighbour on `side`; `None` when there is none.
    pub fn focus_side(&mut self, side: Side) -> Option<PaneId> {
        let pane = self.neighbour(side)?;
        self.focus = pane;
        Some(pane)
    }

    pub fn focus_next(&mut self) -> PaneId {
        self.focus = self.next();
        self.focus
    }

    pub fn tick(&mut self) -> Tick {
        self.clock.0 += 1;
        self.clock
    }

    /// For history to restore: the first view's selection, else the start of the text.
    pub fn selection_of(&self, buffer: BufferId) -> Selection {
        (self.panes.values())
            .flat_map(Pane::items)
            .filter_map(|item| item.tab.as_text().map(TextTabs::text))
            .find(|view| view.buffer() == buffer)
            .map_or_else(|| Selection::point(0), |view| view.selection.clone())
    }

    pub fn document(&self, pane: PaneId) -> Option<(&TextView, &DocumentState)> {
        match self.panes.get(pane)?.active_text()?.showing() {
            Showing::Document(text, state, _) => Some((text, state)),
            Showing::Source(_) | Showing::Diff(..) | Showing::Pages(..) => None,
        }
    }

    pub fn document_mut(
        &mut self,
        pane: PaneId,
    ) -> Option<(
        &mut TextView,
        &mut DocumentState,
        BesideMut<'_, DocumentSeen>,
    )> {
        match self.panes.get_mut(pane)?.active_text_mut()?.showing_mut() {
            ShowingMut::Document(text, state, beside) => Some((text, state, beside)),
            ShowingMut::Source(_) | ShowingMut::Diff(..) | ShowingMut::Pages(..) => None,
        }
    }

    pub fn pages(&self, pane: PaneId) -> Option<(&TextView, &PagesState)> {
        match self.panes.get(pane)?.active_text()?.showing() {
            Showing::Pages(text, state, _) => Some((text, state)),
            Showing::Source(_) | Showing::Diff(..) | Showing::Document(..) => None,
        }
    }

    pub fn pages_mut(
        &mut self,
        pane: PaneId,
    ) -> Option<(&mut TextView, &mut PagesState, BesideMut<'_, PagesSeen>)> {
        match self.panes.get_mut(pane)?.active_text_mut()?.showing_mut() {
            ShowingMut::Pages(text, state, beside) => Some((text, state, beside)),
            ShowingMut::Source(_) | ShowingMut::Diff(..) | ShowingMut::Document(..) => None,
        }
    }

    /// Makes `opened`'s preview tab permanent in every pane.
    pub fn keep(&mut self, opened: Opened) {
        for pane in self.panes.values_mut() {
            match &mut pane.tabs {
                Some(tabs)
                    if tabs.preview.is_some_and(|preview| {
                        tabs.items[preview.index].tab.opened() == opened
                    }) =>
                {
                    tabs.keep()
                }
                Some(_) | None => {}
            }
        }
    }

    /// Places a pane holding `tabs` on `side` of `target`, evening out that split, and focuses it;
    /// `None` when `target` has closed.
    pub fn split(&mut self, target: PaneId, side: Side, tabs: Tabs) -> Option<PaneId> {
        self.panes.get(target)?;
        let id = self.panes.insert(Pane { tabs: Some(tabs) });
        beside(&mut self.root, target, side, id);
        self.focus = id;
        Some(id)
    }

    /// A new pane on `side` of `target` holding a copy of its active tab, as zed and vim split, then
    /// focused; `None` when `target` has closed or has no tab to copy.
    pub fn split_copy(&mut self, target: PaneId, side: Side) -> Option<PaneId> {
        let now = self.tick();
        let tab = self.panes.get(target)?.active()?.split_copy();
        self.split(target, side, Tabs::split_copy(tab, now))
    }

    /// Moves tab `index` of `from` to `to` and focuses the pane it lands in; a pane the move
    /// leaves without tabs closes. `None` when a pane or the tab has gone since the move began.
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
                id
            }
            Place::Into { pane, at } if pane == from => {
                let tabs = self.panes[from].tabs.as_mut()?;
                tabs.reorder(index, at.unwrap_or(tabs.active), now);
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
        self.focus = landed;
        Some(landed)
    }

    /// Removes `pane`, moving focus from it to the pane beside where it was.
    pub fn close(&mut self, pane: PaneId) -> Result<(), CloseError> {
        let beside = match &self.root {
            Node::Pane(_) => return Err(CloseError::LastPane),
            Node::Split(_) => remove_node(&mut self.root, pane).ok_or(CloseError::NotInLayout)?,
        };
        self.panes.remove(pane);
        match self.focus == pane {
            true => self.focus = beside,
            false => {}
        }
        Ok(())
    }

    /// Removes tab `index` of `pane`; a pane left without tabs closes unless it is the last one.
    pub fn remove_tab(&mut self, pane: PaneId, index: usize) -> Option<Item> {
        let now = self.tick();
        let target = self.panes.get_mut(pane)?;
        target.tabs.as_ref()?.items.get(index)?;
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
    }

    /// Panes in reading order.
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

    /// `at` is a fraction of the editor area along the split's axis; stale paths are ignored.
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

    /// Nearest pane touching the focused pane's `side` that overlaps it across that side.
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

impl Default for Layout {
    fn default() -> Self {
        Layout::new()
    }
}

/// Places `new` on `side` of `target`; a split along that axis takes it as a sibling.
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

/// Evens out the split `target` leaves; returns the pane now beside where it was.
fn remove_node(node: &mut Node, target: PaneId) -> Option<PaneId> {
    let Node::Split(Split { axis, children }) = node else {
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
    // Re-establish the invariants: no single-child splits, no same-axis nesting.
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
    Some(beside)
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

/// A pane's tabs; `None` is an empty pane.
pub struct Pane {
    pub tabs: Option<Tabs>,
}

impl Pane {
    pub const EMPTY: Pane = Pane { tabs: None };

    pub fn active(&self) -> Option<&Tab> {
        self.tabs.as_ref().map(|tabs| &tabs.items[tabs.active].tab)
    }

    pub fn active_mut(&mut self) -> Option<&mut Tab> {
        self.tabs
            .as_mut()
            .map(|tabs| &mut tabs.items[tabs.active].tab)
    }

    pub fn active_text(&self) -> Option<&TextTabs> {
        self.active()?.as_text()
    }

    pub fn active_text_mut(&mut self) -> Option<&mut TextTabs> {
        self.active_mut()?.as_text_mut()
    }

    pub fn items(&self) -> &[Item] {
        match &self.tabs {
            Some(tabs) => &tabs.items,
            None => &[],
        }
    }

    pub fn items_mut(&mut self) -> &mut [Item] {
        match &mut self.tabs {
            Some(tabs) => &mut tabs.items,
            None => &mut [],
        }
    }
}

/// A non-empty tab list in bar order, private so its indices stay valid and buffers distinct.
pub struct Tabs {
    items: Vec<Item>,         // private: never empty, indices valid, files distinct
    active: usize,            // private: never empty, indices valid, files distinct
    preview: Option<Preview>, // private: never empty, indices valid, files distinct
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Preview {
    pub index: usize,
    pub kind: PreviewKind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PreviewKind {
    /// Opened as a preview, from the tree or a picker.
    Open,
    /// The copy of the active item a split starts the new pane with.
    SplitCopy,
}

pub struct Item {
    pub tab: Tab,
    /// Last activation; closing the active tab activates the most recent other one.
    pub used: Tick,
}

#[derive(Clone)]
pub struct TextView {
    /// Fixed for the view's lifetime so a pane's items keep distinct buffers.
    buffer: BufferId, // private: a tab's buffer is its identity
    pub selection: Selection,
    pub scroll: Scroll,
}

impl TextView {
    pub fn new(buffer: BufferId) -> TextView {
        TextView {
            buffer,
            selection: Selection::point(0),
            scroll: Scroll::default(),
        }
    }

    pub fn buffer(&self) -> BufferId {
        self.buffer
    }
}

/// Activation order across all panes; larger is more recent.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct Tick(pub u64);

#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum InvalidTabs {
    #[error("a tab list needs at least one tab")]
    Empty,
    #[error("active tab {0} does not exist")]
    Active(usize),
    #[error("preview tab {0} does not exist")]
    Preview(usize),
    #[error("tab {0} repeats an earlier tab")]
    Duplicate(usize),
}

impl Tabs {
    pub fn new(tab: Tab, open: OpenAs, now: Tick) -> Tabs {
        Tabs {
            items: vec![Item { tab, used: now }],
            active: 0,
            preview: match open {
                OpenAs::Preview => Some(Preview {
                    index: 0,
                    kind: PreviewKind::Open,
                }),
                OpenAs::Permanent => None,
            },
        }
    }

    pub fn split_copy(tab: Tab, now: Tick) -> Tabs {
        Tabs {
            items: vec![Item { tab, used: now }],
            active: 0,
            preview: Some(Preview {
                index: 0,
                kind: PreviewKind::SplitCopy,
            }),
        }
    }

    pub fn from_parts(
        items: Vec<Item>,
        active: usize,
        preview: Option<usize>,
    ) -> Result<Tabs, InvalidTabs> {
        let duplicate = items.iter().enumerate().find_map(|(index, item)| {
            let opened = item.tab.opened();
            items[..index]
                .iter()
                .any(|earlier| earlier.tab.opened() == opened)
                .then_some(index)
        });
        match (items.len(), duplicate, preview) {
            (0, _, _) => Err(InvalidTabs::Empty),
            (_, Some(index), _) => Err(InvalidTabs::Duplicate(index)),
            (len, None, _) if active >= len => Err(InvalidTabs::Active(active)),
            (len, None, Some(preview)) if preview >= len => Err(InvalidTabs::Preview(preview)),
            (_, None, _) => Ok(Tabs {
                items,
                active,
                preview: preview.map(|index| Preview {
                    index,
                    kind: PreviewKind::Open,
                }),
            }),
        }
    }

    pub fn items(&self) -> &[Item] {
        &self.items
    }

    pub fn items_mut(&mut self) -> &mut [Item] {
        &mut self.items
    }

    pub fn active(&self) -> usize {
        self.active
    }

    pub fn preview(&self) -> Option<usize> {
        self.preview.map(|preview| preview.index)
    }

    pub fn find(&self, opened: Opened) -> Option<usize> {
        self.items
            .iter()
            .position(|item| item.tab.opened() == opened)
    }

    /// Stale indices are ignored.
    pub fn activate(&mut self, index: usize, now: Tick) {
        match self.items.get_mut(index) {
            Some(item) => {
                item.used = now;
                self.active = index;
            }
            None => {}
        }
    }

    pub fn keep(&mut self) {
        self.preview = None;
    }

    /// Zed's rule: activate an open item, replace an active preview, else insert after the active.
    pub fn open(&mut self, tab: Tab, open: OpenAs, now: Tick) -> Option<Item> {
        let item = Item { tab, used: now };
        match (self.find(item.tab.opened()), open, self.preview) {
            (Some(index), OpenAs::Permanent, _) => {
                match self.preview.map(|preview| preview.index) == Some(index) {
                    true => self.preview = None,
                    false => {}
                }
                self.activate(index, now);
                None
            }
            (Some(index), OpenAs::Preview, _) => {
                self.activate(index, now);
                None
            }
            // A split's copy only stands in until the pane gets something, so any open replaces it.
            (
                None,
                OpenAs::Permanent,
                Some(Preview {
                    index,
                    kind: PreviewKind::SplitCopy,
                }),
            ) if index == self.active => {
                self.preview = None;
                Some(std::mem::replace(&mut self.items[index], item))
            }
            (None, OpenAs::Permanent, _) => {
                self.insert_after_active(item);
                None
            }
            (None, OpenAs::Preview, Some(Preview { index: preview, .. }))
                if preview == self.active =>
            {
                Some(std::mem::replace(&mut self.items[preview], item))
            }
            (None, OpenAs::Preview, Some(Preview { index: preview, .. })) => {
                let old = self.items.remove(preview);
                match preview < self.active {
                    true => self.active -= 1,
                    false => {}
                }
                self.preview = Some(Preview {
                    index: self.insert_after_active(item),
                    kind: PreviewKind::Open,
                });
                Some(old)
            }
            (None, OpenAs::Preview, None) => {
                self.preview = Some(Preview {
                    index: self.insert_after_active(item),
                    kind: PreviewKind::Open,
                });
                None
            }
        }
    }

    /// A pane's tabs holding one tab, kept.
    pub fn one(item: Item) -> Tabs {
        Tabs {
            items: vec![item],
            active: 0,
            preview: None,
        }
    }

    /// Takes in a tab moved from another pane, before `at` or after the active tab, and
    /// activates it; a tab on the same file already here is activated instead.
    pub fn take(&mut self, item: Item, at: Option<usize>, now: Tick) {
        match (self.find(item.tab.opened()), at) {
            (Some(index), _) => self.activate(index, now),
            (None, None) => {
                self.insert_after_active(item);
            }
            (None, Some(at)) => self.rearrange(now, |items| {
                let at = at.min(items.len());
                items.insert(at, item);
                at
            }),
        }
    }

    /// Moves tab `from` to sit at `to`, and activates it.
    pub fn reorder(&mut self, from: usize, to: usize, now: Tick) {
        self.rearrange(now, |items| {
            let item = items.remove(from);
            let to = to.min(items.len());
            items.insert(to, item);
            to
        })
    }

    /// Applies `change` to the items, then activates the index it returns; the preview stays on
    /// its tab wherever that went.
    fn rearrange(&mut self, now: Tick, change: impl FnOnce(&mut Vec<Item>) -> usize) {
        let preview =
            (self.preview).map(|preview| (self.items[preview.index].tab.opened(), preview.kind));
        let active = change(&mut self.items);
        self.preview = preview
            .and_then(|(opened, kind)| self.find(opened).map(|index| Preview { index, kind }));
        self.activate(active, now);
    }

    fn insert_after_active(&mut self, item: Item) -> usize {
        let index = self.active + 1;
        self.items.insert(index, item);
        match &mut self.preview {
            Some(preview) if preview.index >= index => preview.index += 1,
            Some(_) | None => {}
        }
        self.active = index;
        index
    }

    /// Activates the most recently used tab when the active one goes; `None` once empty.
    fn remove(mut self, index: usize, now: Tick) -> (Option<Tabs>, Item) {
        let item = self.items.remove(index);
        self.preview = match self.preview {
            Some(preview) if preview.index == index => None,
            Some(preview) if preview.index > index => Some(Preview {
                index: preview.index - 1,
                ..preview
            }),
            other => other,
        };
        let recent = self
            .items
            .iter()
            .enumerate()
            .max_by_key(|(_, item)| item.used)
            .map(|(index, _)| index);
        let active = match (self.active.cmp(&index), recent) {
            (_, None) => return (None, item),
            (Ordering::Less, Some(_)) => self.active,
            (Ordering::Greater, Some(_)) => self.active - 1,
            (Ordering::Equal, Some(recent)) => recent,
        };
        self.activate(active, now);
        (Some(self), item)
    }
}

/// The top row lies `rows` below `anchor`'s visual row; negative only inside leading fillers.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Scroll {
    pub anchor: usize,
    pub rows: isize,
    pub viewport: Viewport,
}

/// Text area size in cells as last drawn; zero before the first frame.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Viewport {
    pub rows: u16,
    pub cols: u16,
}
