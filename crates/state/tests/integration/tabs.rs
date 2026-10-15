use std::fs;

use model::notice::Notice;
use state::apply::Force;
use state::input::picker::Items;
use state::project::session::{self, Session, SessionError};
use state::view::layout::InvalidTabs;
use state::view::panes::{self, Click};
use state::{Overlay, State};

use crate::harness::Harness;

/// A root holding each of `names` with its own name as text, with the tree shown.
fn files(names: &[&str]) -> (tempfile::TempDir, Harness) {
    let dir = tempfile::tempdir().unwrap();
    for name in names {
        let path = dir.path().join(name);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, format!("{name}\n")).unwrap();
    }
    let mut h = Harness::new(dir.path());
    h.keys("<space>e<esc>");
    (dir, h)
}

/// Clicks the tree row of `path`, expanding its directories first.
fn tree_click(h: &mut Harness, path: &str, click: Click) {
    for (end, _) in path.match_indices('/') {
        let dir = &path[..end];
        let row = row(&h.state, dir);
        match h
            .state
            .tree
            .dirs
            .get(&state::project::path::RelPath::parse(dir).unwrap())
        {
            Some(listing) if listing.fold == state::sidebar::tree::Fold::Expanded => {}
            Some(_) | None => h.run(|state, out| panes::click_tree(state, row, Click::Single, out)),
        }
    }
    let row = row(&h.state, path);
    h.run(|state, out| panes::click_tree(state, row, click, out));
}

fn row(state: &State, path: &str) -> usize {
    state
        .tree
        .rows
        .iter()
        .position(|row| row.path.as_str() == path)
        .unwrap_or_else(|| panic!("{path} is not listed"))
}

