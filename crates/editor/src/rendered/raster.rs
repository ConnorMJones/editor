//! Rasters of typeset SVGs at the device size they are drawn at, shared by every pane.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;

use gpui::{
    Canvas, Corners, DevicePixels, RenderImage, Size, SvgSize, WeakEntity, canvas, prelude::*,
};
use model::typeset::Svg;

use crate::workspace::Workspace;

/// Rasters of typeset SVGs at the device size they were drawn at, shared by every pane.
#[derive(Default)]
pub struct Rasters {
    /// Bumped every frame; rasters not drawn in the frame before are dropped.
    pub frame: u64,
    pub entries: HashMap<RasterKey, Raster>,
}

/// An SVG, by the address of its bytes, at a device size.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct RasterKey {
    pub svg: usize,
    pub size: Size<DevicePixels>,
}

pub struct Raster {
    /// Held so no other SVG reuses the address in the key.
    pub svg: Arc<[u8]>,
    pub image: Rasterized,
    pub drawn: u64,
}

pub enum Rasterized {
    Pending,
    Ready(Arc<RenderImage>),
    Failed,
}

/// Draws `svg` over its bounds, rasterised in the background at their device size.
pub(super) fn svg_canvas(
    rasters: &Rc<RefCell<Rasters>>,
    workspace: &WeakEntity<Workspace>,
    svg: &Svg,
) -> Canvas<Option<Arc<RenderImage>>> {
    let (rasters, workspace, bytes) = (rasters.clone(), workspace.clone(), svg.bytes.clone());
    canvas(
        move |bounds, window, cx| {
            let size = match bounds.size.to_device_pixels(window.scale_factor()) {
                size @ Size {
                    width: DevicePixels(1..),
                    height: DevicePixels(1..),
                } => size,
                Size { .. } => return None,
            };
            let key = RasterKey {
                svg: Arc::as_ptr(&bytes) as *const u8 as usize,
                size,
            };
            let mut rasters = rasters.borrow_mut();
            let Rasters { frame, entries } = &mut *rasters;
            let frame = *frame;
            match entries.get_mut(&key) {
                Some(raster) => raster.drawn = frame,
                // One raster of an SVG is made at a time, so a live resize settles on the last size.
                None => match (entries.values_mut()).find(|raster| {
                    Arc::ptr_eq(&raster.svg, &bytes) && matches!(raster.image, Rasterized::Pending)
                }) {
                    Some(pending) => pending.drawn = frame,
                    None => {
                        entries.insert(
                            key,
                            Raster {
                                svg: bytes.clone(),
                                image: Rasterized::Pending,
                                drawn: frame,
                            },
                        );
                        let renderer = cx.svg_renderer();
                        let svg = bytes.clone();
                        let job = cx.background_spawn(async move {
                            (renderer.parse_svg(&svg)).and_then(|tree| {
                                renderer.render_parsed(&tree, SvgSize::ExactSize(size))
                            })
                        });
                        let workspace = workspace.clone();
                        cx.spawn(async move |cx| {
                            let image = job.await;
                            workspace.update(cx, |this, cx| {
                                match this.rasters.borrow_mut().entries.get_mut(&key) {
                                    Some(raster) => {
                                        raster.image = match image {
                                            Ok(image) => Rasterized::Ready(image),
                                            Err(error) => {
                                                tracing::warn!(%error, "rasterising an SVG failed");
                                                Rasterized::Failed
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
                Some(Raster {
                    image: Rasterized::Ready(image),
                    ..
                }) => return Some(image.clone()),
                Some(_) | None => {}
            }
            // While this size is made, a raster of another size stands in.
            (entries.values_mut())
                .filter(|raster| Arc::ptr_eq(&raster.svg, &bytes))
                .find_map(|raster| match &raster.image {
                    Rasterized::Ready(image) => {
                        raster.drawn = frame;
                        Some(image.clone())
                    }
                    Rasterized::Pending | Rasterized::Failed => None,
                })
        },
        |bounds, image, window, _| match image {
            Some(image) => {
                match window.paint_image(bounds, bounds, Corners::default(), image, 0, false) {
                    Ok(()) => {}
                    Err(error) => tracing::warn!(%error, "painting an SVG raster failed"),
                }
            }
            None => {}
        },
    )
}
