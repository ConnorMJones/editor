use std::path::Path;

use git2::{IndexAddOption, Repository, Signature};
use helix_core::Selection;
use model::command::{Answer, Command, FileOp};
use model::notice::Notice;
use state::Confirm;
use state::apply::apply;
use state::git::diff::{Base, BaseRead, Bases, Diff, Hunk, Stage, Tracked, stage};
use state::git::stage::Discard;
use state::git::{RepoPath, track};
use state::project::path::RelPath;

use crate::harness::Harness;

pub fn init(dir: &Path, files: &[(&str, &str)]) -> Repository {
    let repo = Repository::init(dir).expect("init repo");
    for (name, text) in files {
        std::fs::write(dir.join(name), text).expect("write file");
    }
    commit(&repo, "init");
    repo
}

/// Stages everything in the worktree and commits it.
pub fn commit(repo: &Repository, message: &str) {
    let mut index = repo.index().unwrap();
    index.read(true).unwrap();
    index.add_all(["*"], IndexAddOption::DEFAULT, None).unwrap();
    index.update_all(["*"], None).unwrap();
    index.write().unwrap();
    let tree = repo.find_tree(index.write_tree().unwrap()).unwrap();
    let signature = Signature::now("test", "test@example.com").unwrap();
    let parent = repo.head().ok().and_then(|head| head.peel_to_commit().ok());
    let parents: Vec<&git2::Commit> = parent.iter().collect();
    repo.commit(
        Some("HEAD"),
        &signature,
        &signature,
        message,
        &tree,
        &parents,
    )
    .unwrap();
}

pub fn stage_path(repo: &Repository, name: &str) {
    let mut index = repo.index().unwrap();
    index.read(true).unwrap();
    index.add_path(Path::new(name)).unwrap();
    index.write().unwrap();
}

pub fn staged_text(repo: &Repository, name: &str) -> Option<String> {
    // The handle caches its index; the editor writes through its own.
    let mut index = repo.index().unwrap();
    index.read(true).unwrap();
    index.get_path(Path::new(name), 0).map(|entry| {
        let blob = repo.find_blob(entry.id).unwrap();
        String::from_utf8(blob.content().to_vec()).unwrap()
    })
}

fn tracked(h: &Harness) -> &Tracked {
    match &h.state.buffers[h.view().buffer()].diff.bases {
        Bases::Tracked(tracked) => tracked,
        Bases::NoRepo | Bases::Loading { .. } | Bases::Untracked => panic!("buffer is not tracked"),
    }
}

fn fresh(h: &Harness) -> bool {
    tracked(h).is_fresh(h.doc().version)
}

fn hunk(before: std::ops::Range<u32>, after: std::ops::Range<u32>) -> Hunk {
    Hunk { before, after }
}

fn stages(h: &Harness) -> Vec<Stage> {
    let tracked = tracked(h);
    tracked
        .hunks
        .iter()
        .map(|hunk| stage(hunk, &tracked.unstaged))
        .collect()
}

fn run(h: &mut Harness, command: Command) {
    h.run(|state, out| apply(state, command, out));
}

const FIVE: &str = "a\nb\nc\nd\ne\n";

#[test]
fn hunks_follow_edits_against_head() {
    let dir = tempfile::tempdir().unwrap();
    init(dir.path(), &[("a.txt", FIVE)]);
    let mut h = Harness::new(dir.path());
    h.keys(":o a.txt<ret>");
    assert!(fresh(&h));
    assert!(tracked(&h).hunks.is_empty());

    h.keys("2ggiX<esc>");
    assert!(fresh(&h));
    assert_eq!(tracked(&h).hunks, [hunk(1..2, 1..2)]);

    h.keys("4ggonew<esc>");
    assert_eq!(tracked(&h).hunks, [hunk(1..2, 1..2), hunk(4..4, 4..5)]);
    assert_eq!(stages(&h), [Stage::Unstaged, Stage::Unstaged]);

    h.keys("ggxd");
    assert_eq!(
        tracked(&h).hunks,
        [hunk(0..2, 0..1), hunk(4..4, 3..4)],
        "a removed line joins the modified one below it"
    );
}