fn open_names(h: &Harness) -> Vec<String> {
    let mut names: Vec<String> = (h.state.buffers.values())
        .map(|buffer| {
            buffer
                .path
                .file_name()
                .unwrap()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    names.sort();
    names
}

#[test]
fn tree_opens_replace_the_active_preview_in_place() {
    let (_dir, mut h) = files(&["a.txt", "b.txt", "c.txt"]);
    tree_click(&mut h, "a.txt", Click::Single);
    assert_eq!(h.focused_tabs(), ["a.txt*"]);
    tree_click(&mut h, "b.txt", Click::Single);
    assert_eq!(h.focused_tabs(), ["b.txt*"]);
    assert_eq!(
        open_names(&h),
        ["b.txt"],
        "the replaced preview's buffer is dropped"
    );

    h.keys(":o c.txt<ret>gp");
    assert_eq!(h.focused_tabs(), ["b.txt*", "c.txt"]);
    tree_click(&mut h, "a.txt", Click::Single);
    assert_eq!(h.focused_tabs(), ["a.txt*", "c.txt"]);
}

#[test]
fn an_inactive_preview_closes_and_the_new_one_follows_the_active_tab() {
    let (_dir, mut h) = files(&["a.txt", "b.txt", "p.txt", "q.txt"]);
    tree_click(&mut h, "a.txt", Click::Single);
    h.keys(":o p.txt<ret>:o q.txt<ret>gp");
    assert_eq!(h.focused_tabs(), ["a.txt*", "p.txt", "q.txt"]);
    assert_eq!(h.focused_path().unwrap().file_name().unwrap(), "p.txt");

    tree_click(&mut h, "b.txt", Click::Single);
    assert_eq!(h.focused_tabs(), ["p.txt", "b.txt*", "q.txt"]);
    assert_eq!(h.focused_path().unwrap().file_name().unwrap(), "b.txt");
    assert_eq!(open_names(&h), ["b.txt", "p.txt", "q.txt"]);
}

#[test]
fn modified_previews_are_kept_and_never_replaced() {
    let (_dir, mut h) = files(&["a.txt", "b.txt"]);
    tree_click(&mut h, "a.txt", Click::Single);
    h.keys("ix<esc>");
    assert_eq!(h.focused_tabs(), ["a.txt"]);
    tree_click(&mut h, "b.txt", Click::Single);
    assert_eq!(h.focused_tabs(), ["a.txt", "b.txt*"]);
    assert!(h.state.buffers[h.buffer("a.txt")].content.is_modified());

    // A modified buffer opened from the tree in another pane opens permanently.
    h.keys("<C-w>v");
    tree_click(&mut h, "a.txt", Click::Single);
    assert_eq!(h.focused_tabs(), ["a.txt"], "it replaces the split's copy");
}

#[test]
fn edits_from_another_pane_keep_the_preview() {
    let (_dir, mut h) = files(&["a.txt"]);
    tree_click(&mut h, "a.txt", Click::Single);
    let first = h.state.layout.focus;
    h.keys("<C-w>v");
    assert_eq!(h.tabs(first), ["a.txt*"]);
    h.keys("ix<esc>");
    assert_eq!(h.tabs(first), ["a.txt"]);
}

#[test]
fn undo_counts_as_an_edit() {
    let (_dir, mut h) = files(&["a.txt", "b.txt"]);
    // a.txt keeps its history in the first pane while the second opens it as a preview.
    h.keys(":o a.txt<ret>ix<esc>:w<ret><C-w>v:o b.txt<ret>");
    tree_click(&mut h, "a.txt", Click::Single);
    assert_eq!(h.focused_tabs(), ["b.txt", "a.txt*"]);
    h.keys("u");
    assert_eq!(h.focused_tabs(), ["b.txt", "a.txt"]);
}

#[test]
fn saving_and_double_clicks_keep_the_preview() {
    let (_dir, mut h) = files(&["a.txt", "b.txt", "c.txt"]);
    tree_click(&mut h, "a.txt", Click::Single);
    h.keys(":w<ret>");
    assert_eq!(h.focused_tabs(), ["a.txt"]);

    tree_click(&mut h, "b.txt", Click::Single);
    assert_eq!(h.focused_tabs(), ["a.txt", "b.txt*"]);
    let pane = h.state.layout.focus;
    panes::click_tab(&mut h.state, pane, 1, Click::Single);
    assert_eq!(h.focused_tabs(), ["a.txt", "b.txt*"]);
    panes::click_tab(&mut h.state, pane, 1, Click::Double);
    assert_eq!(h.focused_tabs(), ["a.txt", "b.txt"]);

    tree_click(&mut h, "c.txt", Click::Single);
    tree_click(&mut h, "c.txt", Click::Double);
    assert_eq!(h.focused_tabs(), ["a.txt", "b.txt", "c.txt"]);
}

#[test]
fn opening_permanently_unpreviews() {
    let (_dir, mut h) = files(&["a.txt", "b.txt"]);
    tree_click(&mut h, "a.txt", Click::Single);
    h.keys(":o b.txt<ret>");
    assert_eq!(h.focused_tabs(), ["a.txt*", "b.txt"]);
    h.keys(":o a.txt<ret>");
    assert_eq!(h.focused_tabs(), ["a.txt", "b.txt"]);
    assert_eq!(h.focused_path().unwrap().file_name().unwrap(), "a.txt");
}

#[test]
fn file_picker_opens_permanently_by_default() {
    let (_dir, mut h) = files(&["a.txt", "b.txt"]);
    h.keys("<space>fa.txt<ret><space>fb.txt<ret>");
    assert_eq!(h.focused_tabs(), ["a.txt", "b.txt"]);
}

#[test]
fn buffer_picker_activates_the_existing_tab_or_adds_a_permanent_one() {
    let (_dir, mut h) = files(&["a.txt", "b.txt"]);
    h.keys(":o a.txt<ret><C-w>v:o b.txt<ret><C-w>h");
    assert_eq!(h.focused_tabs(), ["a.txt"]);
    h.keys("<space>bb.txt<ret>");
    assert_eq!(h.focused_tabs(), ["a.txt", "b.txt"]);
    h.keys("<space>ba.txt<ret>");
    assert_eq!(h.focused_tabs(), ["a.txt", "b.txt"]);
    assert_eq!(h.focused_path().unwrap().file_name().unwrap(), "a.txt");
}

#[test]
fn closing_the_last_view_drops_the_buffer() {
    let (_dir, mut h) = files(&["a.txt", "b.txt", "c.txt"]);
    h.keys(":o a.txt<ret>:o b.txt<ret>:o c.txt<ret>gp");
    // The previously active tab takes over, not the neighbour.
    h.keys("gn:bc<ret>");
    assert_eq!(h.focused_tabs(), ["a.txt", "b.txt"]);
    assert_eq!(h.focused_path().unwrap().file_name().unwrap(), "b.txt");
    assert_eq!(open_names(&h), ["a.txt", "b.txt"]);

    h.keys(":bc<ret>:bc<ret>");
    let focus = h.state.layout.focus;
    assert!(h.state.layout.panes[focus].tabs.is_none());
    assert!(h.state.buffers.is_empty());
    assert_eq!(h.state.layout.panes.len(), 1, "the empty pane stays");
    h.keys(":bc<ret>");
    assert!(matches!(h.state.notice, Some(Notice::NoBuffer)));
}

#[test]
fn closing_a_modified_buffers_last_view_is_refused_unless_forced() {
    let (_dir, mut h) = files(&["a.txt"]);
    h.keys(":o a.txt<ret>ix<esc><C-w>v:bc<ret>");
    assert!(h.state.notice.is_none(), "another pane still shows it");
    assert_eq!(h.state.layout.panes.len(), 1, "the emptied pane closes");

    h.keys(":bc<ret>");
    assert!(matches!(
        h.state.notice,
        Some(Notice::UnsavedBuffers { count: 1 })
    ));
    assert_eq!(h.focused_tabs(), ["a.txt"]);

    let pane = h.state.layout.focus;
    h.run(|state, _| panes::close_tab(state, pane, 0, Force::No));
    assert_eq!(h.focused_tabs(), ["a.txt"]);
    h.keys(":bc!<ret>");
    assert!(h.state.buffers.is_empty());
}

#[test]
fn one_buffer_in_two_panes_maps_every_tab_through_edits() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("a.txt"), "hello\nworld\n").unwrap();
    fs::write(dir.path().join("b.txt"), "b\n").unwrap();
    let mut h = Harness::new(dir.path());
    h.keys(":o a.txt<ret>");
    let first = h.state.layout.focus;
    // Opened permanently, the split's copy stays when b.txt opens beside it.
    h.keys("<C-w>vj:o a.txt<ret>");
    let second = h.state.layout.focus;
    let selection = |h: &Harness, pane, index: usize| {
        h.state.layout.panes[pane].items()[index]
            .view
            .text()
            .selection
            .clone()
    };
    let parked = selection(&h, second, 0);
    // The second pane's a.txt tab goes inactive behind b.txt.
    h.keys(":o b.txt<ret>");
    assert_eq!(h.tabs(second), ["a.txt", "b.txt"]);

    h.keys("<C-w>hggiXX<esc>");
    assert_eq!(h.state.layout.focus, first);
    let shifted =
        parked.transform(|range| helix_core::Range::new(range.anchor + 2, range.head + 2));
    assert_eq!(selection(&h, second, 0), shifted);

    // And the other way round: edits in the second pane move the first pane's selection.
    h.keys("j");
    let before = selection(&h, first, 0);
    h.keys("<C-w>lgpggiYYY<esc>");
    assert_eq!(h.focused_path().unwrap().file_name().unwrap(), "a.txt");
    let moved = before.transform(|range| helix_core::Range::new(range.anchor + 3, range.head + 3));
    assert_eq!(selection(&h, first, 0), moved);
    assert_eq!(h.doc().text, "YYYXXhello\nworld\n");
}

