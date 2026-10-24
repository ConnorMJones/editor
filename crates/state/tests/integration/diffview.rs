use std::fs;

use helix_core::Selection;
use model::command::{ChangesCommand, Shown};
use model::git::Group;
use model::notice::Notice;
use state::Confirm;
use state::git::diffview::{self, DiffSides, LEFT, Left, RIGHT, Right};
use state::git::stage::Discard;
use state::input;
use state::input::keymap::Context;
use state::project::session::{self, SessionMode, SessionPane, SessionSidebar, SessionView};
use state::sidebar::changes::{self, ListCursor, ListRow};
use state::sidebar::tree::Fold;
use state::view::draws::{self, Draws, Part, Rows};
use state::view::layout::Viewport;
use state::view::panes::{self, Click};
use state::view::scroll;
use state::view::tab::{Showing, TextTabs};
use state::{Focus, Repo, Sidebar};

use crate::git::{init, stage_path, staged_text};
use crate::harness::Harness;

const FIVE: &str = "a\nb\nc\nd\ne\n";

fn diff(h: &Harness) -> &DiffSides {
    match h
        .state
        .layout
        .focused()
        .active_text()
        .map(TextTabs::showing)
    {
        Some(Showing::Diff(_, diff)) => diff,
        Some(Showing::Source(_) | Showing::Document(..) | Showing::Pages(..)) => {
            panic!("focused tab is not a diff")
        }
        None => panic!("focused pane has no tab"),
    }
}

fn shown(h: &Harness) -> Shown {
    h.state
        .layout
        .focused()
        .active_text()
        .map(TextTabs::shown)
        .unwrap()
}

fn draws(h: &Harness, part: Part) -> Draws<'_> {
    draws::drawing(&h.state, h.state.layout.focus(), part).draws
}

/// Draws nothing but records the size of the focused diff, as a frame would.
fn show(h: &mut Harness, rows: u16) {
    let focus = h.state.layout.focus();
    h.run(|state, _| scroll::measured(state, focus, Viewport { rows, cols: 40 }));
}

fn cursor_line(h: &Harness) -> usize {
    let text = h.doc().text.slice(..);
    h.view().selection.primary().cursor_line(text)
}

fn changed(h: &Harness) -> Vec<(Group, String)> {
    match &h.state.project.git {
        Repo::Found { snapshot, .. } => snapshot
            .changes
            .iter()
            .map(|entry| (entry.entry.group(), entry.path.as_str().to_owned()))
            .collect(),
        Repo::None | Repo::Unknown => panic!("no repository"),
    }
}

#[test]
fn a_tab_toggles_between_its_file_and_its_diff() {
    let dir = tempfile::tempdir().unwrap();
    init(dir.path(), &[("a.txt", FIVE)]);
    fs::write(dir.path().join("a.txt"), "a\nb\nX\nd\ne\n").unwrap();
    let mut h = Harness::new(dir.path());
    h.keys(":o a.txt<ret>3gg<space>d");
    assert_eq!(h.focused_tabs(), ["±a.txt"]);
    assert_eq!(cursor_line(&h), 2, "the selection carries over");
    show(&mut h, 20);
    let shown = diff(&h);
    assert!(matches!(shown.left, Left::Head { .. }));
    assert_eq!(shown.align.sides[LEFT].shades.len(), 1);
    assert_eq!(shown.align.sides[RIGHT].shades[0].lines, 2..3);

    // Toggling back shows the file in the same tab, where the diff left it.
    h.keys("j<space>d");
    assert_eq!(h.focused_tabs(), ["a.txt"]);
    assert_eq!(cursor_line(&h), 3);

    // The diff's buffer side edits the same buffer.
    h.keys("<space>dggiZ<esc>");
    assert_eq!(h.doc().text, "Za\nb\nX\nd\ne\n");
    h.keys("<space>d");
    assert_eq!(h.doc().text, "Za\nb\nX\nd\ne\n");
}

#[test]
fn toggling_a_preview_swaps_it_in_place() {
    let dir = tempfile::tempdir().unwrap();
    init(dir.path(), &[("a.txt", FIVE), ("b.txt", FIVE)]);
    fs::write(dir.path().join("a.txt"), "a\nB\nc\nd\ne\n").unwrap();
    let mut h = Harness::new(dir.path());
    h.keys(":o b.txt<ret>");
    h.run(|state, out| changes::list(state, model::command::ChangesCommand::Open, out));
    assert_eq!(h.focused_tabs(), ["b.txt", "±a.txt*"]);
    h.keys("<space>d");
    assert_eq!(h.focused_tabs(), ["b.txt", "a.txt*"]);
    h.keys("<space>d");
    assert_eq!(h.focused_tabs(), ["b.txt", "±a.txt*"]);
}

