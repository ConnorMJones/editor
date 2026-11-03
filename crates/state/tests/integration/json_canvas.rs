use std::fs;

use model::notice::Notice;
use model::sheet::{PlaneView, Zoom};
use state::json_canvas::{Embedded, JsonCanvasTab, Parsed};
use state::project::session;
use state::render::flow::{BlockKind, NoteBlock};
use state::view::sheet;
use state::view::tab::Tab;

use jsoncanvas::{JsonCanvasColor, Preset};
use model::geom::{Pane, Point};
use model::mode::Mode;
use state::draft::{Beside, Spread};
use state::input::{self, keymap::Context};
use state::json_canvas::{Adding, Doing, Phase, Place, Pointer, Snap};
use state::reading::Taps;
use state::view::panes::{self, Arrive, Dragged, DropOn, PanePoint, Zone};

use crate::drawing::measure;
use crate::harness::Harness;

const BOARD: &str = r##"{"nodes":[
    {"id":"g","type":"group","label":"Ideas","x":-50,"y":-50,"width":900,"height":500},
    {"id":"t","type":"text","text":"# Title\n\nSee [[note]] and $x^2$.","x":0,"y":0,"width":300,"height":200,"color":"1"},
    {"id":"n","type":"file","file":"note.md","x":400,"y":0,"width":300,"height":200},
    {"id":"p","type":"file","file":"pic.png","x":0,"y":300,"width":100,"height":100},
    {"id":"m","type":"file","file":"gone.md","x":200,"y":300,"width":100,"height":100},
    {"id":"l","type":"link","url":"https://example.com","x":400,"y":300,"width":200,"height":100}
],"edges":[{"id":"e","fromNode":"t","toNode":"n","label":"leads to"}]}"##;

fn board() -> (tempfile::TempDir, Harness) {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("board.canvas"), BOARD).unwrap();
    fs::write(dir.path().join("note.md"), "# Note\n\nFirst.\n").unwrap();
    let mut png = std::io::Cursor::new(Vec::new());
    image::RgbImage::new(4, 4)
        .write_to(&mut png, image::ImageFormat::Png)
        .unwrap();
    fs::write(dir.path().join("pic.png"), png.into_inner()).unwrap();
    let mut h = Harness::new(dir.path());
    h.keys(":o board.canvas<ret>");
    (dir, h)
}

fn tab(h: &Harness) -> &JsonCanvasTab {
    match h.state.layout.focused().active() {
        Some(Tab::JsonCanvas(tab)) => tab,
        _ => panic!("a canvas is focused"),
    }
}

#[test]
fn a_canvas_opens_with_its_cards_read_and_parsed() {
    let (dir, h) = board();
    assert_eq!(h.focused_tabs(), ["◫board.canvas"]);
    let file = &h.state.json_canvases[tab(&h).file()];
    assert_eq!(file.canvas().nodes().len(), 6);
    let embedded = |id: &str| {
        let node = file
            .canvas()
            .nodes()
            .iter()
            .find(|node| node.id.as_str() == id)
            .unwrap();
        &file.files[&node.id]
    };
    assert!(matches!(embedded("n"), Embedded::Note { text, .. } if text.contains("First.")));
    assert!(matches!(embedded("p"), Embedded::Picture(_)));
    assert!(matches!(embedded("m"), Embedded::Missing(path) if path.ends_with("gone.md")));
    let Parsed::At { cards, .. } = &file.parsed else {
        panic!("cards are parsed once the index is ready");
    };
    assert_eq!(cards.len(), 2, "the text card and the note");
    let text = &cards[&file.canvas().nodes()[1].id];
    assert!(text.flow.blocks.iter().any(|block| {
        matches!(block.kind, BlockKind::Own(NoteBlock::Math(_)))
            || block
                .inline()
                .is_some_and(|inline| !inline.links.is_empty())
    }));
    drop(dir);
}

