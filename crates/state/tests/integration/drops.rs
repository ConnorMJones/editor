use std::fs;

use model::geom::Point;
use model::sheet::Viewport;
use model::units::Px;
use model::window::{Axis, Side};
use state::project::path::RelPath;
use state::view::layout::{Node, PaneId};
use state::view::panes::{self, Dragged, DropOn, PanePoint, Zone};

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

/// The middle of an 800 by 600 pane.
fn centre() -> Zone {
    Zone::Centre(PanePoint {
        at: Point::new(400.0, 300.0),
        viewport: Viewport::new(Px(800.0), Px(600.0)).unwrap(),
    })
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
    let zone = centre();
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
            zone: centre(),
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
    let point = |x, y| PanePoint {
        at: Point::new(x, y),
        viewport,
    };
    let zone = |x, y| Zone::at(point(x, y));
    assert_eq!(zone(400.0, 200.0), Zone::Centre(point(400.0, 200.0)));
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

/// A root holding a note, a drawing's name and a 40 by 20 image in `pictures/`.
fn with_image() -> (tempfile::TempDir, Harness) {
    let dir = tempfile::tempdir().unwrap();
    fs::create_dir(dir.path().join("pictures")).unwrap();
    image::RgbImage::new(40, 20)
        .save(dir.path().join("pictures/photo.png"))
        .unwrap();
    fs::write(dir.path().join("note.md"), "# Note\n\nSome text.\n").unwrap();
    let h = Harness::new(dir.path());
    (dir, h)
}

fn text(h: &Harness) -> String {
    String::from(&h.doc().text)
}

fn image_marks(h: &Harness) -> usize {
    match h.state.layout.focused().active() {
        Some(state::view::tab::Tab::Ink(state::drawing::InkTabs::Paged(tab))) => {
            let drawing = ink::doc::Drawing::marks(
                h.state.pageds[tab.file()].edited.drawing(),
                ink::doc::PageLayer {
                    page: ink::doc::PageIndex(0),
                    layer: ink::doc::LayerIndex(0),
                },
            );
            (drawing.unwrap().iter())
                .filter(|(_, mark)| matches!(mark.ink, ink::doc::Ink::Image(_)))
                .count()
        }
        Some(_) | None => panic!("a paged drawing is focused"),
    }
}

#[test]
fn an_image_dropped_on_a_note_is_linked_below_the_cursor_line() {
    let (_dir, mut h) = with_image();
    h.keys(":o note.md<ret>jj");
    let pane = h.state.layout.focus();
    let dragged = file(&h, "pictures/photo.png");
    drop(
        &mut h,
        dragged,
        DropOn::Pane {
            pane,
            zone: centre(),
        },
    );
    assert_eq!(text(&h), "# Note\n\nSome text.\n![[photo.png]]\n");
    assert_eq!(h.focused_tabs(), ["note.md"], "the note stays open");
}

#[test]
fn an_image_from_outside_is_copied_beside_the_note_with_a_kebab_name() {
    let (dir, mut h) = with_image();
    let outside = tempfile::tempdir().unwrap();
    let source = outside.path().join("My Photo (1).PNG");
    fs::copy(dir.path().join("pictures/photo.png"), &source).unwrap();
    fs::create_dir(dir.path().join("notes")).unwrap();
    fs::write(dir.path().join("notes/trip.md"), "").unwrap();
    fs::write(dir.path().join("notes/my-photo-1.png"), "taken").unwrap();
    h.keys(":o notes/trip.md<ret>");
    let pane = h.state.layout.focus();
    drop(
        &mut h,
        Dragged::Outside(source),
        DropOn::Pane {
            pane,
            zone: centre(),
        },
    );
    assert_eq!(text(&h), "![[my-photo-1-2.png]]");
    let copied = fs::read(dir.path().join("notes/my-photo-1-2.png")).unwrap();
    assert_eq!(
        copied,
        fs::read(dir.path().join("pictures/photo.png")).unwrap()
    );
}

#[test]
fn an_image_dropped_on_a_drawing_is_embedded_where_it_landed() {
    let (dir, mut h) = with_image();
    h.keys(":o sketch.pdt<ret>");
    let pane = h.state.layout.focus();
    let dragged = file(&h, "pictures/photo.png");
    drop(
        &mut h,
        dragged,
        DropOn::Pane {
            pane,
            zone: centre(),
        },
    );
    assert_eq!(image_marks(&h), 1);
    assert_eq!(
        fs::read_dir(dir.path()).unwrap().count(),
        2,
        "nothing is copied for a drawing: the root still holds pictures/ and note.md"
    );
}

#[test]
fn an_image_dropped_elsewhere_opens() {
    let (_dir, mut h) = with_image();
    fs::write(
        h.state.project.root.path().join("code.rs"),
        "fn main() {}\n",
    )
    .unwrap();
    h.keys(":o code.rs<ret>");
    let pane = h.state.layout.focus();
    let dragged = file(&h, "pictures/photo.png");
    drop(
        &mut h,
        dragged,
        DropOn::Pane {
            pane,
            zone: centre(),
        },
    );
    assert_eq!(h.focused_tabs(), ["code.rs", "▣photo.png"]);
}

#[test]
fn the_image_picker_lists_images_and_puts_the_pick_in_the_drawing() {
    let (_dir, mut h) = with_image();
    h.keys(":o sketch.pdt<ret><space>i");
    match &h.state.overlay {
        state::Overlay::Picker(picker) => {
            let labels: Vec<&str> = (0..picker.items.len() as u32)
                .map(|index| picker.items.label(index))
                .collect();
            assert_eq!(labels, ["pictures/photo.png"]);
        }
        _ => panic!("the picker is open"),
    }
    h.keys("<ret>");
    assert_eq!(image_marks(&h), 1);
}

#[test]
fn a_pasted_image_is_kept_beside_the_note_or_embedded_in_a_drawing() {
    let (dir, mut h) = with_image();
    let bytes: std::sync::Arc<[u8]> = fs::read(dir.path().join("pictures/photo.png"))
        .unwrap()
        .into();
    let encoding = model::picture::Encoding::Bitmap(model::picture::Bitmap::Png);
    let paste = |h: &mut Harness| {
        let into = state::insert::into_focused(&h.state, None).unwrap();
        let image = state::insert::ImageFrom::Pasted(bytes.clone(), encoding);
        h.run(|state, out| state::insert::pasted(state, into, Some(image), out));
    };
    h.keys(":o note.md<ret>ggi<ret><esc>k");
    paste(&mut h);
    let linked = text(&h);
    let name = linked.lines().next().unwrap();
    assert!(
        name.starts_with("![[pasted-image-") && name.ends_with(".png]]"),
        "{linked}"
    );
    let file = &name[3..name.len() - 2];
    assert_eq!(&*fs::read(dir.path().join(file)).unwrap(), &*bytes);
    h.keys(":o sketch.pdt<ret>");
    paste(&mut h);
    assert_eq!(image_marks(&h), 1);
    let into = state::insert::into_focused(&h.state, None).unwrap();
    h.run(|state, out| state::insert::pasted(state, into, None, out));
    assert!(matches!(
        h.state.notice,
        Some(model::notice::Notice::NoImageOnClipboard)
    ));
}
