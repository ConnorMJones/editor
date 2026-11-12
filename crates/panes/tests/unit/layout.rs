use model::window::{Axis, Side};
use panes::{
    Child, CloseError, Flex, Item, Layout, MIN_SHARE, Maximised, Node, PaneId, Place, Rect, STEP,
    Shown, Split, Tabbed, Tabs, Tick,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Note(pub u32);

impl Tabbed for Note {
    type Key = u32;

    fn key(&self) -> u32 {
        self.0
    }

    fn split_copy(&self) -> Note {
        *self
    }
}

fn split_off(layout: &mut Layout<Note>, axis: Axis) -> PaneId {
    let side = match axis {
        Axis::Row => Side::Right,
        Axis::Column => Side::Down,
    };
    let tab = Note(0);
    let tabs = Tabs::one(Item {
        tab,
        used: Tick::default(),
    });
    layout
        .split(layout.focus(), side, tabs)
        .expect("the focus is in the layout")
}

fn split<const N: usize>(axis: Axis, children: [(f32, Node); N]) -> Node {
    let children = (children.into_iter())
        .map(|(flex, node)| Child {
            flex: Flex::new(flex).expect("positive"),
            node,
        })
        .collect();
    Node::Split(Split::new(axis, children).expect("a valid split"))
}

#[track_caller]
fn assert_close(a: f32, b: f32) {
    assert!((a - b).abs() < 1e-5, "{a} != {b}");
}

#[test]
fn same_axis_splits_stay_flat() {
    let mut layout = Layout::<Note>::new();
    let a = layout.focus();
    let b = split_off(&mut layout, Axis::Row);
    let c = split_off(&mut layout, Axis::Row);
    // Splitting evens out the split it joins.
    assert_eq!(
        *layout.root(),
        split(
            Axis::Row,
            [
                (1.0, Node::Pane(a)),
                (1.0, Node::Pane(b)),
                (1.0, Node::Pane(c))
            ]
        )
    );
    assert_eq!(layout.focus(), c);
    assert_eq!(layout.leaves(), [a, b, c]);
}

#[test]
fn cross_axis_split_nests() {
    let mut layout = Layout::<Note>::new();
    let a = layout.focus();
    let b = split_off(&mut layout, Axis::Row);
    let c = split_off(&mut layout, Axis::Column);
    assert_eq!(
        *layout.root(),
        split(
            Axis::Row,
            [
                (1.0, Node::Pane(a)),
                (
                    1.0,
                    split(Axis::Column, [(1.0, Node::Pane(b)), (1.0, Node::Pane(c))])
                ),
            ]
        )
    );
}

#[test]
fn close_collapses_and_merges() {
    let mut layout = Layout::<Note>::new();
    let a = layout.focus();
    let b = split_off(&mut layout, Axis::Row);
    let c = split_off(&mut layout, Axis::Column);
    let d = split_off(&mut layout, Axis::Row);
    // d nests in a row beside c; closing c collapses that row back into the column.
    layout.close(c).unwrap();
    assert_eq!(
        *layout.root(),
        split(
            Axis::Row,
            [
                (1.0, Node::Pane(a)),
                (
                    1.0,
                    split(Axis::Column, [(1.0, Node::Pane(b)), (1.0, Node::Pane(d))])
                ),
            ]
        )
    );
    layout.close(b).unwrap();
    assert_eq!(
        *layout.root(),
        split(Axis::Row, [(1.0, Node::Pane(a)), (1.0, Node::Pane(d))])
    );
    layout.close(d).unwrap();
    assert_eq!(*layout.root(), Node::Pane(a));
    assert_eq!(layout.focus(), a);
    assert_eq!(layout.close(a), Err(CloseError::LastPane));
    assert_eq!(layout.panes().len(), 1);
}

#[test]
fn neighbours_follow_geometry() {
    let mut layout = Layout::<Note>::new();
    let a = layout.focus();
    let b = split_off(&mut layout, Axis::Row);
    let c = split_off(&mut layout, Axis::Column);
    // a | b
    //   | c
    assert_eq!(layout.neighbour(Side::Up), Some(b));
    assert_eq!(layout.neighbour(Side::Left), Some(a));
    assert_eq!(layout.neighbour(Side::Right), None);
    layout.set_focus(a).unwrap();
    assert_eq!(layout.neighbour(Side::Right), Some(b));
    assert_eq!(layout.neighbour(Side::Down), None);
    assert_eq!(layout.next(), b);
    layout.set_focus(c).unwrap();
    assert_eq!(layout.next(), a);
}

#[test]
fn only_keeps_focused() {
    let mut layout = Layout::<Note>::new();
    split_off(&mut layout, Axis::Row);
    let kept = split_off(&mut layout, Axis::Column);
    layout.only();
    assert_eq!(*layout.root(), Node::Pane(kept));
    assert_eq!(layout.panes().len(), 1);
}

#[test]
fn rects_follow_flex_weights() {
    let mut layout = Layout::<Note>::new();
    let a = layout.focus();
    let b = split_off(&mut layout, Axis::Row);
    let c = split_off(&mut layout, Axis::Row);
    let rects = layout.rects();
    assert_eq!(rects[0].0, a);
    assert_close(rects[0].1.w, 1.0 / 3.0);
    assert_eq!(rects[1].0, b);
    assert_close(rects[1].1.x, 1.0 / 3.0);
    assert_close(rects[1].1.w, 1.0 / 3.0);
    assert_eq!(rects[2].0, c);
    assert_close(rects[2].1.x, 2.0 / 3.0);
    assert_close(rects[2].1.w, 1.0 / 3.0);
}

#[test]
fn dragging_a_divider_moves_only_its_neighbours() {
    let mut layout = Layout::<Note>::new();
    split_off(&mut layout, Axis::Row);
    split_off(&mut layout, Axis::Row);
    layout.drag_divider(&[], 0, 0.3);
    let widths: Vec<f32> = layout.rects().iter().map(|(_, r)| r.w).collect();
    assert_close(widths[0], 0.3);
    assert_close(widths[1], 2.0 / 3.0 - 0.3);
    assert_close(widths[2], 1.0 / 3.0);
    // Clamped so neither neighbour vanishes.
    layout.drag_divider(&[], 1, 2.0);
    let widths: Vec<f32> = layout.rects().iter().map(|(_, r)| r.w).collect();
    assert_close(widths[2], MIN_SHARE);
    assert_close(widths.iter().sum(), 1.0);
    // Stale paths and dividers are ignored.
    let before = layout.root().clone();
    layout.drag_divider(&[5], 0, 0.5);
    layout.drag_divider(&[], 2, 0.5);
    layout.drag_divider(&[], 0, f32::NAN);
    assert_eq!(*layout.root(), before);
}

#[test]
fn nested_divider_drags_within_its_split() {
    let mut layout = Layout::<Note>::new();
    let a = layout.focus();
    split_off(&mut layout, Axis::Row);
    let c = split_off(&mut layout, Axis::Column);
    // The column split occupies the right half; its divider position is in editor fractions.
    layout.drag_divider(&[1], 0, 0.25);
    let rects = layout.rects();
    let (_, upper) = rects[1];
    let (_, lower) = rects[2];
    assert_close(upper.h, 0.25);
    assert_close(lower.y, 0.25);
    assert_close(lower.h, 0.75);
    assert_eq!(
        rects[0],
        (
            a,
            Rect {
                x: 0.0,
                y: 0.0,
                w: 0.5,
                h: 1.0
            }
        )
    );
    // Navigation still follows the resized geometry.
    layout.set_focus(c).unwrap();
    assert_eq!(layout.neighbour(Side::Left), Some(a));
}

#[test]
fn merging_a_same_axis_split_evens_it_out() {
    let mut layout = Layout::<Note>::new();
    let a = layout.focus();
    let b = split_off(&mut layout, Axis::Row);
    let c = split_off(&mut layout, Axis::Column);
    layout.set_focus(c).unwrap();
    let d = split_off(&mut layout, Axis::Row);
    // a | b
    //   | c d
    layout.close(b).unwrap();
    // The column collapses and c, d join the root row, which evens out.
    let after = layout.rects();
    assert_eq!(
        after.iter().map(|(id, _)| *id).collect::<Vec<_>>(),
        [a, c, d]
    );
    for (_, rect) in &after {
        assert_close(rect.w, 1.0 / 3.0);
    }
}

#[test]
fn flex_rejects_non_positive_weights() {
    assert_eq!(Flex::new(0.0), None);
    assert_eq!(Flex::new(-1.0), None);
    assert_eq!(Flex::new(f32::INFINITY), None);
    assert_eq!(Flex::new(0.5).map(Flex::get), Some(0.5));
}

#[test]
fn closing_evens_out_the_split_and_focuses_the_neighbour() {
    let mut layout = Layout::<Note>::new();
    let a = layout.focus();
    let b = split_off(&mut layout, Axis::Row);
    let before = layout.root().clone();
    // Opening and closing a split beside b, again and again, returns to even halves.
    for _ in 0..3 {
        let c = split_off(&mut layout, Axis::Row);
        layout.close(c).unwrap();
        assert_eq!(*layout.root(), before);
        assert_eq!(layout.focus(), b, "focus goes to the pane beside it");
        layout.set_focus(b).unwrap();
    }
    // Closing the middle of three leaves two even halves.
    let c = split_off(&mut layout, Axis::Row);
    layout.set_focus(b).unwrap();
    layout.close(b).unwrap();
    assert_eq!(
        *layout.root(),
        split(Axis::Row, [(1.0, Node::Pane(a)), (1.0, Node::Pane(c))])
    );
    assert_eq!(layout.focus(), a);
    layout.close(c).unwrap();
    let b = split_off(&mut layout, Axis::Row);
    // A first child's neighbour is the sibling after it.
    layout.set_focus(a).unwrap();
    layout.close(a).unwrap();
    assert_eq!(*layout.root(), Node::Pane(b));
    assert_eq!(layout.focus(), b);
    assert_eq!(layout.close(a), Err(CloseError::LastPane));
}

#[test]
fn closing_a_pane_outside_the_layout_fails() {
    let mut layout = Layout::<Note>::new();
    let a = layout.focus();
    let b = split_off(&mut layout, Axis::Row);
    layout.close(b).unwrap();
    split_off(&mut layout, Axis::Row);
    assert_eq!(layout.close(b), Err(CloseError::NotInLayout));
    assert_eq!(layout.leaves().len(), 2);
    assert_eq!(layout.leaves()[0], a);
}

fn width(layout: &Layout<Note>, pane: PaneId) -> f32 {
    (layout.rects().into_iter())
        .find(|(id, _)| *id == pane)
        .map(|(_, rect)| rect.w)
        .expect("a pane in the layout")
}

#[test]
fn swapping_moves_the_focused_pane_and_keeps_the_focus_on_it() {
    let mut layout = Layout::<Note>::new();
    let a = layout.focus();
    let b = split_off(&mut layout, Axis::Row);
    assert_eq!(layout.swap(Side::Left), Some(a));
    assert_eq!(layout.leaves(), [b, a]);
    assert_eq!(layout.focus(), b);
    assert_eq!(
        layout.swap(Side::Left),
        None,
        "nothing is left of the first pane"
    );
}

#[test]
fn transposing_flips_the_split_and_merges_into_a_parent_of_the_new_axis() {
    let mut layout = Layout::<Note>::new();
    let a = layout.focus();
    let b = split_off(&mut layout, Axis::Row);
    let c = split_off(&mut layout, Axis::Column);
    assert_eq!(layout.focus(), c);
    layout.transpose();
    assert_eq!(
        *layout.root(),
        split(
            Axis::Row,
            [
                (1.0, Node::Pane(a)),
                (1.0, Node::Pane(b)),
                (1.0, Node::Pane(c))
            ]
        )
    );
    layout.transpose();
    assert!(
        matches!(layout.root(), Node::Split(split) if split.axis() == Axis::Column),
        "{:?}",
        layout.root()
    );
}

#[test]
fn resizing_steps_the_share_within_limits_and_even_resets() {
    let mut layout = Layout::<Note>::new();
    let a = layout.focus();
    let b = split_off(&mut layout, Axis::Row);
    layout.set_focus(a).unwrap();
    layout.resize(model::command::PaneResize::Wider);
    assert_close(width(&layout, a), 0.5 + STEP);
    layout.resize(model::command::PaneResize::WidestHere);
    assert_close(width(&layout, a), 1.0 - MIN_SHARE);
    assert_close(width(&layout, b), MIN_SHARE);
    for _ in 0..40 {
        layout.resize(model::command::PaneResize::Narrower);
    }
    assert_close(width(&layout, a), MIN_SHARE);
    layout.resize(model::command::PaneResize::Even);
    assert_close(width(&layout, a), 0.5);
    layout.resize(model::command::PaneResize::Taller);
    assert_close(width(&layout, a), 0.5);
}

#[test]
fn maximising_shows_one_pane_until_the_focus_or_the_tree_changes() {
    let mut layout = Layout::<Note>::new();
    let a = layout.focus();
    let b = split_off(&mut layout, Axis::Row);
    layout.toggle_maximise();
    assert_eq!(layout.shown(), Shown::One(b));
    layout.set_focus(b).unwrap();
    assert_eq!(
        layout.maximised(),
        Maximised::Pane(b),
        "the same pane stays"
    );
    layout.set_focus(a).unwrap();
    assert_eq!(layout.maximised(), Maximised::No);
    layout.toggle_maximise();
    split_off(&mut layout, Axis::Column);
    assert_eq!(layout.maximised(), Maximised::No);
    layout.toggle_maximise();
    layout.toggle_maximise();
    assert!(matches!(layout.shown(), Shown::Tree(_)));
}

#[test]
fn moving_a_tab_to_a_side_uses_the_neighbour_or_splits() {
    let mut layout = Layout::<Note>::new();
    let a = layout.focus();
    layout.pane(a).tabs = Some(Tabs::new(
        Note(1),
        model::config::OpenAs::Permanent,
        Tick::default(),
    ));
    let now = layout.tick();
    let tabs = layout.pane(a).tabs.as_mut().unwrap();
    tabs.open(Note(2), model::config::OpenAs::Permanent, now);
    let b = layout.move_tab_to(Side::Right).expect("a new pane");
    assert_ne!(a, b);
    assert_eq!(layout.leaves(), [a, b]);
    assert_eq!(layout.focused().active(), Some(&Note(2)));
    layout.set_focus(a).unwrap();
    assert_eq!(layout.move_tab_to(Side::Right), Some(b));
    assert_eq!(layout.leaves(), [b], "the emptied pane closes");
    let keys: Vec<u32> = (layout.focused().items().iter())
        .map(|item| item.tab.0)
        .collect();
    assert_eq!(keys, [2, 1]);
    assert_eq!(
        layout.move_tab(
            b,
            0,
            Place::Into {
                pane: b,
                at: Some(1)
            }
        ),
        Some(b)
    );
}

#[test]
fn shifting_and_last_used_follow_the_bar_and_the_clock() {
    let mut tabs = Tabs::new(Note(1), model::config::OpenAs::Permanent, Tick(1));
    tabs.open(Note(2), model::config::OpenAs::Permanent, Tick(2));
    tabs.open(Note(3), model::config::OpenAs::Permanent, Tick(3));
    assert_eq!(tabs.last_used(), Some(1));
    tabs.shift(model::command::Direction::Backward, Tick(4));
    let keys: Vec<u32> = tabs.items().iter().map(|item| item.tab.0).collect();
    assert_eq!(keys, [1, 3, 2]);
    assert_eq!(tabs.active(), 1);
    tabs.shift(model::command::Direction::Forward, Tick(5));
    tabs.shift(model::command::Direction::Forward, Tick(6));
    assert_eq!(tabs.active(), 2, "the last place stays the last");
}