#[test]
fn the_first_measure_fits_the_cards_and_equals_fits_again() {
    let (_dir, mut h) = board();
    let pane = h.state.layout.focus();
    h.run(|state, _| sheet::measured(state, pane, measure()));
    let fitted = tab(&h).view;
    assert_eq!(
        fitted.centre.x, 400.0,
        "the middle of the cards and the group"
    );
    assert!(
        fitted.zoom.get() < 1.0 && fitted.zoom.get() > 0.7,
        "{:?}",
        fitted.zoom
    );
    h.keys("+++");
    assert!(tab(&h).view.zoom.get() > fitted.zoom.get());
    h.keys("zf");
    assert_eq!(tab(&h).view, fitted);
    h.run(|state, _| sheet::measured(state, pane, measure()));
    h.keys("+");
    let zoomed = tab(&h).view;
    h.run(|state, _| sheet::measured(state, pane, measure()));
    assert_eq!(tab(&h).view, zoomed, "only the first measure fits");
}

#[test]
fn a_session_keeps_the_canvas_view() {
    let (dir, mut before) = board();
    let pane = before.state.layout.focus();
    before.run(|state, _| sheet::measured(state, pane, measure()));
    before.keys("++");
    let kept: PlaneView = tab(&before).view;
    let text = toml::to_string(&session::snapshot(&before.state)).unwrap();
    let mut after = Harness::new(dir.path());
    let saved: session::Session = toml::from_str(&text).unwrap();
    after
        .run(|state, out| session::restore(state, saved, out))
        .unwrap();
    let pane = after.state.layout.focus();
    after.run(|state, _| sheet::measured(state, pane, measure()));
    assert_eq!(
        tab(&after).view,
        kept,
        "a restored view is not fitted again"
    );
    assert!(kept.zoom != Zoom::ONE);
}

#[test]
fn changing_an_embedded_note_reads_the_canvas_again() {
    let (dir, mut h) = board();
    let note = dir.path().join("note.md");
    fs::write(&note, "# Note\n\nSecond.\n").unwrap();
    h.changed(vec![note]);
    let file = &h.state.json_canvases[tab(&h).file()];
    let node = &file.canvas().nodes()[2].id;
    assert!(matches!(&file.files[node], Embedded::Note { text, .. } if text.contains("Second.")));
}

#[test]
fn a_file_card_opens_its_file_and_a_broken_canvas_is_refused() {
    let (dir, mut h) = board();
    let (pane, id) = (h.state.layout.focus(), tab(&h).file());
    h.run(|state, out| state::json_canvas::open_node(state, pane, id, 2, out));
    assert!(h.focused_path().unwrap().ends_with("note.md"));
    let opened = h.run(|state, out| {
        state::json_canvas::open_node(state, pane, id, 5, out);
        (out.iter()).any(
            |effect| matches!(effect, state::Effect::OpenUrl(url) if url == "https://example.com"),
        )
    });
    assert!(opened, "a link card opens its url");

    fs::write(
        dir.path().join("broken.canvas"),
        r#"{"nodes":[{"id":"a"}]}"#,
    )
    .unwrap();
    h.keys(":o broken.canvas<ret>");
    assert!(
        matches!(&h.state.notice, Some(Notice::Unreadable { reason, .. }) if reason.contains("type"))
    );
}

fn measured(h: &mut Harness) {
    let pane = h.state.layout.focus();
    h.run(|state, _| sheet::measured(state, pane, measure()));
}

fn on_pane(h: &Harness, x: f32, y: f32) -> Point<Pane> {
    let viewport = measure().viewport;
    tab(h).view.frame(viewport).to_pane.apply(Point::new(x, y))
}

fn mouse(h: &mut Harness, phase: Phase, (x, y): (f32, f32), adding: Adding, snap: Snap) {
    let pointer = Pointer {
        at: on_pane(h, x, y),
        viewport: measure().viewport,
        adding,
        snap,
        taps: Taps::One,
    };
    let pane = h.state.layout.focus();
    h.run(|state, _| state::json_canvas::pointer(state, pane, phase, pointer));
}

fn drag(h: &mut Harness, from: (f32, f32), to: (f32, f32), snap: Snap) {
    mouse(h, Phase::Down, from, Adding::Replace, snap);
    mouse(h, Phase::Move, to, Adding::Replace, snap);
    mouse(h, Phase::Up, to, Adding::Replace, snap);
}

fn tap(h: &mut Harness, at: (f32, f32), adding: Adding) {
    mouse(h, Phase::Down, at, adding, Snap::Grid);
    mouse(h, Phase::Up, at, adding, Snap::Grid);
}

fn area(h: &Harness, id: &str) -> jsoncanvas::Area {
    let file = &h.state.json_canvases[tab(h).file()];
    (file.canvas().nodes().iter())
        .find(|node| node.id.as_str() == id)
        .unwrap_or_else(|| panic!("node {id}"))
        .area
}

