use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use gpui::{
    App, AppContext as _, Bounds, Pixels, RenderImage, WeakEntity, Window, point, px, size,
};
use helix_core::SmallVec;
use reader::pdf::{Bgra, Document, Grid, Level, PdfError, TILE, Tile, TileBlock};

use crate::workspace::Workspace;

pub const TILE_BYTES_KEPT: usize = 384 << 20;
pub const FRAMES_BEFORE_STALE: u64 = 2;
pub const STRIP_ROWS: u16 = 2;

#[derive(Default)]
pub struct PageTiles {
    pub frame: Arc<AtomicU64>,
    pub entries: HashMap<TileKey, TileEntry>,
}

#[derive(Clone, PartialEq, Eq, Hash)]
pub struct TileKey {
    pub document: SameDocument,
    pub tile: Tile,
}

#[derive(Clone)]
pub struct SameDocument(pub Arc<Document>);

pub struct TileEntry {
    pub wanted: Arc<AtomicU64>,
    pub state: TileState,
}

pub enum TileState {
    Queued,
    Ready(Arc<RenderImage>),
    Failed,
}

pub enum Rendered {
    Done(Result<Vec<(Tile, Bgra)>, PdfError>),
    Stale(SmallVec<[Tile; 16]>),
}

pub enum Looked {
    Ready(Arc<RenderImage>),
    Waiting,
    Missing,
}

pub struct PageOnScreen {
    pub document: Arc<Document>,
    pub page: usize,
    pub area: Bounds<Pixels>,
}

pub struct TileShown {
    pub bounds: Bounds<Pixels>,
    pub image: Arc<RenderImage>,
}

pub fn tiles_for(
    tiles: &Rc<RefCell<PageTiles>>,
    workspace: &WeakEntity<Workspace>,
    shown: &PageOnScreen,
    view: Bounds<Pixels>,
    window: &mut Window,
    cx: &mut App,
) -> SmallVec<[TileShown; 16]> {
    let PageOnScreen {
        document,
        page,
        area,
    } = shown;
    let page_size = document.pages.sizes()[*page];
    let points_to_pixels = f32::from(area.size.width) / page_size.width.get();
    let level = Level::covering(points_to_pixels * window.scale_factor());
    let whole = Level::whole_page(page_size);
    let grid = Grid::of(page_size, level);
    let placed = |tile: Tile| {
        let per_pixel = 1.0 / tile.level.pixels_per_point() * points_to_pixels;
        let edge = f32::from(TILE) * per_pixel;
        let full = Grid::of(page_size, tile.level);
        let width = (full.width - u32::from(tile.column) * u32::from(TILE)).min(u32::from(TILE));
        let height = (full.height - u32::from(tile.row) * u32::from(TILE)).min(u32::from(TILE));
        Bounds::new(
            point(
                area.origin.x + px(f32::from(tile.column) * edge),
                area.origin.y + px(f32::from(tile.row) * edge),
            ),
            size(px(width as f32 * per_pixel), px(height as f32 * per_pixel)),
        )
    };
    let margin = px(f32::from(TILE) / window.scale_factor());
    let reach = Bounds::from_corners(
        point(view.origin.x - margin, view.origin.y - margin),
        point(
            view.origin.x + view.size.width + margin,
            view.origin.y + view.size.height + margin,
        ),
    );
    let seen = area.intersect(&reach);
    let cell = |from: Pixels, offset: Pixels, cells: u16| {
        let pixels = f32::from(from - offset) / points_to_pixels * level.pixels_per_point();
        ((pixels / f32::from(TILE)).floor().max(0.0) as u16).min(cells.saturating_sub(1))
    };
    let (first_column, last_column) = (
        cell(seen.origin.x, area.origin.x, grid.columns),
        cell(seen.origin.x + seen.size.width, area.origin.x, grid.columns),
    );
    let (first_row, last_row) = (
        cell(seen.origin.y, area.origin.y, grid.rows),
        cell(seen.origin.y + seen.size.height, area.origin.y, grid.rows),
    );
    let centre = view.center();
    let mut wanted: SmallVec<[Tile; 32]> = (first_row..=last_row)
        .flat_map(|row| {
            (first_column..=last_column).map(move |column| Tile {
                page: *page,
                level,
                column,
                row,
            })
        })
        .collect();
    wanted.sort_by(|a, b| {
        let distance = |tile: &Tile| {
            let middle = placed(*tile).center();
            f32::from((middle.x - centre.x).abs() + (middle.y - centre.y).abs())
        };
        distance(a).total_cmp(&distance(b))
    });
    let mut cache = tiles.borrow_mut();
    let frame = cache.frame.load(Ordering::Relaxed);
    let whole_block = TileBlock {
        page: *page,
        level: whole,
        columns: 0..1,
        rows: 0..1,
    };
    let under = match look(&cache, document, whole_tile(*page, whole), frame) {
        Looked::Ready(image) => Some(image),
        Looked::Waiting => None,
        Looked::Missing => {
            queue(&mut cache, workspace, document, whole_block, frame, cx);
            None
        }
    };
    let exact: SmallVec<[(Tile, Looked); 32]> = (wanted.into_iter())
        .map(|tile| (tile, look(&cache, document, tile, frame)))
        .collect();
    let mut strips: SmallVec<[TileBlock; 8]> = SmallVec::new();
    for (tile, _) in exact
        .iter()
        .filter(|(_, looked)| matches!(looked, Looked::Missing))
    {
        let band = tile.row / STRIP_ROWS * STRIP_ROWS;
        match strips.iter_mut().find(|strip| strip.rows.start == band) {
            Some(strip) => {
                strip.columns =
                    strip.columns.start.min(tile.column)..strip.columns.end.max(tile.column + 1)
            }
            None => strips.push(TileBlock {
                page: *page,
                level,
                columns: tile.column..tile.column + 1,
                rows: band..(band + STRIP_ROWS).min(grid.rows),
            }),
        }
    }
    let _span = tracing::trace_span!(
        "pdf tiles",
        page,
        level = level.0,
        wanted = exact.len(),
        ready = exact
            .iter()
            .filter(|(_, looked)| matches!(looked, Looked::Ready(_)))
            .count(),
        strips = strips.len()
    )
    .entered();
    for strip in strips {
        queue(&mut cache, workspace, document, strip, frame, cx);
    }
    let mut shown: SmallVec<[TileShown; 16]> = SmallVec::new();
    shown.extend(under.map(|image| TileShown {
        bounds: *area,
        image,
    }));
    let gaps: SmallVec<[Bounds<Pixels>; 32]> = (exact.iter())
        .filter_map(|(tile, looked)| match looked {
            Looked::Ready(_) => None,
            Looked::Waiting | Looked::Missing => Some(placed(*tile)),
        })
        .collect();
    let mut stand_ins: SmallVec<[(Tile, Arc<RenderImage>); 16]> = (cache.entries.iter())
        .filter_map(|(key, entry)| match &entry.state {
            TileState::Ready(image)
                if Arc::ptr_eq(&key.document.0, document)
                    && key.tile.page == *page
                    && key.tile.level != level
                    && key.tile.level != whole
                    && gaps.iter().any(|gap| placed(key.tile).intersects(gap)) =>
            {
                entry.wanted.store(frame, Ordering::Relaxed);
                Some((key.tile, image.clone()))
            }
            TileState::Ready(_) | TileState::Queued | TileState::Failed => None,
        })
        .collect();
    stand_ins.sort_by_key(|(tile, _)| tile.level);
    shown.extend(stand_ins.into_iter().map(|(tile, image)| TileShown {
        bounds: placed(tile),
        image,
    }));
    shown.extend(exact.into_iter().filter_map(|(tile, looked)| match looked {
        Looked::Ready(image) => Some(TileShown {
            bounds: placed(tile),
            image,
        }),
        Looked::Waiting | Looked::Missing => None,
    }));
    shown
}

