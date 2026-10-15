pub mod changes;
pub mod conflict;
pub mod diff;
pub mod diffview;

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use git2::build::CheckoutBuilder;
use git2::{ErrorCode, IndexEntry, IndexTime, ObjectType, Oid, Repository, Status, StatusOptions};
use helix_core::Rope;
use model::git::{Change, GitStatus, Group, Kind};

use crate::git::diff::{Base, BaseRead};
use crate::project::path::RelPath;

/// Change of every non-clean path under the root, plus the status each directory inherits.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct GitSnapshot {
    /// As reported by git; ignored directories appear once without their contents.
    pub paths: HashMap<RelPath, Change>,
    /// Most urgent non-ignored status among each directory's descendants.
    pub dirs: HashMap<RelPath, GitStatus>,
    /// Changed files in sidebar order: by group, then path; a partly staged file appears twice.
    pub changes: Vec<Changed>,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Changed {
    pub group: Group,
    pub path: RelPath,
}

impl GitSnapshot {
    pub fn new(paths: HashMap<RelPath, Change>) -> GitSnapshot {
        let mut changes: Vec<Changed> = paths
            .iter()
            .flat_map(|(path, change)| {
                change.groups().iter().map(|&group| Changed {
                    group,
                    path: path.clone(),
                })
            })
            .collect();
        changes.sort_unstable();
        GitSnapshot {
            dirs: aggregate(&paths),
            paths,
            changes,
        }
    }

    pub fn get(&self, path: &RelPath) -> Option<GitStatus> {
        match (self.paths.get(path), self.dirs.get(path)) {
            (Some(change), _) => Some(change.status()),
            (None, Some(&status)) => Some(status),
            (None, None) => path
                .ancestors()
                .find_map(|ancestor| match self.paths.get(ancestor) {
                    Some(Change::Ignored) => Some(GitStatus::Ignored),
                    Some(_) | None => None,
                }),
        }
    }
}

/// Recomputes after every refresh so parents always agree with `paths`.
pub fn aggregate(paths: &HashMap<RelPath, Change>) -> HashMap<RelPath, GitStatus> {
    let mut dirs: HashMap<RelPath, GitStatus> = HashMap::new();
    for (path, change) in paths {
        let status = match change.status() {
            GitStatus::Ignored => continue,
            status @ (GitStatus::Untracked
            | GitStatus::Added
            | GitStatus::Deleted
            | GitStatus::Modified
            | GitStatus::Conflicted) => status,
        };
        for ancestor in path.ancestors() {
            match dirs.get_mut(ancestor) {
                Some(existing) if *existing >= status => break,
                Some(existing) => *existing = status,
                None => {
                    dirs.insert(RelPath(ancestor.to_owned()), status);
                }
            }
        }
    }
    dirs
}

/// Splits git2's bits into the HEAD-to-index and index-to-worktree sides.
pub fn classify(status: Status) -> Option<Change> {
    let side = |new, modified, deleted, renamed, typechange| match status {
        s if s.intersects(deleted) => Some(Kind::Deleted),
        s if s.intersects(renamed) => Some(Kind::Renamed),
        s if s.intersects(typechange) => Some(Kind::TypeChange),
        s if s.intersects(modified) => Some(Kind::Modified),
        s if s.intersects(new) => Some(Kind::Added),
        _ => None,
    };
    let staged = side(
        Status::INDEX_NEW,
        Status::INDEX_MODIFIED,
        Status::INDEX_DELETED,
        Status::INDEX_RENAMED,
        Status::INDEX_TYPECHANGE,
    );
    let unstaged = side(
        Status::WT_NEW,
        Status::WT_MODIFIED,
        Status::WT_DELETED,
        Status::WT_RENAMED,
        Status::WT_TYPECHANGE,
    );
    match (status, staged, unstaged) {
        (s, ..) if s.is_conflicted() => Some(Change::Conflicted),
        (s, ..) if s.is_ignored() => Some(Change::Ignored),
        (_, None, Some(Kind::Added)) => Some(Change::Untracked),
        (_, Some(staged), Some(unstaged)) => Some(Change::Both { staged, unstaged }),
        (_, Some(staged), None) => Some(Change::Staged(staged)),
        (_, None, Some(unstaged)) => Some(Change::Unstaged(unstaged)),
        (_, None, None) => None,
    }
}

/// A repository holding the root.
pub struct Found {
    pub git_dir: PathBuf,
    /// Canonical, so buffer paths strip it directly.
    pub workdir: PathBuf,
    pub snapshot: GitSnapshot,
}

/// The repository and status; `Ok(None)` when `root` is not in a non-bare repository.
pub fn status(root: &Path) -> Result<Option<Found>, git2::Error> {
    let repo = match Repository::discover(root) {
        Ok(repo) => repo,
        Err(error) if error.code() == git2::ErrorCode::NotFound => return Ok(None),
        Err(error) => return Err(error),
    };
    let Some(workdir) = repo.workdir() else {
        return Ok(None);
    };
    let workdir = workdir
        .canonicalize()
        .map_err(|e| git2::Error::from_str(&e.to_string()))?;
    let Ok(prefix) = RelPath::new(&workdir, root) else {
        return Ok(None);
    };
    let mut options = StatusOptions::new();
    options
        .include_untracked(true)
        .recurse_untracked_dirs(true)
        .include_ignored(true)
        .recurse_ignored_dirs(false)
        .exclude_submodules(true);
    let statuses = repo.statuses(Some(&mut options))?;
    let mut paths = HashMap::with_capacity(statuses.len());
    for entry in statuses.iter() {
        let (Ok(path), Some(change)) = (entry.path(), classify(entry.status())) else {
            continue;
        };
        let path = path.trim_end_matches('/');
        let relative = match prefix.is_root() {
            true => Some(path),
            false => path
                .strip_prefix(prefix.as_str())
                .and_then(|rest| rest.strip_prefix('/')),
        };
        match relative {
            Some(relative) => match RelPath::parse(relative) {
                Ok(relative) if !relative.is_root() => {
                    paths.insert(relative, change);
                }
                Ok(_) | Err(_) => {}
            },
            None => {}
        }
    }
    Ok(Some(Found {
        git_dir: repo.path().to_owned(),
        workdir,
        snapshot: GitSnapshot::new(paths),
    }))
}

