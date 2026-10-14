use std::fs;
use std::path::Path;

use model::command::{Command, PickerCommand};
use model::notice::Notice;
use model::window::PickerKind;
use state::apply::Force;
use state::job::Job;
use state::path::RelPath;
use state::picker::{PREVIEW_SLOTS, Preview, Previews};
use state::tree::{self, Fold, RowKind};
use state::workspace::{self, Click};
use state::{Effect, Focus, Overlay, Sidebar};

use crate::harness::Harness;

fn canonical(dir: &Path, rel: &str) -> std::path::PathBuf {
    dir.canonicalize().unwrap().join(rel)
}

fn row(h: &Harness, path: &str) -> usize {
    h.state
        .tree
        .rows
        .iter()
        .position(|row| row.path.as_str() == path)
        .unwrap_or_else(|| panic!("{path} is not listed"))
}

#[test]
fn tree_clicks_toggle_directories_and_open_files() {
    let dir = tempfile::tempdir().unwrap();
    fs::create_dir(dir.path().join("src")).unwrap();
    fs::write(dir.path().join("src/lib.rs"), "fn x() {}\n").unwrap();
    let mut h = Harness::new(dir.path());
    h.keys("<space>e<esc>");
    assert_eq!(h.state.sidebar, Sidebar::Files(Focus::Editor));

    let src = row(&h, "src");
    h.run(|state, out| workspace::click_tree(state, src, Click::Single, out));
    assert_eq!(h.state.sidebar, Sidebar::Files(Focus::Sidebar));
    assert_eq!(h.state.tree.cursor, src);
    assert_eq!(h.state.tree.rows[src].kind, RowKind::Dir(Fold::Expanded));

    let lib = row(&h, "src/lib.rs");
    h.run(|state, out| workspace::click_tree(state, lib, Click::Single, out));
    assert_eq!(h.focused_path(), Some(canonical(dir.path(), "src/lib.rs")));
    assert_eq!(h.state.sidebar, Sidebar::Files(Focus::Editor));

    h.run(|state, out| workspace::click_tree(state, src, Click::Single, out));
    assert_eq!(h.state.tree.rows[src].kind, RowKind::Dir(Fold::Collapsed));
    // Rows past the end are ignored.
    let rows = h.state.tree.rows.len();
    h.run(|state, out| workspace::click_tree(state, rows, Click::Single, out));
    assert_eq!(h.state.tree.cursor, src);
}

#[test]
fn picker_click_opens_the_row() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("a.txt"), "a\n").unwrap();
    fs::write(dir.path().join("b.txt"), "b\n").unwrap();
    let mut h = Harness::new(dir.path());
    h.keys("<space>f");
    let target = match &h.state.overlay {
        Overlay::Picker(picker) => picker
            .matches
            .iter()
            .position(|&index| picker.items.label(index) == "b.txt")
            .expect("b.txt listed"),
        Overlay::None | Overlay::Prompt(_) => panic!("picker not open"),
    };
    h.run(|state, out| workspace::click_picker(state, target, out));
    assert!(matches!(h.state.overlay, Overlay::None));
    assert_eq!(h.focused_path(), Some(canonical(dir.path(), "b.txt")));
}

#[test]
fn tab_clicks_show_and_close_through_the_close_guard() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("a.txt"), "a\n").unwrap();
    fs::write(dir.path().join("b.txt"), "b\n").unwrap();
    let mut h = Harness::new(dir.path());
    h.keys(":o a.txt<ret>:o b.txt<ret><space>e");
    let pane = h.state.layout.focus;
    assert_eq!(h.focused_tabs(), ["a.txt", "b.txt"]);

    workspace::click_tab(&mut h.state, pane, 0, Click::Single);
    assert_eq!(h.focused_path(), Some(canonical(dir.path(), "a.txt")));
    assert_eq!(h.state.sidebar, Sidebar::Files(Focus::Editor));

    h.keys("ix<esc>");
    workspace::close_tab(&mut h.state, pane, 0, Force::No);
    assert!(matches!(
        h.state.notice,
        Some(Notice::UnsavedBuffers { count: 1 })
    ));
    assert_eq!(h.focused_tabs(), ["a.txt", "b.txt"]);

    // Closing a tab that is not active leaves the active one alone.
    workspace::close_tab(&mut h.state, pane, 1, Force::No);
    assert_eq!(h.focused_tabs(), ["a.txt"]);
    assert_eq!(h.state.buffers.len(), 1);
    assert_eq!(h.focused_path(), Some(canonical(dir.path(), "a.txt")));
}

