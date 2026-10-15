use std::fs;

use state::project::path::RelPath;
use state::sidebar::tree::{self, Dir, Entry, EntryKind, Fold, Listing, RowKind, Tree};

fn entry(name: &str, kind: EntryKind) -> Entry {
    Entry {
        name: name.to_owned(),
        kind,
    }
}

#[test]
fn list_sorts_dirs_first_case_insensitively_and_skips_git() {
    let dir = tempfile::tempdir().unwrap();
    for name in ["b.txt", "A.txt", ".hidden"] {
        fs::write(dir.path().join(name), "").unwrap();
    }
    for name in ["zdir", "Adir", ".git"] {
        fs::create_dir(dir.path().join(name)).unwrap();
    }
    let entries = tree::list(dir.path(), &RelPath::ROOT).unwrap();
    assert_eq!(
        entries,
        [
            entry("Adir", EntryKind::Dir),
            entry("zdir", EntryKind::Dir),
            entry(".hidden", EntryKind::File),
            entry("A.txt", EntryKind::File),
            entry("b.txt", EntryKind::File),
        ]
    );
}

#[test]
fn rebuild_walks_expanded_dirs_and_keeps_cursor() {
    let mut t = Tree::default();
    t.dirs.insert(
        RelPath::ROOT,
        Dir {
            fold: Fold::Expanded,
            listing: Listing::Loaded(vec![
                entry("src", EntryKind::Dir),
                entry("docs", EntryKind::Dir),
                entry("main.rs", EntryKind::File),
            ]),
        },
    );
    t.dirs.insert(
        RelPath::parse("src").unwrap(),
        Dir {
            fold: Fold::Expanded,
            listing: Listing::Loaded(vec![entry("lib.rs", EntryKind::File)]),
        },
    );
    tree::rebuild(&mut t);
    let rows: Vec<(&str, u16, RowKind)> = t
        .rows
        .iter()
        .map(|r| (r.path.as_str(), r.depth, r.kind))
        .collect();
    assert_eq!(
        rows,
        [
            ("src", 0, RowKind::Dir(Fold::Expanded)),
            ("src/lib.rs", 1, RowKind::File),
            ("docs", 0, RowKind::Dir(Fold::Collapsed)),
            ("main.rs", 0, RowKind::File),
        ]
    );

    t.cursor = 3;
    t.dirs
        .get_mut(&RelPath::parse("src").unwrap())
        .unwrap()
        .fold = Fold::Collapsed;
    tree::rebuild(&mut t);
    assert_eq!(t.rows.len(), 3);
    assert_eq!(t.rows[t.cursor].path.as_str(), "main.rs");
}

#[test]
fn loading_dir_shows_no_children() {
    let mut t = Tree::default();
    t.dirs.insert(
        RelPath::ROOT,
        Dir {
            fold: Fold::Expanded,
            listing: Listing::Loaded(vec![entry("src", EntryKind::Dir)]),
        },
    );
    t.dirs.insert(
        RelPath::parse("src").unwrap(),
        Dir {
            fold: Fold::Expanded,
            listing: Listing::Loading,
        },
    );
    tree::rebuild(&mut t);
    assert_eq!(t.rows.len(), 1);
    assert_eq!(t.rows[0].kind, RowKind::Dir(Fold::Expanded));
}
