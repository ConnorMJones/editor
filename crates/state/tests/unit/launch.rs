use state::{Launch, RootError};

#[test]
fn a_folder_is_its_own_root() {
    let dir = tempfile::tempdir().unwrap();
    match Launch::resolve(dir.path()).unwrap() {
        Launch::Folder(root) => assert_eq!(root.path(), dir.path().canonicalize().unwrap()),
        other => panic!("expected a folder, got {other:?}"),
    }
}

#[test]
fn a_file_in_a_repository_opens_on_the_repository() {
    let dir = tempfile::tempdir().unwrap();
    git2::Repository::init(dir.path()).unwrap();
    std::fs::create_dir(dir.path().join("src")).unwrap();
    let path = dir.path().join("src/main.rs");
    std::fs::write(&path, "fn main() {}\n").unwrap();
    match Launch::resolve(&path).unwrap() {
        Launch::File { root, file } => {
            assert_eq!(root.path(), dir.path().canonicalize().unwrap());
            assert_eq!(file, path.canonicalize().unwrap());
        }
        other => panic!("expected a file, got {other:?}"),
    }
}

#[test]
fn a_file_outside_a_repository_opens_on_its_folder() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("notes.md");
    std::fs::write(&path, "# notes\n").unwrap();
    match Launch::resolve(&path).unwrap() {
        Launch::File { root, .. } => assert_eq!(root.path(), dir.path().canonicalize().unwrap()),
        other => panic!("expected a file, got {other:?}"),
    }
}

#[test]
fn a_missing_path_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    assert!(matches!(
        Launch::resolve(&dir.path().join("missing")),
        Err(RootError::Resolve { .. })
    ));
}
