use std::fs;
use std::path::Path;

use model::git::GitStatus;
use model::notice::Notice;
use state::input::picker::Items;
use state::project::files::Files;
use state::project::path::RelPath;
use state::sidebar::tree::{RowKind, TreeAction, TreeTarget};
use state::text::buffer::Disk;
use state::view::layout::Node;
use state::view::panes;
use state::{Focus, Overlay, Repo, Sidebar, State};

use crate::harness::{Harness, disk_of};

fn commit_all(dir: &Path) {
    let repo = git2::Repository::init(dir).unwrap();
    let mut index = repo.index().unwrap();
    index
        .add_all(["*"].iter(), git2::IndexAddOption::DEFAULT, None)
        .unwrap();
    index.write().unwrap();
    let tree = repo.find_tree(index.write_tree().unwrap()).unwrap();
    let sig = git2::Signature::now("t", "t@example.com").unwrap();
    repo.commit(Some("HEAD"), &sig, &sig, "init", &tree, &[])
        .unwrap();
}

fn status(state: &State, path: &str) -> Option<GitStatus> {
    match &state.git.repo {
        Repo::Found { snapshot, .. } => snapshot.get(&RelPath::parse(path).unwrap()),
        Repo::None | Repo::Unknown => panic!("no repository"),
    }
}

fn canonical(dir: &Path, rel: &str) -> std::path::PathBuf {
    dir.canonicalize().unwrap().join(rel)
}

#[test]
fn file_picker_opens_selected_file() {
    let dir = tempfile::tempdir().unwrap();
    fs::create_dir(dir.path().join("src")).unwrap();
    fs::write(dir.path().join("a.txt"), "a").unwrap();
    fs::write(dir.path().join("src/lib.rs"), "fn x() {}").unwrap();
    let mut h = Harness::new(dir.path());
    assert!(matches!(&h.state.files, Files::Ready(files) if files.len() == 2));

    h.keys("<space>flib");
    match &h.state.overlay {
        Overlay::Picker(picker) => assert_eq!(picker.matches.len(), 1),
        Overlay::None | Overlay::Prompt(_) => panic!("picker not open"),
    }
    h.keys("<ret>");
    assert!(matches!(h.state.overlay, Overlay::None));
    assert_eq!(h.focused_path(), Some(canonical(dir.path(), "src/lib.rs")));
}

#[test]
fn buffer_picker_and_cycling() {
    let dir = tempfile::tempdir().unwrap();
    for name in ["a.txt", "b.txt", "c.txt"] {
        fs::write(dir.path().join(name), name).unwrap();
    }
    let mut h = Harness::new(dir.path());
    h.keys(":o a.txt<ret>:o b.txt<ret>:o c.txt<ret>");
    assert_eq!(h.focused_tabs(), ["a.txt", "b.txt", "c.txt"]);

    h.keys("gn");
    assert_eq!(h.focused_path(), Some(canonical(dir.path(), "a.txt")));
    h.keys("gp");
    assert_eq!(h.focused_path(), Some(canonical(dir.path(), "c.txt")));
    h.keys(":bp<ret>");
    assert_eq!(h.focused_path(), Some(canonical(dir.path(), "b.txt")));

    h.keys("<space>ba.txt<ret>");
    assert_eq!(h.focused_path(), Some(canonical(dir.path(), "a.txt")));
}