#[test]
fn buffer_picker_lists_most_recently_used_first() {
    let (_dir, mut h) = files(&["a.txt", "b.txt", "c.txt"]);
    h.keys(":o a.txt<ret>:o b.txt<ret>:o c.txt<ret>gpgp");
    assert_eq!(h.focused_path().unwrap().file_name().unwrap(), "a.txt");
    h.keys("<space>b");
    let labels: Vec<String> = match &h.state.overlay {
        Overlay::Picker(picker) => match &picker.items {
            Items::Buffers(buffers) => buffers.iter().map(|(_, label)| label.clone()).collect(),
            Items::Files(_) | Items::Grep(_) | Items::Commands | Items::Backlinks(_) => {
                panic!("wrong picker")
            }
        },
        Overlay::None | Overlay::Prompt(_) => panic!("picker not open"),
    };
    assert_eq!(labels, ["a.txt", "b.txt", "c.txt"]);

    h.keys("<esc><C-w>v:o c.txt<ret><C-w>h<space>b");
    let first = match &h.state.overlay {
        Overlay::Picker(picker) => picker.items.label(picker.matches[0]).to_owned(),
        Overlay::None | Overlay::Prompt(_) => panic!("picker not open"),
    };
    assert_eq!(first, "a.txt", "focusing a pane activates its tab");
}

