use std::path::PathBuf;
use std::time::Duration;

use model::git::GitStatus;
use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher as _};

use crate::job::{Job, ReloadPolicy};
use crate::project::files::Files;
use crate::project::path::RelPath;
use crate::text::buffer::Content;
use crate::{Effect, Repo, State, project::backlinks, request_git};

/// Long-lived watcher owned by the host; dropping it stops events.
pub type Watcher = RecommendedWatcher;

/// How long the host waits for more events before handling a batch.
pub const DEBOUNCE: Duration = Duration::from_millis(100);

/// `sink` runs on the watcher's thread with the paths of each change; hosts batch them.
pub fn start(sink: impl Fn(Vec<PathBuf>) + Send + 'static) -> notify::Result<Watcher> {
    notify::recommended_watcher(move |result: notify::Result<Event>| match result {
        // Our own reads (listing, git status, previews) would otherwise retrigger refreshes.
        Ok(Event {
            kind: EventKind::Access(_),
            ..
        }) => {}
        Ok(event) => sink(event.paths),
        Err(error) => tracing::warn!(%error, "file watcher error"),
    })
}

/// Directories are watched one level deep so ignored trees such as `target/` cost nothing.
pub fn watch(watcher: &mut Watcher, dirs: &[PathBuf]) {
    for dir in dirs {
        match watcher.watch(dir, RecursiveMode::NonRecursive) {
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
    crate::render::typeset::invalidate(&mut state.previews, &paths);
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
    backlinks::changed(state, &rescan, out);
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