#[test]
fn split_focus_and_close_panes() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("a.txt"), "a").unwrap();
    let mut h = Harness::new(dir.path());
    h.keys(":o a.txt<ret>");
    let first = h.state.layout.focus;

    h.keys("<C-w>v");
    assert_eq!(h.state.layout.panes.len(), 2);
    let second = h.state.layout.focus;
    assert_ne!(first, second);
    assert_eq!(h.focused_path(), Some(canonical(dir.path(), "a.txt")));
    assert_eq!(
        h.focused_tabs(),
        ["a.txt*"],
        "the copy is a preview the next open replaces"
    );

    h.keys("<C-w>h");
    assert_eq!(h.state.layout.focus, first);
    h.keys("<C-w>l");
    assert_eq!(h.state.layout.focus, second);

    h.keys("<space>ws");
    assert_eq!(h.state.layout.panes.len(), 3);
    assert!(matches!(&h.state.layout.root, Node::Split { children, .. } if children.len() == 2));
    h.keys("<C-w>k");
    assert_eq!(h.state.layout.focus, second);

    h.keys("<C-w>q");
    assert_eq!(h.state.layout.panes.len(), 2);
    h.keys("<C-w>o");
    assert_eq!(h.state.layout.panes.len(), 1);
    assert!(matches!(h.state.layout.root, Node::Pane(_)));

    h.keys("<C-w>q");
    assert!(h.quit);
}

#[test]
fn closing_buffer_keeps_panes_valid() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("a.txt"), "a").unwrap();
    fs::write(dir.path().join("b.txt"), "b").unwrap();
    let mut h = Harness::new(dir.path());
    h.keys(":o a.txt<ret><C-w>v:o b.txt<ret>");
    let (first, second) = (h.state.layout.leaves()[0], h.state.layout.leaves()[1]);
    h.keys(":bc<ret>");
    assert_eq!(h.state.buffers.len(), 1);
    for pane in h.state.layout.panes.values() {
        match pane.active() {
            Some(view) => assert!(h.state.buffers.contains_key(view.buffer())),
            None => panic!("pane lost its buffer"),
        }
    }
    assert!(
        !h.state.layout.panes.contains_key(second),
        "the emptied pane closes"
    );
    assert_eq!(h.tabs(first), ["a.txt"]);
    // The last pane stays, empty, once its last buffer closes.
    h.keys(":bc<ret>");
    assert!(h.state.layout.panes[first].tabs.is_none());
    assert!(h.state.buffers.is_empty());
}

#[test]
fn tree_expands_creates_renames_and_deletes() {
    let dir = tempfile::tempdir().unwrap();
    fs::create_dir(dir.path().join("src")).unwrap();
    fs::write(dir.path().join("src/lib.rs"), "").unwrap();
    fs::write(dir.path().join("z.txt"), "").unwrap();
    let mut h = Harness::new(dir.path());

    h.keys("<space>e");
    assert_eq!(h.state.sidebar, Sidebar::Files(Focus::Sidebar));
    let names: Vec<&str> = h.state.tree.rows.iter().map(|r| r.path.as_str()).collect();
    assert_eq!(names, ["src", "z.txt"]);

    h.keys("l");
    let names: Vec<&str> = h.state.tree.rows.iter().map(|r| r.path.as_str()).collect();
    assert_eq!(names, ["src", "src/lib.rs", "z.txt"]);

    h.keys("j");
    h.keys("anew.rs<ret>");
    assert!(dir.path().join("src/new.rs").is_file());
    assert_eq!(h.focused_path(), Some(canonical(dir.path(), "src/new.rs")));
    assert_eq!(h.state.sidebar, Sidebar::Files(Focus::Editor));
    assert!(
        h.state
            .tree
            .rows
            .iter()
            .any(|r| r.path.as_str() == "src/new.rs")
    );
    assert!(
        matches!(&h.state.files, Files::Ready(f) if f.iter().any(|p| p.as_str() == "src/new.rs"))
    );

    h.keys("<space>e");
    let index = h
        .state
        .tree
        .rows
        .iter()
        .position(|r| r.path.as_str() == "src/new.rs")
        .unwrap();
    h.state.tree.cursor = index;
    h.keys("r");
    h.keys("<bs><bs><bs><bs><bs><bs>renamed.rs<ret>");
    assert!(dir.path().join("src/renamed.rs").is_file());
    assert_eq!(
        h.focused_path(),
        Some(canonical(dir.path(), "src/renamed.rs"))
    );

    let index = h
        .state
        .tree
        .rows
        .iter()
        .position(|r| r.path.as_str() == "z.txt")
        .unwrap();
    h.state.tree.cursor = index;
    h.keys("dn<ret>");
    assert!(dir.path().join("z.txt").exists());
    h.keys("dy<ret>");
    assert!(!dir.path().join("z.txt").exists());
    assert!(h.state.tree.rows.iter().all(|r| r.path.as_str() != "z.txt"));

    h.keys("h");
    let src = h
        .state
        .tree
        .rows
        .iter()
        .position(|r| r.path.as_str() == "src")
        .unwrap();
    h.state.tree.cursor = src;
    h.keys("h");
    assert!(matches!(h.state.tree.rows[0].kind, RowKind::Dir(_)));
    assert_eq!(h.state.tree.rows.len(), 1);

    h.keys("<esc>");
    assert_eq!(h.state.sidebar, Sidebar::Files(Focus::Editor));
}

