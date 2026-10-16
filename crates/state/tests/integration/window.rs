use std::fs;

use model::command::{Command, PickerCommand, PickerKind};
use model::notice::Notice;
use model::units::Px;
use state::project::session::{self, Session};
use state::view::panes::{self, Close};
use state::{Dragged, Files, Overlay, Sidebar, SidebarWidth};

use crate::harness::Harness;

fn round_trip(saved: &Session) -> Session {
    toml::from_str(&toml::to_string(saved).expect("serializes")).expect("parses")
}

#[test]
fn close_is_refused_with_unsaved_edits() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("a.txt"), "a\n").unwrap();
    let mut h = Harness::new(dir.path());
    h.keys(":o a.txt<ret>ix<esc><space>e");
    h.run(|state, out| panes::picker(state, PickerCommand::Open(PickerKind::Files), out));
    assert_eq!(h.state.sidebar, Sidebar::Files(Files::Tree));

    assert_eq!(panes::request_close(&mut h.state), Close::Refused);
    assert!(matches!(
        h.state.notice,
        Some(Notice::UnsavedBuffers { count: 1 })
    ));
    assert_eq!(h.state.sidebar, Sidebar::Files(Files::Editor));
    assert!(matches!(h.state.overlay, Overlay::None));

    // The title bar's close button and `:q` take the same path.
    h.run(|state, out| state::apply::apply(state, Command::Quit, out));
    assert!(!h.quit);
    h.keys(":q!<ret>");
    assert!(h.quit);
}

#[test]
fn close_is_allowed_once_saved() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("a.txt"), "a\n").unwrap();
    let mut h = Harness::new(dir.path());
    h.keys(":o a.txt<ret>ix<esc>:w<ret>");
    assert_eq!(panes::request_close(&mut h.state), Close::Allowed);
    assert!(h.state.notice.is_some(), "the write notice is left alone");
    h.keys(":q<ret>");
    assert!(h.quit);
}

#[test]
fn split_ratios_survive_a_session_round_trip() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("a.txt"), "a\n").unwrap();
    let mut before = Harness::new(dir.path());
    before.keys(":o a.txt<ret><C-w>v<C-w>s");
    before.state.layout.drag_divider(&[], 0, 0.7);
    before.state.layout.drag_divider(&[1], 0, 0.2);
    let saved = session::snapshot(&before.state);

    let mut after = Harness::new(dir.path());
    after
        .run(|state, out| session::restore(state, round_trip(&saved), out))
        .expect("valid session");
    assert_eq!(after.state.layout.root(), before.state.layout.root());
    let rects = |h: &Harness| -> Vec<_> {
        h.state
            .layout
            .rects()
            .into_iter()
            .map(|(_, rect)| rect)
            .collect()
    };
    assert_eq!(rects(&after), rects(&before));
    assert!((rects(&after)[0].w - 0.7).abs() < 1e-5);
}

#[test]
fn non_positive_weights_are_rejected() {
    let parsed = toml::from_str::<Session>(
        "focus = 0\nsidebar = \"hidden\"\nexpanded = []\nsidebar_width = \"default\"\ncollapsed = []\n\
         panes = [{ kind = \"empty\" }, { kind = \"empty\" }]\n\
         [layout]\nkind = \"split\"\naxis = \"column\"\nflex = 1.0\n\
         children = [{ kind = \"pane\", pane = 0, flex = 0.0 }, { kind = \"pane\", pane = 1, flex = 1.0 }]\n",
    );
    assert!(parsed.is_err());
}

#[test]
fn dragged_sidebar_widths_persist_and_bad_ones_are_rejected() {
    assert!(Dragged::new(Px(0.0)).is_err());
    assert!(Dragged::new(Px(f32::NAN)).is_err());

    let dir = tempfile::tempdir().unwrap();
    let mut before = Harness::new(dir.path());
    before.keys("<space>e");
    let width = SidebarWidth::Dragged(Dragged::new(Px(333.0)).unwrap());
    before.state.sidebar_width = width;
    let saved = session::snapshot(&before.state);

    let mut after = Harness::new(dir.path());
    after
        .run(|state, out| session::restore(state, round_trip(&saved), out))
        .expect("valid session");
    assert_eq!(after.state.sidebar_width, width);

    let text = toml::to_string(&saved)
        .unwrap()
        .replace("dragged = 333.0", "dragged = -1.0");
    assert!(toml::from_str::<Session>(&text).is_err());
}
