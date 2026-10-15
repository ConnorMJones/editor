use std::path::Path;
use std::sync::Arc;

use ignore::WalkBuilder;

use crate::project::path::RelPath;

/// Every non-ignored, non-hidden file under the root, sorted; listed once per process.
pub enum Files {
    Loading,
    Ready(Arc<Vec<RelPath>>),
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct Walked {
    pub files: Vec<RelPath>,
    pub dirs: Vec<RelPath>,
}

/// Lists `dir` honouring gitignore and hidden-file rules; `depth` 1 lists direct children only.
pub fn walk(root: &Path, dir: &RelPath, depth: Option<usize>) -> Walked {
    let mut walked = Walked::default();
    let start = dir.to_path(root);
    for entry in WalkBuilder::new(&start).max_depth(depth).build() {
        let entry = match entry {
            Ok(entry) => entry,
            Err(error) => {
                tracing::warn!(%error, "skipping unreadable entry");
                continue;
            }
        };
        let (Some(kind), true) = (entry.file_type(), entry.depth() > 0) else {
            continue;
        };
        let path = match RelPath::new(root, entry.path()) {
            Ok(path) => path,
            Err(error) => {
                tracing::warn!(%error, "skipping path");
                continue;
            }
        };
        match kind.is_dir() {
            true => walked.dirs.push(path),
            false => walked.files.push(path),
        }
    }
    walked.files.sort_unstable();
    walked
}

/// Index changes derived from a batch of changed paths.
#[derive(Debug, Default)]
pub struct Rescan {
    /// Paths that no longer exist; files and whole directories alike.
    pub removed: Vec<RelPath>,
    /// Directories whose direct files are replaced by the listing.
    pub relisted: Vec<(RelPath, Walked)>,
    /// New directories whose whole contents are replaced by the walk.
    pub subtrees: Vec<(RelPath, Walked)>,
}

pub fn rescan(root: &Path, paths: &[RelPath]) -> Rescan {
    let mut rescan = Rescan::default();
    for path in paths {
        match std::fs::symlink_metadata(path.to_path(root)) {
            Err(_) => rescan.removed.push(path.clone()),
            Ok(_) => {}
        }
        let Some(parent) = path.parent() else {
            continue;
        };
        let parent = RelPath(parent.to_owned());
        match rescan.relisted.iter().any(|(dir, _)| *dir == parent) {
            true => {}
            false => {
                let listing = walk(root, &parent, Some(1));
                rescan.relisted.push((parent, listing));
            }
        }
    }
    for path in paths {
        let new_dir = rescan
            .relisted
            .iter()
            .any(|(_, listing)| listing.dirs.contains(path));
        match new_dir {
            true => {
                let subtree = walk(root, path, None);
                rescan.subtrees.push((path.clone(), subtree));
            }
            false => {}
        }
    }
    rescan
}

/// Whether a rescan changed the set of indexed files.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Changed {
    Yes,
    No,
}

pub fn apply_rescan(files: &mut Vec<RelPath>, rescan: Rescan) -> Changed {
    let dropped: Vec<RelPath> = files
        .extract_if(.., |file| {
            rescan
                .removed
                .iter()
                .any(|gone| file.is_within(gone.as_str()))
                || rescan
                    .relisted
                    .iter()
                    .any(|(dir, _)| file.parent() == Some(dir.as_str()))
                || rescan
                    .subtrees
                    .iter()
                    .any(|(dir, _)| file.is_within(dir.as_str()))
        })
        .collect();
    let mut added: Vec<RelPath> = Vec::new();
    for (_, walked) in rescan.relisted.into_iter().chain(rescan.subtrees) {
        added.extend(walked.files);
    }
    added.sort_unstable();
    added.dedup();
    let changed = match dropped == added {
        true => Changed::No,
        false => Changed::Yes,
    };
    files.extend(added);
    files.sort_unstable();
    files.dedup();
    changed
}