#[test]
fn hunk_keys_act_from_the_diff() {
    let dir = tempfile::tempdir().unwrap();
    let repo = init(dir.path(), &[("a.txt", FIVE)]);
    fs::write(dir.path().join("a.txt"), "a\nB\nc\nd\ne\n").unwrap();
    let mut h = Harness::new(dir.path());
    h.keys(":o a.txt<ret><space>d2gg<space>hs");
    assert_eq!(staged_text(&repo, "a.txt").unwrap(), "a\nB\nc\nd\ne\n");
    h.keys("<space>hu");
    assert_eq!(staged_text(&repo, "a.txt").unwrap(), FIVE);
    h.keys("<space>hr");
    assert_eq!(h.doc().text, FIVE);
}

#[test]
fn next_hunk_past_the_end_opens_the_next_diff() {
    let dir = tempfile::tempdir().unwrap();
    init(dir.path(), &[("a.txt", FIVE), ("b.txt", FIVE)]);
    fs::write(dir.path().join("a.txt"), "a\nB\nc\nd\ne\n").unwrap();
    fs::write(dir.path().join("b.txt"), "a\nb\nc\nD\ne\n").unwrap();
    let mut h = Harness::new(dir.path());
    h.keys(":o a.txt<ret><space>d]g");
    assert_eq!(cursor_line(&h), 1);
    h.keys("]g");
    assert_eq!(h.focused_tabs(), ["±a.txt", "±b.txt*"]);
    assert_eq!(cursor_line(&h), 3, "lands on the first change of b.txt");
    // From a file it moves on to the next file, which shows as a file too.
    h.keys("<space>d]g");
    assert_eq!(h.focused_tabs(), ["a.txt", "b.txt*"]);
    assert_eq!(shown(&h), Shown::Source);
}

#[test]
fn hunk_keys_wrap_through_changed_files_visiting_each_once() {
    let dir = tempfile::tempdir().unwrap();
    let repo = init(
        dir.path(),
        &[("a.txt", FIVE), ("b.txt", FIVE), ("c.txt", FIVE)],
    );
    fs::write(dir.path().join("a.txt"), "a\nb\nC\nd\ne\n").unwrap();
    fs::write(dir.path().join("b.txt"), "a\nB\nc\nd\ne\n").unwrap();
    stage_path(&repo, "b.txt");
    fs::write(dir.path().join("b.txt"), "a\nB\nc\nD\ne\n").unwrap();
    fs::write(dir.path().join("c.txt"), "a\nb\nc\nd\nE\n").unwrap();
    let mut h = Harness::new(dir.path());
    let file = |h: &Harness| {
        let path = &h.state.buffers[h.view().buffer()].path;
        path.file_name().unwrap().to_str().unwrap().to_owned()
    };
    // Sidebar order is staged b, then a and c; b's unstaged entry is not visited again.
    h.keys(":o a.txt<ret>]g");
    assert_eq!((file(&h), cursor_line(&h)), ("a.txt".to_owned(), 2));
    h.keys("]g");
    assert_eq!((file(&h), cursor_line(&h)), ("c.txt".to_owned(), 4));
    h.keys("]g");
    assert_eq!((file(&h), cursor_line(&h)), ("b.txt".to_owned(), 1));
    h.keys("]g");
    assert_eq!((file(&h), cursor_line(&h)), ("b.txt".to_owned(), 3));
    h.keys("]g");
    assert_eq!((file(&h), cursor_line(&h)), ("a.txt".to_owned(), 2));
    // Backwards lands on each file's last hunk, and past the first file wraps to the last.
    h.keys("[g");
    assert_eq!((file(&h), cursor_line(&h)), ("b.txt".to_owned(), 3));
    h.keys("[g");
    assert_eq!((file(&h), cursor_line(&h)), ("b.txt".to_owned(), 1));
    h.keys("[g");
    assert_eq!((file(&h), cursor_line(&h)), ("c.txt".to_owned(), 4));
}

