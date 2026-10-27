//! `:export-pdf`, `:export-svg` and `:export-png` on the focused tab: typst compiles to PDF, and
//! drawings are written in a job from a snapshot taken now.

use std::ffi::OsString;
use std::io;
use std::path::PathBuf;

use ::export::Export;
use ::export::scene::Region;
use ink::tool::{Selecting, Using};
use model::command::ExportAs;
use model::notice::Notice;

use crate::drawing::{Doing, InkTabs};
use crate::job::Job;
use crate::render::typeset;
use crate::view::tab::Tab;
use crate::{Effect, State};

#[derive(Debug)]
pub enum ExportFailed {
    Render(::export::ExportError),
    Write(io::Error),
}

/// A paged drawing exports every page as PDF, or the page in focus as SVG or PNG; a board, the
/// picked marks when some are held, else all of it.
pub fn export(state: &mut State, to: ExportAs, out: &mut Vec<Effect>) {
    let fonts = state.fonts.clone();
    let job = match (state.layout.focused().active(), to) {
        (Some(Tab::Text(_)), ExportAs::Pdf) => return typeset::export_pdf(state, out),
        (Some(Tab::Ink(InkTabs::Paged(tab))), ExportAs::Pdf) => {
            let file = &state.pageds[tab.file()];
            Ok(Job::ExportDrawing {
                export: Export::Pages(file.edited.drawing().clone()),
                fonts,
                path: file.path.with_extension(to.extension()),
            })
        }
        (Some(Tab::Ink(InkTabs::Paged(tab))), ExportAs::Svg | ExportAs::Png) => {
            let file = &state.pageds[tab.file()];
            let page = tab.view.centre.page;
            let mut name = OsString::from(file.path.file_stem().unwrap_or_default());
            name.push(format!("-{}.{}", page.0 + 1, to.extension()));
            Ok(Job::ExportDrawing {
                export: Export::Page {
                    paged: file.edited.drawing().clone(),
                    page,
                    to,
                },
                fonts,
                path: file.path.with_file_name(name),
            })
        }
        (Some(Tab::Ink(InkTabs::Board(tab))), to) => {
            let file = &state.boards[tab.file()];
            let region = match &tab.hand.doing {
                Doing::Ink {
                    using: Using::Selecting(Selecting::Holding { target, picked, .. }),
                    ..
                } => Region::Picked {
                    layer: *target,
                    ids: picked.ids().iter().copied().collect(),
                },
                Doing::Ink { .. } | Doing::Typing(_) => Region::Everything,
            };
            Ok(Job::ExportDrawing {
                export: Export::Region {
                    board: file.edited.drawing().clone(),
                    region,
                    to,
                },
                fonts,
                path: file.path.with_extension(to.extension()),
            })
        }
        (Some(Tab::Text(_)), ExportAs::Svg | ExportAs::Png) | (Some(Tab::Image(_)) | None, _) => {
            Err(Notice::NotExportable { to })
        }
    };
    match job {
        Ok(job) => out.push(Effect::Job(job)),
        Err(refused) => state.notice = Some(refused),
    }
}

pub fn exported(state: &mut State, path: PathBuf, result: Result<usize, ExportFailed>) {
    state.notice = Some(match result {
        Ok(bytes) => Notice::Exported { path, bytes },
        Err(ExportFailed::Write(error)) => {
            tracing::warn!(?path, %error, "export failed");
            Notice::WriteFailed { path, error }
        }
        Err(ExportFailed::Render(error)) => {
            tracing::warn!(?path, %error, "export failed");
            Notice::ExportFailed {
                path,
                reason: error.to_string(),
            }
        }
    });
}
