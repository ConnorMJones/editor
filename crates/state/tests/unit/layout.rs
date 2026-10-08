use model::window::{Axis, Side};
use state::layout::{Child, Flex, LastPane, Layout, MIN_SHARE, Node, Pane, Rect};

fn pane() -> Pane {
    Pane::EMPTY
}

fn split<const N: usize>(axis: Axis, children: [(f32, Node); N]) -> Node {
    Node::Split {
        axis,
        children: children
            .into_iter()
            .map(|(flex, node)| Child {
                flex: Flex::new(flex).expect("positive"),
                node,
            })
            .collect(),
    }
}

#[track_caller]
fn assert_close(a: f32, b: f32) {
    assert!((a - b).abs() < 1e-5, "{a} != {b}");
}

#[test]
fn same_axis_splits_stay_flat() {
    let mut layout = Layout::new();
    let a = layout.focus;
    let b = layout.split(Axis::Row, pane());
    let c = layout.split(Axis::Row, pane());
    // Splitting halves the focused pane's share.
    assert_eq!(
        layout.root,
        split(
            Axis::Row,
            [
                (1.0, Node::Pane(a)),
                (0.5, Node::Pane(b)),
                (0.5, Node::Pane(c))
            ]
        )
    );
    assert_eq!(layout.focus, c);
    assert_eq!(layout.leaves(), [a, b, c]);
}

#[test]
fn cross_axis_split_nests() {
    let mut layout = Layout::new();
    let a = layout.focus;
    let b = layout.split(Axis::Row, pane());
    let c = layout.split(Axis::Column, pane());
    assert_eq!(
        layout.root,
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
    let mut layout = Layout::new();
    let a = layout.focus;
    let b = layout.split(Axis::Row, pane());
    let c = layout.split(Axis::Column, pane());
    let d = layout.split(Axis::Row, pane());
    // d nests in a row beside c; closing c collapses that row back into the column.
    layout.close(c).unwrap();
    assert_eq!(
        layout.root,
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
        layout.root,
        split(Axis::Row, [(1.0, Node::Pane(a)), (1.0, Node::Pane(d))])
    );
    layout.close(d).unwrap();
    assert_eq!(layout.root, Node::Pane(a));
    assert_eq!(layout.focus, a);
    assert_eq!(layout.close(a), Err(LastPane));
    assert_eq!(layout.panes.len(), 1);
}

#[test]
fn neighbours_follow_geometry() {
    let mut layout = Layout::new();
    let a = layout.focus;
    let b = layout.split(Axis::Row, pane());
    let c = layout.split(Axis::Column, pane());
    // a | b
    //   | c
    assert_eq!(layout.neighbour(Side::Up), Some(b));
    assert_eq!(layout.neighbour(Side::Left), Some(a));
    assert_eq!(layout.neighbour(Side::Right), None);
    layout.focus = a;
    assert_eq!(layout.neighbour(Side::Right), Some(b));
    assert_eq!(layout.neighbour(Side::Down), None);
    assert_eq!(layout.next(), b);
    layout.focus = c;
    assert_eq!(layout.next(), a);
}

#[test]
fn only_keeps_focused() {
    let mut layout = Layout::new();
    layout.split(Axis::Row, pane());
    let kept = layout.split(Axis::Column, pane());
    layout.only();
    assert_eq!(layout.root, Node::Pane(kept));
    assert_eq!(layout.panes.len(), 1);
}

#[test]
fn rects_follow_flex_weights() {
    let mut layout = Layout::new();
    let a = layout.focus;
    let b = layout.split(Axis::Row, pane());
    let c = layout.split(Axis::Row, pane());
    let rects = layout.rects();
    assert_eq!(rects[0].0, a);
    assert_close(rects[0].1.w, 0.5);
    assert_eq!(rects[1].0, b);
    assert_close(rects[1].1.x, 0.5);
    assert_close(rects[1].1.w, 0.25);
    assert_eq!(rects[2].0, c);
    assert_close(rects[2].1.x, 0.75);
    assert_close(rects[2].1.w, 0.25);
}

#[test]
fn dragging_a_divider_moves_only_its_neighbours() {
    let mut layout = Layout::new();
    layout.split(Axis::Row, pane());
    layout.split(Axis::Row, pane());
    layout.drag_divider(&[], 0, 0.3);
    let widths: Vec<f32> = layout.rects().iter().map(|(_, r)| r.w).collect();
    assert_close(widths[0], 0.3);
    assert_close(widths[1], 0.45);
    assert_close(widths[2], 0.25);
    // Clamped so neither neighbour vanishes.
    layout.drag_divider(&[], 1, 2.0);
    let widths: Vec<f32> = layout.rects().iter().map(|(_, r)| r.w).collect();
    assert_close(widths[2], MIN_SHARE);
    assert_close(widths.iter().sum(), 1.0);
    // Stale paths and dividers are ignored.
    let before = layout.root.clone();
    layout.drag_divider(&[5], 0, 0.5);
    layout.drag_divider(&[], 2, 0.5);
    layout.drag_divider(&[], 0, f32::NAN);
    assert_eq!(layout.root, before);
}

#[test]
fn nested_divider_drags_within_its_split() {
    let mut layout = Layout::new();
    let a = layout.focus;
    layout.split(Axis::Row, pane());
    let c = layout.split(Axis::Column, pane());
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
    layout.focus = c;
    assert_eq!(layout.neighbour(Side::Left), Some(a));
}

#[test]
fn merging_a_same_axis_split_keeps_proportions() {
    let mut layout = Layout::new();
    let a = layout.focus;
    let b = layout.split(Axis::Row, pane());
    let c = layout.split(Axis::Column, pane());
    layout.focus = c;
    let d = layout.split(Axis::Row, pane());
    // a | b
    //   | c d
    layout.drag_divider(&[1, 1], 0, 0.6);
    let before: Vec<f32> = layout.rects().iter().map(|(_, r)| r.w).collect();
    layout.close(b).unwrap();
    // The column collapses and c, d join the root row with their widths intact.
    let after = layout.rects();
    assert_eq!(
        after.iter().map(|(id, _)| *id).collect::<Vec<_>>(),
        [a, c, d]
    );
    assert_close(after[0].1.w, before[0]);
    assert_close(after[1].1.w, before[2]);
    assert_close(after[2].1.w, before[3]);
}

#[test]
fn flex_rejects_non_positive_weights() {
    assert_eq!(Flex::new(0.0), None);
    assert_eq!(Flex::new(-1.0), None);
    assert_eq!(Flex::new(f32::INFINITY), None);
    assert_eq!(Flex::new(0.5).map(Flex::get), Some(0.5));
}
