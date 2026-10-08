use model::window::{Axis, Side};
use state::layout::{LastPane, Layout, Node, Pane, View};

fn pane() -> Pane {
    Pane {
        view: View::Empty,
        parked: Vec::new(),
    }
}

#[test]
fn same_axis_splits_stay_flat() {
    let mut layout = Layout::new();
    let a = layout.focus;
    let b = layout.split(Axis::Row, pane());
    let c = layout.split(Axis::Row, pane());
    assert_eq!(
        layout.root,
        Node::Split {
            axis: Axis::Row,
            children: vec![Node::Pane(a), Node::Pane(b), Node::Pane(c)],
        }
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
        Node::Split {
            axis: Axis::Row,
            children: vec![
                Node::Pane(a),
                Node::Split {
                    axis: Axis::Column,
                    children: vec![Node::Pane(b), Node::Pane(c)],
                },
            ],
        }
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
        Node::Split {
            axis: Axis::Row,
            children: vec![
                Node::Pane(a),
                Node::Split {
                    axis: Axis::Column,
                    children: vec![Node::Pane(b), Node::Pane(d)],
                }
            ],
        }
    );
    layout.close(b).unwrap();
    assert_eq!(
        layout.root,
        Node::Split {
            axis: Axis::Row,
            children: vec![Node::Pane(a), Node::Pane(d)],
        }
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
