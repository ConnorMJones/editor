use std::fs;

use state::project::session::{self, Session, SessionError, SessionPane};
use state::{Files, Sidebar};

use crate::harness::Harness;

fn round_trip(saved: &Session) -> Session {
    toml::from_str(&toml::to_string(saved).expect("serializes")).expect("parses")
}

#[test]
fn restores_tabs_splits_selections_and_tree() {
    let dir = tempfile::tempdir().unwrap();
    fs::create_dir(dir.path().join("src")).unwrap();
    fs::write(dir.path().join("a.txt"), "alpha\nbeta\ngamma\n").unwrap();
    fs::write(dir.path().join("src/b.rs"), "fn b() {}\n").unwrap();

    let mut before = Harness::new(dir.path());
    before.keys(":o src/b.rs<ret>:o a.txt<ret>jjl<C-w>v:o src/b.rs<ret>w");
    before.keys("<space>e");
    let root_row = before
        .state
        .tree
        .rows
        .iter()
        .position(|row| row.path.as_str() == "src")
        .expect("src listed");
    before.state.tree.cursor = root_row;
    before.keys("l<esc>");
    let saved = session::snapshot(&before.state);
    assert_eq!(saved.panes.len(), 2);
    assert_eq!(
        saved.expanded,
        [state::project::path::RelPath::parse("src").unwrap()]
    );

    let mut after = Harness::new(dir.path());
    after
        .run(|state, out| session::restore(state, round_trip(&saved), out))
        .expect("valid session");
    assert_eq!(session::snapshot(&after.state), saved);
    assert_eq!(after.state.sidebar, Sidebar::Files(Files::Editor));
    assert!(
        after
            .state
            .tree
            .rows
            .iter()
            .any(|row| row.path.as_str() == "src/b.rs"),
        "expanded directory is listed again"
    );
}

#[test]
fn deleted_files_close_their_panes() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("keep.txt"), "k").unwrap();
    fs::write(dir.path().join("gone.txt"), "g").unwrap();

    let mut before = Harness::new(dir.path());
    before.keys(":o keep.txt<ret><C-w>v:o gone.txt<ret>");
    assert_eq!(before.focused_tabs(), ["gone.txt"]);
    let saved = session::snapshot(&before.state);
    fs::remove_file(dir.path().join("gone.txt")).unwrap();

    let mut after = Harness::new(dir.path());
    after
        .run(|state, out| session::restore(state, saved, out))
        .expect("valid session");
    assert_eq!(after.state.buffers.len(), 1);
    assert_eq!(
        after.state.layout.leaves().len(),
        1,
        "the emptied pane closes"
    );
    assert_eq!(after.focused_tabs(), ["keep.txt"]);
}

#[test]
fn selections_are_clamped_to_shorter_files() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("a.txt"), "0123456789\n").unwrap();

    let mut before = Harness::new(dir.path());
    before.keys(":o a.txt<ret>gl");
    let saved = session::snapshot(&before.state);
    fs::write(dir.path().join("a.txt"), "01\n").unwrap();

    let mut after = Harness::new(dir.path());
    after
        .run(|state, out| session::restore(state, saved, out))
        .expect("valid session");
    assert!(after.view().selection.primary().head <= 3);
}

#[test]
fn a_primary_selection_that_does_not_exist_is_rejected() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("a.txt"), "a\n").unwrap();

    let mut before = Harness::new(dir.path());
    before.keys(":o a.txt<ret>");
    let mut saved = session::snapshot(&before.state);
    match &mut saved.panes[0] {
        SessionPane::Tabs { tabs, .. } => tabs[0].primary = 1,
        SessionPane::Empty => panic!("the pane has a tab"),
    }

    let mut after = Harness::new(dir.path());
    assert!(matches!(
        after.run(|state, out| session::restore(state, saved, out)),
        Err(SessionError::Selection { pane: 0, tab: 0 })
    ));
}
#[test]
fn invalid_layouts_are_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let mut h = Harness::new(dir.path());
    let parse = |text: &str| toml::from_str::<Session>(text).expect("well-formed toml");
    let header = "focus = 0\nsidebar = \"hidden\"\nexpanded = []\nsidebar_width = \"default\"\ncollapsed = []\n";

    let missing = parse(&format!(
        "{header}panes = [{{ kind = \"empty\" }}]\nlayout = {{ kind = \"pane\", pane = 3, flex = 1.0 }}\n"
    ));
    assert!(matches!(
        h.run(|state, out| session::restore(state, missing, out)),
        Err(SessionError::MissingPane(3))
    ));

    let unused = parse(&format!(
        "{header}panes = [{{ kind = \"empty\" }}, {{ kind = \"empty\" }}]\nlayout = {{ kind = \"pane\", pane = 0, flex = 1.0 }}\n"
    ));
    assert!(matches!(
        h.run(|state, out| session::restore(state, unused, out)),
        Err(SessionError::UnusedPane(1))
    ));
}

#[test]
fn load_distinguishes_missing_from_corrupt() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("s.toml");
    assert!(matches!(session::load(&path), Ok(None)));
    fs::write(&path, "tabs = 3").unwrap();
    assert!(matches!(session::load(&path), Err(SessionError::Parse(_))));
}
