use std::collections::HashSet;
use std::path::PathBuf;
use std::time::Duration;

use model::git::GitStatus;
use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher as _};

use crate::job::{Job, ReloadIf, ReloadPolicy};
use crate::project::files::FileIndex;
use crate::project::path::{Folder, RelPath, RelPathError};
use crate::text::buffer::Disk;
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
    let root = state.project.root.path().to_owned();
    let mut refresh_git = false;
    let mut rescan: Vec<RelPath> = Vec::new();
    let mut relist: HashSet<Folder> = HashSet::new();
    crate::render::typeset::invalidate(&mut state.compiles, &paths);
    for path in paths {
        for (id, buffer) in (state.buffers.iter()).filter(|(_, buffer)| buffer.path == path) {
            out.push(Effect::Job(Job::Reload {
                buffer: id,
                path: path.clone(),
                when: match buffer.disk {
                    Disk::Synced { mtime } | Disk::ChangedExternally { mtime } => {
                        ReloadIf::ChangedSince(mtime)
                    }
                    Disk::New | Disk::Deleted => ReloadIf::Always,
                },
                policy: ReloadPolicy::KeepEdits,
            }));
        }
        match &state.project.git {
            Repo::Found { git_dir, .. } if path.starts_with(git_dir) => {
                refresh_git = true;
                continue;
            }
            Repo::Found { .. } | Repo::None | Repo::Unknown => {}
        }
        let rel = match RelPath::new(&root, &path) {
            Ok(rel) => rel,
            // The entries that changed under the root report their own paths.
            Err(RelPathError::Root) => continue,
            Err(error @ (RelPathError::Outside(_) | RelPathError::NotUtf8(_))) => {
                tracing::debug!(%error, "ignoring a watched path");
                continue;
            }
        };
        relist.insert(rel.folder());
        let ignored = match &state.project.git {
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
    for dir in relist
        .into_iter()
        .filter(|dir| state.tree.dirs.contains_key(dir))
    {
        out.push(Effect::Job(Job::ListDir {
            root: root.clone(),
            dir,
        }));
    }
    match &mut state.project.index {
        FileIndex::Ready(indexed) => backlinks::changed(
            &mut indexed.backlinks,
            state.project.root.path(),
            &rescan,
            out,
        ),
        // The first walk reads every note once it ends.
        FileIndex::Loading { .. } => {}
    }
    match (&state.project.index, rescan.is_empty()) {
        (FileIndex::Ready(_), false) => out.push(Effect::Job(Job::Rescan {
            root,
            paths: rescan,
        })),
        (FileIndex::Ready(_), true) | (FileIndex::Loading { .. }, _) => {}
    }
    match refresh_git {
        true => request_git(state, out),
        false => {}
    }
}