#[test]
fn hunks_are_classified_by_what_the_index_holds() {
    let dir = tempfile::tempdir().unwrap();
    let repo = init(dir.path(), &[("a.txt", "1\n2\n3\n4\n5\n6\n7\n8\n9\n")]);
    // Line 2 is fully staged, 4-5 half staged, 8 only in the worktree.
    std::fs::write(dir.path().join("a.txt"), "1\n2s\n3\n4p\n5\n6\n7\n8\n9\n").unwrap();
    stage_path(&repo, "a.txt");
    std::fs::write(dir.path().join("a.txt"), "1\n2s\n3\n4p\n5p\n6\n7\n8u\n9\n").unwrap();
    let mut h = Harness::new(dir.path());
    h.keys(":o a.txt<ret>");
    assert_eq!(
        tracked(&h).hunks,
        [hunk(1..2, 1..2), hunk(3..5, 3..5), hunk(7..8, 7..8)]
    );
    assert_eq!(stages(&h), [Stage::Staged, Stage::Partial, Stage::Unstaged]);
    assert_eq!(tracked(&h).staged, [hunk(1..2, 1..2), hunk(3..4, 3..4)]);
}

#[test]
fn stage_unstage_and_revert_a_hunk() {
    let dir = tempfile::tempdir().unwrap();
    let repo = init(dir.path(), &[("a.txt", FIVE)]);
    std::fs::write(dir.path().join("a.txt"), "a\nB\nc\nD\ne\n").unwrap();
    let mut h = Harness::new(dir.path());
    h.keys(":o a.txt<ret>");
    assert_eq!(tracked(&h).hunks.len(), 2);

    h.keys("2gg<space>hs");
    assert_eq!(h.state.notice.as_ref().map(ToString::to_string), None);
    assert_eq!(staged_text(&repo, "a.txt").unwrap(), "a\nB\nc\nd\ne\n");
    assert!(fresh(&h));
    assert_eq!(stages(&h), [Stage::Staged, Stage::Unstaged]);

    // A second stage of the same hunk finds nothing left to stage.
    h.keys("<space>hs");
    assert!(matches!(h.state.notice, Some(Notice::NoHunks)));

    h.keys("<space>hu");
    assert_eq!(staged_text(&repo, "a.txt").unwrap(), FIVE);
    assert_eq!(stages(&h), [Stage::Unstaged, Stage::Unstaged]);

    h.keys("4gg<space>hr");
    assert_eq!(h.doc().text, "a\nB\nc\nd\ne\n");
    assert_eq!(tracked(&h).hunks, [hunk(1..2, 1..2)]);
    h.keys("u");
    assert_eq!(h.doc().text, "a\nB\nc\nD\ne\n");
}

#[test]
fn staging_uses_the_buffer_text() {
    let dir = tempfile::tempdir().unwrap();
    let repo = init(dir.path(), &[("a.txt", FIVE)]);
    let mut h = Harness::new(dir.path());
    h.keys(":o a.txt<ret>");
    h.keys("3ggiC<esc>xd<space>hs");
    assert_eq!(staged_text(&repo, "a.txt").unwrap(), "a\nb\nd\ne\n");
    assert_eq!(stages(&h), [Stage::Staged]);
}