#[test]
fn tab_cycling_stays_within_the_focused_pane() {
    let (_dir, mut h) = files(&["a.txt", "b.txt", "c.txt"]);
    h.keys(":o a.txt<ret>:o b.txt<ret><C-w>v:o b.txt<ret>:o c.txt<ret>");
    assert_eq!(h.focused_tabs(), ["b.txt", "c.txt"]);
    h.keys("gn");
    assert_eq!(h.focused_path().unwrap().file_name().unwrap(), "b.txt");
    h.keys("gn");
    assert_eq!(h.focused_path().unwrap().file_name().unwrap(), "c.txt");
    h.keys(":bn<ret>gp");
    assert_eq!(h.focused_path().unwrap().file_name().unwrap(), "c.txt");
    h.keys("<C-w>h");
    assert_eq!(h.focused_tabs(), ["a.txt", "b.txt"]);
    h.keys("gp");
    assert_eq!(h.focused_path().unwrap().file_name().unwrap(), "a.txt");
}

#[test]
fn split_copies_only_the_active_item() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("a.txt"), "a\n").unwrap();
    fs::write(dir.path().join("b.txt"), "one two\n").unwrap();
    let mut h = Harness::new(dir.path());
    h.keys(":o a.txt<ret>:o b.txt<ret>w");
    let first = h.state.layout.focus;
    let selection = h.view().selection.clone();
    h.keys("<C-w>s");
    assert_eq!(h.focused_tabs(), ["b.txt*"]);
    assert_eq!(h.view().selection, selection);

    h.keys("b");
    assert_ne!(h.view().selection, selection);
    match h.state.layout.panes[first].active() {
        Some(view) => assert_eq!(view.text().selection, selection),
        None => panic!("first pane lost its tab"),
    }
    h.keys("<C-w>k");
    assert_eq!(h.focused_tabs(), ["a.txt", "b.txt"]);
}

#[test]
fn split_of_an_empty_pane_is_empty() {
    let dir = tempfile::tempdir().unwrap();
    let mut h = Harness::new(dir.path());
    h.keys("<C-w>v");
    let focus = h.state.layout.focus;
    assert!(h.state.layout.panes[focus].tabs.is_none());
}

fn round_trip(saved: &Session) -> Session {
    toml::from_str(&toml::to_string(saved).expect("serializes")).expect("parses")
}

