//! Base reads and diff jobs that keep each tracked buffer's hunks current.

use crate::git::diff::{Base, BaseRead, Bases, Compute, Finished, Lines, Outcome, Stamp, Tracked};
use crate::git::navigate::land;
use crate::git::{RepoPath, locate};
use crate::job::Job;
use crate::text::buffer::{BufferId, Content};
use crate::{Effect, Refresh, Repo, State};

/// Starts a base read for `buffer`, or queues one behind the running read.
pub fn request_bases(state: &mut State, buffer: BufferId, out: &mut Vec<Effect>) {
    let Some(entry) = state.buffers.get_mut(buffer) else {
        return;
    };
    let Content::Text { diff, .. } = &mut entry.content else {
        return;
    };
    match (&state.project.git, locate(&state.project.git, &entry.path)) {
        // The first status refresh requests every buffer's bases.
        (Repo::Unknown, _) => {}
        (Repo::None | Repo::Found { .. }, None) => diff.bases = Bases::NoRepo,
        (Repo::None | Repo::Found { .. }, Some((git_dir, path))) => {
            diff.read = match diff.read {
                Refresh::Idle => {
                    let known = match &diff.bases {
                        Bases::Tracked(tracked) => [tracked.head.oid(), tracked.index.oid()],
                        Bases::NoRepo | Bases::Loading { .. } | Bases::Untracked => [None, None],
                    };
                    out.push(Effect::Job(Job::DiffBase {
                        buffer,
                        git_dir,
                        path,
                        known,
                    }));
                    Refresh::Running
                }
                Refresh::Running | Refresh::Queued => Refresh::Queued,
            };
        }
    }
}

/// Re-reads the bases of every text buffer, as after a status refresh.
pub fn request_all_bases(state: &mut State, out: &mut Vec<Effect>) {
    let ids: Vec<BufferId> = state.buffers.keys().collect();
    for buffer in ids {
        request_bases(state, buffer, out);
    }
}

/// Takes the bases read for `buffer` at repository `path`.
pub fn bases_read(
    state: &mut State,
    buffer: BufferId,
    path: RepoPath,
    result: Result<[BaseRead; 2], git2::Error>,
    out: &mut Vec<Effect>,
) {
    let Some(entry) = state.buffers.get_mut(buffer) else {
        return;
    };
    let Content::Text { diff, .. } = &mut entry.content else {
        return;
    };
    let again = std::mem::replace(&mut diff.read, Refresh::Idle);
    let result = match (result, locate(&state.project.git, &entry.path)) {
        (Ok(read), Some((_, now))) if now == path => Ok(read),
        // The buffer was renamed or left the repository while the read ran.
        (Ok(_), Some(_) | None) => return request_bases(state, buffer, out),
        (Err(error), _) => Err(error),
    };
    match result {
        Err(error) => tracing::warn!(%error, "reading diff bases failed"),
        Ok([head, index]) => {
            // A kept side is only good while the state still holds that blob.
            let resolve = |read: BaseRead, held: Option<&Base>| match read {
                BaseRead::Fresh(base) => Some(base),
                BaseRead::Kept(oid) => held.filter(|base| base.oid() == Some(oid)).cloned(),
            };
            let resolved = match &diff.bases {
                Bases::Tracked(tracked) => (
                    resolve(head, Some(&tracked.head)),
                    resolve(index, Some(&tracked.index)),
                ),
                Bases::NoRepo | Bases::Loading { .. } | Bases::Untracked => {
                    (resolve(head, None), resolve(index, None))
                }
            };
            match resolved {
                (Some(Base::Absent), Some(Base::Absent)) => diff.bases = Bases::Untracked,
                (Some(head), Some(index)) => match &mut diff.bases {
                    Bases::Tracked(tracked) => (tracked.head, tracked.index) = (head, index),
                    Bases::Loading { landing } => {
                        diff.bases = Bases::Tracked(Tracked::new(head, index, *landing))
                    }
                    Bases::NoRepo | Bases::Untracked => {
                        diff.bases = Bases::Tracked(Tracked::new(head, index, None))
                    }
                },
                // The bases moved while the read ran; read them again.
                (None, _) | (_, None) => {
                    diff.read = Refresh::Idle;
                    return request_bases(state, buffer, out);
                }
            }
        }
    }
    match again {
        Refresh::Queued => request_bases(state, buffer, out),
        Refresh::Running | Refresh::Idle => {}
    }
}

/// Takes a finished diff; a result for older text or bases is dropped and `sync` re-queues.
pub fn diffed(
    state: &mut State,
    buffer: BufferId,
    stamp: Stamp,
    lines: Box<Lines>,
    outcome: Outcome,
) {
    let Some(Content::Text { doc, diff, .. }) =
        state.buffers.get_mut(buffer).map(|b| &mut b.content)
    else {
        return;
    };
    let Bases::Tracked(tracked) = &mut diff.bases else {
        return;
    };
    match tracked.finish(stamp, lines, outcome, doc.version) {
        Finished::Current => land(state, buffer),
        Finished::Stale => {}
    }
}

/// Starts a diff for every tracked buffer whose hunks no longer match its text or bases.
pub fn sync(state: &mut State, out: &mut Vec<Effect>) {
    for (buffer, entry) in state.buffers.iter_mut() {
        let (doc, tracked) = match &mut entry.content {
            Content::Text {
                doc,
                diff:
                    crate::git::diff::Diff {
                        bases: Bases::Tracked(tracked),
                        ..
                    },
                ..
            } => (doc, tracked),
            Content::Text { .. } | Content::Loading { .. } => continue,
        };
        let stamp = tracked.current(doc.version);
        match tracked.stamp == Some(stamp) {
            true => continue,
            false => {}
        }
        match std::mem::replace(&mut tracked.compute, Compute::Running) {
            Compute::Idle(lines) => out.push(Effect::Job(Job::Diff {
                buffer,
                stamp,
                head: tracked.head.clone(),
                index: tracked.index.clone(),
                text: doc.text.clone(),
                lines,
            })),
            Compute::Running => {}
        }
    }
}
