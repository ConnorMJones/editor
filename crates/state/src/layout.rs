use std::cmp::Ordering;

use helix_core::{Selection, SmallVec};
use model::config::OpenAs;
use model::window::{Axis, Side};
use slotmap::{SlotMap, new_key_type};

use crate::buffer::BufferId;
use crate::diffview::DiffView;
use crate::rendered::RenderedView;

new_key_type! {
    pub struct PaneId;
}

/// Panes arranged in a split tree; `focus` always names a leaf of `root`.
pub struct Layout {
    pub panes: SlotMap<PaneId, Pane>,
    pub root: Node,
    pub focus: PaneId,
    /// Last stamp handed to an activated tab.
    pub clock: Tick,
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
pub enum CloseError {
    LastPane,
    NotInLayout,
}

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

    pub fn tick(&mut self) -> Tick {
        self.clock.0 += 1;
        self.clock
    }

    pub fn rendered(&self, pane: PaneId) -> Option<&RenderedView> {
        match self.panes.get(pane)?.active()? {
            View::Rendered(view) => Some(view),
            View::Text(_) | View::Diff(_) => None,
        }
    }

    pub fn rendered_mut(&mut self, pane: PaneId) -> Option<&mut RenderedView> {
        match self.panes.get_mut(pane)?.active_mut()? {
            View::Rendered(view) => Some(view),
            View::Text(_) | View::Diff(_) => None,
        }
    }

    /// Makes every preview tab of `buffer`, text or diff, permanent.
    pub fn keep(&mut self, buffer: BufferId) {
        for pane in self.panes.values_mut() {
            match &mut pane.tabs {
                Some(tabs)
                    if tabs.preview.is_some_and(|preview| {
                        tabs.items[preview.index].view.buffer() == buffer
                    }) =>
                {
                    tabs.keep()
                }
                Some(_) | None => {}
            }
        }
    }

    /// Places `pane` after the focused pane along `axis`, evening out that split, and focuses it.
    pub fn split(&mut self, axis: Axis, pane: Pane) -> PaneId {
        let id = self.panes.insert(pane);
        split_node(&mut self.root, self.focus, axis, id);
        self.focus = id;
        id
    }

    /// Removes `pane`, moving focus from it to the pane beside where it was.
    pub fn close(&mut self, pane: PaneId) -> Result<(), CloseError> {
        let beside = match &self.root {
            Node::Pane(_) => return Err(CloseError::LastPane),
            Node::Split { .. } => {
                remove_node(&mut self.root, pane).ok_or(CloseError::NotInLayout)?
            }
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
                // Like vim's equalalways and zed, adding a pane evens out its split.
                (Some(i), true) => {
                    children.insert(i + 1, Child::even(Node::Pane(new)));
                    for child in children.iter_mut() {
                        child.flex = Flex::EVEN;
                    }
                    true
                }
                (Some(_), false) | (None, _) => children
                    .iter_mut()
                    .any(|child| split_node(&mut child.node, target, axis, new)),
            }
        }
    }
}

