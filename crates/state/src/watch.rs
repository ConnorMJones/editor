use std::path::PathBuf;
use std::time::Duration;

use model::git::GitStatus;
use notify_debouncer_mini::notify::{RecommendedWatcher, RecursiveMode};
use notify_debouncer_mini::{DebounceEventResult, Debouncer, new_debouncer};

use crate::buffer::Content;
use crate::files::Files;
use crate::job::{Job, ReloadPolicy};
use crate::path::RelPath;
use crate::{Effect, Repo, State, request_git};

/// Long-lived watcher owned by the host; dropping it stops events.
pub type Watcher = Debouncer<RecommendedWatcher>;

const DEBOUNCE: Duration = Duration::from_millis(100);

/// `sink` runs on the watcher's thread with each debounced batch of absolute paths.
pub fn start(
    sink: impl Fn(Vec<PathBuf>) + Send + 'static,
) -> Result<Watcher, notify_debouncer_mini::notify::Error> {
    new_debouncer(DEBOUNCE, move |result: DebounceEventResult| match result {
        Ok(events) => sink(events.into_iter().map(|event| event.path).collect()),
        Err(error) => tracing::warn!(%error, "file watcher error"),
    })
}

/// Directories are watched one level deep so ignored trees such as `target/` cost nothing.
pub fn watch(watcher: &mut Watcher, dirs: &[PathBuf]) {
    for dir in dirs {
        match watcher.watcher().watch(dir, RecursiveMode::NonRecursive) {
            Ok(()) => {}
            Err(error) => tracing::warn!(%error, ?dir, "cannot watch directory"),
        }
    }
}

/// Routes changed paths to buffer reloads, tree relistings, index rescans and git refreshes.
pub fn changed(state: &mut State, paths: Vec<PathBuf>, out: &mut Vec<Effect>) {
    let root = state.root.path().to_owned();
    let mut refresh_git = false;
    let mut rescan: Vec<RelPath> = Vec::new();
    let mut relist: Vec<RelPath> = Vec::new();
    for path in paths {
        for (id, buffer) in state.buffers.iter() {
            // A loading buffer's pending read already sees the change.
            match (&buffer.content, buffer.path == path) {
                (Content::Text { mtime, .. }, true) => out.push(Effect::Job(Job::Reload {
                    buffer: id,
                    path: path.clone(),
                    mtime: *mtime,
                    policy: ReloadPolicy::KeepEdits,
                })),
                (Content::Text { .. } | Content::Loading { .. }, _) => {}
            }
        }
        match &state.git.repo {
            Repo::Found { git_dir, .. } if path.starts_with(git_dir) => {
                refresh_git = true;
                continue;
            }
            Repo::Found { .. } | Repo::None | Repo::Unknown => {}
        }
        let Ok(rel) = RelPath::new(&root, &path) else {
            continue;
        };
        let parent = rel.parent().map(|p| RelPath(p.to_owned()));
        match parent {
            Some(parent) if state.tree.dirs.contains_key(&parent) && !relist.contains(&parent) => {
                relist.push(parent)
            }
            Some(_) | None => {}
        }
        let ignored = match &state.git.repo {
            Repo::Found { snapshot, .. } => snapshot.get(&rel) == Some(GitStatus::Ignored),
            Repo::None | Repo::Unknown => false,
        };
        let hidden = rel.as_str().split('/').any(|part| part.starts_with('.'));
        match (ignored, hidden) {
            (true, _) => {}
            (false, true) => refresh_git = true,
            (false, false) => {
                refresh_git = true;
                rescan.push(rel);
            }
        }
    }
    for dir in relist {
        out.push(Effect::Job(Job::ListDir {
            root: root.clone(),
            dir,
        }));
    }
    match (&state.files, rescan.is_empty()) {
        (Files::Ready(_), false) => out.push(Effect::Job(Job::Rescan {
            root,
            paths: rescan,
        })),
        (Files::Ready(_), true) | (Files::Loading, _) => {}
    }
    match refresh_git {
        true => request_git(state, out),
        false => {}
    }
}
