use jsoncanvas::{
    Change, ChangeError, Edited, HistoryError, JsonCanvasColor, Offset, Picked, Preset, read, write,
};

const BOARD: &str = r##"{"nodes":[
    {"id":"g","type":"group","x":0,"y":0,"width":500,"height":300},
    {"id":"a","type":"text","text":"a","x":20,"y":20,"width":100,"height":100},
    {"id":"b","type":"text","text":"b","x":600,"y":20,"width":100,"height":100}
],"edges":[{"id":"e","fromNode":"a","toNode":"b"}]}"##;

fn ids(edited: &Edited) -> Vec<&str> {
    (edited.canvas().nodes().iter())
        .map(|node| node.id.as_str())
        .collect()
}

fn node(edited: &Edited, id: &str) -> jsoncanvas::Area {
    (edited.canvas().nodes().iter())
        .find(|node| node.id.as_str() == id)
        .unwrap()
        .area
}

fn pick(edited: &Edited, id: &str) -> Picked {
    Picked::node(
        (edited.canvas().nodes().iter())
            .find(|node| node.id.as_str() == id)
            .unwrap()
            .id
            .clone(),
    )
}

#[test]
fn moving_a_group_carries_what_lies_inside_it_and_undoes_as_one_step() {
    let mut edited = Edited::new(read(BOARD).unwrap());
    let saved = edited.revision();
    let by = Offset { dx: 40, dy: -20 };
    edited
        .change(&Change::Move {
            picked: pick(&edited, "g"),
            by,
        })
        .unwrap();
    assert_eq!((node(&edited, "g").x, node(&edited, "a").x), (40, 60));
    assert_eq!(node(&edited, "b").x, 600, "outside the group");
    assert_ne!(edited.revision(), saved);
    assert_eq!(edited.undo().unwrap(), saved);
    assert_eq!(node(&edited, "a").x, 20);
    edited.redo().unwrap();
    assert_eq!(node(&edited, "a").y, 0);
    assert_eq!(edited.redo(), Err(HistoryError::NothingToRedo));
}

#[test]
fn removing_a_node_takes_its_edges() {
    let mut edited = Edited::new(read(BOARD).unwrap());
    edited.change(&Change::Remove(pick(&edited, "b"))).unwrap();
    assert_eq!(ids(&edited), ["g", "a"]);
    assert!(edited.canvas().edges().is_empty());
    let text = write(edited.canvas());
    assert_eq!(read(&text).unwrap(), *edited.canvas());
}

#[test]
fn duplicating_gives_fresh_ids_and_copies_edges_between_the_copies() {
    let mut edited = Edited::new(read(BOARD).unwrap());
    let both = Picked::new(
        vec![
            pick(&edited, "a").nodes()[0].clone(),
            pick(&edited, "b").nodes()[0].clone(),
        ],
        Vec::new(),
    )
    .unwrap();
    let copies = edited.duplicate(&both, Offset { dx: 0, dy: 200 }).unwrap();
    assert_eq!(edited.canvas().nodes().len(), 5);
    assert_eq!(edited.canvas().edges().len(), 2);
    assert_eq!(copies.picked.nodes().len(), 2);
    for id in copies.picked.nodes() {
        assert_eq!(id.as_str().len(), 16);
        assert_eq!(edited.canvas().node(id).unwrap().area.y, 220);
        assert!(copies.from.contains_key(id));
    }
    assert!(read(&write(edited.canvas())).is_ok(), "ids stay unique");
}

#[test]
fn a_change_naming_a_missing_node_changes_nothing() {
    let mut edited = Edited::new(read(BOARD).unwrap());
    let gone = pick(&edited, "b");
    edited.change(&Change::Remove(gone.clone())).unwrap();
    let before = edited.revision();
    assert!(matches!(
        edited.change(&Change::Recolor {
            picked: gone,
            color: Some(JsonCanvasColor::Preset(Preset::Red)),
        }),
        Err(ChangeError::UnknownNode(_))
    ));
    assert_eq!(edited.revision(), before);
}

#[test]
fn toggling_a_pick_adds_and_drops() {
    let edited = Edited::new(read(BOARD).unwrap());
    let a = pick(&edited, "a");
    let both = a.clone().toggled(pick(&edited, "b")).unwrap();
    assert_eq!(both.nodes().len(), 2);
    assert_eq!(both.toggled(pick(&edited, "b")), Some(a.clone()));
    assert_eq!(a.clone().toggled(a), None);
}

#[test]
fn added_groups_sit_under_cards_and_edges_join_two_existing_nodes() {
    let mut edited = Edited::new(read(BOARD).unwrap());
    let area = node(&edited, "a");
    let group = edited.add_node(jsoncanvas::NewNode {
        area,
        color: None,
        kind: jsoncanvas::NodeKind::Group {
            label: None,
            background: None,
        },
    });
    assert_eq!(edited.canvas().nodes()[0].id, group);
    let card = edited.add_node(jsoncanvas::NewNode {
        area,
        color: None,
        kind: jsoncanvas::NodeKind::Text(String::new()),
    });
    assert_eq!(edited.canvas().nodes().last().unwrap().id, card);
    let end = |node: &jsoncanvas::NodeId| jsoncanvas::End {
        node: node.clone(),
        side: None,
        cap: None,
    };
    let a = pick(&edited, "a").nodes()[0].clone();
    assert!(matches!(
        edited.add_edge(jsoncanvas::NewEdge {
            from: end(&a),
            to: end(&a),
        }),
        Err(ChangeError::SameNode(_))
    ));
    edited
        .add_edge(jsoncanvas::NewEdge {
            from: end(&a),
            to: end(&card),
        })
        .unwrap();
    assert!(read(&write(edited.canvas())).is_ok());
    edited.undo().unwrap();
    edited.undo().unwrap();
    assert_eq!(ids(&edited).len(), 4, "the edge and the card are undone");
}
