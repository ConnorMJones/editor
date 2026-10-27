//! Buffers coming and going: loading, opening, placing, writing, reloading and closing.

use std::io;
use std::path::{Path, PathBuf};

use helix_core::line_ending::line_end_char_index;
use helix_core::{Rope, Selection, SmallVec};
use model::command::{Command, Dirs};
use model::config::OpenAs;
use model::notice::Notice;

use ink::doc::Drawing;

use super::map_views;
use crate::drawing::{InkFile, InkTabs};
use crate::git::diff::Diff;
use crate::git::navigate::{self, Landing};
use crate::git::track;
use crate::job::{
    Data, DiskRead, Job, Missed, NotUtf8, OnChange, OpenRead, ReloadIf, ReloadPolicy, Reloaded,
    Saving, Written, read_file,
};
use crate::project::path::FileKind;
use crate::text::buffer::{Buffer, BufferId, Disk, Position, Read, ReadId, Target, Waiter};
use crate::view::layout::{Layout, PaneId, Tabs};
use crate::view::panes::Arrive;
use crate::view::tab::{BesideMut, Opened, ShowingMut, Tab, TextTabs, Writable};
use crate::viewer::ImageFile;
use crate::{
    AfterWrite, Effect, State,
    project::links,
    render::document::{self, DocumentScroll},
    render::pages::Locating,
    request_git,
    view::panes,
    view::scroll,
};
use model::sheet::Stack;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Force {
    No,
    Yes,
}

pub fn close_active(state: &mut State, force: Force) {
    let focus = state.layout.focus();
    match (state.layout.panes().get(focus))
        .and_then(|pane| pane.tabs.as_ref())
        .map(Tabs::active)
    {
        Some(index) => panes::close_tab(state, focus, index, force),
        None => state.notice = Some(Notice::NoBuffer),
    }
}

/// The file the focused tab shows, for commands on whole files.
pub fn focused_file(layout: &Layout) -> Result<Opened, Notice> {
    (layout.focused().active().map(Tab::opened)).ok_or(Notice::NoBuffer)
}

/// The focused tab's file when it can be written.
pub fn focused_writable(layout: &Layout) -> Result<Writable, Notice> {
    focused_file(layout)?.writable().ok_or(Notice::ReadOnly)
}

/// Commits pending text edits, and text being typed on a drawing, first; writing keeps the
/// file's preview tabs.
pub fn write_job(state: &mut State, file: Writable, dirs: Dirs) -> Job {
    let opened = Opened::from(file);
    match crate::drawing::typing::settle_on(state, |other| other == opened) {
        Ok(()) => {}
        Err(refused) => state.notice = Some(refused),
    }
    state.layout.keep(file.into());
    let path = state.path_of(file.into()).to_owned();
    let (saving, data) = match file {
        Writable::Text(buffer) => {
            let entry = &mut state.buffers[buffer];
            entry.doc.commit(&state.layout.selection_of(buffer));
            let revision = entry.doc.history.current_revision();
            (
                Saving::Text { buffer, revision },
                Data::Text(entry.doc.text.clone()),
            )
        }
        Writable::Paged(file) => {
            let edited = &state.pageds[file].edited;
            let mut bytes = Vec::new();
            ink::format::write_paged(edited.drawing(), &mut bytes);
            let revision = edited.revision();
            (Saving::Paged { file, revision }, Data::Bytes(bytes))
        }
        Writable::Board(file) => {
            let edited = &state.boards[file].edited;
            let mut bytes = Vec::new();
            ink::format::write_board(edited.drawing(), &mut bytes);
            let revision = edited.revision();
            (Saving::Board { file, revision }, Data::Bytes(bytes))
        }
    };
    Job::Write {
        path,
        saving,
        data,
        dirs,
    }
}

