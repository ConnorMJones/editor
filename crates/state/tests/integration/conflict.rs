use std::path::Path;

use git2::build::CheckoutBuilder;
use git2::{IndexAddOption, Oid, Repository, Signature};
use helix_core::{Range, Selection};
use model::command::{Command, FileOp};
use model::git::Group;
use model::notice::Notice;
use state::Repo;
use state::apply::apply;
use state::git::conflict::{self, Conflict, Sections};
use state::view::layout::Viewport;
use state::view::scroll::{self, Press, Unit};

use crate::harness::Harness;

const BASE: &str = "\
fn main() {
    let a = 1;
    let b = 2;
    print(a, b);
}
";

const OURS: &str = "\
fn main() {
    let a = 10;
    let b = 2;
    print(a, b);
}
";

const THEIRS: &str = "\
fn main() {
    let a = 100;
    let b = 2;
    print(a, b);
}
";

fn commit(repo: &Repository, update: &str, parents: &[Oid]) -> Oid {
    let mut index = repo.index().unwrap();
    index.read(true).unwrap();
    index.add_all(["*"], IndexAddOption::DEFAULT, None).unwrap();
    index.write().unwrap();
    let tree = repo.find_tree(index.write_tree().unwrap()).unwrap();
    let signature = Signature::now("test", "test@example.com").unwrap();
    let parents: Vec<git2::Commit> = parents
        .iter()
        .map(|oid| repo.find_commit(*oid).unwrap())
        .collect();
    let parents: Vec<&git2::Commit> = parents.iter().collect();
    repo.commit(
        Some(update),
        &signature,
        &signature,
        "commit",
        &tree,
        &parents,
    )
    .unwrap()
}

/// A repository whose `a.rs` holds merge markers: HEAD and `feature` both changed line 2.
fn merged(dir: &Path, diff3: bool) {
    let repo = Repository::init(dir).unwrap();
    std::fs::write(dir.join("a.rs"), BASE).unwrap();
    let base = commit(&repo, "HEAD", &[]);
    std::fs::write(dir.join("a.rs"), THEIRS).unwrap();
    let feature = commit(&repo, "refs/heads/feature", &[base]);
    std::fs::write(dir.join("a.rs"), OURS).unwrap();
    commit(&repo, "HEAD", &[base]);
    let theirs = repo.find_annotated_commit(feature).unwrap();
    let mut checkout = CheckoutBuilder::new();
    checkout.our_label("HEAD").their_label("feature");
    match diff3 {
        true => checkout.conflict_style_diff3(true),
        false => checkout.conflict_style_merge(true),
    };
    repo.merge(&[&theirs], None, Some(&mut checkout)).unwrap();
}

fn run(h: &mut Harness, command: Command) {
    h.run(|state, out| apply(state, command, out));
}

fn groups(h: &Harness, name: &str) -> Vec<Group> {
    match &h.state.project.git {
        Repo::Found { snapshot, .. } => snapshot
            .changes
            .iter()
            .filter(|changed| changed.path.as_str() == name)
            .map(|changed| changed.entry.group())
            .collect(),
        Repo::Unknown | Repo::None => panic!("no repository"),
    }
}

fn conflicts(h: &Harness) -> Vec<Conflict> {
    conflict::current(h.doc()).list.clone()
}

/// Line range of the primary selection, inclusive, with the cursor's line.
fn selected(h: &Harness) -> (usize, usize, usize) {
    let text = h.doc().text.slice(..);
    let primary = h.view().selection.primary();
    let (start, end) = primary.line_range(text);
    (start, end, primary.cursor_line(text))
}

#[test]
fn a_real_merge_conflict_resolves_and_leaves_the_conflicts_group_once_staged() {
    let dir = tempfile::tempdir().unwrap();
    merged(dir.path(), false);
    let mut h = Harness::new(dir.path());
    assert_eq!(groups(&h, "a.rs"), [Group::Conflicts]);
    h.keys(":o a.rs<ret>");
    assert_eq!(
        h.doc().text,
        "fn main() {\n<<<<<<< HEAD\n    let a = 10;\n=======\n    let a = 100;\n>>>>>>> feature\n    let b = 2;\n    print(a, b);\n}\n"
    );
    assert_eq!(
        conflicts(&h),
        [Conflict {
            open: 1,
            sections: Sections::TwoWay,
            separator: 3,
            close: 5
        }]
    );

    h.keys("]m");
    assert_eq!(selected(&h), (1, 5, 5));
    h.keys("<space>mb");
    assert_eq!(
        h.doc().text,
        "fn main() {\n    let a = 10;\n    let a = 100;\n    let b = 2;\n    print(a, b);\n}\n"
    );
    assert!(conflicts(&h).is_empty());
    h.keys("u");
    assert_eq!(conflicts(&h).len(), 1, "undo brings the markers back");
    h.keys("<space>mo");
    assert_eq!(h.doc().text, OURS);
    h.keys("u<space>mt");
    assert_eq!(h.doc().text, THEIRS);

    h.keys(":w<ret>");
    assert_eq!(
        std::fs::read_to_string(dir.path().join("a.rs")).unwrap(),
        THEIRS
    );
    assert_eq!(
        groups(&h, "a.rs"),
        [Group::Conflicts],
        "git still records the conflict until the file is staged"
    );
    run(&mut h, Command::File(FileOp::Stage("a.rs".into())));
    assert!(matches!(h.state.notice, Some(Notice::Written { .. })));
    assert_eq!(groups(&h, "a.rs"), [Group::Staged]);
}

