use std::path::PathBuf;
use std::sync::Arc;

use ink::text::Fonts;
use library::Markup;
use model::notice::Notice;
use reader::pdf::Document;

use crate::drawing::{InkTabs, Notes, PdfFile, PdfId};
use crate::job::{Job, PdfWrite, PdfWriteError, PdfWriting, PdfWritten, Written};
use crate::project::path::RelPath;
use crate::text::buffer::Disk;
use crate::view::tab::{Opened, Tab};
use crate::{AfterWrite, Effect, State};

pub fn write(file: PdfId, pdf: &PdfFile, fonts: &Fonts, how: PdfWriting) -> Job {
    Job::WritePdf(PdfWrite {
        path: pdf.ink.path.clone(),
        file,
        revision: pdf.ink.edited.revision(),
        on_disk: pdf.on_disk().clone(),
        ink: pdf.ink.edited.drawing().clone(),
        fonts: fonts.clone(),
        how,
    })
}

pub fn written(
    state: &mut State,
    write: PdfWritten,
    result: Result<(Arc<Document>, Written), PdfWriteError>,
    out: &mut Vec<Effect>,
) {
    let PdfWritten {
        path,
        file,
        revision,
        how,
    } = write;
    let (on_disk, Written { bytes, mtime }) = match result {
        Ok(written) => written,
        Err(error) => {
            tracing::warn!(?path, %error, "writing a PDF failed");
            state.notice = Some(Notice::MarkupFailed {
                path,
                reason: error.to_string(),
            });
            return state.exit.failed(Opened::Pdf(file));
        }
    };
    let State {
        pdfs,
        layout,
        tree,
        project,
        ..
    } = state;
    let Some(pdf) = pdfs.get_mut(file) else {
        return;
    };
    let disk = Disk::Synced { mtime };
    match how {
        PdfWriting::Save => pdf.saved(on_disk.clone(), revision, disk),
        PdfWriting::Flatten => {
            pdf.flattened(on_disk.clone(), disk);
            for (_, pane) in layout.panes_mut() {
                for item in pane.items_mut() {
                    match &mut item.tab {
                        Tab::Ink(tabs @ InkTabs::Pdf(_)) if tabs.opened() == Opened::Pdf(file) => {
                            tabs.reloaded()
                        }
                        Tab::Ink(_) | Tab::Text(_) | Tab::Image(_) => {}
                    }
                }
            }
        }
    }
    let marks: usize = (pdf.ink.edited.drawing().pages().iter())
        .flat_map(|page| page.marks().iter())
        .map(|marks| marks.len())
        .sum();
    let markup = match (how, marks) {
        (PdfWriting::Flatten, _) | (PdfWriting::Save, 0) => Markup::None,
        (PdfWriting::Save, 1..) => Markup::Unflattened,
    };
    match RelPath::new(project.root.path(), &path) {
        Ok(relative) => match markup {
            Markup::Unflattened => {
                tree.marked_up.insert(relative);
            }
            Markup::None => {
                tree.marked_up.remove(&relative);
            }
        },
        Err(_) => {}
    }
    match &pdf.notes {
        Notes::Listed(listed) => out.push(Effect::Job(Job::Rewritten {
            library: listed.library.clone(),
            document: listed.document,
            written: on_disk,
            markup,
        })),
        Notes::Unlisted(_) => {}
    }
    state.notice = Some(match how {
        PdfWriting::Save => Notice::Written { path, bytes },
        PdfWriting::Flatten => Notice::Flattened { path },
    });
    crate::request_git(state, out);
    match state.exit.written(Opened::Pdf(file)) {
        AfterWrite::Quit => crate::apply::apply(state, model::command::Command::Quit, out),
        AfterWrite::Stay => {}
    }
}

pub fn marked_up(state: &mut State, paths: Vec<PathBuf>) {
    let root = state.project.root.path();
    state.tree.marked_up = (paths.iter())
        .filter_map(|path| RelPath::new(root, path).ok())
        .collect();
}
