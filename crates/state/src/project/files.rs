use std::path::{Path, PathBuf};
use std::sync::Arc;

use ignore::WalkBuilder;

use crate::input::picker::{self, Items};
use crate::project::Project;
use crate::project::backlinks::{self, Backlinks};
use crate::project::links::Index;
use crate::project::path::{Folder, RelPath};
use crate::{Effect, Overlay, State};

/// What is known of the files under the root.
pub enum FileIndex {
    /// The first walk runs; what was asked for meanwhile starts when it finishes.
    Loading {
        then: OnIndexed,
    },
    Ready(Indexed),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OnIndexed {
    Nothing,
    BuildBacklinks,
}

/// The files under the root and what is derived from them.
pub struct Indexed {
    /// Every non-ignored, non-hidden file, sorted.
    pub files: Arc<Vec<RelPath>>,
    /// Bumped whenever `files` changes; parses note the generation their links resolved in.
    pub generation: u64,
    /// Built on first use after `files` changed.
    pub links: Option<Index>,
    pub backlinks: Backlinks,
}

impl Indexed {
    pub fn new(files: Arc<Vec<RelPath>>) -> Indexed {
        Indexed {
            files,
            generation: 1,
            links: None,
            backlinks: Backlinks::default(),
        }
    }

    /// The link index for the current files, and the backlinks read beside it.
    pub fn links(&mut self) -> (&Index, &mut Backlinks) {
        let Indexed {
            files,
            generation,
            links,
            backlinks,
        } = self;
        let index = links.get_or_insert_with(|| Index::new(*generation, files.clone()));
        (index, backlinks)
    }

    /// The files changed: links resolve again on next use.
    pub fn changed(&mut self) {
        self.generation += 1;
        self.links = None;
    }
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct Walked {
    pub files: Vec<RelPath>,
    pub dirs: Vec<RelPath>,
}

/// Honours gitignore and hidden-file rules; `depth` 1 lists direct children only.
pub fn walk(root: &Path, dir: &Folder, depth: Option<usize>) -> Walked {
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
    pub relisted: Vec<(Folder, Walked)>,
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
        let folder = path.folder();
        match rescan.relisted.iter().any(|(dir, _)| *dir == folder) {
            true => {}
            false => {
                let listing = walk(root, &folder, Some(1));
                rescan.relisted.push((folder, listing));
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
                let subtree = walk(root, &Folder::Sub(path.clone()), None);
                rescan.subtrees.push((path.clone(), subtree));
            }
            false => {}
        }
    }
    rescan
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Changed {
    Yes,
    No,
}

pub fn apply_rescan(files: &mut Vec<RelPath>, rescan: Rescan) -> Changed {
    let dropped: Vec<RelPath> = files
        .extract_if(.., |file| {
            rescan.removed.iter().any(|gone| file.is_within(gone))
                || rescan
                    .relisted
                    .iter()
                    .any(|(dir, _)| file.folder_path() == dir.as_str())
                || rescan.subtrees.iter().any(|(dir, _)| file.is_within(dir))
        })
        .collect();
    let mut added: Vec<RelPath> = Vec::new();
    let relisted = rescan.relisted.into_iter().map(|(_, walked)| walked);
    for walked in relisted.chain(rescan.subtrees.into_iter().map(|(_, walked)| walked)) {
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

/// Takes a finished walk of the root: watches its folders and refills a file picker.
pub fn indexed(state: &mut State, walked: Walked, out: &mut Vec<Effect>) {
    let root = state.project.root.path();
    let dirs = walked.dirs.iter().map(|dir| dir.to_path(root));
    out.push(Effect::Watch(
        std::iter::once(root.to_owned()).chain(dirs).collect(),
    ));
    let files = Arc::new(walked.files);
    match &mut state.overlay {
        Overlay::Picker(picker) => match &mut picker.items {
            Items::Files(items) => {
                *items = files.clone();
                picker::filter(picker, &mut state.matcher, picker::Narrowing::All);
            }
            Items::Images(items) => {
                *items = picker::images(&files);
                picker::filter(picker, &mut state.matcher, picker::Narrowing::All);
            }
            Items::Buffers(_)
            | Items::Commands
            | Items::Grep(_)
            | Items::Backlinks(_)
            | Items::Outline(_) => {}
        },
        Overlay::None | Overlay::Prompt(_) | Overlay::Confirm(_) => {}
    }
    let Project { root, index, .. } = &mut state.project;
    match index {
        FileIndex::Loading { then } => {
            let then = *then;
            *index = FileIndex::Ready(Indexed::new(files));
            match then {
                OnIndexed::Nothing => {}
                OnIndexed::BuildBacklinks => backlinks::ensure(state, out),
            }
        }
        // A second walk keeps what the first built and resolves links again.
        FileIndex::Ready(indexed) => {
            indexed.files = files;
            indexed.changed();
            backlinks::reconcile(indexed, root.path(), out);
        }
    }
    crate::view::panes::request_preview(state, out);
}

/// Takes a rescan of paths the watcher reported.
pub fn rescanned(state: &mut State, rescan: Rescan, out: &mut Vec<Effect>) {
    let root = state.project.root.path();
    let new_dirs: Vec<PathBuf> = rescan
        .subtrees
        .iter()
        .flat_map(|(dir, walked)| std::iter::once(dir).chain(&walked.dirs))
        .map(|dir| dir.to_path(root))
        .collect();
    match new_dirs.is_empty() {
        true => {}
        false => out.push(Effect::Watch(new_dirs)),
    }
    // Before the first walk ends, the walk itself sees these changes.
    let FileIndex::Ready(indexed) = &mut state.project.index else {
        return;
    };
    match apply_rescan(Arc::make_mut(&mut indexed.files), rescan) {
        Changed::Yes => {
            indexed.changed();
            backlinks::reconcile(indexed, root, out);
        }
        Changed::No => {}
    }
}