#[test]
fn save_all_updates_git_status() {
    let dir = tempfile::tempdir().unwrap();
    fs::create_dir(dir.path().join("src")).unwrap();
    fs::write(dir.path().join("src/a.txt"), "one\n").unwrap();
    fs::write(dir.path().join(".gitignore"), "target/\n").unwrap();
    commit_all(dir.path());
    fs::create_dir(dir.path().join("target")).unwrap();
    fs::write(dir.path().join("target/out"), "").unwrap();
    fs::write(dir.path().join("new.txt"), "").unwrap();
    let mut h = Harness::new(dir.path());

    assert_eq!(status(&h.state, "src/a.txt"), None);
    assert_eq!(status(&h.state, "new.txt"), Some(GitStatus::Untracked));
    assert_eq!(status(&h.state, "target"), Some(GitStatus::Ignored));
    assert_eq!(status(&h.state, "target/out"), Some(GitStatus::Ignored));
    assert!(
        matches!(&h.state.files, Files::Ready(f) if f.iter().all(|p| !p.as_str().starts_with("target")))
    );

    h.keys(":o src/a.txt<ret>ix<esc>");
    assert!(h.doc().is_modified());
    h.keys("<C-k><C-s>");
    assert!(!h.doc().is_modified());
    assert_eq!(status(&h.state, "src/a.txt"), Some(GitStatus::Modified));
    assert_eq!(status(&h.state, "src"), Some(GitStatus::Modified));
}

#[test]
fn external_changes_reload_clean_and_flag_dirty() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("clean.txt"), "old\n").unwrap();
    fs::write(dir.path().join("dirty.txt"), "old\n").unwrap();
    let mut h = Harness::new(dir.path());
    h.keys(":o dirty.txt<ret>iedit <esc>:o clean.txt<ret>");

    // Different length so the mtime check cannot hide the change on coarse clocks.
    fs::write(dir.path().join("clean.txt"), "new contents\n").unwrap();
    fs::write(dir.path().join("dirty.txt"), "new contents\n").unwrap();
    h.changed(vec![
        canonical(dir.path(), "clean.txt"),
        canonical(dir.path(), "dirty.txt"),
    ]);

    assert_eq!(h.doc().text.to_string(), "new contents\n");
    assert!(!h.doc().is_modified());
    let disk = |h: &Harness, name: &str| {
        let path = canonical(dir.path(), name);
        disk_of(h.state.buffers.values().find(|b| b.path == path).unwrap())
    };
    assert_eq!(disk(&h, "clean.txt"), Disk::Synced);
    assert_eq!(disk(&h, "dirty.txt"), Disk::ChangedExternally);

    h.keys("u");
    assert_eq!(h.doc().text.to_string(), "old\n");

    fs::remove_file(dir.path().join("clean.txt")).unwrap();
    h.changed(vec![canonical(dir.path(), "clean.txt")]);
    assert_eq!(disk(&h, "clean.txt"), Disk::Deleted);
}

