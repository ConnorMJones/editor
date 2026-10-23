use std::fs;
use std::time::Duration;

use ink::doc::{Drawing, LayerIndex, PageIndex, PageLayer};
use ink::input::Pressure;
use model::geom::{Pane, Point};
use model::notice::Notice;
use model::sheet::{Fit, PageIndex as Page, Viewport, Zoom};
use model::units::Px;
use state::drawing::pen::{self, PenPhase, PenSample, Stylus};
use state::drawing::{self as drawing, InkTabs};
use state::input::{self, keymap::Context};
use state::project::session;
use state::view::tab::Tab;

use crate::harness::Harness;

const FIRST: PageLayer = PageLayer {
    page: PageIndex(0),
    layer: LayerIndex(0),
};

fn viewport() -> Viewport {
    Viewport::new(Px(800.0), Px(600.0)).unwrap()
}

fn sample(at: Point<Pane>, ms: u64) -> PenSample {
    PenSample {
        at,
        viewport: viewport(),
        time: Duration::from_millis(ms),
        pressure: Some(Pressure::new(0.6).unwrap()),
        tilt: None,
    }
}

/// Where sheet point `(x, y)` is on the focused pane, as its view shows it at `viewport()`; on
/// a board the sheet is the board.
fn on_pane(h: &Harness, (x, y): (f32, f32)) -> Point<Pane> {
    match h.state.layout.focused().active() {
        Some(Tab::Ink(InkTabs::Paged(tab))) => {
            let stack = h.state.pageds[tab.file()].edited.drawing().stack();
            tab.view
                .frame(stack, viewport())
                .to_pane
                .apply(Point::new(x, y))
        }
        Some(Tab::Ink(InkTabs::Board(tab))) => {
            tab.view.frame(viewport()).to_pane.apply(Point::new(x, y))
        }
        Some(Tab::Text(_)) | None => panic!("no drawing is focused"),
    }
}

/// One stroke through `points` on the focused pane, in sheet coordinates.
fn draw(h: &mut Harness, stylus: Stylus, points: &[(f32, f32)]) {
    let pane = h.state.layout.focus();
    h.run(|state, _| drawing::measured(state, pane, viewport()));
    let points: Vec<Point<Pane>> = points.iter().map(|at| on_pane(h, *at)).collect();
    let last = points.len() - 1;
    for (index, &at) in points.iter().enumerate() {
        let phase = match index {
            0 => PenPhase::Down(stylus),
            index if index == last => PenPhase::Up,
            _ => PenPhase::Move,
        };
        h.run(|state, _| pen::pen(state, pane, phase, sample(at, index as u64 * 5)));
    }
}

fn line(from: (f32, f32), to: (f32, f32)) -> Vec<(f32, f32)> {
    (0..=20)
        .map(|step| {
            let t = step as f32 / 20.0;
            (from.0 + (to.0 - from.0) * t, from.1 + (to.1 - from.1) * t)
        })
        .collect()
}

/// The focused paged document's marks on the first page's first layer.
fn marks(h: &Harness) -> usize {
    match h.state.layout.focused().active() {
        Some(Tab::Ink(InkTabs::Paged(tab))) => {
            let drawing = h.state.pageds[tab.file()].edited.drawing();
            drawing.marks(FIRST).unwrap().len()
        }
        Some(Tab::Ink(InkTabs::Board(tab))) => {
            let drawing = h.state.boards[tab.file()].edited.drawing();
            drawing.marks(LayerIndex(0)).unwrap().len()
        }
        Some(Tab::Text(_)) | None => panic!("no drawing is focused"),
    }
}

fn modified(h: &Harness) -> bool {
    let opened = h.state.layout.focused().active().unwrap().opened();
    h.state.is_modified(opened)
}

#[test]
fn a_new_paged_document_opens_blank_and_takes_drawing_keys() {
    let dir = tempfile::tempdir().unwrap();
    let mut h = Harness::new(dir.path());
    h.keys(":o notes.pdt<ret>");
    assert_eq!(h.focused_tabs(), ["✎notes.pdt"]);
    assert_eq!(input::context(&h.state), Context::Drawing);
    assert_eq!(marks(&h), 0);
    assert!(!modified(&h));
}

