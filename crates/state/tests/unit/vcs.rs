use std::collections::HashMap;

use model::git::GitStatus;
use state::git::{GitSnapshot, aggregate};
use state::path::RelPath;

fn p(path: &str) -> RelPath {
    RelPath::parse(path).unwrap()
}

#[test]
fn most_urgent_status_bubbles_to_every_ancestor() {
    let paths = HashMap::from([
        (p("src/a/x.rs"), GitStatus::Untracked),
        (p("src/a/y.rs"), GitStatus::Modified),
        (p("src/b.rs"), GitStatus::Added),
        (p("docs/c.md"), GitStatus::Conflicted),
        (p("target"), GitStatus::Ignored),
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
    let paths = HashMap::from([
        (p("target"), GitStatus::Ignored),
        (p("a.rs"), GitStatus::Deleted),
    ]);
    let snapshot = GitSnapshot {
        dirs: aggregate(&paths),
        paths,
    };
    assert_eq!(snapshot.get(&p("target/debug/x")), Some(GitStatus::Ignored));
    assert_eq!(snapshot.get(&p("a.rs")), Some(GitStatus::Deleted));
    assert_eq!(snapshot.get(&RelPath::ROOT), Some(GitStatus::Deleted));
    assert_eq!(snapshot.get(&p("clean.rs")), None);
}