fn picked(h: &Harness) -> Vec<String> {
    (tab(h)
        .arranging()
        .and_then(|arranging| arranging.picked.as_ref())
        .into_iter())
    .flat_map(|picked| picked.nodes())
    .map(|id| id.as_str().to_owned())
    .collect()
}

fn modified(h: &Harness) -> bool {
    h.state.json_canvases[tab(h).file()].is_modified()
}

#[test]
fn dragging_a_card_snaps_to_the_grid_unless_alt_is_held_and_undo_puts_it_back() {
    let (_dir, mut h) = board();
    measured(&mut h);
    drag(&mut h, (150.0, 100.0), (163.0, 87.0), Snap::Grid);
    assert_eq!((area(&h, "t").x, area(&h, "t").y), (20, -20));
    assert_eq!(area(&h, "g").x, -50, "the group under the card stays");
    assert_eq!(picked(&h), ["t"]);
    assert!(modified(&h));
    drag(&mut h, (170.0, 80.0), (183.0, 87.0), Snap::Free);
    assert_eq!((area(&h, "t").x, area(&h, "t").y), (33, -13));
    h.keys("uu");
    assert_eq!((area(&h, "t").x, area(&h, "t").y), (0, 0));
    assert!(!modified(&h));
    h.keys("U");
    assert_eq!(area(&h, "t").x, 20);
}

#[test]
fn a_tap_picks_without_moving_and_shift_adds() {
    let (_dir, mut h) = board();
    measured(&mut h);
    tap(&mut h, (150.0, 100.0), Adding::Replace);
    assert_eq!(picked(&h), ["t"]);
    assert!(!modified(&h), "a tap changes nothing");
    tap(&mut h, (500.0, 100.0), Adding::Toggle);
    assert_eq!(picked(&h), ["t", "n"]);
    tap(&mut h, (150.0, 100.0), Adding::Toggle);
    assert_eq!(picked(&h), ["n"]);
    tap(&mut h, (-200.0, -200.0), Adding::Replace);
    assert!(picked(&h).is_empty(), "a tap on nothing lets go");
}

#[test]
fn moving_a_group_carries_its_cards() {
    let (_dir, mut h) = board();
    measured(&mut h);
    drag(&mut h, (350.0, 250.0), (395.0, 250.0), Snap::Grid);
    assert_eq!(picked(&h), ["g"]);
    assert_eq!(area(&h, "g").x, 0, "the group's corner lands on the grid");
    assert_eq!(area(&h, "l").x, 450);
}

#[test]
fn a_corner_handle_resizes_the_picked_card_on_the_grid() {
    let (_dir, mut h) = board();
    measured(&mut h);
    tap(&mut h, (150.0, 100.0), Adding::Replace);
    drag(&mut h, (300.0, 200.0), (347.0, 251.0), Snap::Grid);
    let resized = area(&h, "t");
    assert_eq!(
        (
            resized.x,
            resized.y,
            resized.width.get(),
            resized.height.get()
        ),
        (0, 0, 340, 260)
    );
    drag(&mut h, (340.0, 260.0), (-500.0, -500.0), Snap::Grid);
    let least = area(&h, "t");
    assert_eq!((least.width.get(), least.height.get()), (40, 40));
}

#[test]
fn a_box_picks_whole_cards_and_deleting_takes_their_edges_back_with_undo() {
    let (_dir, mut h) = board();
    measured(&mut h);
    drag(&mut h, (-100.0, -100.0), (320.0, 220.0), Snap::Grid);
    assert_eq!(picked(&h), ["t"]);
    h.keys("d");
    let file = &h.state.json_canvases[tab(&h).file()];
    assert_eq!(file.canvas().nodes().len(), 5);
    assert!(file.canvas().edges().is_empty());
    assert!(picked(&h).is_empty());
    h.keys("u");
    let file = &h.state.json_canvases[tab(&h).file()];
    assert_eq!(file.canvas().edges().len(), 1);
    h.keys("d");
    assert!(matches!(h.state.notice, Some(Notice::NothingPicked)));
}