#[test]
fn a_stroke_on_a_page_is_drawn_and_saved() {
    let dir = tempfile::tempdir().unwrap();
    let mut h = Harness::new(dir.path());
    h.keys(":o notes.pdt<ret>");
    draw(&mut h, Stylus::Tip, &line((100.0, 100.0), (300.0, 160.0)));
    assert_eq!(marks(&h), 1);
    assert!(modified(&h));
    h.keys(":w<ret>");
    assert!(!modified(&h));
    let saved = ink::format::read_paged(&fs::read(dir.path().join("notes.pdt")).unwrap()).unwrap();
    assert_eq!(saved.marks(FIRST).unwrap().len(), 1);
}

#[test]
fn a_stroke_starting_between_pages_draws_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let mut h = Harness::new(dir.path());
    h.keys(":o notes.pdt<ret>A");
    // A4 is 841.89 points tall; the next page starts 16 points lower.
    draw(&mut h, Stylus::Tip, &line((100.0, 850.0), (200.0, 900.0)));
    assert_eq!(marks(&h), 0);
}

#[test]
fn the_eraser_end_erases_and_undo_brings_the_stroke_back() {
    let dir = tempfile::tempdir().unwrap();
    let mut h = Harness::new(dir.path());
    h.keys(":o notes.pdt<ret>");
    draw(&mut h, Stylus::Tip, &line((100.0, 100.0), (300.0, 100.0)));
    draw(&mut h, Stylus::Eraser, &line((200.0, 60.0), (200.0, 140.0)));
    assert_eq!(marks(&h), 0);
    h.keys("u");
    assert_eq!(marks(&h), 1);
    h.keys("U");
    assert_eq!(marks(&h), 0);
    h.keys("uu");
    assert_eq!(marks(&h), 0);
    assert!(!modified(&h), "undone back to the file as read");
}

#[test]
fn the_tip_erases_once_switched_to_the_eraser() {
    let dir = tempfile::tempdir().unwrap();
    let mut h = Harness::new(dir.path());
    h.keys(":o notes.pdt<ret>");
    draw(&mut h, Stylus::Tip, &line((100.0, 100.0), (300.0, 100.0)));
    h.keys("e");
    draw(&mut h, Stylus::Tip, &line((200.0, 60.0), (200.0, 140.0)));
    assert_eq!(marks(&h), 0);
}

#[test]
fn a_hidden_layer_refuses_strokes_and_a_new_layer_takes_them() {
    let dir = tempfile::tempdir().unwrap();
    let mut h = Harness::new(dir.path());
    h.keys(":o notes.pdt<ret><space>lh");
    draw(&mut h, Stylus::Tip, &line((100.0, 100.0), (300.0, 100.0)));
    assert!(matches!(h.state.notice, Some(Notice::LayerHidden)));
    h.keys("<space>ln");
    draw(&mut h, Stylus::Tip, &line((100.0, 100.0), (300.0, 100.0)));
    match h.state.layout.focused().active() {
        Some(Tab::Ink(InkTabs::Paged(tab))) => {
            assert_eq!(tab.hand.layer, LayerIndex(1));
            let drawing = h.state.pageds[tab.file()].edited.drawing();
            let second = PageLayer {
                page: PageIndex(0),
                layer: LayerIndex(1),
            };
            assert_eq!(drawing.marks(second).unwrap().len(), 1);
            assert_eq!(drawing.marks(FIRST).unwrap().len(), 0);
        }
        Some(_) | None => panic!("the drawing stays focused"),
    }
}

#[test]
fn a_board_takes_strokes_anywhere() {
    let dir = tempfile::tempdir().unwrap();
    let mut h = Harness::new(dir.path());
    h.keys(":o plan.bdt<ret>");
    draw(
        &mut h,
        Stylus::Tip,
        &line((-5000.0, -300.0), (9000.0, 4000.0)),
    );
    assert_eq!(marks(&h), 1);
    h.keys(":w<ret>");
    let saved = ink::format::read_board(&fs::read(dir.path().join("plan.bdt")).unwrap()).unwrap();
    assert_eq!(saved.marks(LayerIndex(0)).unwrap().len(), 1);
}

