//! Buffers coming and going: loading, opening, placing, writing, reloading and closing.

use std::io;
use std::path::{Path, PathBuf};

use helix_core::line_ending::line_end_char_index;
use helix_core::{Rope, Selection, SmallVec};
use model::command::{Command, Dirs};
use model::config::OpenAs;
use model::notice::Notice;
use slotmap::SlotMap;

use super::map_views;
use crate::git::diff::Diff;
use crate::git::navigate::{self, Landing};
use crate::git::track;
use crate::job::{DiskRead, Job, OnChange, OpenRead, ReloadIf, ReloadPolicy, Written, read_file};
use crate::text::buffer::{
    Buffer, BufferId, Disk, Position, Read, ReadId, Target, TextDoc, Waiter,
};
use crate::view::layout::{Layout, PaneId, Tabs};
use crate::view::panes::Arrive;
use crate::view::tab::{BesideMut, ShowingMut, Tab};
use crate::{
    AfterWrite, Effect, State,
    project::links,
    render::document::{self, DocumentScroll},
    render::pages::PagesScroll,
    request_git,
    view::panes,
    view::scroll,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Force {
    No,
    Yes,
}

pub(super) fn close_active(state: &mut State, force: Force) {
    let focus = state.layout.focus();
    match (state.layout.panes().get(focus))
        .and_then(|pane| pane.tabs.as_ref())
        .map(Tabs::active)
    {
        Some(index) => panes::close_tab(state, focus, index, force),
        None => state.notice = Some(Notice::NoBuffer),
    }
}

pub(super) fn focused_buffer(layout: &Layout) -> Result<BufferId, Notice> {
    (layout.focused().active().map(Tab::buffer)).ok_or(Notice::NoBuffer)
}

/// Commits pending edits first; writing keeps the buffer's preview tabs.
pub(super) fn write_job(layout: &mut Layout, id: BufferId, buffer: &mut Buffer, dirs: Dirs) -> Job {
    buffer.doc.commit(&layout.selection_of(id));
    layout.keep(id);
    Job::Write {
        buffer: id,
        path: buffer.path.clone(),
        text: buffer.doc.text.clone(),
        revision: buffer.doc.history.current_revision(),
        dirs,
    }
}

pub(super) fn write_modified(
    layout: &mut Layout,
    buffers: &mut SlotMap<BufferId, Buffer>,
    out: &mut Vec<Effect>,
) -> Vec<BufferId> {
    let modified = buffers
        .iter_mut()
        .filter(|(_, buffer)| buffer.doc.is_modified());
    let mut written = Vec::new();
    for (id, buffer) in modified {
        written.push(id);
        out.push(Effect::Job(write_job(layout, id, buffer, Dirs::Existing)));
    }
    written
}

/// `:reload` reads the file whatever its mtime and replaces unsaved edits.
pub(super) fn reload_job(layout: &Layout, id: BufferId, buffer: &mut Buffer) -> Job {
    buffer.doc.commit(&layout.selection_of(id));
    Job::Reload {
        buffer: id,
        path: buffer.path.clone(),
        when: ReloadIf::Always,
        policy: ReloadPolicy::Discard,
    }
}

/// Char index of `to`, with the column clamped to the line's end.
pub(crate) fn line_column(text: &Rope, to: Position) -> usize {
    let line = to.line.min(text.len_lines().saturating_sub(1));
    let start = text.line_to_char(line);
    (start + to.column).min(line_end_char_index(&text.slice(..), line))
}

/// Where a file stands once asked for.
pub(crate) enum Found {
    Open(BufferId),
    Reading(ReadId),
}

/// The canonical form of `path` under the root; a missing file keeps its normalized path.
fn canonical(root: &Path, path: &Path) -> Result<PathBuf, Notice> {
    let joined = helix_stdx::path::normalize(root.join(path));
    match joined.canonicalize() {
        Ok(path) if path.is_dir() => Err(Notice::NotAFile { path }),
        Ok(path) => Ok(path),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(joined),
        Err(error) => Err(Notice::OpenFailed {
            path: joined,
            error,
        }),
    }
}

fn open_buffer(state: &State, path: &Path) -> Option<BufferId> {
    (state.buffers.iter())
        .find(|(_, buffer)| buffer.path == path)
        .map(|(id, _)| id)
}

fn insert(
    state: &mut State,
    path: PathBuf,
    doc: TextDoc,
    disk: Disk,
    out: &mut Vec<Effect>,
) -> BufferId {
    let id = state.buffers.insert(Buffer {
        path,
        doc,
        disk,
        diff: Diff::LOADING,
    });
    track::request_bases(state, id, out);
    id
}

/// The buffer of `path`, or the read of it, started as a job when it is new.
pub(crate) fn load(state: &mut State, path: &Path, out: &mut Vec<Effect>) -> Result<Found, Notice> {
    let path = canonical(state.project.root.path(), path)?;
    let reading = (state.reads.iter()).find(|(_, read)| read.path == path);
    match (open_buffer(state, &path), reading) {
        (Some(id), _) => Ok(Found::Open(id)),
        (None, Some((read, _))) => Ok(Found::Reading(read)),
        (None, None) => {
            let read = state.reads.insert(Read {
                path: path.clone(),
                waiting: SmallVec::new(),
            });
            out.push(Effect::Job(Job::Open {
                read,
                path,
                languages: state.languages.clone(),
            }));
            Ok(Found::Reading(read))
        }
    }
}

/// Reads `path` on the spot, for a session restored before anything draws.
pub(crate) fn read_now(
    state: &mut State,
    path: &Path,
    out: &mut Vec<Effect>,
) -> Result<BufferId, Notice> {
    let path = canonical(state.project.root.path(), path)?;
    match open_buffer(state, &path) {
        Some(id) => Ok(id),
        None => match read_file(&path, &state.languages) {
            OpenRead::Text { doc, disk } => Ok(insert(state, path, doc, disk, out)),
            OpenRead::NotUtf8 => Err(Notice::NotUtf8 { path }),
            OpenRead::NotAFile => Err(Notice::NotAFile { path }),
            OpenRead::Failed(error) => Err(Notice::OpenFailed { path, error }),
        },
    }
}

/// Shows `path` in the focused pane; `to` places the cursor once the text or diff arrives.
pub(crate) fn open(
    state: &mut State,
    path: PathBuf,
    arrive: Arrive,
    to: Option<Target>,
    open: OpenAs,
    out: &mut Vec<Effect>,
) {
    let waiter = Waiter {
        pane: state.layout.focus(),
        arrive,
        open,
        to,
    };
    match load(state, &path, out) {
        Ok(Found::Open(buffer)) => show(state, buffer, waiter),
        Ok(Found::Reading(read)) => state.reads[read].waiting.push(waiter),
        Err(notice) => state.notice = Some(notice),
    }
}

/// Shows `buffer` as `waiter` asked; a pane closed while the file was read gives way to the focus.
fn show(state: &mut State, buffer: BufferId, waiter: Waiter) {
    let Waiter {
        pane,
        arrive,
        open,
        to,
    } = waiter;
    let pane = match state.layout.find_pane(pane) {
        Some(_) => pane,
        None => state.layout.focus(),
    };
    panes::show(state, pane, buffer, arrive, open);
    match to {
        Some(to) => place(state, pane, buffer, to),
        None => {}
    }
}

/// Puts the cursor of `buffer`'s tab in `pane` at `to`; a hunk waits for the buffer's diff.
pub(crate) fn place(state: &mut State, pane: PaneId, buffer: BufferId, to: Target) {
    let State {
        layout,
        buffers,
        documents,
        notice,
        ..
    } = state;
    let Buffer { doc, diff, .. } = &mut buffers[buffer];
    let (at, line) = match to {
        Target::Hunk(edge) => {
            navigate::expect_landing(diff, Landing { pane, edge });
            return navigate::land(state, buffer);
        }
        Target::Saved(saved) => {
            let len = doc.text.len_chars();
            return match layout.tab_of(pane, buffer).map(Tab::text_mut) {
                Some(view) => {
                    view.selection = saved.selection(len);
                    view.scroll.anchor = saved.scroll.min(len);
                }
                None => {}
            };
        }
        Target::Position(to) => {
            let at = line_column(&doc.text, to);
            (at, doc.text.char_to_line(at))
        }
        Target::Heading(heading) => {
            let parsed = document::markdown(documents, buffer, doc);
            match links::heading_line(doc, parsed, &heading) {
                Some(line) => (doc.text.line_to_char(line), line),
                None => return *notice = Some(Notice::HeadingNotFound { heading }),
            }
        }
    };
    match layout.tab_of(pane, buffer).map(Tab::showing_mut) {
        // The render sync brings the line to the top once parsed or located.
        Some(ShowingMut::Document(text, view, BesideMut::Alone)) => {
            text.selection = Selection::point(at);
            view.scroll = DocumentScroll::Line(line);
        }
        Some(ShowingMut::Pages(text, view, BesideMut::Alone)) => {
            text.selection = Selection::point(at);
            view.scroll = PagesScroll::Line(line);
        }
        // Beside the source, the rendering follows its cursor.
        Some(
            ShowingMut::Source(text)
            | ShowingMut::Diff(text, _)
            | ShowingMut::Document(text, _, BesideMut::Source(..))
            | ShowingMut::Pages(text, _, BesideMut::Source(..)),
        ) => {
            text.selection = Selection::point(at);
            scroll::follow_cursor(state, pane);
        }
        None => {}
    }
}

/// Takes a finished read: its buffer is made and shown as each waiting open asked.
pub(crate) fn opened(state: &mut State, read: ReadId, result: OpenRead, out: &mut Vec<Effect>) {
    let Read { path, waiting } = state.reads.remove(read).expect("each read lands once");
    match result {
        OpenRead::Text { doc, disk } => {
            let buffer = insert(state, path, doc, disk, out);
            for waiter in waiting {
                show(state, buffer, waiter);
            }
        }
        OpenRead::NotUtf8 => state.notice = Some(Notice::NotUtf8 { path }),
        OpenRead::NotAFile => state.notice = Some(Notice::NotAFile { path }),
        OpenRead::Failed(error) => state.notice = Some(Notice::OpenFailed { path, error }),
    }
}

pub(crate) fn written(
    state: &mut State,
    buffer: BufferId,
    path: PathBuf,
    revision: usize,
    result: io::Result<Written>,
    out: &mut Vec<Effect>,
) {
    match result {
        Ok(Written { bytes, mtime }) => {
            // A buffer closed since the write began has nothing to update.
            match state.buffers.get_mut(buffer) {
                Some(entry) => {
                    entry.doc.saved_revision = revision;
                    entry.disk = Disk::Synced { mtime };
                }
                None => {}
            }
            state.notice = Some(Notice::Written { path, bytes });
            request_git(state, out);
            match state.exit.written(buffer) {
                AfterWrite::Quit => super::apply(state, Command::Quit, out),
                AfterWrite::Stay => {}
            }
        }
        Err(error) => {
            tracing::warn!(?path, %error, "write failed");
            state.notice = Some(Notice::WriteFailed { path, error });
            state.exit.failed(buffer);
        }
    }
}

pub(crate) fn reloaded(state: &mut State, buffer: BufferId, read: DiskRead, policy: ReloadPolicy) {
    let State {
        buffers,
        layout,
        languages,
        notice,
        ..
    } = state;
    let Some(entry) = buffers.get_mut(buffer) else {
        return;
    };
    let Buffer { doc, disk, .. } = entry;
    match read {
        DiskRead::Unchanged => {}
        DiskRead::Missing => {
            *disk = Disk::Deleted;
            *notice = Some(Notice::DeletedOnDisk {
                path: entry.path.clone(),
            });
        }
        DiskRead::NotUtf8 => {
            *notice = Some(Notice::NotUtf8 {
                path: entry.path.clone(),
            })
        }
        DiskRead::Failed(error) => {
            tracing::warn!(%error, path = ?entry.path, "reload failed");
            *notice = Some(Notice::ReloadFailed {
                path: entry.path.clone(),
                error,
            });
        }
        DiskRead::Changed { text, mtime } => match policy.on_change(doc) {
            OnChange::Flag => {
                *disk = Disk::ChangedExternally { mtime };
                *notice = Some(Notice::ChangedOnDisk {
                    path: entry.path.clone(),
                });
            }
            OnChange::Replace => {
                let tx = helix_core::diff::compare_ropes(&doc.text, &text);
                let selection = layout.selection_of(buffer);
                // Unsaved edits stay their own undo step below the reload.
                doc.commit(&selection);
                doc.apply(&tx, &selection, languages);
                map_views(layout, buffer, tx.changes(), doc.text.slice(..));
                doc.commit(&selection.map(tx.changes()));
                doc.saved_revision = doc.history.current_revision();
                *disk = Disk::Synced { mtime };
            }
        },
    }
}

pub(super) fn write_focused(state: &mut State, dirs: Dirs, out: &mut Vec<Effect>) {
    match focused_buffer(&state.layout) {
        Ok(id) => {
            let job = write_job(&mut state.layout, id, &mut state.buffers[id], dirs);
            out.push(Effect::Job(job))
        }
        Err(refused) => state.notice = Some(refused),
    }
}
