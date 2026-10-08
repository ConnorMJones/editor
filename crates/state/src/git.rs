use std::collections::HashMap;
use std::path::{Path, PathBuf};

use git2::{Repository, Status, StatusOptions};
use model::git::GitStatus;

use crate::path::RelPath;

/// Status of every non-clean path under the root, plus the status each directory inherits.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct GitSnapshot {
    /// As reported by git; ignored directories appear once without their contents.
    pub paths: HashMap<RelPath, GitStatus>,
    /// Most urgent non-ignored status among each directory's descendants.
    pub dirs: HashMap<RelPath, GitStatus>,
}

impl GitSnapshot {
    pub fn get(&self, path: &RelPath) -> Option<GitStatus> {
        match (self.paths.get(path), self.dirs.get(path)) {
            (Some(&status), _) | (None, Some(&status)) => Some(status),
            (None, None) => path
                .ancestors()
                .find_map(|ancestor| match self.paths.get(ancestor) {
                    Some(GitStatus::Ignored) => Some(GitStatus::Ignored),
                    Some(_) | None => None,
                }),
        }
    }
}

/// Recomputes after every refresh so parents always agree with `paths`.
pub fn aggregate(paths: &HashMap<RelPath, GitStatus>) -> HashMap<RelPath, GitStatus> {
    let mut dirs: HashMap<RelPath, GitStatus> = HashMap::new();
    for (path, &status) in paths {
        match status {
            GitStatus::Ignored => continue,
            GitStatus::Untracked
            | GitStatus::Added
            | GitStatus::Deleted
            | GitStatus::Modified
            | GitStatus::Conflicted => {}
        }
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

fn classify(status: Status) -> Option<GitStatus> {
    match status {
        s if s.is_conflicted() => Some(GitStatus::Conflicted),
        s if s.is_ignored() => Some(GitStatus::Ignored),
        s if s.intersects(Status::WT_DELETED | Status::INDEX_DELETED) => Some(GitStatus::Deleted),
        s if s.intersects(
            Status::WT_MODIFIED
                | Status::INDEX_MODIFIED
                | Status::WT_RENAMED
                | Status::INDEX_RENAMED
                | Status::WT_TYPECHANGE
                | Status::INDEX_TYPECHANGE,
        ) =>
        {
            Some(GitStatus::Modified)
        }
        s if s.contains(Status::INDEX_NEW) => Some(GitStatus::Added),
        s if s.contains(Status::WT_NEW) => Some(GitStatus::Untracked),
        _ => None,
    }
}

/// The repository's git dir and status; `Ok(None)` when `root` is not in a non-bare repository.
pub fn status(root: &Path) -> Result<Option<(PathBuf, GitSnapshot)>, git2::Error> {
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
        let (Ok(path), Some(status)) = (entry.path(), classify(entry.status())) else {
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
                    paths.insert(relative, status);
                }
                Ok(_) | Err(_) => {}
            },
            None => {}
        }
    }
    let dirs = aggregate(&paths);
    Ok(Some((repo.path().to_owned(), GitSnapshot { paths, dirs })))
}