#[test]
fn an_unsaved_drawing_holds_quit_until_written() {
    let dir = tempfile::tempdir().unwrap();
    let mut h = Harness::new(dir.path());
    h.keys(":o notes.pdt<ret>");
    draw(&mut h, Stylus::Tip, &line((100.0, 100.0), (300.0, 100.0)));
    h.keys(":q<ret>");
    assert!(!h.quit);
    assert!(matches!(
        h.state.notice,
        Some(Notice::UnsavedBuffers { count: 1 })
    ));
    h.keys(":wq<ret>");
    assert!(h.quit);
    assert!(dir.path().join("notes.pdt").is_file());
}

#[test]
fn a_drawing_cannot_be_saved_under_a_text_name() {
    let dir = tempfile::tempdir().unwrap();
    let mut h = Harness::new(dir.path());
    h.keys(":o notes.pdt<ret>:w notes.md<ret>");
    assert!(matches!(
        h.state.notice,
        Some(Notice::WrongExtension { .. })
    ));
    assert!(!dir.path().join("notes.md").exists());
}

/// The focused paged tab's view.
fn view(h: &Harness) -> model::sheet::SheetView {
    match h.state.layout.focused().active() {
        Some(Tab::Ink(InkTabs::Paged(tab))) => tab.view,
        Some(_) | None => panic!("a paged drawing is focused"),
    }
}

fn pages(h: &Harness) -> usize {
    match h.state.layout.focused().active() {
        Some(Tab::Ink(InkTabs::Paged(tab))) => {
            (h.state.pageds[tab.file()].edited.drawing().stack().sizes()).len()
        }
        Some(_) | None => panic!("a paged drawing is focused"),
    }
}

#[test]
fn zoom_keys_scale_the_fit_and_keep_the_page_in_view() {
    let dir = tempfile::tempdir().unwrap();
    let mut h = Harness::new(dir.path());
    h.keys(":o notes.pdt<ret>");
    let before = view(&h);
    assert_eq!((before.fit, before.zoom), (Fit::Width, Zoom::ONE));
    h.keys("++");
    assert!((view(&h).zoom.get() - 1.5625).abs() < 1e-4);
    assert_eq!(view(&h).centre, before.centre, "zoom keeps the centre");
    h.keys("zf");
    assert_eq!(view(&h).fit, Fit::Height);
    h.keys("=");
    assert_eq!(view(&h).zoom, Zoom::ONE);
}

#[test]
fn pages_are_added_removed_and_jumped_between() {
    let dir = tempfile::tempdir().unwrap();
    let mut h = Harness::new(dir.path());
    h.keys(":o notes.pdt<ret>");
    draw(&mut h, Stylus::Tip, &line((100.0, 100.0), (300.0, 100.0)));
    h.keys("o");
    assert_eq!(pages(&h), 2);
    assert_eq!(view(&h).centre.page, Page(1), "the new page is shown");
    h.keys("O");
    assert_eq!(pages(&h), 3);
    assert_eq!(
        view(&h).centre.page,
        Page(1),
        "added above the page in focus"
    );
    h.keys("gg");
    assert_eq!(view(&h).centre.page, Page(0));
    assert_eq!(marks(&h), 1, "the first page keeps its stroke");
    h.keys("G");
    assert_eq!(view(&h).centre.page, Page(2));
    h.keys("[p");
    assert_eq!(view(&h).centre.page, Page(1));
    h.keys("dp");
    assert_eq!(pages(&h), 2);
    h.keys("ggdpdp");
    assert_eq!(pages(&h), 1);
    assert!(matches!(
        h.state.notice,
        Some(Notice::DrawingRefused { .. })
    ));
    h.keys("uu");
    assert_eq!(pages(&h), 3);
}

#[test]
fn a_split_stays_on_its_page_when_pages_are_added_before_it() {
    let dir = tempfile::tempdir().unwrap();
    let mut h = Harness::new(dir.path());
    h.keys(":o notes.pdt<ret>oo<C-w>v");
    h.keys("gg");
    let first = h.state.layout.leaves()[0];
    let shown = |h: &Harness, pane| match h.state.layout.panes()[pane].active() {
        Some(Tab::Ink(InkTabs::Paged(tab))) => tab.view.centre.page,
        Some(_) | None => panic!("both panes show the drawing"),
    };
    let before = shown(&h, first);
    h.keys("O");
    assert_eq!(shown(&h, first), Page(before.0 + 1));
}

