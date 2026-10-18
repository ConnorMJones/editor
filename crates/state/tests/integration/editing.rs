use std::fs;

use model::notice::Notice;
use state::text::buffer::Disk;

use crate::harness::Harness;

#[test]
fn edit_and_save_all_writes_disk() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("a.txt"), "world\n").unwrap();
    let mut h = Harness::new(dir.path());

    h.keys(":o a.txt<ret>");
    assert!(!h.doc().is_modified());

    h.keys("ihello <esc>");
    assert_eq!(h.doc().text.to_string(), "hello world\n");
    assert!(h.doc().is_modified());

    h.keys("<C-k><C-s>");
    assert!(!h.doc().is_modified());
    assert_eq!(
        fs::read_to_string(dir.path().join("a.txt")).unwrap(),
        "hello world\n"
    );
}

#[test]
fn save_all_in_insert_mode_keeps_editing() {
    let dir = tempfile::tempdir().unwrap();
    let mut h = Harness::new(dir.path());

    h.keys(":o new.txt<ret>");
    assert_eq!(h.state.buffers.values().next().unwrap().disk, Disk::New);

    h.keys("iab<C-k><C-s>c<esc>");
    assert_eq!(
        fs::read_to_string(dir.path().join("new.txt")).unwrap(),
        "ab"
    );
    assert_eq!(h.doc().text.to_string(), "abc");
    assert!(h.doc().is_modified());
    assert!(matches!(
        h.state.buffers.values().next().unwrap().disk,
        Disk::Synced { .. }
    ));
}

#[test]
fn undo_redo_tracks_modified() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("a.txt"), "x").unwrap();
    let mut h = Harness::new(dir.path());

    h.keys(":o a.txt<ret>iab<esc>");
    assert_eq!(h.doc().text.to_string(), "abx");
    h.keys("u");
    assert_eq!(h.doc().text.to_string(), "x");
    assert!(!h.doc().is_modified());
    h.keys("U");
    assert_eq!(h.doc().text.to_string(), "abx");
    assert!(h.doc().is_modified());
}

#[test]
fn quit_refuses_unsaved_and_write_quit_exits_after_write() {
    let dir = tempfile::tempdir().unwrap();
    let mut h = Harness::new(dir.path());

    h.keys(":o b.txt<ret>iz<esc>:q<ret>");
    assert!(!h.quit);
    assert!(matches!(
        h.state.notice,
        Some(Notice::UnsavedBuffers { count: 1 })
    ));

    h.keys(":wq<ret>");
    assert!(h.quit);
    assert_eq!(fs::read_to_string(dir.path().join("b.txt")).unwrap(), "z");
}

#[test]
fn backspace_and_newline() {
    let dir = tempfile::tempdir().unwrap();
    let mut h = Harness::new(dir.path());

    h.keys(":o c.txt<ret>iabc<bs><ret>d<esc>");
    assert_eq!(h.doc().text.to_string(), "ab\nd");
}

#[test]
fn reopening_reuses_buffer() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("a.txt"), "").unwrap();
    let mut h = Harness::new(dir.path());

    h.keys(":o a.txt<ret>:o ./a.txt<ret>");
    assert_eq!(h.state.buffers.len(), 1);
}