#[test]
fn stage_unstage_and_discard_files() {
    let dir = tempfile::tempdir().unwrap();
    let repo = init(dir.path(), &[("a.txt", FIVE)]);
    std::fs::write(dir.path().join("a.txt"), "changed\n").unwrap();
    std::fs::write(dir.path().join("new.txt"), "new\n").unwrap();
    let mut h = Harness::new(dir.path());

    run(&mut h, Command::File(FileOp::Stage("a.txt".into())));
    assert_eq!(staged_text(&repo, "a.txt").unwrap(), "changed\n");
    run(&mut h, Command::File(FileOp::Stage("new.txt".into())));
    assert_eq!(staged_text(&repo, "new.txt").unwrap(), "new\n");
    run(&mut h, Command::File(FileOp::Unstage("new.txt".into())));
    assert_eq!(staged_text(&repo, "new.txt"), None);
    run(&mut h, Command::File(FileOp::Unstage("a.txt".into())));
    assert_eq!(staged_text(&repo, "a.txt").unwrap(), FIVE);

    // Discarding waits for confirmation and restores the index copy.
    run(&mut h, Command::File(FileOp::Discard("a.txt".into())));
    assert!(matches!(
        h.confirm(),
        Some(Confirm::Discard {
            how: Discard::Checkout,
            ..
        })
    ));
    run(&mut h, Command::Confirm(Answer::No));
    assert!(matches!(h.state.notice, Some(Notice::DiscardNotConfirmed)));
    assert_eq!(
        std::fs::read_to_string(dir.path().join("a.txt")).unwrap(),
        "changed\n"
    );
    run(&mut h, Command::File(FileOp::Discard("a.txt".into())));
    run(&mut h, Command::Confirm(Answer::Yes));
    assert_eq!(
        std::fs::read_to_string(dir.path().join("a.txt")).unwrap(),
        FIVE
    );

    // An untracked file is deleted.
    run(&mut h, Command::File(FileOp::Discard("new.txt".into())));
    assert!(matches!(
        h.confirm(),
        Some(Confirm::Discard {
            how: Discard::Delete(_),
            ..
        })
    ));
    run(&mut h, Command::Confirm(Answer::Yes));
    assert!(!dir.path().join("new.txt").exists());
    assert_eq!(h.confirm(), None);

    run(&mut h, Command::File(FileOp::Discard("a.txt".into())));
    assert!(matches!(h.state.notice, Some(Notice::Unchanged { .. })));
}

#[test]
fn next_change_crosses_into_the_next_changed_file() {
    let dir = tempfile::tempdir().unwrap();
    let repo = init(
        dir.path(),
        &[("a.txt", FIVE), ("b.txt", FIVE), ("c.txt", FIVE)],
    );
    std::fs::write(dir.path().join("a.txt"), "a\nB\nc\nD\ne\n").unwrap();
    std::fs::write(dir.path().join("c.txt"), "a\nb\nC\nd\ne\n").unwrap();
    // A staged file comes before unstaged ones in sidebar order.
    std::fs::write(dir.path().join("b.txt"), "a\nb\nc\nd\nE\n").unwrap();
    stage_path(&repo, "b.txt");
    let mut h = Harness::new(dir.path());
    h.keys(":o a.txt<ret>");

    h.keys("]g");
    assert_eq!(h.view().selection, Selection::single(2, 4));
    h.keys("]g");
    assert_eq!(h.view().selection, Selection::single(6, 8));
    h.keys("]g");
    assert!(h.focused_path().unwrap().ends_with("c.txt"));
    assert_eq!(h.view().selection, Selection::single(4, 6));
    // Wraps around to the staged file, which leads the order.
    h.keys("]g");
    assert!(h.focused_path().unwrap().ends_with("b.txt"));
    assert_eq!(h.view().selection, Selection::single(8, 10));
    h.keys("[g");
    assert!(h.focused_path().unwrap().ends_with("c.txt"));
    assert_eq!(h.view().selection, Selection::single(4, 6));
    h.keys("[g");
    assert!(h.focused_path().unwrap().ends_with("a.txt"));
    assert_eq!(h.view().selection, Selection::single(6, 8));
    h.keys("[G");
    assert_eq!(h.view().selection, Selection::single(2, 4));
    h.keys("]G");
    assert_eq!(h.view().selection, Selection::single(6, 8));
}

#[test]
fn next_change_extends_in_select_mode() {
    let dir = tempfile::tempdir().unwrap();
    init(dir.path(), &[("a.txt", FIVE)]);
    std::fs::write(dir.path().join("a.txt"), "a\nB\nc\nD\ne\n").unwrap();
    let mut h = Harness::new(dir.path());
    h.keys(":o a.txt<ret>");
    h.keys("v]g]g");
    assert_eq!(h.view().selection, Selection::single(0, 8));
}

