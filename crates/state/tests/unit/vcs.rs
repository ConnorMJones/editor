use std::collections::HashMap;

use git2::Status;
use model::git::{Change, GitStatus, Group, Kind};
use state::git::{Changed, GitSnapshot, aggregate, classify};
use state::path::RelPath;

fn p(path: &str) -> RelPath {
    RelPath::parse(path).unwrap()
}

#[test]
fn most_urgent_status_bubbles_to_every_ancestor() {
    let paths = HashMap::from([
        (p("src/a/x.rs"), Change::Untracked),
        (p("src/a/y.rs"), Change::Unstaged(Kind::Modified)),
        (p("src/b.rs"), Change::Staged(Kind::Added)),
        (p("docs/c.md"), Change::Conflicted),
        (p("target"), Change::Ignored),
    ]);
    let dirs = aggregate(&paths);
    assert_eq!(dirs.get("src/a"), Some(&GitStatus::Modified));
    assert_eq!(dirs.get("src"), Some(&GitStatus::Modified));
    assert_eq!(dirs.get("docs"), Some(&GitStatus::Conflicted));
    assert_eq!(dirs.get(""), Some(&GitStatus::Conflicted));
    assert_eq!(dirs.get("target"), None);
}

#[test]
fn ignored_dirs_cover_their_contents() {
    let snapshot = GitSnapshot::new(HashMap::from([
        (p("target"), Change::Ignored),
        (p("a.rs"), Change::Unstaged(Kind::Deleted)),
    ]));
    assert_eq!(snapshot.get(&p("target/debug/x")), Some(GitStatus::Ignored));
    assert_eq!(snapshot.get(&p("a.rs")), Some(GitStatus::Deleted));
    assert_eq!(snapshot.get(&RelPath::ROOT), Some(GitStatus::Deleted));
    assert_eq!(snapshot.get(&p("clean.rs")), None);
}

#[test]
fn status_bits_split_into_staged_and_unstaged_sides() {
    assert_eq!(
        classify(Status::INDEX_MODIFIED | Status::WT_MODIFIED),
        Some(Change::Both {
            staged: Kind::Modified,
            unstaged: Kind::Modified
        })
    );
    assert_eq!(
        classify(Status::INDEX_NEW),
        Some(Change::Staged(Kind::Added))
    );
    assert_eq!(classify(Status::WT_NEW), Some(Change::Untracked));
    assert_eq!(
        classify(Status::INDEX_DELETED | Status::WT_NEW),
        Some(Change::Both {
            staged: Kind::Deleted,
            unstaged: Kind::Added
        })
    );
    assert_eq!(
        classify(Status::CONFLICTED | Status::INDEX_MODIFIED),
        Some(Change::Conflicted)
    );
    assert_eq!(classify(Status::IGNORED), Some(Change::Ignored));
    assert_eq!(classify(Status::CURRENT), None);
}

#[test]
fn tree_colour_matches_the_old_single_status() {
    assert_eq!(Change::Staged(Kind::Added).status(), GitStatus::Added);
    assert_eq!(
        Change::Both {
            staged: Kind::Added,
            unstaged: Kind::Modified
        }
        .status(),
        GitStatus::Modified
    );
    assert_eq!(
        Change::Both {
            staged: Kind::Modified,
            unstaged: Kind::Deleted
        }
        .status(),
        GitStatus::Deleted
    );
    assert_eq!(
        Change::Unstaged(Kind::Renamed).status(),
        GitStatus::Modified
    );
}

#[test]
fn changes_list_in_sidebar_order() {
    let snapshot = GitSnapshot::new(HashMap::from([
        (p("z.rs"), Change::Untracked),
        (p("b.rs"), Change::Unstaged(Kind::Modified)),
        (
            p("a.rs"),
            Change::Both {
                staged: Kind::Modified,
                unstaged: Kind::Modified,
            },
        ),
        (p("m.rs"), Change::Conflicted),
        (p("target"), Change::Ignored),
    ]));
    let order: Vec<(Group, &str)> = snapshot
        .changes
        .iter()
        .map(|Changed { group, path }| (*group, path.as_str()))
        .collect();
    assert_eq!(
        order,
        [
            (Group::Conflicts, "m.rs"),
            (Group::Staged, "a.rs"),
            (Group::Changes, "a.rs"),
            (Group::Changes, "b.rs"),
            (Group::Changes, "z.rs"),
        ]
    );
}
