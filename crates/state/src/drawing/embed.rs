//! Images placed in drawings: the file is read and measured in a job, then embedded at the
//! place asked for when the job started.

use std::path::PathBuf;
use std::sync::Arc;

use helix_core::smallvec;
use ink::doc::{Drawing, ImageInk, Ink, LayerIndex, Mark, PageLayer, PageSize, Placement};
use ink::tool::{Frame, Gather, Grip, Picked, Selecting, ToolKind};
use model::geom::{Point, Surface, Vector};
use model::notice::Notice;
use model::picture::Encoding;

use crate::State;
use crate::drawing::{BoardId, Hand, InkFile, InkTabs, PagedId};
use crate::view::tab::Tab;
use crate::viewer::{ProbeError, probe};

/// Points per image pixel, at the usual 96 pixels to the inch.
pub const POINTS_PER_PIXEL: f32 = 0.75;
/// The widest part of a page a new image takes.
pub const PAGE_SHARE: f32 = 0.8;

/// Where an image goes: the drawing, the layer of the surface, and the point its centre lands on.
#[derive(Clone, Copy, Debug)]
pub enum Destination {
    Paged {
        file: PagedId,
        target: PageLayer,
        centre: Point<Surface>,
    },
    Board {
        file: BoardId,
        target: LayerIndex,
        centre: Point<Surface>,
    },
}

/// An image file read whole, and its size in pixels.
pub struct Read {
    pub encoded: Arc<[u8]>,
    pub encoding: Encoding,
    pub pixels: PageSize,
}

/// Reads `path` whole and measures it, in a job.
pub fn read(path: &std::path::Path, encoding: Encoding) -> Result<Read, ProbeError> {
    let encoded: Arc<[u8]> = std::fs::read(path)?.into();
    let pixels = probe(&encoded, encoding)?;
    Ok(Read {
        encoded,
        encoding,
        pixels,
    })
}

/// Places a read image; a drawing closed since the job started is left alone.
pub fn embedded(
    state: &mut State,
    path: PathBuf,
    into: Destination,
    read: Result<Read, ProbeError>,
) {
    let read = match read {
        Ok(read) => read,
        Err(error) => {
            state.notice = Some(Notice::Unreadable {
                path,
                reason: error.to_string(),
            });
            return;
        }
    };
    let result = match into {
        Destination::Paged {
            file,
            target,
            centre,
        } => match state.pageds.get_mut(file) {
            Some(drawing) => {
                let page = drawing.edited.drawing().stack().place(target.page).size;
                let widest = page.width.get() * PAGE_SHARE;
                place(drawing, target, centre, &read, widest).map(|picked| {
                    match state.layout.focused_mut().active_mut() {
                        Some(Tab::Ink(InkTabs::Paged(tab))) if tab.file() == file => {
                            pick(&mut tab.hand, target, picked)
                        }
                        Some(_) | None => {}
                    }
                })
            }
            None => Ok(()),
        },
        Destination::Board {
            file,
            target,
            centre,
        } => match state.boards.get_mut(file) {
            Some(drawing) => place(drawing, target, centre, &read, f32::INFINITY).map(|picked| {
                match state.layout.focused_mut().active_mut() {
                    Some(Tab::Ink(InkTabs::Board(tab))) if tab.file() == file => {
                        pick(&mut tab.hand, target, picked)
                    }
                    Some(_) | None => {}
                }
            }),
            None => Ok(()),
        },
    };
    match result {
        Ok(()) => {}
        Err(notice) => state.notice = Some(notice),
    }
}

/// Adds the image centred on `centre`, at its pixel size in points but no wider than `widest`.
pub fn place<D: Drawing>(
    file: &mut InkFile<D>,
    target: D::Target,
    centre: Point<Surface>,
    read: &Read,
    widest: f32,
) -> Result<Picked, Notice> {
    let natural = read.pixels.width.get() * POINTS_PER_PIXEL;
    let scale = POINTS_PER_PIXEL * (widest / natural).min(1.0);
    let extent = PageSize::new(
        read.pixels.width.get() * scale,
        read.pixels.height.get() * scale,
    )
    .map_err(|error| Notice::DrawingRefused {
        reason: error.to_string(),
    })?;
    let half = Vector::new(extent.width.get() * 0.5, extent.height.get() * 0.5);
    let mark = Mark {
        placement: Placement::at(centre - half),
        paint: ink::brush::Paint {
            color: model::theme::Color::opaque(model::color::Rgb::hex(0x000000)),
        },
        ink: Ink::Image(Arc::new(ImageInk::new(
            read.encoded.clone(),
            read.encoding,
            extent,
        ))),
    };
    let frame = Frame::of(&mark);
    let id = (file.edited.add(target, mark)).map_err(|error| Notice::DrawingRefused {
        reason: error.to_string(),
    })?;
    Ok(Picked {
        ids: smallvec![id],
        frame,
    })
}

/// The tip takes the box select tool with `picked` held.
pub fn pick<T: Copy + PartialEq>(hand: &mut Hand<T>, target: T, picked: Picked) {
    hand.cancel_stroke();
    hand.tip = ToolKind::Select(Gather::Box);
    hand.tools.select = Selecting::Holding {
        target,
        picked,
        grip: Grip::Still,
    };
}