#[test]
fn own_writes_do_not_flag_changes() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("a.txt"), "a").unwrap();
    let mut h = Harness::new(dir.path());
    h.keys(":o a.txt<ret>ix<esc>:w<ret>iy<esc>");
    h.changed(vec![canonical(dir.path(), "a.txt")]);
    let buffer = h.state.buffers.values().next().unwrap();
    assert_eq!(disk_of(buffer), Disk::Synced);
    assert_eq!(h.doc().text.to_string(), "xya");
}

#[test]
fn live_watcher_reloads_and_indexes() {
    let dir = tempfile::tempdir().unwrap();
    fs::create_dir(dir.path().join("src")).unwrap();
    fs::write(dir.path().join("src/a.txt"), "old\n").unwrap();
    let mut h = Harness::watched(dir.path());
    h.keys(":o src/a.txt<ret>");

    fs::write(dir.path().join("src/a.txt"), "changed on disk\n").unwrap();
    fs::write(dir.path().join("src/b.txt"), "").unwrap();
    let reloaded = h.wait_for_events(|state| {
        let indexed = matches!(&state.files, Files::Ready(f) if f.iter().any(|p| p.as_str() == "src/b.txt"));
        let text = state
            .buffers
            .values()
            .any(|b| matches!(&b.content, state::text::buffer::Content::Text { doc, .. } if doc.text == "changed on disk\n"));
        indexed && text
    });
    assert!(reloaded);
}

#[test]
fn grep_picker_jumps_to_hit() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("a.txt"), "one\ntwo needle\nthree\n").unwrap();
    fs::write(dir.path().join("b.txt"), "nothing here\n").unwrap();
    let mut h = Harness::new(dir.path());

    h.keys("<space>/needle");
    match &h.state.overlay {
        Overlay::Picker(picker) => match &picker.items {
            Items::Grep(grep) => {
                assert_eq!(grep.hits.len(), 1);
                assert_eq!(grep.hits[0].line, 1);
                assert_eq!(grep.hits[0].column, 4);
            }
            Items::Files(_) | Items::Buffers(_) | Items::Commands | Items::Backlinks(_) => {
                panic!("wrong picker")
            }
        },
        Overlay::None | Overlay::Prompt(_) => panic!("picker not open"),
    }
    h.keys("<ret>");
    let doc = h.doc();
    let cursor = h.view().selection.primary().cursor(doc.text.slice(..));
    assert_eq!(cursor, doc.text.line_to_char(1) + 4);
}

#[test]
fn command_palette_runs_command() {
    let dir = tempfile::tempdir().unwrap();
    let mut h = Harness::new(dir.path());
    h.keys("<space>?vsplit<ret>");
    assert_eq!(h.state.layout.panes.len(), 2);
}

#[test]
fn reading_watched_files_raises_no_events() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("a.txt"), "a").unwrap();
    let mut h = Harness::watched(dir.path());
    // Settle events from boot's own listing and git status.
    h.wait_for_events(|_| false);
    let _ = fs::read(dir.path().join("a.txt")).unwrap();
    let _ = fs::read_dir(dir.path()).unwrap().count();
    assert!(
        !h.events_arrive(),
        "opening files must not look like changes"
    );
}

fn row(h: &Harness, path: &str) -> usize {
    h.state
        .tree
        .rows
        .iter()
        .position(|r| r.path.as_str() == path)
        .unwrap_or_else(|| panic!("{path} not listed"))
}