/// Removes `target` under `node`, evening out the split it leaves, and returns the pane now
/// beside where it was.
fn remove_node(node: &mut Node, target: PaneId) -> Option<PaneId> {
    let Node::Split { axis, children } = node else {
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
                    Node::Split { children, .. } => match index {
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
            Node::Split {
                axis: inner,
                children: grand,
            } if inner == axis => flat.extend(grand),
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
        _ => Node::Split {
            axis,
            children: flat,
        },
    };
    Some(beside)
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

/// A pane's tabs; `None` is an empty pane.
pub struct Pane {
    pub tabs: Option<Tabs>,
}

impl Pane {
    pub const EMPTY: Pane = Pane { tabs: None };

    pub fn active(&self) -> Option<&View> {
        self.tabs.as_ref().map(|tabs| &tabs.items[tabs.active].view)
    }

    pub fn active_mut(&mut self) -> Option<&mut View> {
        self.tabs
            .as_mut()
            .map(|tabs| &mut tabs.items[tabs.active].view)
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
    items: Vec<Item>,
    active: usize,
    preview: Option<Preview>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Preview {
    pub index: usize,
    pub kind: PreviewKind,
}

/// How a preview tab came to be.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PreviewKind {
    /// Opened as a preview, from the tree or a picker.
    Open,
    /// The copy of the active item a split starts the new pane with.
    SplitCopy,
}

pub struct Item {
    pub view: View,
    /// Last activation; closing the active tab activates the most recent other one.
    pub used: Tick,
}

/// What a tab shows.
#[derive(Clone)]
pub enum View {
    Text(TextView),
    Diff(DiffView),
    Rendered(RenderedView),
}

/// Identifies a tab within its pane, so one pane can hold a file and its diff.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TabKey {
    Text(BufferId),
    Diff(BufferId),
    Rendered(BufferId),
}

impl TabKey {
    pub fn buffer(self) -> BufferId {
        match self {
            TabKey::Text(buffer) | TabKey::Diff(buffer) | TabKey::Rendered(buffer) => buffer,
        }
    }
}

impl View {
    pub fn key(&self) -> TabKey {
        match self {
            View::Text(view) => TabKey::Text(view.buffer),
            View::Diff(diff) => TabKey::Diff(diff.text.buffer),
            View::Rendered(rendered) => TabKey::Rendered(rendered.text.buffer),
        }
    }

    pub fn buffer(&self) -> BufferId {
        self.key().buffer()
    }

    /// The buffer's selection and scroll: the text view, a diff's right side, or the source
    /// position a rendered view returns to.
    pub fn text(&self) -> &TextView {
        match self {
            View::Text(view) => view,
            View::Diff(diff) => &diff.text,
            View::Rendered(rendered) => &rendered.text,
        }
    }

    pub fn text_mut(&mut self) -> &mut TextView {
        match self {
            View::Text(view) => view,
            View::Diff(diff) => &mut diff.text,
            View::Rendered(rendered) => &mut rendered.text,
        }
    }
}

#[derive(Clone)]
pub struct TextView {
    /// Fixed for the view's lifetime so a pane's items keep distinct buffers.
    buffer: BufferId,
    pub selection: Selection,
    pub scroll: Scroll,
}

impl TextView {
    pub(crate) fn new(buffer: BufferId) -> TextView {
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
    pub(crate) fn new(view: View, open: OpenAs, now: Tick) -> Tabs {
        Tabs {
            items: vec![Item { view, used: now }],
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

    pub(crate) fn split_copy(view: View, now: Tick) -> Tabs {
        Tabs {
            items: vec![Item { view, used: now }],
            active: 0,
            preview: Some(Preview {
                index: 0,
                kind: PreviewKind::SplitCopy,
            }),
        }
    }

    pub(crate) fn from_parts(
        items: Vec<Item>,
        active: usize,
        preview: Option<usize>,
    ) -> Result<Tabs, InvalidTabs> {
        let duplicate = items.iter().enumerate().find_map(|(index, item)| {
            let key = item.view.key();
            items[..index]
                .iter()
                .any(|earlier| earlier.view.key() == key)
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

    pub fn find(&self, key: TabKey) -> Option<usize> {
        self.items.iter().position(|item| item.view.key() == key)
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

    /// Makes the preview tab, if any, permanent.
    pub fn keep(&mut self) {
        self.preview = None;
    }

    /// Zed's rule: activates an open item, replaces an active preview in place, else inserts
    /// after the active tab; returns the displaced preview.
    pub(crate) fn open(&mut self, view: View, open: OpenAs, now: Tick) -> Option<Item> {
        let item = Item { view, used: now };
        match (self.find(item.view.key()), open, self.preview) {
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
            // A split's copy only stands in until the pane is given something to show, so any
            // open replaces it; ordinary previews keep zed's rule and stay beside permanent opens.
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

    /// Removes tab `index`, activating the most recently used tab if it was active; `None`
    /// once empty.
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

/// Visible window of a text view: the top row lies `rows` below `anchor`'s visual row, like
/// helix's `vertical_offset`, and is negative only inside filler rows above the first line.
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