#[test]
fn deleted_files_show_head_beside_an_inert_side() {
    let dir = tempfile::tempdir().unwrap();
    init(dir.path(), &[("a.txt", FIVE), ("gone.txt", "x\ny\nz\n")]);
    fs::remove_file(dir.path().join("gone.txt")).unwrap();
    let mut h = Harness::new(dir.path());
    h.keys(":o gone.txt<ret><space>d");
    show(&mut h, 10);
    let buffer = h.view().buffer();
    assert_eq!(diffview::right(&h.state.buffers[buffer]), Right::Deleted);
    let shown = diff(&h);
    assert!(matches!(shown.left, Left::Head { .. }));
    assert!(matches!(draws(&h, Part::Head), Draws::Head { .. }));
    assert!(matches!(draws(&h, Part::Buffer), Draws::Deleted { .. }));
    assert!(matches!(draws(&h, Part::Text), Draws::Absent));
    // HEAD's three lines face filler rows above the empty buffer.
    assert_eq!(shown.align.sides[RIGHT].lead, 3);
    h.keys("ityped");
    assert_eq!(h.doc().text, "");
    assert!(matches!(h.state.notice, Some(Notice::ReadOnly)));
    h.keys("<esc>");
    // The wheel still scrolls HEAD through the lead.
    let focus = h.state.layout.focus();
    h.run(|state, _| state::view::scroll::wheel(state, focus, 1));
    match &diff(&h).left {
        // The view opened at the top of the filler rows; one row down faces HEAD's `y`.
        Left::Head { scroll, .. } => assert_eq!((scroll.anchor, scroll.rows), (2, 0)),
        Left::Pending | Left::Absent | Left::Parsing { .. } => panic!("HEAD side not shown"),
    }
}

#[test]
fn untracked_files_show_a_blank_head_side() {
    let dir = tempfile::tempdir().unwrap();
    init(dir.path(), &[("a.txt", FIVE)]);
    fs::write(dir.path().join("new.txt"), "fresh\n").unwrap();
    let mut h = Harness::new(dir.path());
    h.keys(":o new.txt<ret><space>d");
    show(&mut h, 10);
    let shown = diff(&h);
    assert!(matches!(shown.left, Left::Absent));
    assert_eq!(shown.align.key, None);
    assert!(matches!(draws(&h, Part::Head), Draws::NoHead));
    assert!(matches!(
        draws(&h, Part::Buffer),
        Draws::Editable {
            rows: Rows::Diff(_),
            ..
        }
    ));
    // The buffer side is live.
    h.keys("iok <esc>");
    assert_eq!(h.doc().text, "ok fresh\n");
}

#[test]
fn changes_sidebar_groups_files_and_acts_on_them() {
    let dir = tempfile::tempdir().unwrap();
    let repo = init(dir.path(), &[("a.txt", FIVE), ("b.txt", FIVE)]);
    fs::write(dir.path().join("a.txt"), "a\nB\nc\nd\ne\n").unwrap();
    fs::write(dir.path().join("b.txt"), "a\nb\nC\nd\ne\n").unwrap();
    stage_path(&repo, "b.txt");
    fs::write(dir.path().join("new.txt"), "new\n").unwrap();
    let mut h = Harness::new(dir.path());
    h.keys("<space>g");
    assert_eq!(h.state.sidebar, Sidebar::Changes(Focus::Sidebar));
    assert_eq!(input::context(&h.state), Context::Changes);
    assert_eq!(
        changed(&h),
        [
            (Group::Staged, "b.txt".to_owned()),
            (Group::Changes, "a.txt".to_owned()),
            (Group::Changes, "new.txt".to_owned()),
        ]
    );
    let Repo::Found { snapshot, .. } = &h.state.project.git else {
        panic!("no repository");
    };
    let folds = h.state.changes.folds;
    let rows: Vec<Option<ListRow>> = (0..changes::list_len(&snapshot.changes, &folds) + 1)
        .map(|row| changes::rows(&snapshot.changes, &folds).nth(row))
        .collect();
    assert_eq!(
        rows,
        [
            Some(ListRow::Header {
                group: Group::Staged,
                count: 1,
                fold: Fold::Expanded,
            }),
            Some(ListRow::Entry(0)),
            Some(ListRow::Header {
                group: Group::Changes,
                count: 2,
                fold: Fold::Expanded,
            }),
            Some(ListRow::Entry(1)),
            Some(ListRow::Entry(2)),
            None,
        ]
    );
    let new = ListCursor::on(&snapshot.changes[2]);
    assert_eq!(
        changes::cursor_row(&snapshot.changes, &folds, Some(&new)),
        Some(4)
    );

    // Opening keeps the sidebar focused and shows the diff as a preview; `j` steps over the header.
    h.keys("jjl");
    assert_eq!(h.focused_tabs(), ["±a.txt*"]);
    assert_eq!(h.state.sidebar, Sidebar::Changes(Focus::Sidebar));

    h.keys("s");
    assert_eq!(staged_text(&repo, "a.txt").unwrap(), "a\nB\nc\nd\ne\n");
    assert_eq!(changed(&h)[0], (Group::Staged, "a.txt".to_owned()));
    // The cursor stays at the vanished entry's place, now new.txt.
    h.keys("kk");
    assert_eq!(cursor_path(&h), Some("b.txt"));
    h.keys("u");
    assert_eq!(staged_text(&repo, "b.txt").unwrap(), FIVE);
    assert_eq!(
        changed(&h),
        [
            (Group::Staged, "a.txt".to_owned()),
            (Group::Changes, "b.txt".to_owned()),
            (Group::Changes, "new.txt".to_owned()),
        ]
    );

    // Discarding asks first; `n` keeps the file, `y` deletes the untracked file.
    h.keys("jd");
    assert!(matches!(
        h.confirm(),
        Some(Confirm::Discard {
            how: Discard::Delete(_),
            ..
        })
    ));
    assert_eq!(input::context(&h.state), Context::Confirm);
    h.keys("n");
    assert!(matches!(h.state.notice, Some(Notice::DiscardNotConfirmed)));
    assert!(dir.path().join("new.txt").exists());
    h.keys("dy");
    assert!(!dir.path().join("new.txt").exists());

    // A double click opens for good, beside the preview.
    assert_eq!(
        changed(&h),
        [
            (Group::Staged, "a.txt".to_owned()),
            (Group::Changes, "b.txt".to_owned()),
        ]
    );
    h.run(|state, out| panes::click_changes(state, 3, Click::Double, out));
    assert_eq!(h.focused_tabs(), ["±a.txt*", "±b.txt"]);

    h.keys("<esc>");
    assert_eq!(h.state.sidebar, Sidebar::Changes(Focus::Editor));
    h.keys("<space>g<space>g");
    assert_eq!(h.state.sidebar, Sidebar::Hidden);
}