/// Writes every modified file, returning what is being written.
pub fn write_modified(state: &mut State, out: &mut Vec<Effect>) -> Vec<Opened> {
    match crate::drawing::typing::settle_on(state, |_| true) {
        Ok(()) => {}
        Err(refused) => state.notice = Some(refused),
    }
    // Images are never modified, so every modified file can be written.
    let modified: Vec<Writable> = (state.files())
        .filter(|opened| state.is_modified(*opened))
        .filter_map(Opened::writable)
        .collect();
    for file in &modified {
        let job = write_job(state, *file, Dirs::Existing);
        out.push(Effect::Job(job));
    }
    modified.into_iter().map(Opened::from).collect()
}

/// `:reload` reads the file whatever its mtime and replaces unsaved edits, which stay undoable.
pub fn reload_job(state: &mut State, opened: Opened) -> Job {
    match opened {
        Opened::Text(id) => state.buffers[id].doc.commit(&state.layout.selection_of(id)),
        Opened::Paged(_) | Opened::Board(_) | Opened::Pdf(_) | Opened::Image(_) => {}
    }
    Job::Reload {
        file: state.reload_of(opened),
        path: state.path_of(opened).to_owned(),
        when: ReloadIf::Always,
        policy: ReloadPolicy::Discard,
    }
}

/// Char index of `to`, with the column clamped to the line's end.
pub fn line_column(text: &Rope, to: Position) -> usize {
    let line = to.line.min(text.len_lines().saturating_sub(1));
    let start = text.line_to_char(line);
    (start + to.column).min(line_end_char_index(&text.slice(..), line))
}

/// Where a file stands once asked for.
pub enum Found {
    Open(Opened),
    Reading(ReadId),
}

/// The canonical form of `path` under the root; a missing file keeps its normalized path.
pub fn canonical(root: &Path, path: &Path) -> Result<PathBuf, Notice> {
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

/// What already holds `path`, in the store its kind of file lives in.
fn open_file(state: &State, path: &Path) -> Option<Opened> {
    match FileKind::of(path) {
        FileKind::Text(_) => (state.buffers.iter())
            .find(|(_, buffer)| buffer.path == path)
            .map(|(id, _)| Opened::Text(id)),
        FileKind::Paged => (state.pageds.iter())
            .find(|(_, file)| file.path == path)
            .map(|(id, _)| Opened::Paged(id)),
        FileKind::Board => (state.boards.iter())
            .find(|(_, file)| file.path == path)
            .map(|(id, _)| Opened::Board(id)),
        FileKind::Pdf => (state.pdfs.iter())
            .find(|(_, file)| file.path == path)
            .map(|(id, _)| Opened::Pdf(id)),
        FileKind::Image(_) => (state.images.iter())
            .find(|(_, file)| *file.path == *path)
            .map(|(id, _)| Opened::Image(id)),
    }
}

/// Puts a read file in its store, or says why it couldn't be read.
fn insert(
    state: &mut State,
    path: PathBuf,
    read: OpenRead,
    out: &mut Vec<Effect>,
) -> Result<Opened, Notice> {
    match read {
        OpenRead::Text { kind, doc, disk } => {
            let id = state.buffers.insert(Buffer {
                path,
                kind,
                doc,
                disk,
                diff: Diff::LOADING,
            });
            track::request_bases(state, id, out);
            Ok(Opened::Text(id))
        }
        OpenRead::Paged { drawing, disk } => Ok(Opened::Paged(
            state.pageds.insert(InkFile::new(path, drawing, disk)),
        )),
        OpenRead::Board { drawing, disk } => Ok(Opened::Board(
            state.boards.insert(InkFile::new(path, drawing, disk)),
        )),
        OpenRead::Pdf { drawing, disk } => Ok(Opened::Pdf(
            state.pdfs.insert(InkFile::new(path, drawing, disk)),
        )),
        OpenRead::Image {
            encoding,
            stack,
            modified,
        } => Ok(Opened::Image(state.images.insert(ImageFile {
            path: path.into(),
            encoding,
            stack,
            modified,
            disk: Disk::Synced { mtime: modified },
        }))),
        OpenRead::NotUtf8 => Err(Notice::NotUtf8 { path }),
        OpenRead::NotAFile => Err(Notice::NotAFile { path }),
        OpenRead::Failed(error) => Err(Notice::OpenFailed { path, error }),
        OpenRead::Unreadable(error) => Err(Notice::Unreadable {
            path,
            reason: error.to_string(),
        }),
    }
}

/// The buffer of `path`, or the read of it, started as a job when it is new.
pub fn load(state: &mut State, path: &Path, out: &mut Vec<Effect>) -> Result<Found, Notice> {
    let path = canonical(state.project.root.path(), path)?;
    let reading = (state.reads.iter()).find(|(_, read)| read.path == path);
    match (open_file(state, &path), reading) {
        (Some(opened), _) => Ok(Found::Open(opened)),
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
                fonts: state.fonts.clone(),
            }));
            Ok(Found::Reading(read))
        }
    }
}

