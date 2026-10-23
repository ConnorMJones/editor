use std::fs;

use model::geom::Point;
use model::sheet::Viewport;
use model::units::Px;
use model::window::{Axis, Side};
use state::project::path::RelPath;
use state::view::layout::{Node, PaneId};
use state::view::panes::{self, Dragged, DropOn, Zone};

use crate::harness::Harness;

/// A harness on a root holding `a.txt`, `b.txt` and `c.txt`, with `a.txt` open.
fn three_files() -> (tempfile::TempDir, Harness) {
    let dir = tempfile::tempdir().unwrap();
    for name in ["a.txt", "b.txt", "c.txt"] {
        fs::write(dir.path().join(name), format!("{name}\n")).unwrap();
    }
    let mut h = Harness::new(dir.path());
    h.keys(":o a.txt<ret>");
    (dir, h)
}

fn file(h: &Harness, name: &str) -> Dragged {
    Dragged::File(
        RelPath::new(
            h.state.project.root.path(),
            &h.state.project.root.path().join(name),
        )
        .unwrap(),
    )
}

fn drop(h: &mut Harness, dragged: Dragged, on: DropOn) {
    h.run(|state, out| panes::dropped(state, dragged, on, out));
}

/// The root split's axis and its panes in order, for a layout one split deep.
fn split(h: &Harness) -> (Axis, Vec<PaneId>) {
    match h.state.layout.root() {
        Node::Split(split) => {
            let panes = (split.children().iter())
                .map(|child| match child.node {
                    Node::Pane(pane) => pane,
                    Node::Split(_) => panic!("one split deep"),
                })
                .collect();
            (split.axis(), panes)
        }
        Node::Pane(_) => panic!("the layout is split"),
    }
}

#[test]
fn a_file_dropped_on_a_pane_opens_there() {
    let (_dir, mut h) = three_files();
    let pane = h.state.layout.focus();
    let zone = Zone::Centre;
    let dragged = file(&h, "b.txt");
    drop(&mut h, dragged, DropOn::Pane { pane, zone });
    assert_eq!(h.focused_tabs(), ["a.txt", "b.txt"]);
    assert_eq!(h.state.layout.panes().len(), 1);
}

#[test]
fn a_file_dropped_near_an_edge_splits_that_way() {
    let (_dir, mut h) = three_files();
    let pane = h.state.layout.focus();
    let zone = Zone::Edge(Side::Left);
    let dragged = file(&h, "b.txt");
    drop(&mut h, dragged, DropOn::Pane { pane, zone });
    let (axis, panes) = split(&h);
    assert_eq!((axis, panes.len()), (Axis::Row, 2));
    assert_eq!(
        h.tabs(panes[0]),
        ["b.txt"],
        "the new pane holds only the dropped file"
    );
    assert_eq!(h.tabs(panes[1]), ["a.txt"]);
    assert_eq!(h.state.layout.focus(), panes[0]);
}

#[test]
fn a_file_dropped_on_an_empty_panes_edge_opens_in_it() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("a.txt"), "a\n").unwrap();
    let mut h = Harness::new(dir.path());
    let pane = h.state.layout.focus();
    let zone = Zone::Edge(Side::Down);
    let dragged = file(&h, "a.txt");
    drop(&mut h, dragged, DropOn::Pane { pane, zone });
    assert_eq!(h.state.layout.panes().len(), 1);
    assert_eq!(h.focused_tabs(), ["a.txt"]);
}

#[test]
fn a_tab_moves_to_another_pane_and_its_emptied_pane_closes() {
    let (_dir, mut h) = three_files();
    h.keys("<C-w>v:o b.txt<ret>");
    let (_, panes) = split(&h);
    // The right pane holds a.txt (its split copy was replaced) and b.txt.
    drop(
        &mut h,
        Dragged::Tab {
            pane: panes[0],
            index: 0,
        },
        DropOn::Pane {
            pane: panes[1],
            zone: Zone::Centre,
        },
    );
    assert_eq!(
        h.state.layout.panes().len(),
        1,
        "the left pane had only that tab"
    );
    assert!(h.focused_tabs().contains(&"b.txt".to_owned()));
    assert_eq!(h.focused_tabs().len(), 2, "a.txt is not doubled");
}

#[test]
fn a_tab_dropped_near_another_panes_edge_splits_beside_it() {
    let (_dir, mut h) = three_files();
    h.keys(":o b.txt<ret>");
    let pane = h.state.layout.focus();
    drop(
        &mut h,
        Dragged::Tab { pane, index: 1 },
        DropOn::Pane {
            pane,
            zone: Zone::Edge(Side::Down),
        },
    );
    let (axis, panes) = split(&h);
    assert_eq!(axis, Axis::Column);
    assert_eq!(h.tabs(panes[0]), ["a.txt"]);
    assert_eq!(h.tabs(panes[1]), ["b.txt"]);
    // Splitting a pane's only tab off beside itself changes nothing.
    drop(
        &mut h,
        Dragged::Tab {
            pane: panes[1],
            index: 0,
        },
        DropOn::Pane {
            pane: panes[1],
            zone: Zone::Edge(Side::Right),
        },
    );
    assert_eq!(split(&h).1, panes);
}

#[test]
fn a_tab_dropped_on_the_tab_bar_takes_that_place() {
    let (_dir, mut h) = three_files();
    h.keys(":o b.txt<ret>:o c.txt<ret>");
    let pane = h.state.layout.focus();
    assert_eq!(h.focused_tabs(), ["a.txt", "b.txt", "c.txt"]);
    drop(
        &mut h,
        Dragged::Tab { pane, index: 2 },
        DropOn::Tabs { pane, at: 0 },
    );
    assert_eq!(h.focused_tabs(), ["c.txt", "a.txt", "b.txt"]);
    assert_eq!(h.state.layout.focused().tabs.as_ref().unwrap().active(), 0);
}

#[test]
fn a_stale_drag_changes_nothing() {
    let (_dir, mut h) = three_files();
    let pane = h.state.layout.focus();
    drop(
        &mut h,
        Dragged::Tab { pane, index: 5 },
        DropOn::Pane {
            pane,
            zone: Zone::Edge(Side::Left),
        },
    );
    assert_eq!(h.state.layout.panes().len(), 1);
    assert_eq!(h.focused_tabs(), ["a.txt"]);
}

#[test]
fn zones_are_the_nearest_edge_within_a_quarter() {
    let viewport = Viewport::new(Px(800.0), Px(400.0)).unwrap();
    let zone = |x, y| Zone::at(Point::new(x, y), viewport);
    assert_eq!(zone(400.0, 200.0), Zone::Centre);
    assert_eq!(zone(100.0, 200.0), Zone::Edge(Side::Left));
    assert_eq!(zone(790.0, 200.0), Zone::Edge(Side::Right));
    assert_eq!(zone(400.0, 20.0), Zone::Edge(Side::Up));
    assert_eq!(zone(400.0, 380.0), Zone::Edge(Side::Down));
    assert_eq!(
        zone(20.0, 380.0),
        Zone::Edge(Side::Left),
        "nearer the left than the bottom"
    );
}