#[test]
fn panning_stops_at_the_last_page() {
    let dir = tempfile::tempdir().unwrap();
    let mut h = Harness::new(dir.path());
    h.keys(":o notes.pdt<ret>o");
    let pane = h.state.layout.focus();
    h.run(|state, _| drawing::measured(state, pane, viewport()));
    for _ in 0..200 {
        h.keys("j");
    }
    let held = view(&h);
    h.keys("j");
    assert_eq!(view(&h), held, "nothing past the last page");
    assert_eq!(held.centre.page, Page(1));
}

#[test]
fn a_session_restores_a_drawing_on_its_layer_and_page() {
    let dir = tempfile::tempdir().unwrap();
    let mut before = Harness::new(dir.path());
    before.keys(":o notes.pdt<ret><space>lnoo+zf:w<ret>");
    let kept = view(&before);
    let text = toml::to_string(&session::snapshot(&before.state)).unwrap();

    let mut after = Harness::new(dir.path());
    let saved: session::Session = toml::from_str(&text).unwrap();
    after
        .run(|state, out| session::restore(state, saved, out))
        .unwrap();
    assert_eq!(after.focused_tabs(), ["✎notes.pdt"]);
    match after.state.layout.focused().active() {
        Some(Tab::Ink(InkTabs::Paged(tab))) => {
            assert_eq!(tab.hand.layer, LayerIndex(1));
            assert_eq!(tab.view, kept);
        }
        Some(_) | None => panic!("the drawing is restored"),
    }
}

#[test]
fn a_split_shows_the_same_drawing() {
    let dir = tempfile::tempdir().unwrap();
    let mut h = Harness::new(dir.path());
    h.keys(":o notes.pdt<ret><C-w>v");
    draw(&mut h, Stylus::Tip, &line((100.0, 100.0), (300.0, 100.0)));
    let first = h.state.layout.leaves()[0];
    assert_eq!(h.tabs(first), ["✎notes.pdt"]);
    h.keys("<C-w>w");
    assert_eq!(marks(&h), 1, "both panes show one drawing");
}

/// The bounds of the focused board's marks on its first layer, bottom first.
fn board_bounds(h: &Harness) -> Vec<ink::geom::Rect<ink::geom::Surface>> {
    match h.state.layout.focused().active() {
        Some(Tab::Ink(InkTabs::Board(tab))) => {
            let drawing = h.state.boards[tab.file()].edited.drawing();
            (drawing.marks(LayerIndex(0)).unwrap().iter())
                .map(|(_, mark)| mark.bounds())
                .collect()
        }
        Some(_) | None => panic!("a board is focused"),
    }
}

#[test]
fn a_selection_moves_as_one_undo_step_and_deletes() {
    let dir = tempfile::tempdir().unwrap();
    let mut h = Harness::new(dir.path());
    h.keys(":o plan.bdt<ret>");
    draw(&mut h, Stylus::Tip, &line((0.0, 0.0), (100.0, 0.0)));
    draw(&mut h, Stylus::Tip, &line((0.0, 50.0), (100.0, 50.0)));
    let before = board_bounds(&h);
    h.keys("S");
    let lasso = [
        (-20.0, -20.0),
        (120.0, -20.0),
        (120.0, 70.0),
        (-20.0, 70.0),
        (-20.0, -20.0),
    ];
    let around: Vec<_> = lasso
        .windows(2)
        .flat_map(|pair| line(pair[0], pair[1]))
        .collect();
    draw(&mut h, Stylus::Tip, &around);
    draw(&mut h, Stylus::Tip, &line((50.0, 25.0), (50.0, 125.0)));
    let after = board_bounds(&h);
    for (before, after) in before.iter().zip(&after) {
        assert!(
            (after.min().y - before.min().y - 100.0).abs() < 1e-2,
            "{after:?}"
        );
    }
    h.keys("u");
    assert_eq!(board_bounds(&h), before, "the move undoes as one step");
    h.keys("dd");
    assert!(matches!(h.state.notice, Some(Notice::NothingPicked)));
    draw(&mut h, Stylus::Tip, &around);
    h.keys("dd");
    assert_eq!(marks(&h), 0);
    h.keys("u");
    assert_eq!(marks(&h), 2);
}