#[test]
fn colour_keys_recolour_the_pick_and_duplicates_keep_their_files() {
    let (_dir, mut h) = board();
    measured(&mut h);
    tap(&mut h, (500.0, 100.0), Adding::Replace);
    h.keys("c3");
    let color = |h: &Harness, id: &str| {
        let file = &h.state.json_canvases[tab(h).file()];
        (file.canvas().nodes().iter())
            .find(|node| node.id.as_str() == id)
            .unwrap()
            .color
    };
    assert_eq!(
        color(&h, "n"),
        Some(JsonCanvasColor::Preset(Preset::Yellow))
    );
    h.keys("c0");
    assert_eq!(color(&h, "n"), None);
    h.keys("y");
    let copy = picked(&h);
    assert_eq!(copy.len(), 1);
    assert_ne!(copy[0], "n");
    assert_eq!(area(&h, &copy[0]).x, 440);
    let file = &h.state.json_canvases[tab(&h).file()];
    let id = &held(&h).nodes()[0];
    assert!(matches!(&file.files[id], Embedded::Note { text, .. } if text.contains("First.")));
    assert!(file.card(id).is_some(), "the copy is parsed");
}

#[test]
fn writing_saves_the_layout_and_a_changed_file_waits_while_edits_are_unsaved() {
    let (dir, mut h) = board();
    measured(&mut h);
    drag(&mut h, (150.0, 100.0), (170.0, 100.0), Snap::Grid);
    h.keys(":w<ret>");
    assert!(!modified(&h));
    let path = dir.path().join("board.canvas");
    let saved = jsoncanvas::read(&fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(saved.node(&held(&h).nodes()[0]).unwrap().area.x, 20);
    assert_eq!(saved.nodes().len(), 6);

    drag(&mut h, (170.0, 100.0), (190.0, 100.0), Snap::Grid);
    std::thread::sleep(std::time::Duration::from_millis(20));
    fs::write(&path, BOARD).unwrap();
    h.changed(vec![path.clone()]);
    assert!(matches!(h.state.notice, Some(Notice::ChangedOnDisk { .. })));
    assert_eq!(area(&h, "t").x, 40, "unsaved edits are kept");

    let note = dir.path().join("note.md");
    fs::write(&note, "# Note\n\nSecond.\n").unwrap();
    h.state.notice = None;
    h.changed(vec![note]);
    let file = &h.state.json_canvases[tab(&h).file()];
    let node = &file.canvas().nodes()[2].id;
    assert!(matches!(&file.files[node], Embedded::Note { text, .. } if text.contains("Second.")));
    assert!(
        modified(&h),
        "an embedded file changing leaves the edits alone"
    );

    h.keys("u");
    assert!(!modified(&h));
    std::thread::sleep(std::time::Duration::from_millis(20));
    fs::write(&path, BOARD).unwrap();
    h.changed(vec![path]);
    assert_eq!(area(&h, "t").x, 0, "a clean canvas takes the disk's");
}

fn double_tap(h: &mut Harness, (x, y): (f32, f32)) {
    let pointer = |taps| Pointer {
        at: on_pane(h, x, y),
        viewport: measure().viewport,
        adding: Adding::Replace,
        snap: Snap::Grid,
        taps,
    };
    let (first, second) = (pointer(Taps::One), pointer(Taps::Two));
    let pane = h.state.layout.focus();
    for (phase, pointer) in [
        (Phase::Down, first),
        (Phase::Up, first),
        (Phase::Down, second),
        (Phase::Up, second),
    ] {
        h.run(|state, _| state::json_canvas::pointer(state, pane, phase, pointer));
    }
}

fn nodes(h: &Harness) -> usize {
    h.state.json_canvases[tab(h).file()].canvas().nodes().len()
}

fn edges(
    h: &Harness,
) -> Vec<(
    String,
    String,
    Option<jsoncanvas::Side>,
    Option<jsoncanvas::Side>,
)> {
    (h.state.json_canvases[tab(h).file()].canvas().edges().iter())
        .map(|edge| {
            (
                edge.from.node.as_str().to_owned(),
                edge.to.node.as_str().to_owned(),
                edge.from.side,
                edge.to.side,
            )
        })
        .collect()
}

#[test]
fn a_double_tap_on_nothing_or_at_adds_a_text_card_on_the_grid() {
    let (_dir, mut h) = board();
    measured(&mut h);
    double_tap(&mut h, (-300.0, -300.0));
    assert_eq!(nodes(&h), 7);
    let added = written(&h).expect("the new card is open");
    let area = area(&h, &added);
    assert_eq!(
        (area.x, area.y, area.width.get(), area.height.get()),
        (-440, -340, 260, 60)
    );
    h.keys("<esc><esc>u");
    assert_eq!(nodes(&h), 6);
    h.keys("at");
    assert_eq!(nodes(&h), 7);
    assert!(written(&h).is_some());
}

#[test]
fn dragging_a_side_dot_onto_another_card_connects_them() {
    let (_dir, mut h) = board();
    measured(&mut h);
    tap(&mut h, (150.0, 100.0), Adding::Replace);
    drag(&mut h, (150.0, 200.0), (420.0, 330.0), Snap::Grid);
    assert_eq!(edges(&h).len(), 2);
    assert_eq!(
        edges(&h)[1],
        (
            "t".to_owned(),
            "l".to_owned(),
            Some(jsoncanvas::Side::Bottom),
            Some(jsoncanvas::Side::Left)
        )
    );
    assert_eq!(held(&h).edges().len(), 1);
    tap(&mut h, (150.0, 100.0), Adding::Replace);
    drag(&mut h, (300.0, 100.0), (200.0, 100.0), Snap::Grid);
    assert_eq!(
        edges(&h).len(),
        2,
        "letting go on the same card connects nothing"
    );
    assert!(h.state.notice.is_none());
}

#[test]
fn an_edge_is_picked_by_its_line_recoloured_and_deleted_alone() {
    let (_dir, mut h) = board();
    measured(&mut h);
    tap(&mut h, (350.0, 101.0), Adding::Replace);
    assert!(picked(&h).is_empty());
    assert_eq!(held(&h).edges().len(), 1);
    h.keys("c1");
    let file = &h.state.json_canvases[tab(&h).file()];
    assert_eq!(
        file.canvas().edges()[0].color,
        Some(JsonCanvasColor::Preset(Preset::Red))
    );
    h.keys("d");
    assert!(edges(&h).is_empty());
    assert_eq!(nodes(&h), 6);
}

#[test]
fn grouping_wraps_the_pick_under_the_cards() {
    let (_dir, mut h) = board();
    measured(&mut h);
    tap(&mut h, (50.0, 350.0), Adding::Replace);
    tap(&mut h, (250.0, 350.0), Adding::Toggle);
    h.keys("ag");
    let file = &h.state.json_canvases[tab(&h).file()];
    let group = &file.canvas().nodes()[0];
    assert!(matches!(group.kind, jsoncanvas::NodeKind::Group { .. }));
    assert_eq!(
        (
            group.area.x,
            group.area.y,
            group.area.right(),
            group.area.bottom()
        ),
        (-40, 260, 340, 440)
    );
    assert_eq!(picked(&h), [group.id.as_str().to_owned()]);
    h.keys("u");
    assert_eq!(nodes(&h), 6);
}

#[test]
fn a_file_dropped_on_the_canvas_becomes_a_card_where_it_landed() {
    let (dir, mut h) = board();
    measured(&mut h);
    fs::write(dir.path().join("other.md"), "# Other\n").unwrap();
    let pane = h.state.layout.focus();
    let zone = Zone::Centre(PanePoint {
        at: on_pane(&h, -400.0, 600.0),
        viewport: measure().viewport,
    });
    let dragged = Dragged::File {
        file: state::project::path::RelPath::parse("other.md").unwrap(),
        arrive: Arrive::AsIs,
    };
    h.run(|state, out| panes::dropped(state, dragged, DropOn::Pane { pane, zone }, out));
    assert_eq!(
        h.focused_tabs(),
        ["◫board.canvas"],
        "the canvas keeps the file"
    );
    let added = &held(&h).nodes()[0];
    let file = &h.state.json_canvases[tab(&h).file()];
    let node = file.canvas().node(added).unwrap();
    assert!(matches!(&node.kind, jsoncanvas::NodeKind::File { file, .. } if file == "other.md"));
    assert_eq!((node.area.x, node.area.y), (-400, 600));
    assert!(matches!(&file.files[added], Embedded::Note { text, .. } if text.contains("Other")));
    assert!(file.card(added).is_some());

    let outside = tempfile::tempdir().unwrap();
    fs::write(outside.path().join("far.md"), "far").unwrap();
    let dragged = Dragged::Outside(outside.path().join("far.md"));
    h.run(|state, out| panes::dropped(state, dragged, DropOn::Pane { pane, zone }, out));
    assert!(matches!(
        h.state.notice,
        Some(Notice::FileOutsideProject { .. })
    ));
    assert_eq!(nodes(&h), 7);
}

fn held(h: &Harness) -> &jsoncanvas::Picked {
    (tab(h).arranging())
        .and_then(|arranging| arranging.picked.as_ref())
        .expect("something is picked")
}

fn written(h: &Harness) -> Option<String> {
    match &tab(h).doing {
        Doing::Writing(writing) => Some(writing.node.as_str().to_owned()),
        Doing::Arranging(_) => None,
    }
}

fn card_text(h: &Harness, id: &str) -> String {
    let file = &h.state.json_canvases[tab(h).file()];
    match &(file.canvas().nodes().iter())
        .find(|node| node.id.as_str() == id)
        .unwrap()
        .kind
    {
        jsoncanvas::NodeKind::Text(text) => text.clone(),
        other => panic!("{other:?} is not text"),
    }
}

#[test]
fn a_double_tap_writes_in_a_text_card_and_esc_keeps_the_text_as_one_step() {
    let (_dir, mut h) = board();
    measured(&mut h);
    double_tap(&mut h, (150.0, 100.0));
    assert_eq!(written(&h).as_deref(), Some("t"));
    assert_eq!(h.state.input.mode, Mode::Insert);
    assert_eq!(input::context(&h.state), Context::DraftInsert);
    h.keys(" More.<esc><esc>");
    assert_eq!(written(&h), None);
    assert_eq!(picked(&h), ["t"]);
    assert!(card_text(&h, "t").ends_with("$x^2$. More."));
    assert!(modified(&h));
    h.keys("u");
    assert!(card_text(&h, "t").ends_with("$x^2$."));
    assert!(!modified(&h));
}

#[test]
fn a_new_card_opens_for_writing_and_a_tap_elsewhere_keeps_it() {
    let (_dir, mut h) = board();
    measured(&mut h);
    h.keys("at");
    let card = written(&h).expect("the new card is open");
    h.keys("hello");
    tap(&mut h, (-300.0, -300.0), Adding::Replace);
    assert_eq!(written(&h), None);
    assert_eq!(card_text(&h, &card), "hello");
    assert_eq!(h.state.input.mode, Mode::Normal);
}

#[test]
fn a_card_draft_moves_beside_and_writing_saves_the_canvas_with_the_draft() {
    let (dir, mut h) = board();
    measured(&mut h);
    tap(&mut h, (150.0, 100.0), Adding::Replace);
    h.keys("i<esc>");
    assert_eq!(input::context(&h.state), Context::Draft);
    h.keys("<C-v>");
    let beside = |h: &Harness| match &tab(h).doing {
        Doing::Writing(writing) => writing.place,
        Doing::Arranging(_) => panic!("writing"),
    };
    assert!(matches!(beside(&h), Place::Beside(_)));
    h.keys("<space>v");
    assert!(matches!(
        beside(&h),
        Place::Beside(Beside {
            spread: Spread::Full,
            ..
        })
    ));
    h.keys("ggOTop<esc>:w<ret>");
    assert!(written(&h).is_some(), "writing goes on");
    let saved =
        jsoncanvas::read(&fs::read_to_string(dir.path().join("board.canvas")).unwrap()).unwrap();
    let text = (saved.nodes().iter())
        .find_map(|node| match (&node.kind, node.id.as_str()) {
            (jsoncanvas::NodeKind::Text(text), "t") => Some(text.clone()),
            _ => None,
        })
        .unwrap();
    assert!(text.starts_with("Top\n# Title"));
    assert!(!modified(&h));
}

#[test]
fn dropping_an_unsaved_card_draft_asks_first_and_keeps_the_card_as_it_was() {
    let (_dir, mut h) = board();
    measured(&mut h);
    tap(&mut h, (150.0, 100.0), Adding::Replace);
    h.keys("iextra<esc><space>q");
    assert!(matches!(
        h.state.overlay,
        state::Overlay::Confirm(state::Confirm::DropDraft)
    ));
    h.keys("y");
    assert_eq!(written(&h), None);
    assert!(card_text(&h, "t").ends_with("$x^2$."));
    assert!(!modified(&h));
}
