use std::cell::RefCell;
use std::collections::HashMap;
use std::path::Path;
use std::rc::Rc;
use std::sync::Arc;
use std::time::SystemTime;

use gpui::{
    App, Bounds, Canvas, Corners, DevicePixels, Pixels, RenderImage, Size, SvgRenderer, SvgSize,
    WeakEntity, Window, canvas, prelude::*,
};
use helix_core::SmallVec;
use model::picture::{Encoding, Picture, Source};
use model::units::Rung;

use crate::workspace::Workspace;

pub const PICTURE_BYTES_KEPT: usize = 256 << 20;

#[derive(Default)]
pub struct Pictures {
    pub frame: u64,
    pub entries: HashMap<PictureKey, Entry>,
}

/// A picture by where its pixels come from, at a device size.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct PictureKey {
    pub source: SourceKey,
    pub size: Size<DevicePixels>,
}

/// What a cache entry draws.
#[derive(Clone)]
pub enum Drawn {
    Picture(Picture),
}

/// A file by its path and the time it was read at, or bytes and documents by their address.
#[derive(Clone, PartialEq, Eq, Hash)]
pub enum SourceKey {
    File(Arc<Path>, SystemTime),
    Bytes(usize),
    Zipped(Arc<Path>, SystemTime, Arc<str>),
}

pub struct Entry {
    /// Held so no other bytes reuse the address in the key.
    pub what: Drawn,
    pub image: Decoded,
    pub drawn: u64,
}

pub enum Decoded {
    Pending,
    Ready(Arc<RenderImage>),
    Failed,
}

impl SourceKey {
    pub fn of(what: &Drawn) -> SourceKey {
        match what {
            Drawn::Picture(Picture {
                source: Source::File { path, modified },
                ..
            }) => SourceKey::File(path.clone(), *modified),
            Drawn::Picture(Picture {
                source: Source::Bytes(bytes),
                ..
            }) => SourceKey::Bytes(Arc::as_ptr(bytes) as *const u8 as usize),
            Drawn::Picture(Picture {
                source:
                    Source::Zipped {
                        archive,
                        modified,
                        name,
                    },
                ..
            }) => SourceKey::Zipped(archive.clone(), *modified, name.clone()),
        }
    }
}

/// Draws `picture` over its bounds, decoded in the background at their device size.
pub fn picture_canvas(
    pictures: &Rc<RefCell<Pictures>>,
    workspace: &WeakEntity<Workspace>,
    picture: Picture,
) -> Canvas<Option<Arc<RenderImage>>> {
    let (pictures, workspace) = (pictures.clone(), workspace.clone());
    canvas(
        move |bounds, window, cx| {
            decoded(
                &pictures,
                &workspace,
                &Drawn::Picture(picture.clone()),
                bounds,
                window,
                cx,
            )
        },
        |bounds, image, window, _| match image {
            Some(image) => paint_picture(bounds, image, window),
            None => {}
        },
    )
}

pub fn paint_picture(bounds: Bounds<Pixels>, image: Arc<RenderImage>, window: &mut Window) {
    match window.paint_image(bounds, bounds, Corners::default(), image, 0, false) {
        Ok(()) => {}
        Err(error) => tracing::warn!(%error, "painting a picture failed"),
    }
}