#[test]
fn session_restores_diff_tabs_and_the_changes_sidebar() {
    let dir = tempfile::tempdir().unwrap();
    init(dir.path(), &[("a.txt", FIVE)]);
    fs::write(dir.path().join("a.txt"), "a\nB\nc\nd\ne\n").unwrap();
    let mut before = Harness::new(dir.path());
    before.keys(":o a.txt<ret><space>d3gg<space>g<esc>");
    let saved = session::snapshot(&before.state);
    assert_eq!(saved.sidebar, SessionSidebar::Changes);
    match &saved.panes[0] {
        SessionPane::Tabs { tabs, active, .. } => {
            assert_eq!(tabs.len(), 1);
            assert!(matches!(
                tabs[0].view,
                SessionView::Text {
                    mode: SessionMode::Diff,
                    ..
                }
            ));
            assert_eq!(*active, 0);
        }
        SessionPane::Empty => panic!("pane lost its tabs"),
    }
    let text = toml::to_string(&saved).unwrap();

    let mut after = Harness::new(dir.path());
    after
        .run(|state, out| session::restore(state, toml::from_str(&text).unwrap(), out))
        .expect("valid session");
    assert_eq!(session::snapshot(&after.state), saved);
    assert_eq!(after.state.sidebar, Sidebar::Changes(Focus::Editor));
    assert_eq!(after.focused_tabs(), ["±a.txt"]);
    assert_eq!(cursor_line(&after), 2);
    assert!(matches!(diff(&after).left, Left::Head { .. }));
}

#[test]
fn clicking_the_head_side_moves_the_cursor_to_the_facing_line() {
    let dir = tempfile::tempdir().unwrap();
    init(dir.path(), &[("a.txt", FIVE)]);
    // Two lines inserted at the top push every HEAD line down by two.
    fs::write(dir.path().join("a.txt"), "x\ny\na\nb\nc\nd\ne\n").unwrap();
    let mut h = Harness::new(dir.path());
    h.keys(":o a.txt<ret><space>dge");
    show(&mut h, 20);
    assert_eq!(diff(&h).align.sides[LEFT].lead, 2);
    let focus = h.state.layout.focus();
    // Row 3 of the HEAD side is its second line, `b`, below two filler rows.
    h.run(|state, _| diffview::click_left(state, focus, 3, 0));
    assert_eq!(cursor_line(&h), 3);
    assert_eq!(h.view().selection, Selection::point(6));
}

