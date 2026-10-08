use std::fs;

use crate::harness::Harness;

#[test]
fn yank_and_paste_through_system_clipboard() {
    let mut h = Harness::with("a.txt", "#[ab|]# #(cd|)#\n");
    h.keys("<space>y");
    assert_eq!(h.clipboard[0].as_deref(), Some("ab\ncd"));
    h.keys("<space>p");
    assert_eq!(h.plain(), "ab#[ab|]# cd#(cd|)#\n");
}

#[test]
fn foreign_clipboard_pastes_whole() {
    let mut h = Harness::with("a.txt", "#[a|]#\n");
    h.clipboard[0] = Some("xyz".into());
    h.keys("<space>P");
    assert_eq!(h.plain(), "#[xyz|]#a\n");
    h.keys("<space>R");
    assert_eq!(h.plain(), "#[xyz|]#a\n");
    h.keys("uu");
    assert_eq!(h.plain(), "#[a|]#\n");
}

#[test]
fn register_selection_targets_clipboard() {
    let mut h = Harness::with("a.txt", "#[ab|]#\n");
    h.keys("\"*y");
    assert_eq!(h.clipboard[1].as_deref(), Some("ab"));
    assert_eq!(h.clipboard[0], None);
    h.keys("\"*p");
    assert_eq!(h.plain(), "ab#[ab|]#\n");
}

#[test]
fn write_as_and_reload() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("a.txt"), "one\n").unwrap();
    let mut h = Harness::new(dir.path());
    h.keys(":o a.txt<ret>ix<esc>:w! sub/b.txt<ret>");
    assert_eq!(
        fs::read_to_string(dir.path().join("sub/b.txt")).unwrap(),
        "xone\n"
    );
    assert!(!h.doc().is_modified());

    fs::write(dir.path().join("sub/b.txt"), "changed\n").unwrap();
    h.keys(":reload<ret>");
    assert_eq!(h.doc().text.to_string(), "changed\n");
    assert!(!h.doc().is_modified());
    h.keys("u");
    assert_eq!(h.doc().text.to_string(), "xone\n");
}

#[test]
fn write_all_quit_waits_for_every_write() {
    let dir = tempfile::tempdir().unwrap();
    let mut h = Harness::new(dir.path());
    h.keys(":o a.txt<ret>ia<esc>:o b.txt<ret>ib<esc>:wqa<ret>");
    assert!(h.quit);
    assert_eq!(fs::read_to_string(dir.path().join("a.txt")).unwrap(), "a");
    assert_eq!(fs::read_to_string(dir.path().join("b.txt")).unwrap(), "b");
}