pub fn prefetch(
    tiles: &Rc<RefCell<PageTiles>>,
    workspace: &WeakEntity<Workspace>,
    document: &Arc<Document>,
    page: usize,
    cx: &mut App,
) {
    let Some(size) = document.pages.sizes().get(page) else {
        return;
    };
    let whole = Level::whole_page(*size);
    let mut cache = tiles.borrow_mut();
    let frame = cache.frame.load(Ordering::Relaxed);
    match look(&cache, document, whole_tile(page, whole), frame) {
        Looked::Missing => {
            let block = TileBlock {
                page,
                level: whole,
                columns: 0..1,
                rows: 0..1,
            };
            queue(&mut cache, workspace, document, block, frame, cx)
        }
        Looked::Ready(_) | Looked::Waiting => {}
    }
}

fn whole_tile(page: usize, level: Level) -> Tile {
    Tile {
        page,
        level,
        column: 0,
        row: 0,
    }
}

fn look(cache: &PageTiles, document: &Arc<Document>, tile: Tile, frame: u64) -> Looked {
    let key = TileKey {
        document: SameDocument(document.clone()),
        tile,
    };
    match cache.entries.get(&key) {
        Some(entry) => {
            entry.wanted.store(frame, Ordering::Relaxed);
            match &entry.state {
                TileState::Ready(image) => Looked::Ready(image.clone()),
                TileState::Queued | TileState::Failed => Looked::Waiting,
            }
        }
        None => Looked::Missing,
    }
}