#[test]
fn clicking_a_pane_focuses_it() {
    let dir = tempfile::tempdir().unwrap();
    let mut h = Harness::new(dir.path());
    h.keys("<C-w>v<space>e");
    let first = h.state.layout.leaves()[0];
    assert_ne!(h.state.layout.focus, first);
    workspace::focus_pane(&mut h.state, first);
    assert_eq!(h.state.layout.focus, first);
    assert_eq!(h.state.sidebar, Sidebar::Files(Focus::Editor));
}

#[test]
fn previews_load_in_jobs_and_are_cached() {
    let dir = tempfile::tempdir().unwrap();
    for i in 0..=PREVIEW_SLOTS {
        fs::write(dir.path().join(format!("f{i}.txt")), format!("file {i}\n")).unwrap();
    }
    fs::write(dir.path().join("bin.dat"), [1u8, 0, 2]).unwrap();
    let mut h = Harness::new(dir.path());
    h.keys(":o f0.txt<ret><space>f");
    match &mut h.state.overlay {
        Overlay::Picker(picker) => picker.previews = Previews::default(),
        Overlay::None | Overlay::Prompt(_) => panic!("picker closed"),
    }

    let preview = |h: &Harness, path: &str| match &h.state.overlay {
        Overlay::Picker(picker) => match picker.previews.get(&RelPath::parse(path).unwrap()) {
            None => "none",
            Some(Preview::Loading) => "loading",
            Some(Preview::Text(_)) => "text",
            Some(Preview::Binary) => "binary",
            Some(Preview::TooLarge) => "large",
            Some(Preview::Failed(_)) => "failed",
        },
        Overlay::None | Overlay::Prompt(_) => panic!("picker closed"),
    };
    let select = |h: &mut Harness, label: &str| {
        let Overlay::Picker(picker) = &mut h.state.overlay else {
            panic!("picker closed");
        };
        picker.cursor = picker
            .matches
            .iter()
            .position(|&index| picker.items.label(index) == label)
            .expect("listed");
        let mut out = Vec::new();
        workspace::request_preview(&mut h.state, &mut out);
        let queued = out
            .iter()
            .filter(|effect| matches!(effect, Effect::Job(Job::Preview { .. })))
            .count();
        h.run(|state, more| {
            for effect in out {
                match effect {
                    Effect::Job(work) => state::job::complete(state, state::job::run(work), more),
                    Effect::Quit
                    | Effect::Watch(_)
                    | Effect::WriteClipboard { .. }
                    | Effect::ReadClipboard { .. }
                    | Effect::OpenUrl(_)
                    | Effect::Typeset(_) => {}
                }
            }
        });
        queued
    };

    assert_eq!(
        select(&mut h, "f0.txt"),
        0,
        "open buffers preview from memory"
    );
    assert_eq!(select(&mut h, "bin.dat"), 1);
    assert_eq!(preview(&h, "bin.dat"), "binary");
    assert_eq!(select(&mut h, "f1.txt"), 1);
    assert_eq!(preview(&h, "f1.txt"), "text");
    assert_eq!(select(&mut h, "f1.txt"), 0, "cached");
    for i in 2..=PREVIEW_SLOTS {
        select(&mut h, &format!("f{i}.txt"));
    }
    assert_eq!(preview(&h, "bin.dat"), "none", "oldest entry evicted");

    h.run(|state, out| workspace::picker(state, PickerCommand::Cancel, out));
    h.run(|state, out| workspace::picker(state, PickerCommand::Open(PickerKind::Commands), out));
    assert!(matches!(h.state.overlay, Overlay::Picker(_)));
}

#[test]
fn unsaved_markers_cover_ancestors_and_clear_on_save() {
    let dir = tempfile::tempdir().unwrap();
    fs::create_dir_all(dir.path().join("a/b")).unwrap();
    fs::write(dir.path().join("a/b/c.txt"), "c\n").unwrap();
    let mut h = Harness::new(dir.path());
    h.keys(":o a/b/c.txt<ret>ix<esc>");
    let sync = |h: &mut Harness| {
        let state = &mut h.state;
        tree::sync_unsaved(&mut state.tree.unsaved, state.root.path(), &state.buffers);
    };
    sync(&mut h);
    let marked = |h: &Harness, path: &str| {
        h.state
            .tree
            .unsaved
            .paths
            .contains(&RelPath::parse(path).unwrap())
    };
    assert!(marked(&h, "a"));
    assert!(marked(&h, "a/b"));
    assert!(marked(&h, "a/b/c.txt"));
    assert!(!marked(&h, "a/b/d.txt"));

    h.keys(":w<ret>");
    sync(&mut h);
    assert!(h.state.tree.unsaved.paths.is_empty());
    h.run(|state, out| state::apply::apply(state, Command::Quit, out));
    assert!(h.quit);
}