/// Reads `path` on the spot, for a session restored before anything draws.
pub fn read_now(state: &mut State, path: &Path, out: &mut Vec<Effect>) -> Result<Opened, Notice> {
    let path = canonical(state.project.root.path(), path)?;
    match open_file(state, &path) {
        Some(opened) => Ok(opened),
        None => {
            let read = read_file(&path, &state.languages, &state.fonts);
            insert(state, path, read, out)
        }
    }
}

/// Shows `path` in the focused pane; `to` places the cursor once the text or diff arrives.
pub fn open(
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
        Ok(Found::Open(opened)) => show(state, opened, waiter),
        Ok(Found::Reading(read)) => state.reads[read].waiting.push(waiter),
        Err(notice) => state.notice = Some(notice),
    }
}

/// Shows `opened` as `waiter` asked; a pane closed while the file was read gives way to the focus.
fn show(state: &mut State, opened: Opened, waiter: Waiter) {
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
    panes::show(state, pane, opened, arrive, open);
    match (to, opened) {
        (Some(to), Opened::Text(buffer)) => place(state, pane, buffer, to),
        (Some(_), Opened::Paged(_) | Opened::Board(_) | Opened::Pdf(_) | Opened::Image(_))
        | (None, _) => {}
    }
}

/// Puts the cursor of `buffer`'s tab in `pane` at `to`; a hunk waits for the buffer's diff.
pub fn place(state: &mut State, pane: PaneId, buffer: BufferId, to: Target) {
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
            return match layout.tab_of(pane, buffer).map(TextTabs::text_mut) {
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
    match layout.tab_of(pane, buffer).map(TextTabs::showing_mut) {
        // The render sync brings the line to the top once parsed or located.
        Some(ShowingMut::Document(text, view, BesideMut::Alone)) => {
            text.selection = Selection::point(at);
            view.scroll = DocumentScroll::Line(line);
        }
        Some(ShowingMut::Pages(text, view, BesideMut::Alone)) => {
            text.selection = Selection::point(at);
            view.locating = Locating::Line(line);
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
pub fn opened(state: &mut State, read: ReadId, result: OpenRead, out: &mut Vec<Effect>) {
    let Read { path, waiting } = state.reads.remove(read).expect("each read lands once");
    match insert(state, path, result, out) {
        Ok(opened) => {
            for waiter in waiting {
                show(state, opened, waiter);
            }
        }
        Err(notice) => state.notice = Some(notice),
    }
}

pub fn written(
    state: &mut State,
    saving: Saving,
    path: PathBuf,
    result: io::Result<Written>,
    out: &mut Vec<Effect>,
) {
    match result {
        Ok(Written { bytes, mtime }) => {
            // A file closed since the write began has nothing to update.
            let disk = Disk::Synced { mtime };
            match saving {
                Saving::Text { buffer, revision } => match state.buffers.get_mut(buffer) {
                    Some(entry) => {
                        entry.doc.saved_revision = revision;
                        entry.disk = disk;
                    }
                    None => {}
                },
                Saving::Paged { file, revision } => match state.pageds.get_mut(file) {
                    Some(entry) => {
                        entry.saved = revision;
                        entry.disk = disk;
                    }
                    None => {}
                },
                Saving::Board { file, revision } => match state.boards.get_mut(file) {
                    Some(entry) => {
                        entry.saved = revision;
                        entry.disk = disk;
                    }
                    None => {}
                },
            }
            state.notice = Some(Notice::Written { path, bytes });
            request_git(state, out);
            match state.exit.written(saving.opened()) {
                AfterWrite::Quit => super::apply(state, Command::Quit, out),
                AfterWrite::Stay => {}
            }
        }
        Err(error) => {
            tracing::warn!(?path, %error, "write failed");
            state.notice = Some(Notice::WriteFailed { path, error });
            state.exit.failed(saving.opened());
        }
    }
}

pub fn reloaded(state: &mut State, read: Reloaded, policy: ReloadPolicy) {
    let State {
        buffers,
        pageds,
        boards,
        pdfs,
        images,
        layout,
        languages,
        notice,
        ..
    } = state;
    match read {
        Reloaded::Text(buffer, read) => {
            let Some(entry) = buffers.get_mut(buffer) else {
                return;
            };
            let Buffer {
                doc, disk, path, ..
            } = entry;
            match read {
                DiskRead::Changed { content, mtime } => {
                    match policy.on_change(doc.is_modified()) {
                        OnChange::Flag => {
                            *disk = Disk::ChangedExternally { mtime };
                            *notice = Some(Notice::ChangedOnDisk { path: path.clone() });
                        }
                        OnChange::Replace => {
                            let tx = helix_core::diff::compare_ropes(&doc.text, &content);
                            let selection = layout.selection_of(buffer);
                            // Unsaved edits stay their own undo step below the reload.
                            doc.commit(&selection);
                            doc.apply(&tx, &selection, languages);
                            map_views(layout, buffer, tx.changes(), doc.text.slice(..));
                            doc.commit(&selection.map(tx.changes()));
                            doc.saved_revision = doc.history.current_revision();
                            *disk = Disk::Synced { mtime };
                        }
                    }
                }
                DiskRead::Unreadable(NotUtf8) => {
                    *notice = Some(Notice::NotUtf8 { path: path.clone() })
                }
                DiskRead::Missed(missed_read) => missed(missed_read, path, disk, notice),
            }
        }
        Reloaded::Paged(id, read) => match pageds.get_mut(id) {
            Some(file) => match drawing_reloaded(file, read, policy, notice) {
                Drawn::Replaced => {
                    for tab in drawing_tabs(layout, Opened::Paged(id)) {
                        tab.cancel_strokes()
                    }
                }
                Drawn::Kept => {}
            },
            None => {}
        },
        Reloaded::Pdf(id, read) => match pdfs.get_mut(id) {
            Some(file) => match drawing_reloaded(file, read, policy, notice) {
                Drawn::Replaced => {
                    for tab in drawing_tabs(layout, Opened::Pdf(id)) {
                        tab.cancel_strokes()
                    }
                }
                Drawn::Kept => {}
            },
            None => {}
        },
        Reloaded::Board(id, read) => match boards.get_mut(id) {
            Some(file) => match drawing_reloaded(file, read, policy, notice) {
                Drawn::Replaced => {
                    for tab in drawing_tabs(layout, Opened::Board(id)) {
                        tab.cancel_strokes()
                    }
                }
                Drawn::Kept => {}
            },
            None => {}
        },
        // Images are never modified, so a change on disk always replaces the picture.
        Reloaded::Image(id, read) => match images.get_mut(id) {
            Some(file) => match read {
                DiskRead::Changed { content, mtime } => {
                    file.stack = Stack::one(content);
                    file.modified = mtime;
                    file.disk = Disk::Synced { mtime };
                }
                DiskRead::Unreadable(error) => {
                    *notice = Some(Notice::Unreadable {
                        path: file.path.to_path_buf(),
                        reason: error.to_string(),
                    })
                }
                DiskRead::Missed(missed_read) => {
                    missed(missed_read, &file.path, &mut file.disk, notice)
                }
            },
            None => {}
        },
    }
}

/// Whether a reload put the disk's drawing in place.
enum Drawn {
    Replaced,
    Kept,
}

fn drawing_reloaded<D: Drawing, E: std::fmt::Display>(
    file: &mut InkFile<D>,
    read: DiskRead<D, E>,
    policy: ReloadPolicy,
    notice: &mut Option<Notice>,
) -> Drawn {
    match read {
        DiskRead::Changed { content, mtime } => match policy.on_change(file.is_modified()) {
            OnChange::Flag => {
                file.disk = Disk::ChangedExternally { mtime };
                *notice = Some(Notice::ChangedOnDisk {
                    path: file.path.clone(),
                });
                Drawn::Kept
            }
            OnChange::Replace => {
                file.saved = file.edited.replace(content);
                file.disk = Disk::Synced { mtime };
                Drawn::Replaced
            }
        },
        DiskRead::Unreadable(error) => {
            *notice = Some(Notice::Unreadable {
                path: file.path.clone(),
                reason: error.to_string(),
            });
            Drawn::Kept
        }
        DiskRead::Missed(missed_read) => {
            missed(missed_read, &file.path, &mut file.disk, notice);
            Drawn::Kept
        }
    }
}

/// A read that found nothing to load: the file unchanged, gone, or failing to read.
fn missed(read: Missed, path: &Path, disk: &mut Disk, notice: &mut Option<Notice>) {
    match read {
        Missed::Unchanged => {}
        Missed::Missing => {
            *disk = Disk::Deleted;
            *notice = Some(Notice::DeletedOnDisk {
                path: path.to_owned(),
            });
        }
        Missed::Failed(error) => {
            tracing::warn!(%error, ?path, "reload failed");
            *notice = Some(Notice::ReloadFailed {
                path: path.to_owned(),
                error,
            });
        }
    }
}

/// Every tab showing the drawing `opened`.
fn drawing_tabs(layout: &mut Layout, opened: Opened) -> impl Iterator<Item = &mut InkTabs> {
    (layout.panes_mut())
        .flat_map(|(_, pane)| pane.items_mut())
        .filter_map(move |item| match &mut item.tab {
            Tab::Ink(ink) if ink.opened() == opened => Some(ink),
            Tab::Ink(_) | Tab::Text(_) | Tab::Image(_) => None,
        })
}

pub fn write_focused(state: &mut State, dirs: Dirs, out: &mut Vec<Effect>) {
    match focused_writable(&state.layout) {
        Ok(file) => {
            let job = write_job(state, file, dirs);
            out.push(Effect::Job(job))
        }
        Err(refused) => state.notice = Some(refused),
    }
}

/// `opened` saved under `path` from now on; a path naming another kind of file is refused.
pub fn renamed(state: &mut State, file: Writable, path: PathBuf) -> Result<Writable, Notice> {
    match (file, FileKind::of(&path)) {
        (Writable::Text(buffer), FileKind::Text(_)) => {
            let entry = &mut state.buffers[buffer];
            entry.path = path;
            entry.disk = Disk::New;
        }
        (Writable::Paged(id), FileKind::Paged) => {
            let entry = &mut state.pageds[id];
            entry.path = path;
            entry.disk = Disk::New;
        }
        (Writable::Board(id), FileKind::Board) => {
            let entry = &mut state.boards[id];
            entry.path = path;
            entry.disk = Disk::New;
        }
        (
            Writable::Text(_),
            FileKind::Paged | FileKind::Board | FileKind::Pdf | FileKind::Image(_),
        )
        | (
            Writable::Paged(_),
            FileKind::Text(_) | FileKind::Board | FileKind::Pdf | FileKind::Image(_),
        )
        | (
            Writable::Board(_),
            FileKind::Text(_) | FileKind::Paged | FileKind::Pdf | FileKind::Image(_),
        ) => {
            return Err(Notice::WrongExtension { path });
        }
    }
    Ok(file)
}