#[test]
fn a_rectangle_is_dragged_out_and_saved() {
    let dir = tempfile::tempdir().unwrap();
    let mut h = Harness::new(dir.path());
    h.keys(":o notes.pdt<ret>tr");
    draw(&mut h, Stylus::Tip, &line((100.0, 100.0), (300.0, 200.0)));
    assert_eq!(marks(&h), 1);
    h.keys(":w<ret>");
    let saved = ink::format::read_paged(&fs::read(dir.path().join("notes.pdt")).unwrap()).unwrap();
    let (_, mark) = saved.marks(FIRST).unwrap().iter().next().unwrap();
    assert!(matches!(
        &mark.ink,
        ink::doc::Ink::Shape(shape) if matches!(shape.shape, ink::doc::Shape::Rectangle(_))
    ));
}

/// Writes a paged document of `pages` blank pages over `path`, as another program would.
fn write_pages(path: &std::path::Path, pages: usize) {
    let mut paged = ink::doc::Paged::new(ink::doc::PageSize::A4, drawing::PAPER);
    for at in 1..pages {
        let insert = ink::doc::PagedChange::InsertPage {
            at: Page(at),
            size: ink::doc::PageSize::A4,
            background: drawing::PAPER,
        };
        paged.apply(insert).unwrap();
    }
    let mut bytes = Vec::new();
    ink::format::write_paged(&paged, &mut bytes);
    // A later mtime than the one read, so the reload sees a change.
    std::thread::sleep(Duration::from_millis(20));
    fs::write(path, bytes).unwrap();
}

#[test]
fn a_saved_drawing_changed_on_disk_reloads_as_one_undo_step() {
    let dir = tempfile::tempdir().unwrap();
    let mut h = Harness::new(dir.path());
    h.keys(":o notes.pdt<ret>");
    draw(&mut h, Stylus::Tip, &line((100.0, 100.0), (300.0, 100.0)));
    h.keys(":w<ret>");
    let path = dir.path().canonicalize().unwrap().join("notes.pdt");
    write_pages(&path, 2);
    h.changed(vec![path]);
    assert_eq!(pages(&h), 2);
    assert_eq!(marks(&h), 0);
    assert!(!modified(&h), "the file as read is the saved one");
    h.keys("u");
    assert_eq!(
        (pages(&h), marks(&h)),
        (1, 1),
        "undo goes back before the reload"
    );
}

#[test]
fn an_unsaved_drawing_changed_on_disk_is_flagged_and_kept() {
    let dir = tempfile::tempdir().unwrap();
    let mut h = Harness::new(dir.path());
    h.keys(":o notes.pdt<ret>:w<ret>");
    draw(&mut h, Stylus::Tip, &line((100.0, 100.0), (300.0, 100.0)));
    let path = dir.path().canonicalize().unwrap().join("notes.pdt");
    write_pages(&path, 3);
    h.changed(vec![path]);
    assert!(matches!(h.state.notice, Some(Notice::ChangedOnDisk { .. })));
    assert_eq!((pages(&h), marks(&h)), (1, 1));
    h.keys(":reload<ret>");
    assert_eq!(pages(&h), 3, ":reload takes the disk's drawing");
}

#[test]
fn a_box_selects_what_it_holds_and_moves_it() {
    let dir = tempfile::tempdir().unwrap();
    let mut h = Harness::new(dir.path());
    h.keys(":o plan.bdt<ret>");
    draw(&mut h, Stylus::Tip, &line((0.0, 0.0), (100.0, 0.0)));
    draw(&mut h, Stylus::Tip, &line((0.0, 200.0), (100.0, 200.0)));
    let before = board_bounds(&h);
    h.keys("s");
    draw(&mut h, Stylus::Tip, &line((-20.0, -20.0), (120.0, 20.0)));
    draw(&mut h, Stylus::Tip, &line((50.0, 0.0), (50.0, 40.0)));
    let after = board_bounds(&h);
    assert!((after[0].min().y - before[0].min().y - 40.0).abs() < 1e-2);
    assert_eq!(after[1], before[1], "the stroke outside the box stays");
}