#[test]
fn diff_refreshes_after_an_external_commit() {
    let dir = tempfile::tempdir().unwrap();
    let repo = init(dir.path(), &[("a.txt", FIVE)]);
    std::fs::write(dir.path().join("a.txt"), "a\nB\nc\nd\ne\n").unwrap();
    let mut h = Harness::watched(dir.path());
    h.keys(":o a.txt<ret>");
    assert_eq!(tracked(&h).hunks.len(), 1);
    commit(&repo, "external");
    assert!(
        h.wait_for_events(|state| {
            let buffer = state.buffers.values().next().unwrap();
            match &buffer.diff.bases {
                Bases::Tracked(tracked) => {
                    tracked.is_fresh(buffer.doc.version) && tracked.hunks.is_empty()
                }
                Bases::NoRepo | Bases::Loading { .. } | Bases::Untracked => false,
            }
        }),
        "the commit's ref and index updates refresh the bases"
    );
}

#[test]
fn new_file_in_the_index_is_one_staged_hunk() {
    let dir = tempfile::tempdir().unwrap();
    let repo = init(dir.path(), &[("a.txt", FIVE)]);
    std::fs::write(dir.path().join("n.txt"), "x\ny\n").unwrap();
    stage_path(&repo, "n.txt");
    std::fs::write(dir.path().join("u.txt"), "untracked\n").unwrap();
    let mut h = Harness::new(dir.path());
    h.keys(":o n.txt<ret>");
    assert_eq!(tracked(&h).hunks, [hunk(0..0, 0..2)]);
    assert_eq!(stages(&h), [Stage::Staged]);
    h.keys("<space>hu");
    assert_eq!(staged_text(&repo, "n.txt").unwrap(), "");

    h.keys(":o u.txt<ret>");
    assert!(matches!(
        h.state.buffers[h.view().buffer()].diff.bases,
        Bases::Untracked
    ));
    h.keys("<space>hs");
    assert!(matches!(h.state.notice, Some(Notice::NotTracked { .. })));
}

#[test]
fn deleted_file_shows_one_removal() {
    let dir = tempfile::tempdir().unwrap();
    init(dir.path(), &[("a.txt", FIVE), ("gone.txt", "x\ny\n")]);
    std::fs::remove_file(dir.path().join("gone.txt")).unwrap();
    std::fs::write(dir.path().join("a.txt"), "a\nB\nc\nd\ne\n").unwrap();
    let mut h = Harness::new(dir.path());
    h.keys(":o a.txt<ret>");
    h.keys("]g]g");
    assert!(h.focused_path().unwrap().ends_with("gone.txt"));
    assert_eq!(tracked(&h).hunks, [hunk(0..2, 0..0)]);
    assert_eq!(stages(&h), [Stage::Unstaged]);
    assert_eq!(h.view().selection, Selection::point(0));
    h.keys("<space>hr");
    assert_eq!(h.doc().text, "x\ny\n");
}

#[test]
fn bases_read_for_a_path_the_buffer_left_are_dropped() {
    let dir = tempfile::tempdir().unwrap();
    init(dir.path(), &[("a.txt", FIVE)]);
    std::fs::write(dir.path().join("a.txt"), "changed\n").unwrap();
    let mut h = Harness::new(dir.path());
    h.keys(":o a.txt<ret>");
    let head = tracked(&h).head.oid();
    let buffer = h.view().buffer();
    // As if the buffer moved away from `old.txt` while its read ran.
    let old = RepoPath(RelPath::parse("old.txt").unwrap());
    let absent = || BaseRead::Fresh(Base::Absent);
    h.run(|state, out| track::bases_read(state, buffer, old, Ok([absent(), absent()]), out));
    assert_eq!(tracked(&h).head.oid(), head);
}
