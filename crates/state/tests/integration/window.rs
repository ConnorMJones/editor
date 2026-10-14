use std::fs;

use model::command::{Command, PickerCommand};
use model::notice::Notice;
use model::window::{Axis, PickerKind};
use state::layout::{Flex, Node};
use state::session::{self, Session, SessionError};
use state::workspace::{self, Close};
use state::{Focus, Overlay, Sidebar, SidebarWidth};

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
    h.run(|state, out| workspace::picker(state, PickerCommand::Open(PickerKind::Files), out));
    assert_eq!(h.state.sidebar, Sidebar::Files(Focus::Sidebar));

    assert_eq!(workspace::request_close(&mut h.state), Close::Refused);
    assert!(matches!(
        h.state.notice,
        Some(Notice::UnsavedBuffers { count: 1 })
    ));
    assert_eq!(h.state.sidebar, Sidebar::Files(Focus::Editor));
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
    assert_eq!(workspace::request_close(&mut h.state), Close::Allowed);
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
    assert_eq!(after.state.layout.root, before.state.layout.root);
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
fn sessions_without_weights_or_width_still_load() {
    let dir = tempfile::tempdir().unwrap();
    for name in ["a.txt", "b.txt"] {
        std::fs::write(dir.path().join(name), name).unwrap();
    }
    let mut h = Harness::new(dir.path());
    let root = dir.path().canonicalize().unwrap();
    let pane = |name: &str| {
        format!(
            "{{ kind = \"tabs\", active = 0, tabs = [{{ kind = \"text\", path = {:?}, \
             ranges = [[0, 0]], primary = 0, scroll = 0 }}] }}",
            root.join(name)
        )
    };
    let saved: Session = toml::from_str(&format!(
        "focus = 0\nsidebar = \"shown\"\nexpanded = []\n\
         panes = [{}, {}]\n\
         [layout]\nkind = \"split\"\naxis = \"row\"\n\
         children = [{{ kind = \"pane\", pane = 0 }}, {{ kind = \"pane\", pane = 1 }}]\n",
        pane("a.txt"),
        pane("b.txt")
    ))
    .expect("older session parses");
    h.run(|state, out| session::restore(state, saved, out))
        .expect("valid session");
    assert_eq!(h.state.sidebar_width, SidebarWidth::DEFAULT);
    match &h.state.layout.root {
        Node::Split { axis, children } => {
            assert_eq!(*axis, Axis::Row);
            assert!(children.iter().all(|child| child.flex == Flex::EVEN));
        }
        Node::Pane(_) => panic!("split was not restored"),
    }
}

#[test]
fn non_positive_weights_are_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let mut h = Harness::new(dir.path());
    let saved: Session = toml::from_str(
        "focus = 0\nsidebar = \"hidden\"\nexpanded = []\n\
         panes = [{ kind = \"empty\" }, { kind = \"empty\" }]\n\
         [layout]\nkind = \"split\"\naxis = \"column\"\n\
         children = [{ kind = \"pane\", pane = 0, flex = 0.0 }, { kind = \"pane\", pane = 1 }]\n",
    )
    .expect("well-formed toml");
    assert!(matches!(
        h.run(|state, out| session::restore(state, saved, out)),
        Err(SessionError::InvalidFlex(_))
    ));
}

#[test]
fn sidebar_width_is_clamped_and_persisted() {
    assert_eq!(SidebarWidth::new(1.0).get(), SidebarWidth::MIN);
    assert_eq!(SidebarWidth::new(1e6).get(), SidebarWidth::MAX);
    assert_eq!(SidebarWidth::new(f32::NAN), SidebarWidth::DEFAULT);

    let dir = tempfile::tempdir().unwrap();
    let mut before = Harness::new(dir.path());
    before.keys("<space>e");
    before.state.sidebar_width = SidebarWidth::new(333.0);
    let saved = session::snapshot(&before.state);
    assert_eq!(saved.sidebar_width, 333.0);

    let mut after = Harness::new(dir.path());
    after
        .run(|state, out| session::restore(state, round_trip(&saved), out))
        .expect("valid session");
    assert_eq!(after.state.sidebar_width.get(), 333.0);
}
