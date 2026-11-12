use std::fs;

use model::notice::Notice;
use panes::{Maximised, Node};

use crate::harness::Harness;

fn files(names: &[&str]) -> (tempfile::TempDir, Harness) {
    let dir = tempfile::tempdir().unwrap();
    for name in names {
        fs::write(dir.path().join(name), format!("{name}\n")).unwrap();
    }
    let h = Harness::new(dir.path());
    (dir, h)
}

fn name(h: &Harness) -> String {
    let path = h.focused_path().expect("a file is focused");
    path.file_name().unwrap().to_string_lossy().into_owned()
}

fn bar(h: &Harness) -> Vec<String> {
    h.focused_tabs()
}

#[test]
fn panes_swap_transpose_maximise_and_resize_by_key() {
    let (_dir, mut h) = files(&["a.txt", "b.txt", "c.txt"]);
    h.keys(":o a.txt<ret><C-w>v:o b.txt<ret>");
    let leaves = h.state.layout.leaves();
    h.keys("<C-w>H");
    assert_eq!(h.state.layout.leaves(), [leaves[1], leaves[0]]);
    assert_eq!(name(&h), "b.txt", "the focus moves with the pane");
    h.keys("<C-w>t");
    assert!(
        matches!(h.state.layout.root(), Node::Split(split) if split.axis() == model::window::Axis::Column)
    );
    h.keys("<C-w>z");
    assert_eq!(
        h.state.layout.maximised(),
        Maximised::Pane(h.state.layout.focus())
    );
    h.keys("<C-w>j");
    assert_eq!(
        h.state.layout.maximised(),
        Maximised::No,
        "moving the focus restores"
    );
    h.keys("<C-w>+");
    let tall = h.state.layout.rects();
    assert!(tall.iter().any(|(_, rect)| rect.h > 0.5));
    h.keys("<C-w>=");
    assert!(
        h.state
            .layout
            .rects()
            .iter()
            .all(|(_, rect)| (rect.h - 0.5).abs() < 1e-4)
    );
    h.keys("<space>wo");
    assert_eq!(h.state.layout.leaves().len(), 1);
}

#[test]
fn a_tab_moves_to_a_side_and_tabs_shift_and_return() {
    let (_dir, mut h) = files(&["a.txt", "b.txt", "c.txt"]);
    h.keys(":o a.txt<ret>:o b.txt<ret>:o c.txt<ret>");
    assert_eq!(bar(&h), ["a.txt", "b.txt", "c.txt"]);
    h.keys("g<");
    assert_eq!(bar(&h), ["a.txt", "c.txt", "b.txt"]);
    h.keys("gnga");
    assert_eq!(name(&h), "c.txt", "ga goes back to the tab used before");
    h.keys("<C-w>ml");
    assert_eq!(h.state.layout.leaves().len(), 2);
    assert_eq!(bar(&h), ["c.txt"]);
    h.keys("<C-w>mh");
    assert_eq!(h.state.layout.leaves().len(), 1, "the emptied pane closes");
    assert_eq!(name(&h), "c.txt");
}

#[test]
fn closing_other_tabs_keeps_unsaved_ones_unless_forced() {
    let (_dir, mut h) = files(&["a.txt", "b.txt", "c.txt"]);
    h.keys(":o a.txt<ret>ix<esc>:o b.txt<ret>:o c.txt<ret>");
    h.keys(":bco<ret>");
    assert_eq!(bar(&h), ["a.txt", "c.txt"]);
    assert!(matches!(
        h.state.notice,
        Some(Notice::UnsavedBuffers { .. })
    ));
    h.keys(":bco!<ret>");
    assert_eq!(bar(&h), ["c.txt"]);
    h.keys(":bca<ret>");
    assert!(bar(&h).is_empty());
}

#[test]
fn window_f_opens_the_file_under_the_cursor_in_a_split() {
    let (dir, mut h) = files(&["b.txt"]);
    fs::write(dir.path().join("a.txt"), "b.txt\n").unwrap();
    h.keys(":o a.txt<ret><C-w>f");
    assert_eq!(h.state.layout.leaves().len(), 2);
    assert!(
        matches!(h.state.layout.root(), Node::Split(split) if split.axis() == model::window::Axis::Column)
    );
    assert_eq!(name(&h), "b.txt");
}
