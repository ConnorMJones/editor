use std::path::Path;

use state::project::files::{self, Changed, Rescan, Walked};
use state::project::path::{Folder, RelPath, RelPathError};

fn p(path: &str) -> RelPath {
    RelPath::parse(path).unwrap()
}

#[test]
fn relative_to_root() {
    let root = Path::new("/r");
    assert_eq!(
        RelPath::new(root, Path::new("/r/a/b")).unwrap().as_str(),
        "a/b"
    );
    assert_eq!(RelPath::new(root, Path::new("/r")), Err(RelPathError::Root));
    assert_eq!(RelPath::parse(""), Err(RelPathError::Root));
    assert!(matches!(
        RelPath::new(root, Path::new("/elsewhere")),
        Err(RelPathError::Outside(_))
    ));
    assert!(RelPath::parse("a/../b").is_err());
    assert!(RelPath::parse("/a").is_err());
}

#[test]
fn components() {
    let path = p("a/b/c.rs");
    assert_eq!(path.name(), "c.rs");
    assert_eq!(path.folder(), Folder::Sub(p("a/b")));
    assert_eq!(path.ancestors().collect::<Vec<_>>(), ["a/b", "a", ""]);
    assert_eq!(p("x").folder(), Folder::Root);
    assert_eq!(p("x").folder_path(), "");
    assert!(path.is_within(&p("a/b")));
    assert!(!p("ab/c").is_within(&p("a")));
    assert!(Folder::Root.contains(&path));
    assert_eq!(path.rebase(&p("a"), &p("z")).as_str(), "z/b/c.rs");
    assert_eq!(p("a").rebase(&p("a"), &p("z")).as_str(), "z");
}

#[test]
fn rescan_replaces_direct_children_and_subtrees() {
    let mut index = vec![
        p("a/old.rs"),
        p("a/sub/keep.rs"),
        p("b/gone/x.rs"),
        p("c.rs"),
    ];
    let changed = files::apply_rescan(
        &mut index,
        Rescan {
            removed: vec![p("b/gone")],
            relisted: vec![(
                Folder::Sub(p("a")),
                Walked {
                    files: vec![p("a/new.rs")],
                    dirs: vec![p("a/sub")],
                },
            )],
            subtrees: vec![(
                p("d"),
                Walked {
                    files: vec![p("d/e/f.rs")],
                    dirs: vec![p("d/e")],
                },
            )],
        },
    );
    assert_eq!(
        index,
        [p("a/new.rs"), p("a/sub/keep.rs"), p("c.rs"), p("d/e/f.rs")]
    );
    assert_eq!(changed, Changed::Yes);
    // Relisting a folder whose files stayed the same changes nothing.
    let unchanged = files::apply_rescan(
        &mut index,
        Rescan {
            removed: Vec::new(),
            relisted: vec![(
                Folder::Sub(p("a")),
                Walked {
                    files: vec![p("a/new.rs")],
                    dirs: vec![p("a/sub")],
                },
            )],
            subtrees: Vec::new(),
        },
    );
    assert_eq!(unchanged, Changed::No);
    assert_eq!(
        index,
        [p("a/new.rs"), p("a/sub/keep.rs"), p("c.rs"), p("d/e/f.rs")]
    );
}