fn cursor_path(h: &Harness) -> Option<&str> {
    match h.state.changes.cursor.as_ref() {
        Some(ListCursor::Entry { path, .. }) => Some(path.as_str()),
        Some(ListCursor::Header(_)) | None => None,
    }
}

#[test]
fn changes_groups_collapse_and_act_as_a_whole() {
    let dir = tempfile::tempdir().unwrap();
    let repo = init(dir.path(), &[("a.txt", FIVE), ("b.txt", FIVE)]);
    fs::write(dir.path().join("a.txt"), "a\nB\nc\nd\ne\n").unwrap();
    fs::write(dir.path().join("b.txt"), "a\nb\nC\nd\ne\n").unwrap();
    let mut h = Harness::new(dir.path());
    h.keys("<space>g");
    // Nothing selected yet: the cursor shows on the first file, below the empty Staged header and
    // its own header.
    let Repo::Found { snapshot, .. } = &h.state.project.git else {
        panic!("no repository");
    };
    assert_eq!(
        changes::cursor_row(&snapshot.changes, &h.state.changes.folds, None),
        Some(2)
    );

    // `h` on an entry moves to its header; `h` there collapses, `l` expands again.
    h.keys("h");
    assert_eq!(
        h.state.changes.cursor,
        Some(ListCursor::Header(Group::Changes))
    );
    h.keys("h");
    assert_eq!(
        h.state.changes.folds[Group::Changes as usize],
        Fold::Collapsed
    );
    let Repo::Found { snapshot, .. } = &h.state.project.git else {
        panic!("no repository");
    };
    assert_eq!(
        changes::list_len(&snapshot.changes, &h.state.changes.folds),
        2,
        "both headers stay listed"
    );
    h.keys("l");
    assert_eq!(
        h.state.changes.folds[Group::Changes as usize],
        Fold::Expanded
    );

    // `s` on the header stages the whole group; the Staged header's button unstages it all.
    h.keys("s");
    assert_eq!(staged_text(&repo, "a.txt").unwrap(), "a\nB\nc\nd\ne\n");
    assert_eq!(staged_text(&repo, "b.txt").unwrap(), "a\nb\nC\nd\ne\n");
    h.run(|state, out| panes::changes_action(state, 0, ChangesCommand::Unstage, out));
    assert_eq!(staged_text(&repo, "a.txt").unwrap(), FIVE);
    assert_eq!(staged_text(&repo, "b.txt").unwrap(), FIVE);

    // An entry's `+` button stages just that file; a header click toggles the group.
    h.run(|state, out| panes::changes_action(state, 3, ChangesCommand::Stage, out));
    assert_eq!(staged_text(&repo, "b.txt").unwrap(), "a\nb\nC\nd\ne\n");
    assert_eq!(staged_text(&repo, "a.txt").unwrap(), FIVE);
    h.run(|state, out| panes::click_changes(state, 0, Click::Single, out));
    assert_eq!(
        h.state.changes.folds[Group::Staged as usize],
        Fold::Collapsed
    );

    // Discard refuses whole groups.
    h.keys("d");
    assert!(matches!(h.state.notice, Some(Notice::DiscardGroup)));

    let saved = session::snapshot(&h.state);
    assert_eq!(saved.collapsed, [Group::Staged]);
    let mut after = Harness::new(dir.path());
    after
        .run(|state, out| session::restore(state, saved, out))
        .expect("valid session");
    assert_eq!(
        after.state.changes.folds[Group::Staged as usize],
        Fold::Collapsed
    );
}

#[test]
fn a_clean_repository_still_lists_staged_and_unstaged_groups() {
    let dir = tempfile::tempdir().unwrap();
    init(dir.path(), &[("a.txt", FIVE)]);
    let mut h = Harness::new(dir.path());
    h.keys("<space>g");
    let headers: Vec<ListRow> =
        changes::rows(h.state.project.git.changes(), &h.state.changes.folds).collect();
    assert_eq!(
        headers,
        [
            ListRow::Header {
                group: Group::Staged,
                count: 0,
                fold: Fold::Expanded
            },
            ListRow::Header {
                group: Group::Changes,
                count: 0,
                fold: Fold::Expanded
            },
        ]
    );
    // The cursor rests on the first header, and a click there folds it.
    h.run(|state, out| panes::click_changes(state, 0, Click::Single, out));
    assert_eq!(
        h.state.changes.folds[Group::Staged as usize],
        Fold::Collapsed
    );
}
