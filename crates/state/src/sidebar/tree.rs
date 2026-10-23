use std::collections::{HashMap, HashSet};
use std::io;
use std::path::{Path, PathBuf};

use slotmap::SlotMap;

use crate::job::FsOp;
use crate::project::path::{Folder, RelPath};
use crate::project::watch;
use crate::text::buffer::{Buffer, BufferId};
use crate::{Effect, State};
use model::command::Command;
use model::notice::Notice;

/// File explorer data; directories are listed lazily as they are expanded.
#[derive(Default)]
pub struct Tree {
    /// Folders expanded at least once.
    pub dirs: HashMap<Folder, Dir>,
    /// Visible rows in display order, rebuilt whenever `dirs` changes.
    pub rows: Vec<Row>,
    pub cursor: usize,
    pub unsaved: Unsaved,
}

pub struct Dir {
    pub fold: Fold,
    pub listing: Listing,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fold {
    Expanded,
    Collapsed,
}

pub enum Listing {
    Loading,
    Loaded(Vec<Entry>),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub name: String,
    pub kind: EntryKind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum EntryKind {
    Dir,
    File,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Row {
    pub path: RelPath,
    pub depth: u16,
    pub kind: RowKind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RowKind {
    File,
    Dir(Fold),
}

/// A file operation being typed in the tree's input line.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TreeEdit {
    /// Path relative to the root; with `NewEntry::Infer` a trailing `/` creates a directory.
    Create {
        line: String,
        kind: NewEntry,
    },
    Rename {
        from: RelPath,
        line: String,
    },
    /// Proceeds only when the line reads `y`.
    Delete {
        path: RelPath,
        kind: EntryKind,
        line: String,
    },
}

impl TreeEdit {
    pub fn line_mut(&mut self) -> &mut String {
        match self {
            TreeEdit::Create { line, .. }
            | TreeEdit::Rename { line, .. }
            | TreeEdit::Delete { line, .. } => line,
        }
    }
}

/// Directory entries sorted directories-first, then case-insensitively; `.git` is skipped.
pub fn list(root: &Path, dir: &Folder) -> io::Result<Vec<Entry>> {
    let mut entries = Vec::new();
    for entry in std::fs::read_dir(dir.to_path(root))? {
        let entry = entry?;
        let Ok(name) = entry.file_name().into_string() else {
            tracing::warn!(path = ?entry.path(), "skipping non-UTF-8 name");
            continue;
        };
        match name.as_str() {
            ".git" => continue,
            _ => {}
        }
        // Symlinks are shown as what they point to.
        let kind = match std::fs::metadata(entry.path()).map(|m| m.is_dir()) {
            Ok(true) => EntryKind::Dir,
            Ok(false) | Err(_) => EntryKind::File,
        };
        entries.push(Entry { name, kind });
    }
    entries.sort_unstable_by(|a, b| {
        a.kind.cmp(&b.kind).then_with(|| {
            a.name
                .chars()
                .flat_map(char::to_lowercase)
                .cmp(b.name.chars().flat_map(char::to_lowercase))
                .then_with(|| a.name.cmp(&b.name))
        })
    });
    Ok(entries)
}

/// Recomputes `rows`, keeping the cursor on the same path when it is still visible.
pub fn rebuild(tree: &mut Tree) {
    let selected = tree.rows.get(tree.cursor).map(|row| row.path.clone());
    tree.rows.clear();
    push_rows(&tree.dirs, &Folder::Root, 0, &mut tree.rows);
    tree.cursor = match selected {
        Some(path) => match tree.rows.iter().position(|row| row.path == path) {
            Some(index) => index,
            None => tree.cursor.min(tree.rows.len().saturating_sub(1)),
        },
        None => 0,
    };
}

fn push_rows(dirs: &HashMap<Folder, Dir>, dir: &Folder, depth: u16, rows: &mut Vec<Row>) {
    let Some(Dir {
        fold: Fold::Expanded,
        listing: Listing::Loaded(entries),
    }) = dirs.get(dir)
    else {
        return;
    };
    for entry in entries {
        let path = dir.join(&entry.name);
        match entry.kind {
            EntryKind::File => rows.push(Row {
                path,
                depth,
                kind: RowKind::File,
            }),
            EntryKind::Dir => {
                let fold = match dirs.get(path.as_str()) {
                    Some(Dir { fold, .. }) => *fold,
                    None => Fold::Collapsed,
                };
                rows.push(Row {
                    path: path.clone(),
                    depth,
                    kind: RowKind::Dir(fold),
                });
                push_rows(dirs, &Folder::Sub(path), depth + 1, rows);
            }
        }
    }
}

/// Files with unsaved edits and every directory above them, for the tree's markers.
#[derive(Default)]
pub struct Unsaved {
    /// Modified buffers as of the last sync.
    pub buffers: Vec<(BufferId, PathBuf)>,
    pub paths: HashSet<RelPath>,
}

/// Brings `unsaved` up to date; it only allocates when the set of modified buffers changed.
pub fn sync_unsaved(unsaved: &mut Unsaved, root: &Path, buffers: &SlotMap<BufferId, Buffer>) {
    let modified = buffers
        .iter()
        .filter(|(_, buffer)| buffer.doc.is_modified());
    let unchanged = modified.clone().count() == unsaved.buffers.len()
        && modified.clone().all(|(id, buffer)| {
            unsaved
                .buffers
                .iter()
                .any(|(known, path)| *known == id && *path == buffer.path)
        });
    match unchanged {
        true => return,
        false => {}
    }
    unsaved.buffers.clear();
    unsaved
        .buffers
        .extend(modified.map(|(id, buffer)| (id, buffer.path.clone())));
    unsaved.paths.clear();
    for (_, path) in &unsaved.buffers {
        let Ok(file) = RelPath::new(root, path) else {
            continue;
        };
        // The root's empty path is no folder under it, so `parse` skips it.
        unsaved.paths.extend(
            file.ancestors()
                .filter_map(|folder| RelPath::parse(folder).ok()),
        );
        unsaved.paths.insert(file);
    }
}

/// What a create prompt makes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NewEntry {
    /// A file, or a directory when the name ends in `/`.
    Infer,
    Folder,
}

/// Where a tree context menu or drop applies: a row, or the root below the last row.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TreeTarget {
    Row(usize),
    Root,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TreeAction {
    NewFile,
    NewFolder,
    Rename,
    Delete,
}

/// Takes a folder's listing; a folder that cannot be listed is forgotten.
pub fn listed(
    state: &mut State,
    dir: Folder,
    result: io::Result<Vec<Entry>>,
    out: &mut Vec<Effect>,
) {
    let entries = match result {
        Ok(entries) => entries,
        Err(error) => {
            tracing::warn!(%error, %dir, "cannot list directory");
            state.tree.dirs.remove(&dir);
            rebuild(&mut state.tree);
            return;
        }
    };
    out.push(Effect::Watch(vec![dir.to_path(state.project.root.path())]));
    // Forget expansion state of subdirectories that disappeared.
    state.tree.dirs.retain(|folder, _| match folder {
        Folder::Sub(path) if path.folder_path() == dir.as_str() => entries
            .iter()
            .any(|e| e.kind == EntryKind::Dir && e.name == path.name()),
        Folder::Sub(_) | Folder::Root => true,
    });
    match state.tree.dirs.get_mut(&dir) {
        Some(existing) => existing.listing = Listing::Loaded(entries),
        None => {
            state.tree.dirs.insert(
                dir,
                Dir {
                    fold: Fold::Collapsed,
                    listing: Listing::Loaded(entries),
                },
            );
        }
    }
    rebuild(&mut state.tree);
}

/// Opens a created file, follows a rename with buffers and folders, and reports what changed.
pub fn fs_done(state: &mut State, op: FsOp, result: io::Result<()>, out: &mut Vec<Effect>) {
    let root = state.project.root.path().to_owned();
    match result {
        Err(error) => {
            let path = match &op {
                FsOp::Create { path, .. } | FsOp::Remove { path, .. } => path,
                FsOp::Rename { from, .. } => from,
            };
            state.notice = Some(Notice::FileOpFailed {
                path: path.to_path(&root),
                error,
            });
            return;
        }
        Ok(()) => {}
    }
    let changed = match op {
        FsOp::Create { path, kind } => {
            let changed = vec![path.to_path(&root)];
            match kind {
                EntryKind::File => {
                    crate::apply::apply(state, Command::Open(path.to_path(&root)), out);
                    crate::view::panes::focus_editor(state);
                }
                EntryKind::Dir => {}
            }
            changed
        }
        FsOp::Rename { from, to } => {
            for buffer in state.buffers.values_mut() {
                match RelPath::new(&root, &buffer.path).map(|rel| rel.rebased(&from, &to)) {
                    Ok(Some(moved)) => buffer.path = moved.to_path(&root),
                    // Outside the root, or outside what moved.
                    Ok(None) | Err(_) => {}
                }
            }
            state.tree.dirs = std::mem::take(&mut state.tree.dirs)
                .into_iter()
                .map(|(folder, dir)| (folder.moved(&from, &to), dir))
                .collect();
            vec![from.to_path(&root), to.to_path(&root)]
        }
        FsOp::Remove { path, .. } => {
            state
                .tree
                .dirs
                .retain(|folder, _| !matches!(folder, Folder::Sub(dir) if dir.is_within(&path)));
            vec![path.to_path(&root)]
        }
    };
    rebuild(&mut state.tree);
    watch::changed(state, changed, out);
}