#[test]
fn every_resolution_is_one_undoable_edit() {
    let dir = tempfile::tempdir().unwrap();
    merged(dir.path(), true);
    let mut h = Harness::new(dir.path());
    h.keys(":o a.rs<ret>");
    let marked = h.doc().text.clone();
    assert_eq!(
        conflicts(&h),
        [Conflict {
            open: 1,
            sections: Sections::Diff3 { base: 3 },
            separator: 5,
            close: 7
        }]
    );
    for (keys, expected) in [
        ("<space>mo", OURS.to_owned()),
        ("<space>mt", THEIRS.to_owned()),
        (
            "<space>mb",
            OURS.replace("    let a = 10;\n", "    let a = 10;\n    let a = 100;\n"),
        ),
        ("<space>mr", BASE.to_owned()),
        (":conflict-theirs<ret>", THEIRS.to_owned()),
        (":conflict-base<ret>", BASE.to_owned()),
    ] {
        h.keys("3gg");
        h.keys(keys);
        assert_eq!(h.doc().text, expected, "{keys}");
        assert!(h.doc().is_modified());
        h.keys("u");
        assert_eq!(h.doc().text, marked, "undo after {keys}");
        assert_eq!(conflicts(&h).len(), 1);
    }
}

#[test]
fn taking_the_base_needs_a_diff3_conflict() {
    let mut h = Harness::with("a.txt", "#[x|]#\n<<<<<<< HEAD\na\n=======\nb\n>>>>>>> x\n");
    h.keys("j<space>mr");
    assert!(matches!(h.state.notice, Some(Notice::NoConflictBase)));
    assert_eq!(conflicts(&h).len(), 1);
    h.keys("gg<space>mo");
    assert!(matches!(
        h.state.notice,
        Some(Notice::NoConflictUnderSelection)
    ));
}

const THREE: &str = "\
top
<<<<<<< HEAD
a
=======
b
>>>>>>> x
mid
<<<<<<< HEAD
c
=======
d
>>>>>>> x
mid
<<<<<<< HEAD
e
=======
f
>>>>>>> x
end
";

#[test]
fn conflict_navigation_takes_counts_and_extends_in_select_mode() {
    let mut h = Harness::with("a.txt", &format!("#[t|]#{}", &THREE[1..]));
    h.keys("]m");
    assert_eq!(selected(&h), (1, 5, 5));
    h.keys("]m");
    assert_eq!(selected(&h), (7, 11, 11));
    h.keys("[m");
    assert_eq!(
        selected(&h),
        (1, 5, 1),
        "backwards puts the cursor on the start"
    );
    h.keys("[m");
    assert!(matches!(h.state.notice, Some(Notice::NoConflicts)));
    assert_eq!(selected(&h), (1, 5, 1));

    h.keys("gg2]m");
    assert_eq!(selected(&h), (7, 11, 11));
    h.keys("gg9]m");
    assert_eq!(
        selected(&h),
        (13, 17, 17),
        "counts stop at the last conflict"
    );
    h.keys("ge2[m");
    assert_eq!(selected(&h), (7, 11, 7));

    h.keys("gg");
    h.keys("v]m]m");
    assert_eq!(
        selected(&h),
        (0, 11, 11),
        "select mode extends from the anchor"
    );
    h.keys("<esc>ge");
    h.keys("v[m");
    assert_eq!(selected(&h), (13, 17, 13));
}

#[test]
fn every_selected_conflict_resolves_in_one_edit() {
    let mut h = Harness::with("a.txt", &format!("#[t|]#{}", &THREE[1..]));
    let text = h.doc().text.clone();
    // Cursors in the first and last conflict; the middle one stays.
    let at = |line: usize| text.line_to_char(line);
    h.view_mut().selection = Selection::point(at(2)).push(Range::point(at(16)));
    h.keys("<space>mt");
    assert_eq!(
        h.doc().text,
        "top\nb\nmid\n<<<<<<< HEAD\nc\n=======\nd\n>>>>>>> x\nmid\nf\nend\n"
    );
    assert_eq!(conflicts(&h).len(), 1);
    h.keys("u");
    assert_eq!(h.doc().text, text, "one undo restores both");

    h.keys("%<space>mb");
    assert_eq!(h.doc().text, "top\na\nb\nmid\nc\nd\nmid\ne\nf\nend\n");
    assert!(conflicts(&h).is_empty());
}

#[test]
fn clicks_and_scrolling_count_the_label_rows() {
    let mut h = Harness::with("a.txt", &format!("#[t|]#{}", &THREE[1..]));
    let pane = h.state.layout.focus();
    scroll::measured(&mut h.state, pane, Viewport { rows: 8, cols: 40 });
    // Rows: top, ours label, <<<<<<<, a, =======, b, >>>>>>>, theirs label, mid.
    scroll::press(&mut h.state, pane, 3, 0, Press::Select(Unit::Char));
    assert_eq!(selected(&h), (2, 2, 2));
    scroll::press(&mut h.state, pane, 6, 0, Press::Select(Unit::Char));
    assert_eq!(selected(&h), (5, 5, 5));

    // Line 6 lands below the theirs label, inside the scrolloff band; without the label rows the
    // view would stop at line 2.
    h.keys("7gg");
    let anchor = h.view().scroll.anchor;
    assert_eq!(h.doc().text.char_to_line(anchor), 4);
}