/// `what` decoded at `bounds`' device size, asked for in the background when missing;
/// meanwhile a decode of another size stands in.
pub fn decoded(
    pictures: &Rc<RefCell<Pictures>>,
    workspace: &WeakEntity<Workspace>,
    what: &Drawn,
    bounds: Bounds<Pixels>,
    window: &mut Window,
    cx: &mut App,
) -> Option<Arc<RenderImage>> {
    let size = match bounds.size.to_device_pixels(window.scale_factor()) {
        size @ Size {
            width: DevicePixels(1..),
            height: DevicePixels(1..),
        } => size,
        Size { .. } => return None,
    };
    let size = {
        let width = size.width.0 as f32;
        let snapped = Rung::up(width).value();
        let grow =
            |side: i32| DevicePixels(((side as f32) * snapped / width).round().max(1.0) as i32);
        Size {
            width: grow(size.width.0),
            height: grow(size.height.0),
        }
    };
    let source = SourceKey::of(what);
    let key = PictureKey {
        source: source.clone(),
        size,
    };
    let mut pictures = pictures.borrow_mut();
    let Pictures { frame, entries } = &mut *pictures;
    let frame = *frame;
    let same = |entry: &Entry| SourceKey::of(&entry.what) == source;
    match entries.get_mut(&key) {
        Some(entry) => entry.drawn = frame,
        // One decode of a picture runs at a time, so a live resize settles on the last size.
        None => match (entries.values_mut())
            .find(|entry| same(entry) && matches!(entry.image, Decoded::Pending))
        {
            Some(pending) => pending.drawn = frame,
            None => {
                entries.insert(
                    key.clone(),
                    Entry {
                        what: what.clone(),
                        image: Decoded::Pending,
                        drawn: frame,
                    },
                );
                let (renderer, what) = (cx.svg_renderer(), what.clone());
                let job = cx.background_spawn(async move { decode(&what, size, &renderer) });
                let (workspace, key) = (workspace.clone(), key.clone());
                cx.spawn(async move |cx| {
                    let image = job.await;
                    workspace.update(cx, |this, cx| {
                        match this.pictures.borrow_mut().entries.get_mut(&key) {
                            Some(entry) => {
                                entry.image = match image {
                                    Ok(image) => Decoded::Ready(image),
                                    Err(error) => {
                                        tracing::warn!(%error, "decoding a picture failed");
                                        Decoded::Failed
                                    }
                                };
                                cx.notify();
                            }
                            None => {}
                        }
                    })
                })
                .detach();
            }
        },
    }
    match entries.get(&key) {
        Some(Entry {
            image: Decoded::Ready(image),
            ..
        }) => return Some(image.clone()),
        Some(_) | None => {}
    }
    (entries.values_mut())
        .filter(|entry| same(entry))
        .find_map(|entry| match &entry.image {
            Decoded::Ready(image) => {
                entry.drawn = frame;
                Some(image.clone())
            }
            Decoded::Pending | Decoded::Failed => None,
        })
}

pub fn decode(
    what: &Drawn,
    size: Size<DevicePixels>,
    renderer: &SvgRenderer,
) -> anyhow::Result<Arc<RenderImage>> {
    let Drawn::Picture(picture) = what;
    let read;
    let bytes: &[u8] = match &picture.source {
        Source::File { path, .. } => {
            read = std::fs::read(path)?;
            &read
        }
        Source::Zipped { archive, name, .. } => {
            read = reader::zip::Archive::open(std::fs::File::open(archive)?)?.read(name)?;
            &read
        }
        Source::Bytes(bytes) => bytes,
    };
    match picture.encoding {
        Encoding::Svg => {
            let tree = renderer.parse_svg(bytes)?;
            Ok(renderer.render_parsed(&tree, SvgSize::ExactSize(size))?)
        }
        Encoding::Bitmap(bitmap) => {
            let mut decoder = state::viewer::decoder(bytes, bitmap)?;
            let orientation = image::ImageDecoder::orientation(&mut decoder)?;
            let mut image = image::DynamicImage::from_decoder(decoder)?;
            image.apply_orientation(orientation);
            let (width, height) = (size.width.0 as u32, size.height.0 as u32);
            let image = match image.width() > width || image.height() > height {
                true => image.resize(width, height, image::imageops::FilterType::Triangle),
                false => image,
            };
            let mut pixels = image.into_rgba8();
            for pixel in pixels.chunks_exact_mut(4) {
                pixel.swap(0, 2);
            }
            let frames: SmallVec<[image::Frame; 1]> =
                SmallVec::from_elem(image::Frame::new(pixels), 1);
            Ok(Arc::new(RenderImage::new(frames)))
        }
    }
}

pub fn evict(pictures: &mut Pictures, window: &mut Window, cx: &mut App) {
    let frame = pictures.frame;
    pictures
        .entries
        .retain(|_, entry| match (&entry.image, entry.drawn == frame) {
            (Decoded::Failed, false) => false,
            (Decoded::Failed, true) | (Decoded::Pending | Decoded::Ready(_), _) => true,
        });
    let trimmed = crate::budget::keep_within(
        &mut pictures.entries,
        PICTURE_BYTES_KEPT,
        |entry| frame.saturating_sub(entry.drawn),
        |entry| match &entry.image {
            Decoded::Ready(image) => Some(image),
            Decoded::Pending | Decoded::Failed => None,
        },
        window,
        cx,
    );
    let _span = tracing::trace_span!("pictures kept", bytes = trimmed.kept).entered();
    pictures.frame += 1;
}