/// Reads `path`'s HEAD and index blobs; a side whose id is still `known` is not read again.
/// A conflicted path compares its index side against HEAD.
pub fn read_bases(
    git_dir: &Path,
    path: &RepoPath,
    known: [Option<Oid>; 2],
) -> Result<[BaseRead; 2], git2::Error> {
    let repo = Repository::open(git_dir)?;
    let file = Path::new(path.0.as_str());
    let head = match repo.head().and_then(|head| head.peel_to_tree()) {
        Ok(tree) => match tree.get_path(file) {
            Ok(entry) => match entry.kind() {
                Some(ObjectType::Blob) => Some(entry.id()),
                Some(_) | None => None,
            },
            Err(error) if error.code() == ErrorCode::NotFound => None,
            Err(error) => return Err(error),
        },
        Err(error) if matches!(error.code(), ErrorCode::UnbornBranch | ErrorCode::NotFound) => None,
        Err(error) => return Err(error),
    };
    let index = repo.index()?;
    let staged = match (index.get_path(file, 0), index.get_path(file, 2)) {
        (Some(entry), _) => Some(entry.id),
        (None, Some(_)) => head,
        (None, None) => None,
    };
    let read = |oid: Option<Oid>, known: Option<Oid>| match (oid, oid == known) {
        (_, true) => Ok(BaseRead::Kept),
        (None, false) => Ok(BaseRead::Fresh(Base::Absent)),
        (Some(oid), false) => repo.find_blob(oid).map(|blob| {
            BaseRead::Fresh(Base::Blob {
                oid,
                text: Rope::from(String::from_utf8_lossy(blob.content()).as_ref()),
            })
        }),
    };
    Ok([read(head, known[0])?, read(staged, known[1])?])
}

/// Index changes made for hunk and file commands.
#[derive(Clone, Debug)]
pub enum IndexOp {
    /// Replaces the staged text, provided the entry still holds `expected`.
    Write { expected: Option<Oid>, text: Rope },
    /// Stages the worktree file, or its deletion.
    Stage,
    /// Resets the entry to HEAD.
    Unstage,
    /// Restores the worktree file from the index.
    Checkout,
}

#[derive(Debug, thiserror::Error)]
pub enum IndexError {
    #[error(transparent)]
    Git(#[from] git2::Error),
    #[error("the index entry changed since the diff")]
    Changed,
}

pub fn change_index(git_dir: &Path, path: &RepoPath, op: &IndexOp) -> Result<(), IndexError> {
    let repo = Repository::open(git_dir)?;
    let file = Path::new(path.0.as_str());
    let mut index = repo.index()?;
    match op {
        IndexOp::Write { expected, text } => {
            let current = index.get_path(file, 0);
            match current.as_ref().map(|entry| entry.id) == *expected {
                true => {}
                false => return Err(IndexError::Changed),
            }
            let zero = IndexTime::new(0, 0);
            // Zeroed stat data makes git re-read the worktree file instead of trusting the entry.
            let entry = match current {
                Some(entry) => IndexEntry {
                    ctime: zero,
                    mtime: zero,
                    dev: 0,
                    ino: 0,
                    uid: 0,
                    gid: 0,
                    file_size: 0,
                    ..entry
                },
                None => IndexEntry {
                    ctime: zero,
                    mtime: zero,
                    dev: 0,
                    ino: 0,
                    mode: 0o100644,
                    uid: 0,
                    gid: 0,
                    file_size: 0,
                    id: Oid::ZERO_SHA1,
                    flags: 0,
                    flags_extended: 0,
                    path: path.0.as_str().as_bytes().to_vec(),
                },
            };
            let mut bytes = Vec::with_capacity(text.len_bytes());
            for chunk in text.chunks() {
                bytes.extend_from_slice(chunk.as_bytes());
            }
            index.add_frombuffer(&entry, &bytes)?;
            index.write()?;
        }
        IndexOp::Stage => {
            let workdir = repo
                .workdir()
                .ok_or_else(|| git2::Error::from_str("repository has no worktree"))?;
            match workdir.join(file).exists() {
                true => index.add_path(file)?,
                false => index.remove_path(file)?,
            }
            index.write()?;
        }
        IndexOp::Unstage => {
            let head = match repo.head().and_then(|head| head.peel_to_commit()) {
                Ok(commit) => Some(commit.into_object()),
                Err(error)
                    if matches!(error.code(), ErrorCode::UnbornBranch | ErrorCode::NotFound) =>
                {
                    None
                }
                Err(error) => return Err(error.into()),
            };
            match head {
                Some(head) => repo.reset_default(Some(&head), [file])?,
                None => {
                    index.remove_path(file)?;
                    index.write()?;
                }
            }
        }
        IndexOp::Checkout => {
            let mut checkout = CheckoutBuilder::new();
            checkout.force().path(file);
            repo.checkout_index(Some(&mut index), Some(&mut checkout))?;
        }
    }
    Ok(())
}

/// A `/`-separated path relative to the repository's worktree, which may lie above the root.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct RepoPath(pub RelPath);