#[test]
fn session_restores_tabs_selections_and_the_preview() {
    let (dir, mut before) = files(&["a.txt", "b.txt", "c.txt", "d.txt"]);
    // Opening the split's preview copy permanently keeps it.
    before.keys(":o a.txt<ret>:o b.txt<ret>l<C-w>v:o b.txt<ret>:o c.txt<ret>");
    tree_click(&mut before, "d.txt", Click::Single);
    before.keys("gp");
    assert_eq!(before.focused_tabs(), ["b.txt", "c.txt", "d.txt*"]);
    let saved = session::snapshot(&before.state);

    let mut after = Harness::new(dir.path());
    after
        .run(|state, out| session::restore(state, round_trip(&saved), out))
        .expect("valid session");
    assert_eq!(session::snapshot(&after.state), saved);
    let leaves = after.state.layout.leaves();
    assert_eq!(after.tabs(leaves[0]), ["a.txt", "b.txt"]);
    assert_eq!(after.focused_tabs(), ["b.txt", "c.txt", "d.txt*"]);
    assert_eq!(after.focused_path().unwrap().file_name().unwrap(), "c.txt");

    // A deleted active tab hands over to its nearest neighbour; a deleted preview is gone.
    fs::remove_file(dir.path().join("c.txt")).unwrap();
    fs::remove_file(dir.path().join("d.txt")).unwrap();
    let mut pruned = Harness::new(dir.path());
    pruned
        .run(|state, out| session::restore(state, round_trip(&saved), out))
        .expect("valid session");
    assert_eq!(pruned.focused_tabs(), ["b.txt"]);
}

#[test]
fn session_tab_lists_are_validated() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("a.txt"), "a").unwrap();
    let mut h = Harness::new(dir.path());
    let path = dir.path().join("a.txt");
    let tab = format!(
        "{{ kind = \"text\", path = {:?}, ranges = [[0, 0]], primary = 0, scroll = 0 }}",
        path.display().to_string()
    );
    let session = |pane: &str| -> Session {
        toml::from_str(&format!(
            "focus = 0\nsidebar = \"hidden\"\nexpanded = []\npanes = [{pane}]\n\
             layout = {{ kind = \"pane\", pane = 0 }}\n"
        ))
        .expect("well-formed toml")
    };
    let rejected = |h: &mut Harness, pane: String| {
        let saved = session(&pane);
        h.run(|state, out| session::restore(state, saved, out))
    };
    assert!(matches!(
        rejected(
            &mut h,
            format!("{{ kind = \"tabs\", tabs = [{tab}], active = 1 }}")
        ),
        Err(SessionError::Tabs {
            pane: 0,
            reason: InvalidTabs::Active(1)
        })
    ));
    assert!(matches!(
        rejected(
            &mut h,
            format!("{{ kind = \"tabs\", tabs = [{tab}], active = 0, preview = 2 }}")
        ),
        Err(SessionError::Tabs {
            reason: InvalidTabs::Preview(2),
            ..
        })
    ));
    assert!(matches!(
        rejected(
            &mut h,
            format!("{{ kind = \"tabs\", tabs = [{tab}, {tab}], active = 0 }}")
        ),
        Err(SessionError::Tabs {
            reason: InvalidTabs::Duplicate(1),
            ..
        })
    ));
    assert!(matches!(
        rejected(
            &mut h,
            "{ kind = \"tabs\", tabs = [], active = 0 }".to_owned()
        ),
        Err(SessionError::Tabs {
            reason: InvalidTabs::Empty,
            ..
        })
    ));
    assert!(
        h.state.buffers.is_empty(),
        "nothing opens before validation"
    );
}

#[test]
fn sessions_from_the_global_tab_line_are_rejected() {
    let old = "tabs = [\"/a.txt\"]\nfocus = 0\nsidebar = \"hidden\"\nexpanded = []\n\
               panes = [{ kind = \"text\", path = \"/a.txt\", ranges = [[0, 0]], primary = 0, scroll = 0 }]\n\
               layout = { kind = \"pane\", pane = 0 }\n";
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("old.toml");
    fs::write(&path, old).unwrap();
    assert!(matches!(session::load(&path), Err(SessionError::Parse(_))));
}

#[test]
fn window_mode_arrows_move_focus() {
    let (_dir, mut h) = files(&["a.txt", "b.txt"]);
    h.keys(":o a.txt<ret><C-w>v:o b.txt<ret><C-w><left>");
    assert_eq!(h.focused_path().unwrap().file_name().unwrap(), "a.txt");
    h.keys("<space>w<right>");
    assert_eq!(h.focused_path().unwrap().file_name().unwrap(), "b.txt");
}