#[test]
fn tree_menu_creates_files_and_folders_renames_and_deletes() {
    let dir = tempfile::tempdir().unwrap();
    fs::create_dir(dir.path().join("src")).unwrap();
    fs::write(dir.path().join("a.txt"), "").unwrap();
    let mut h = Harness::new(dir.path());
    h.keys("<space>e");

    // New Folder on the root never needs a trailing slash.
    h.run(|state, out| panes::tree_action(state, TreeTarget::Root, TreeAction::NewFolder, out));
    h.keys("docs<ret>");
    assert!(dir.path().join("docs").is_dir());

    // New File on a folder row creates inside it.
    let src = row(&h, "src");
    h.run(|state, out| panes::tree_action(state, TreeTarget::Row(src), TreeAction::NewFile, out));
    h.keys("main.rs<ret>");
    assert!(dir.path().join("src/main.rs").is_file());

    h.keys("<space>e");
    let a = row(&h, "a.txt");
    h.run(|state, out| panes::tree_action(state, TreeTarget::Row(a), TreeAction::Rename, out));
    h.keys("<bs><bs><bs><bs><bs>b.txt<ret>");
    assert!(dir.path().join("b.txt").is_file());

    let b = row(&h, "b.txt");
    h.run(|state, out| panes::tree_action(state, TreeTarget::Row(b), TreeAction::Delete, out));
    h.keys("y<ret>");
    assert!(!dir.path().join("b.txt").exists());
}

#[test]
fn dragging_moves_entries_between_folders() {
    let dir = tempfile::tempdir().unwrap();
    fs::create_dir_all(dir.path().join("src/inner")).unwrap();
    fs::create_dir(dir.path().join("docs")).unwrap();
    fs::write(dir.path().join("a.txt"), "a").unwrap();
    fs::write(dir.path().join("docs/b.md"), "b").unwrap();
    fs::write(dir.path().join("src/c.rs"), "c").unwrap();
    let mut h = Harness::new(dir.path());
    h.keys(":o a.txt<ret><space>e");

    // Onto a folder moves into it, and an open buffer follows the file.
    let docs = row(&h, "docs");
    h.run(|state, out| {
        panes::move_entry(
            state,
            RelPath::parse("a.txt").unwrap(),
            TreeTarget::Row(docs),
            out,
        )
    });
    assert!(dir.path().join("docs/a.txt").is_file());
    assert_eq!(h.focused_path(), Some(canonical(dir.path(), "docs/a.txt")));

    // Onto a file moves into that file's folder; onto the empty area moves to the root.
    let docs = row(&h, "docs");
    h.state.tree.cursor = docs;
    match h.state.tree.rows[docs].kind {
        RowKind::Dir(state::sidebar::tree::Fold::Collapsed) => h.keys("l"),
        RowKind::Dir(state::sidebar::tree::Fold::Expanded) | RowKind::File => {}
    }
    let b = row(&h, "docs/b.md");
    h.run(|state, out| {
        panes::move_entry(
            state,
            RelPath::parse("src/c.rs").unwrap(),
            TreeTarget::Row(b),
            out,
        )
    });
    assert!(dir.path().join("docs/c.rs").is_file());
    h.run(|state, out| {
        panes::move_entry(
            state,
            RelPath::parse("docs/b.md").unwrap(),
            TreeTarget::Root,
            out,
        )
    });
    assert!(dir.path().join("b.md").is_file());

    // A folder cannot move into itself or below itself.
    let src = row(&h, "src");
    h.state.tree.cursor = src;
    h.keys("l");
    let inner = row(&h, "src/inner");
    h.run(|state, out| {
        panes::move_entry(
            state,
            RelPath::parse("src").unwrap(),
            TreeTarget::Row(inner),
            out,
        )
    });
    assert!(matches!(h.state.notice, Some(Notice::MoveIntoItself)));
    assert!(dir.path().join("src/inner").is_dir());

    // An existing name at the destination is never overwritten.
    fs::write(dir.path().join("src/b.md"), "other").unwrap();
    let src = row(&h, "src");
    h.run(|state, out| {
        panes::move_entry(
            state,
            RelPath::parse("b.md").unwrap(),
            TreeTarget::Row(src),
            out,
        )
    });
    assert_eq!(
        fs::read_to_string(dir.path().join("src/b.md")).unwrap(),
        "other"
    );
    assert!(dir.path().join("b.md").is_file());
}