fn queue(
    cache: &mut PageTiles,
    workspace: &WeakEntity<Workspace>,
    document: &Arc<Document>,
    block: TileBlock,
    frame: u64,
    cx: &mut App,
) {
    let tiles: SmallVec<[Tile; 16]> = (block.rows.clone())
        .flat_map(|row| {
            block.columns.clone().map(move |column| Tile {
                page: block.page,
                level: block.level,
                column,
                row,
            })
        })
        .collect();
    let wanted: SmallVec<[Arc<AtomicU64>; 16]> = (tiles.iter())
        .map(|tile| {
            let key = TileKey {
                document: SameDocument(document.clone()),
                tile: *tile,
            };
            let entry = cache.entries.entry(key).or_insert_with(|| TileEntry {
                wanted: Arc::new(AtomicU64::new(frame)),
                state: TileState::Queued,
            });
            entry.wanted.clone()
        })
        .collect();
    let (now, rendering) = (cache.frame.clone(), document.clone());
    let job = cx.background_spawn(async move {
        let current = now.load(Ordering::Relaxed);
        let since = (wanted.iter())
            .map(|wanted| current.saturating_sub(wanted.load(Ordering::Relaxed)))
            .min();
        match since {
            Some(0..=FRAMES_BEFORE_STALE) => Rendered::Done(rendering.render_tiles(block)),
            Some(_) | None => Rendered::Stale(tiles),
        }
    });
    let (workspace, document) = (workspace.clone(), document.clone());
    cx.spawn(async move |cx| {
        let rendered = job.await;
        workspace.update(cx, |this, cx| {
            let mut cache = this.page_tiles.borrow_mut();
            let key = |tile: Tile| TileKey {
                document: SameDocument(document.clone()),
                tile,
            };
            match rendered {
                Rendered::Stale(tiles) => {
                    let _span =
                        tracing::trace_span!("pdf tiles stale", count = tiles.len()).entered();
                    for tile in tiles {
                        cache.entries.remove(&key(tile));
                    }
                }
                Rendered::Done(Err(error)) => {
                    tracing::warn!(%error, "rendering page tiles failed")
                }
                Rendered::Done(Ok(rendered)) => {
                    for (tile, bgra) in rendered {
                        let image = image::RgbaImage::from_raw(
                            u32::from(bgra.width),
                            u32::from(bgra.height),
                            bgra.pixels,
                        );
                        match (cache.entries.get_mut(&key(tile)), image) {
                            (Some(entry), Some(pixels)) => {
                                entry.state = TileState::Ready(Arc::new(RenderImage::new(
                                    SmallVec::from_elem(image::Frame::new(pixels), 1),
                                )))
                            }
                            (Some(entry), None) => entry.state = TileState::Failed,
                            (None, _) => {}
                        }
                    }
                    cx.notify();
                }
            }
        })
    })
    .detach();
}

pub fn evict(tiles: &mut PageTiles, window: &mut Window, cx: &mut App) {
    let frame = tiles.frame.load(Ordering::Relaxed);
    let since = |entry: &TileEntry| frame.saturating_sub(entry.wanted.load(Ordering::Relaxed));
    tiles
        .entries
        .retain(|_, entry| match (&entry.state, since(entry)) {
            (TileState::Queued | TileState::Failed, 0..=FRAMES_BEFORE_STALE) => true,
            (TileState::Queued | TileState::Failed, _) => false,
            (TileState::Ready(_), _) => true,
        });
    let bytes = |image: &RenderImage| {
        let size = image.size(0);
        size.width.0 as usize * size.height.0 as usize * 4
    };
    let mut ready: SmallVec<[(u64, TileKey, usize); 64]> = (tiles.entries.iter())
        .filter_map(|(key, entry)| match &entry.state {
            TileState::Ready(image) => Some((since(entry), key.clone(), bytes(image))),
            TileState::Queued | TileState::Failed => None,
        })
        .collect();
    let mut kept: usize = ready.iter().map(|(_, _, bytes)| bytes).sum();
    ready.sort_by_key(|(since, ..)| std::cmp::Reverse(*since));
    let mut dropped = 0;
    for (_, key, bytes) in ready.into_iter().filter(|(since, ..)| *since > 0) {
        match kept.checked_sub(TILE_BYTES_KEPT) {
            Some(1..) => {}
            Some(0) | None => break,
        }
        match tiles.entries.remove(&key).map(|entry| entry.state) {
            Some(TileState::Ready(image)) => cx.drop_image(image, Some(window)),
            Some(TileState::Queued | TileState::Failed) | None => {}
        }
        kept -= bytes;
        dropped += 1;
    }
    let _span = tracing::trace_span!("pdf tiles kept", bytes = kept, dropped).entered();
    tiles.frame.store(frame + 1, Ordering::Relaxed);
}

impl PartialEq for SameDocument {
    fn eq(&self, other: &SameDocument) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

impl Eq for SameDocument {}

impl std::hash::Hash for SameDocument {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        Arc::as_ptr(&self.0).hash(state)
    }
}
