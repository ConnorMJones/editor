//! Rendered markdown and typst views: GPUI lists of blocks or pages, with typeset SVGs
//! rasterised at their drawn device size.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;

use gpui::{
    AnyElement, App, Bounds, Canvas, ClickEvent, Context, Corners, DevicePixels, Div,
    InteractiveText, IntoElement, ListAlignment, ListOffset, ListState, MouseButton, ObjectFit,
    Pixels, RenderImage, SharedString, Size, StrikethroughStyle, StyledText, SvgSize, TextAlign,
    TextRun, UnderlineStyle, WeakEntity, Window, canvas, div, img, list, prelude::*, px, white,
};
use helix_core::SmallVec;
use model::color::Base16;
use model::theme::Modifiers;
use model::typeset::{Diagnostic, Origin, PX_PER_PT, PagePoint, Pt, SourceFile, Svg};
use model::window::Px;
use state::project::links::{self, Origin as LinkOrigin, Resolution};
use state::render::markdown::{
    self, BlockKind, CalloutFold, CalloutKind, Column, Display, EmbedKind, Emphasis, Figure,
    FigureKind, ImageFile, Inline, LinkTarget, ListKind, Markdown, RowKind, Span, SpanKind, Task,
};
use state::render::rendered::{
    self as reading, BlockScroll, Fit, Parsed, Preview, RenderedView, Scroll, Zoom,
};
use state::render::typeset::{Compiled, Figures, Outcome, Render, Typst};
use state::text::buffer::{self, BufferId};
use state::view::layout::PaneId;

use crate::theme::{PROSE_LINE, Theme};
use crate::workspace::Workspace;

/// A rendered pane's list and what its measured items describe.
pub struct RenderedPane {
    pub list: ListState,
    pub buffer: BufferId,
    /// Document version (markdown) or compiled version (typst) the list was spliced to.
    pub version: u64,
    /// Item hashes: block source hashes, or typst page sizes.
    pub hashes: Vec<u64>,
    /// What else the items were measured with.
    pub measured: Measured,
    /// How far the pages are drawn panned left, as last applied.
    pub left: Pixels,
    /// Scroll last exchanged between the list and the state.
    pub applied: Applied,
}

#[derive(Clone, PartialEq)]
pub struct Measured {
    /// Vault generation, which resolves embeds.
    pub generation: u64,
    /// `Figures::generation`, which sizes figures.
    pub figures: u64,
    pub toggled: Vec<u32>,
    pub fit: Fit,
    pub zoom: Zoom,
    /// Viewport size the list was last laid out in; pages are sized from it.
    pub width: Pixels,
    pub height: Pixels,
}

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

/// The scroll the list was last set to or reported, unless items moved under it since.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Applied {
    Stale,
    At(BlockScroll),
}

pub enum Rasterized {
    Pending,
    Ready(Arc<RenderImage>),
    Failed,
}

/// Width of the reading column in ems of the prose, about 75 characters; narrower panes wrap sooner.
const MEASURE: f32 = 45.0;
/// Heading sizes relative to prose, `#` to `######`.
const SCALES: [f32; 6] = [1.6, 1.4, 1.25, 1.12, 1.05, 1.0];
/// Space between typst pages.
const PAGE_GAP: Pixels = px(16.);

/// Brings each rendered pane's list in line with the state and carries out its scroll.
pub fn sync(workspace: &mut Workspace, cx: &mut Context<Workspace>) {
    let Workspace {
        state,
        rendered,
        theme,
        window_height,
        ..
    } = workspace;
    rendered.retain(|pane, cache| {
        (state.layout.rendered(pane)).is_some_and(|view| view.text.buffer() == cache.buffer)
    });
    let panes: SmallVec<[PaneId; 4]> = (state.layout.panes.keys())
        .filter(|pane| state.layout.rendered(*pane).is_some())
        .collect();
    for pane in panes {
        let Some(view) = state.layout.rendered(pane) else {
            continue;
        };
        let (buffer, scroll, fit, zoom) = (view.text.buffer(), view.scroll, view.fit, view.zoom);
        let (version, hashes) = match state.previews.get(buffer) {
            Some(Preview::Markdown(parsed)) => (parsed.version, &parsed.markdown.hashes),
            Some(Preview::Typst(Typst {
                pages: Some(compiled),
                ..
            })) => (compiled.version, &compiled.sizes),
            Some(Preview::Typst(Typst { pages: None, .. })) | None => continue,
        };
        let cache = match rendered.get_mut(pane) {
            Some(cache) => cache,
            None => {
                let list = ListState::new(hashes.len(), ListAlignment::Top, *window_height);
                let workspace = cx.weak_entity();
                // The list scrolls itself on the wheel; the state hears of it once the list lets go.
                list.set_scroll_handler(move |_, window, cx| {
                    let workspace = workspace.clone();
                    window.defer(cx, move |_, cx| {
                        let _ = workspace.update(cx, |this, cx| {
                            let Some(cache) = this.rendered.get_mut(pane) else {
                                return;
                            };
                            let top = cache.list.logical_scroll_top();
                            let at = BlockScroll {
                                item: top.item_ix,
                                offset: Px(top.offset_in_item.into()),
                                left: Px(cache.left.into()),
                            };
                            cache.applied = Applied::At(at);
                            reading::scrolled(&mut this.state, pane, at);
                            this.flush(cx);
                        });
                    });
                });
                rendered.insert(
                    pane,
                    RenderedPane {
                        list,
                        buffer,
                        version,
                        hashes: hashes.clone(),
                        measured: Measured {
                            generation: state.vault.generation,
                            figures: state.figures.generation,
                            toggled: view.toggled.clone(),
                            fit: view.fit,
                            zoom: view.zoom,
                            width: px(0.),
                            height: px(0.),
                        },
                        left: px(0.),
                        applied: Applied::Stale,
                    },
                );
                &mut rendered[pane]
            }
        };
        match cache.version == version {
            true => {}
            false => {
                // Only the run between the unchanged ends is measured again.
                let old = &cache.hashes;
                let prefix = old.iter().zip(hashes).take_while(|(a, b)| a == b).count();
                let suffix = (old[prefix..].iter().rev())
                    .zip(hashes[prefix..].iter().rev())
                    .take_while(|(a, b)| a == b)
                    .count();
                cache
                    .list
                    .splice(prefix..old.len() - suffix, hashes.len() - prefix - suffix);
                cache.hashes.clone_from(hashes);
                cache.version = version;
                cache.applied = Applied::Stale;
            }
        }
        let measured = &mut cache.measured;
        match (
            measured.generation,
            measured.figures,
            measured.fit,
            measured.zoom,
            &measured.toggled,
        ) == (
            state.vault.generation,
            state.figures.generation,
            view.fit,
            view.zoom,
            &view.toggled,
        ) {
            true => {}
            false => {
                measured.generation = state.vault.generation;
                measured.figures = state.figures.generation;
                measured.fit = view.fit;
                measured.zoom = view.zoom;
                measured.toggled.clone_from(&view.toggled);
                cache.list.remeasure();
            }
        }
        let viewport = cache.list.viewport_bounds().size;
        let line = theme.prose_size * PROSE_LINE;
        let pages = match state.previews.get(buffer) {
            Some(Preview::Typst(Typst {
                pages: Some(compiled),
                ..
            })) => &compiled.pages[..],
            Some(Preview::Typst(Typst { pages: None, .. }) | Preview::Markdown(_)) | None => &[],
        };
        let (from, down, across) = match (scroll, cache.applied) {
            (Scroll::Line(_), _) => continue,
            (Scroll::At(at), Applied::At(applied)) if at == applied => continue,
            (Scroll::At(at), Applied::At(_) | Applied::Stale) => (at, px(0.), px(0.)),
            (Scroll::Moved { from, by }, _) => (
                from,
                line * by.lines as f32 + viewport.height * (by.half_pages as f32 / 2.),
                line * by.columns as f32,
            ),
            (Scroll::Point(at), _) => {
                let Some(page) = pages.get(at.page) else {
                    continue;
                };
                let scale = page_width(&cache.measured, fit, zoom, page) / px(page.width_pt);
                let gap = match at.page {
                    0 => PAGE_GAP,
                    _ => px(0.),
                };
                // The point lands a third of the way down, centred across.
                (
                    BlockScroll {
                        item: at.page,
                        offset: Px(0.0),
                        left: Px(0.0),
                    },
                    gap + px(at.y.0 * scale) - viewport.height / 3.,
                    PAGE_GAP + px(at.x.0 * scale) - viewport.width / 2.,
                )
            }
        };
        cache.list.scroll_to(ListOffset {
            item_ix: from.item,
            offset_in_item: px(0.),
        });
        cache.list.scroll_by(px(from.offset.0) + down);
        let widest = (pages.iter())
            .map(|page| page_width(&cache.measured, fit, zoom, page))
            .fold(px(0.), |widest, width| widest.max(width));
        let room = (widest + PAGE_GAP * 2. - viewport.width).max(px(0.));
        cache.left = (px(from.left.0) + across).max(px(0.)).min(room);
        let top = cache.list.logical_scroll_top();
        let landed = BlockScroll {
            item: top.item_ix,
            offset: Px(top.offset_in_item.into()),
            left: Px(cache.left.into()),
        };
        cache.applied = Applied::At(landed);
        match scroll {
            Scroll::At(at) if at == landed => {}
            Scroll::At(_) | Scroll::Moved { .. } | Scroll::Line(_) | Scroll::Point(_) => {
                reading::scrolled(state, pane, landed)
            }
        }
    }
}

/// Draws `svg` over its bounds, rasterised in the background at their device size.
fn svg_canvas(
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

/// The list drawing `pane`'s rendered view, under a typst preview's error strip.
pub fn body(workspace: &Workspace, pane: PaneId, cx: &mut Context<Workspace>) -> AnyElement {
    let Some(cache) = workspace.rendered.get(pane) else {
        return div().flex_1().into_any_element();
    };
    let list_state = cache.list.clone();
    let buffer = cache.buffer;
    let theme = &workspace.theme;
    let state = &workspace.state;
    let typst = match state.previews.get(buffer) {
        Some(Preview::Typst(typst)) => Some(typst),
        Some(Preview::Markdown(_)) | None => None,
    };
    let strip = |color: gpui::Hsla| {
        div()
            .flex_none()
            .w_full()
            .px_3()
            .py_1()
            .font(theme.ui.clone())
            .text_size(theme.ui_size)
            .text_color(color)
            .bg(color.opacity(0.12))
            .border_b_1()
            .border_color(theme.chrome.border)
    };
    // Clicking a diagnostic strip opens the first entry's place.
    let diagnostics = |listed: &[Diagnostic], mark: &str, color: gpui::Hsla| {
        let first = &listed[0];
        let place = match &first.origin {
            Origin::Span {
                file: SourceFile::Project(path),
                start,
                ..
            } => format!(
                "{}:{}:{}: ",
                path.strip_prefix(state.root.path())
                    .unwrap_or(path)
                    .display(),
                start.line + 1,
                start.column + 1
            ),
            Origin::Span {
                file: SourceFile::Embedded(name),
                start,
                ..
            } => format!("{name}:{}:{}: ", start.line + 1, start.column + 1),
            Origin::Detached => String::new(),
        };
        let more = match listed.len() {
            1 => String::new(),
            count => format!("  (+{} more)", count - 1),
        };
        let workspace = cx.weak_entity();
        strip(color)
            .id("typst-diagnostics")
            .cursor_pointer()
            .on_click(move |_: &ClickEvent, window, cx| {
                cx.stop_propagation();
                update(&workspace, window, cx, |this| {
                    reading::goto_diagnostic(&mut this.state, pane, &mut this.effects)
                });
            })
            .child(SharedString::from(format!(
                "{mark} {place}{}{more}",
                first.message
            )))
            .into_any_element()
    };
    let header = match typst.map(|typst| (&typst.outcome, &typst.pages)) {
        Some((Outcome::Failed(errors), _)) => {
            Some(diagnostics(errors, "✘", theme.colors[Base16::Base08]))
        }
        Some((Outcome::Pending | Outcome::Compiled, Some(Compiled { warnings, .. }))) => {
            match warnings.as_slice() {
                [] => None,
                warnings => Some(diagnostics(warnings, "⚠", theme.colors[Base16::Base0A])),
            }
        }
        Some((Outcome::Pending, None)) => Some(
            strip(theme.text_muted)
                .child("compiling…")
                .into_any_element(),
        ),
        Some((Outcome::Compiled, None)) | None => None,
    };
    div()
        .flex_1()
        .min_h_0()
        .flex()
        .flex_col()
        .children(header)
        .child(
            list(
                list_state,
                cx.processor(move |this, index: usize, _window, cx| item(this, pane, index, cx)),
            )
            .flex_1()
            .min_h_0(),
        )
        .into_any_element()
}

/// What a block's drawing needs from the workspace.
struct Draw<'a> {
    theme: &'a Theme,
    view: &'a RenderedView,
    markdown: &'a Markdown,
    figures: &'a Figures,
    rasters: &'a Rc<RefCell<Rasters>>,
    pane: PaneId,
    buffer: BufferId,
    version: u64,
    workspace: WeakEntity<Workspace>,
}

/// How a block's text is inked: done tasks read struck through and muted.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Ink {
    Plain,
    Done,
}

fn item(
    this: &mut Workspace,
    pane: PaneId,
    index: usize,
    cx: &mut Context<Workspace>,
) -> AnyElement {
    let state = &this.state;
    let theme = &this.theme;
    let Some(view) = state.layout.rendered(pane) else {
        return div().into_any_element();
    };
    let buffer = view.text.buffer();
    let content = match state.previews.get(buffer) {
        Some(Preview::Markdown(Parsed {
            version, markdown, ..
        })) => match markdown.roots.get(index) {
            Some(root) => {
                let draw = Draw {
                    theme,
                    view,
                    markdown,
                    figures: &state.figures,
                    rasters: &this.rasters,
                    pane,
                    buffer,
                    version: *version,
                    workspace: cx.weak_entity(),
                };
                block(&draw, *root, Ink::Plain)
            }
            None => div().into_any_element(),
        },
        Some(Preview::Typst(typst)) => {
            let (Some(page), Some(cache)) = (
                (typst.pages.as_ref()).and_then(|compiled| compiled.pages.get(index)),
                this.rendered.get(pane),
            ) else {
                return div().into_any_element();
            };
            let width = page_width(&cache.measured, view.fit, view.zoom, page);
            let scale = width / px(page.width_pt);
            // Recorded each frame so a click turns into a point on the page.
            let bounds = Rc::new(Cell::new(Bounds::default()));
            let (recorded, workspace) = (bounds.clone(), cx.weak_entity());
            let sheet = div()
                .relative()
                .flex_none()
                .mx_auto()
                .w(width)
                .h(width * (page.height_pt / page.width_pt))
                .bg(white())
                .shadow_md()
                .child(svg_canvas(&this.rasters, &cx.weak_entity(), page).size_full())
                .child(
                    canvas(move |area, _, _| recorded.set(area), |_, _, _, _| {})
                        .absolute()
                        .size_full(),
                )
                .on_mouse_down(MouseButton::Left, move |event, window, cx| {
                    cx.stop_propagation();
                    let origin = bounds.get().origin;
                    let at = PagePoint {
                        page: index,
                        x: Pt((event.position.x - origin.x) / px(1.) / scale),
                        y: Pt((event.position.y - origin.y) / px(1.) / scale),
                    };
                    update(&workspace, window, cx, |this| {
                        reading::click_page(&mut this.state, pane, at, &mut this.effects)
                    });
                });
            let sheet = match typst.outcome {
                Outcome::Failed(_) => sheet.opacity(0.45),
                Outcome::Pending | Outcome::Compiled => sheet,
            };
            return div()
                .w_full()
                .overflow_hidden()
                .px(PAGE_GAP)
                .pt(match index {
                    0 => PAGE_GAP,
                    _ => px(0.),
                })
                .pb(PAGE_GAP)
                .child(div().relative().left(-cache.left).flex().child(sheet))
                .into_any_element();
        }
        None => div().into_any_element(),
    };
    let top = match index {
        0 => theme.prose_size * 1.5,
        _ => theme.prose_size * 0.375,
    };
    div()
        .w_full()
        .flex()
        .justify_center()
        .px_6()
        .pt(top)
        .pb(theme.prose_size * 0.375)
        .font(theme.prose[0].clone())
        .text_size(theme.prose_size)
        .line_height(theme.prose_size * PROSE_LINE)
        .text_color(theme.text)
        .child(
            div()
                .w_full()
                .max_w(theme.prose_size * MEASURE)
                .child(content),
        )
        .into_any_element()
}

/// Runs the workspace with `f`, then redraws and keeps keys on the workspace.
fn update(
    workspace: &WeakEntity<Workspace>,
    window: &mut Window,
    cx: &mut App,
    f: impl FnOnce(&mut Workspace),
) {
    let _ = workspace.update(cx, |this, cx| {
        f(this);
        window.focus(&this.focus, cx);
        this.changed(cx);
    });
}

/// A block that puts the source cursor on its first line when clicked.
fn clickable(draw: &Draw, index: usize) -> gpui::Stateful<Div> {
    let (workspace, pane) = (draw.workspace.clone(), draw.pane);
    div()
        .id(("block", index))
        .on_click(move |_: &ClickEvent, window, cx| {
            cx.stop_propagation();
            update(&workspace, window, cx, |this| {
                reading::click(&mut this.state, pane, index)
            });
        })
}

fn block(draw: &Draw, index: usize, ink: Ink) -> AnyElement {
    let theme = draw.theme;
    let markdown = draw.markdown;
    let target = &markdown.blocks[index];
    let children = || markdown.children(index);
    match &target.kind {
        BlockKind::Heading { level, text } => {
            let scale = SCALES[*level as usize - 1];
            let size = theme.prose_size * scale;
            let heading = clickable(draw, index)
                .font_weight(gpui::FontWeight::BOLD)
                .text_size(size)
                .line_height(size * 1.3)
                .mt(size * 0.4)
                .child(inline(draw, text, index, ink, Modifiers::BOLD));
            match level {
                markdown::Level::H1 | markdown::Level::H2 => heading
                    .pb(theme.prose_size * 0.25)
                    .border_b_1()
                    .border_color(theme.chrome.border)
                    .into_any_element(),
                markdown::Level::H3
                | markdown::Level::H4
                | markdown::Level::H5
                | markdown::Level::H6 => heading.into_any_element(),
            }
        }
        BlockKind::Paragraph(text) => clickable(draw, index)
            .child(inline(draw, text, index, ink, Modifiers::empty()))
            .into_any_element(),
        BlockKind::List(kind) => {
            let depth = (markdown.blocks[..index].iter())
                .filter(|outer| matches!(outer.kind, BlockKind::List(_)) && outer.end >= target.end)
                .count();
            let bullet = match depth % 2 {
                0 => "•",
                _ => "◦",
            };
            div()
                .flex()
                .flex_col()
                .gap_1()
                .children(children().enumerate().map(|(n, item)| {
                    let task = match markdown.blocks[item].kind {
                        BlockKind::Item(task) => task,
                        _ => Task::None,
                    };
                    let marker = match (task, kind) {
                        (Task::Open { marker }, _) => checkbox(draw, marker, Ink::Plain),
                        (Task::Done { marker }, _) => checkbox(draw, marker, Ink::Done),
                        (Task::None, ListKind::Bullet) => div()
                            .text_color(theme.colors[Base16::Base03])
                            .child(bullet)
                            .into_any_element(),
                        (Task::None, ListKind::Ordered { start }) => div()
                            .text_color(theme.colors[Base16::Base03])
                            .child(format!("{}.", start + n as u64))
                            .into_any_element(),
                    };
                    let ink = match task {
                        Task::Done { .. } => Ink::Done,
                        Task::Open { .. } | Task::None => ink,
                    };
                    div()
                        .flex()
                        .flex_row()
                        .gap_2()
                        .child(
                            div()
                                .flex_none()
                                .min_w(theme.prose_size * 1.125)
                                .flex()
                                .justify_end()
                                .child(marker),
                        )
                        .child(
                            div().flex_1().min_w_0().flex().flex_col().gap_1().children(
                                markdown.children(item).map(|child| block(draw, child, ink)),
                            ),
                        )
                }))
                .into_any_element()
        }
        BlockKind::Item(_) => div()
            .children(children().map(|child| block(draw, child, ink)))
            .into_any_element(),
        BlockKind::Quote => div()
            .flex()
            .flex_col()
            .gap_2()
            .pl_4()
            .border_l(px(3.))
            .border_color(theme.colors[Base16::Base03])
            .text_color(theme.colors[Base16::Base04])
            .children(children().map(|child| block(draw, child, ink)))
            .into_any_element(),
        BlockKind::Callout(callout) => {
            let color = theme.colors[match callout.kind {
                CalloutKind::Note | CalloutKind::Info | CalloutKind::Todo => Base16::Base0D,
                CalloutKind::Abstract | CalloutKind::Tip => Base16::Base0C,
                CalloutKind::Success => Base16::Base0B,
                CalloutKind::Question => Base16::Base0A,
                CalloutKind::Warning => Base16::Base09,
                CalloutKind::Failure | CalloutKind::Danger | CalloutKind::Bug => Base16::Base08,
                CalloutKind::Example => Base16::Base0E,
                CalloutKind::Quote => Base16::Base04,
            }];
            let icon = match callout.kind {
                CalloutKind::Note => "✎",
                CalloutKind::Abstract | CalloutKind::Example => "☰",
                CalloutKind::Info => "ℹ",
                CalloutKind::Todo => "☑",
                CalloutKind::Tip => "✦",
                CalloutKind::Success => "✔",
                CalloutKind::Question => "?",
                CalloutKind::Warning => "⚠",
                CalloutKind::Failure => "✘",
                CalloutKind::Danger => "⚡",
                CalloutKind::Bug => "✱",
                CalloutKind::Quote => "❝",
            };
            let fold = draw.view.fold(callout);
            let chevron = match fold {
                CalloutFold::Static => "",
                CalloutFold::Open => "▾",
                CalloutFold::Closed => "▸",
            };
            let (workspace, pane, ordinal) = (draw.workspace.clone(), draw.pane, callout.index);
            let header = div()
                .id(("callout", index))
                .flex()
                .flex_row()
                .items_center()
                .gap_2()
                .text_color(color)
                .font_weight(gpui::FontWeight::BOLD)
                .child(icon)
                .child(
                    div()
                        .flex_1()
                        .child(SharedString::from(callout.title.clone())),
                )
                .child(chevron);
            let header = match fold {
                CalloutFold::Static => header,
                CalloutFold::Open | CalloutFold::Closed => {
                    header
                        .cursor_pointer()
                        .on_click(move |_: &ClickEvent, window, cx| {
                            cx.stop_propagation();
                            update(&workspace, window, cx, |this| {
                                reading::toggle_fold(&mut this.state, pane, ordinal)
                            });
                        })
                }
            };
            let body = match fold {
                CalloutFold::Static | CalloutFold::Open => Some(
                    div()
                        .flex()
                        .flex_col()
                        .gap_2()
                        .children(children().map(|child| block(draw, child, ink))),
                ),
                CalloutFold::Closed => None,
            };
            div()
                .flex()
                .flex_col()
                .gap_2()
                .px_3()
                .py_2()
                .rounded_sm()
                .border_l(px(3.))
                .border_color(color)
                .bg(color.opacity(0.08))
                .child(header)
                .children(body)
                .into_any_element()
        }
        BlockKind::Code(code) => {
            let runs = markdown.highlights.get(&code.hash).map(|runs| {
                let mut start = 0;
                runs.iter()
                    .map(|run| {
                        let len = run.end - start;
                        start = run.end;
                        TextRun {
                            len,
                            font: theme.mono_font(run.style.modifiers).clone(),
                            color: run.style.fg.map_or(theme.text, |slot| theme.colors[slot]),
                            background_color: None,
                            underline: None,
                            strikethrough: None,
                        }
                    })
                    .collect::<Vec<_>>()
            });
            clickable(draw, index)
                .child(code_box(
                    theme,
                    code.lang.clone().map(SharedString::from),
                    SharedString::from(code.text.clone()),
                    runs.filter(|runs| !runs.is_empty()),
                ))
                .into_any_element()
        }
        BlockKind::Math(math) => clickable(draw, index)
            .child(figure_element(draw, math.figure, &math.tex))
            .into_any_element(),
        BlockKind::Mermaid(mermaid) => clickable(draw, index)
            .child(figure_element(draw, mermaid.figure, &mermaid.src))
            .into_any_element(),
        BlockKind::Table(columns) => {
            let columns = columns.clone();
            div()
                .flex()
                .flex_col()
                .border_1()
                .border_color(theme.chrome.border)
                .children(children().map(|row| {
                    let head = match markdown.blocks[row].kind {
                        BlockKind::Row(RowKind::Head) => RowKind::Head,
                        _ => RowKind::Body,
                    };
                    let line = div()
                        .flex()
                        .flex_row()
                        .border_b_1()
                        .border_color(theme.chrome.border);
                    let line = match head {
                        RowKind::Head => line.bg(theme.surface).font_weight(gpui::FontWeight::BOLD),
                        RowKind::Body => line,
                    };
                    line.children(markdown.children(row).enumerate().map(|(n, cell)| {
                        let align = match columns.get(n).copied().unwrap_or(Column::None) {
                            Column::None | Column::Left => TextAlign::Left,
                            Column::Center => TextAlign::Center,
                            Column::Right => TextAlign::Right,
                        };
                        let content = match &markdown.blocks[cell].kind {
                            BlockKind::Cell(text) => {
                                let modifiers = match head {
                                    RowKind::Head => Modifiers::BOLD,
                                    RowKind::Body => Modifiers::empty(),
                                };
                                Some(inline(draw, text, cell, ink, modifiers))
                            }
                            _ => None,
                        };
                        div()
                            .flex_1()
                            .min_w_0()
                            .px_2()
                            .py_1()
                            .border_r_1()
                            .border_color(theme.chrome.border)
                            .text_align(align)
                            .children(content)
                    }))
                }))
                .into_any_element()
        }
        BlockKind::Row(_) | BlockKind::Cell(_) => div().into_any_element(),
        BlockKind::Image(image) => clickable(draw, index)
            .child(picture(theme, &image.file, &image.src))
            .into_any_element(),
        BlockKind::Embed(embed) => match &embed.kind {
            EmbedKind::Image(file) => clickable(draw, index)
                .child(picture(theme, file, &embed.link.target))
                .into_any_element(),
            EmbedKind::Note(resolution) => {
                let (label, shown) = match resolution {
                    Resolution::File(path) => ("embedded note", format!("⧉ {path}")),
                    Resolution::Missing(path) => ("embedded note not found", format!("⧉ {path}")),
                    Resolution::Invalid => ("embed", format!("⧉ {}", embed.link.target)),
                };
                let (workspace, pane, buffer) = (draw.workspace.clone(), draw.pane, draw.buffer);
                let target = LinkTarget::Wiki(embed.link.clone());
                div()
                    .id(("embed", index))
                    .cursor_pointer()
                    .on_click(move |_: &ClickEvent, window, cx| {
                        cx.stop_propagation();
                        update(&workspace, window, cx, |this| {
                            links::follow(
                                &mut this.state,
                                buffer,
                                &target,
                                LinkOrigin::Rendered(pane),
                                &mut this.effects,
                            )
                        });
                    })
                    .child(placeholder(theme, label, SharedString::from(shown)))
                    .into_any_element()
            }
        },
        BlockKind::Rule => div()
            .my_2()
            .h(px(1.))
            .w_full()
            .bg(theme.chrome.border)
            .into_any_element(),
        BlockKind::Html(html) => clickable(draw, index)
            .child(code_box(
                theme,
                Some(SharedString::from("html")),
                SharedString::from(html.clone()),
                None,
            ))
            .into_any_element(),
        BlockKind::Frontmatter(text) => clickable(draw, index)
            .child(code_box(
                theme,
                Some(SharedString::from("properties")),
                SharedString::from(text.clone()),
                None,
            ))
            .into_any_element(),
        BlockKind::Footnote { label } => div()
            .flex()
            .flex_row()
            .gap_2()
            .text_size(theme.prose_size * 0.9)
            .text_color(theme.text_muted)
            .child(
                div()
                    .flex_none()
                    .child(SharedString::from(format!("[{label}]"))),
            )
            .child(
                div()
                    .flex_1()
                    .flex()
                    .flex_col()
                    .children(children().map(|child| block(draw, child, ink))),
            )
            .into_any_element(),
    }
}

/// A clickable task checkbox flipping the `[ ]` at byte `marker` of the source.
fn checkbox(draw: &Draw, marker: usize, ink: Ink) -> AnyElement {
    let theme = draw.theme;
    let (workspace, buffer, version) = (draw.workspace.clone(), draw.buffer, draw.version);
    let accent = theme.chrome.accent;
    let box_ = div()
        .id(("task", marker))
        .mt(theme.prose_size * 0.25)
        .size(theme.prose_size)
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(3.))
        .border_1()
        .cursor_pointer()
        .text_size(theme.prose_size * 0.75)
        .hover(move |style| style.border_color(accent))
        .on_click(move |_: &ClickEvent, window, cx| {
            cx.stop_propagation();
            update(&workspace, window, cx, |this| {
                reading::toggle_task(&mut this.state, buffer, version, marker)
            });
        });
    match ink {
        Ink::Plain => box_.border_color(theme.colors[Base16::Base03]),
        Ink::Done => box_
            .border_color(accent)
            .bg(accent)
            .text_color(theme.background)
            .child("✓"),
    }
    .into_any_element()
}

/// Follows link `link` of block `index`'s text, looked up again at click time.
fn follow_link(
    workspace: &WeakEntity<Workspace>,
    buffer: BufferId,
    pane: PaneId,
    index: usize,
    link: usize,
    window: &mut Window,
    cx: &mut App,
) {
    update(workspace, window, cx, |this| {
        let state = &this.state;
        let target = (buffer::doc(&state.buffers, buffer))
            .and_then(|doc| reading::markdown(&state.previews, buffer, doc))
            .and_then(|parsed| parsed.blocks.get(index))
            .and_then(|block| block.inline())
            .and_then(|text| text.links.get(link))
            .map(|link| link.target.clone());
        match target {
            Some(target) => links::follow(
                &mut this.state,
                buffer,
                &target,
                LinkOrigin::Rendered(pane),
                &mut this.effects,
            ),
            None => {}
        }
    });
}

/// Styled text with clickable links; with math it flows as words and formulas on a shared baseline.
fn inline(draw: &Draw, text: &Inline, index: usize, ink: Ink, base: Modifiers) -> AnyElement {
    let theme = draw.theme;
    let flows = (text.spans.iter()).any(|span| matches!(span.kind, SpanKind::Math(_)));
    match flows {
        false => {
            let runs: Vec<TextRun> = (text.spans.iter())
                .map(|span| run(theme, span, ink, base))
                .collect();
            let styled = StyledText::new(SharedString::from(text.text.clone())).with_runs(runs);
            match text.links.is_empty() {
                true => styled.into_any_element(),
                false => {
                    let ranges = text.links.iter().map(|link| link.range.clone()).collect();
                    let (workspace, buffer, pane) =
                        (draw.workspace.clone(), draw.buffer, draw.pane);
                    InteractiveText::new(("links", index), styled)
                        .on_click(ranges, move |link, window, cx| {
                            cx.stop_propagation();
                            follow_link(&workspace, buffer, pane, index, link, window, cx);
                        })
                        .into_any_element()
                }
            }
        }
        true => {
            let depth =
                |svg: &Svg| (svg.height_pt - svg.baseline_pt.unwrap_or(svg.height_pt)) * PX_PER_PT;
            let text_depth = theme.prose_depth / px(1.);
            let inline_ready = |figure: Figure| match (figure.kind, draw.figures.get(figure)) {
                (FigureKind::Math(Display::Inline), Some(Render::Ready(svg))) => Some(svg),
                (FigureKind::Math(_) | FigureKind::Mermaid, _) => None,
            };
            let deepest = (text.spans.iter())
                .filter_map(|span| match span.kind {
                    SpanKind::Math(figure) => inline_ready(figure).map(depth),
                    SpanKind::Text
                    | SpanKind::Code
                    | SpanKind::Html
                    | SpanKind::Footnote
                    | SpanKind::Image => None,
                })
                .fold(text_depth, f32::max);
            let piece = |range: std::ops::Range<usize>, span: &Span| {
                let styled =
                    StyledText::new(SharedString::from(text.text[range.clone()].to_owned()))
                        .with_runs(vec![TextRun {
                            len: range.len(),
                            ..run(theme, span, ink, base)
                        }]);
                let piece = div().flex_none().pb(px(deepest - text_depth));
                match text
                    .links
                    .iter()
                    .position(|link| link.range.contains(&range.start))
                {
                    None => piece.child(styled).into_any_element(),
                    Some(link) => {
                        let (workspace, buffer, pane) =
                            (draw.workspace.clone(), draw.buffer, draw.pane);
                        piece
                            .id(("link", range.start))
                            .cursor_pointer()
                            .on_click(move |_: &ClickEvent, window, cx| {
                                cx.stop_propagation();
                                follow_link(&workspace, buffer, pane, index, link, window, cx);
                            })
                            .child(styled)
                            .into_any_element()
                    }
                }
            };
            let mut pieces: Vec<AnyElement> = Vec::with_capacity(text.spans.len() * 4);
            for span in text.spans.iter() {
                match span.kind {
                    SpanKind::Math(figure) => match (figure.kind, inline_ready(figure)) {
                        (_, Some(svg)) => pieces.push(
                            div()
                                .flex_none()
                                .pb(px(deepest - depth(svg)))
                                .child(
                                    svg_canvas(draw.rasters, &draw.workspace, svg)
                                        .w(px(svg.width_pt * PX_PER_PT))
                                        .h(px(svg.height_pt * PX_PER_PT)),
                                )
                                .into_any_element(),
                        ),
                        (FigureKind::Math(Display::Block) | FigureKind::Mermaid, _) => pieces.push(
                            div()
                                .w_full()
                                .py_1()
                                .child(figure_element(draw, figure, &text.text[span.range.clone()]))
                                .into_any_element(),
                        ),
                        (FigureKind::Math(Display::Inline), None) => {
                            pieces.push(piece(span.range.clone(), span))
                        }
                    },
                    SpanKind::Text
                    | SpanKind::Code
                    | SpanKind::Html
                    | SpanKind::Footnote
                    | SpanKind::Image => {
                        let mut start = span.range.start;
                        for word in
                            text.text[span.range.clone()].split_inclusive(char::is_whitespace)
                        {
                            let end = start + word.len();
                            match word.strip_suffix('\n') {
                                Some(line) => {
                                    match line.is_empty() {
                                        true => {}
                                        false => {
                                            pieces.push(piece(start..start + line.len(), span))
                                        }
                                    }
                                    pieces.push(div().w_full().into_any_element());
                                }
                                None => pieces.push(piece(start..end, span)),
                            }
                            start = end;
                        }
                    }
                }
            }
            div()
                .flex()
                .flex_row()
                .flex_wrap()
                .items_end()
                .children(pieces)
                .into_any_element()
        }
    }
}

fn run(theme: &Theme, span: &Span, ink: Ink, base: Modifiers) -> TextRun {
    let style = span.style;
    let missing = style.contains(Emphasis::MISSING);
    let modifiers =
        base | match style.contains(Emphasis::STRONG) {
            true => Modifiers::BOLD,
            false => Modifiers::empty(),
        } | match style.contains(Emphasis::ITALIC) || missing {
            true => Modifiers::ITALIC,
            false => Modifiers::empty(),
        };
    let prose = &theme.prose[(modifiers & (Modifiers::BOLD | Modifiers::ITALIC)).bits() as usize];
    let link = match missing {
        true => theme.text_muted,
        false => theme.colors[Base16::Base0D],
    };
    let (font, color, background) = match span.kind {
        SpanKind::Text | SpanKind::Image => (prose.clone(), theme.text, None),
        SpanKind::Code | SpanKind::Html => (
            theme.mono_font(modifiers).clone(),
            theme.colors[Base16::Base09],
            Some(theme.surface),
        ),
        SpanKind::Math(_) => (
            theme.mono_font(modifiers).clone(),
            theme.colors[Base16::Base0E],
            Some(theme.surface),
        ),
        SpanKind::Footnote => (prose.clone(), link, None),
    };
    let color = match (style.contains(Emphasis::LINK), span.kind, ink) {
        (true, _, _) => link,
        (false, SpanKind::Image, _) => theme.text_muted,
        (false, _, Ink::Done) => theme.text_muted,
        (false, _, Ink::Plain) => color,
    };
    let struck = style.contains(Emphasis::STRIKE) || ink == Ink::Done;
    TextRun {
        len: span.range.len(),
        font,
        color,
        background_color: background,
        underline: match style.contains(Emphasis::LINK) {
            true => Some(UnderlineStyle {
                thickness: px(1.),
                color: Some(link),
                wavy: false,
            }),
            false => None,
        },
        strikethrough: match struck {
            true => Some(StrikethroughStyle {
                thickness: px(1.),
                color: Some(color),
            }),
            false => None,
        },
    }
}

/// A monospace box with an optional label, highlighted when `runs` are given.
fn code_box(
    theme: &Theme,
    label: Option<SharedString>,
    text: SharedString,
    runs: Option<Vec<TextRun>>,
) -> Div {
    let styled = match runs {
        Some(runs) => StyledText::new(text).with_runs(runs),
        None => StyledText::new(text),
    };
    div()
        .relative()
        .w_full()
        .px_3()
        .py_2()
        .rounded_md()
        .bg(theme.surface)
        .font(theme.mono[0].clone())
        .text_size(theme.mono_size * 0.9)
        .line_height(theme.mono_size * 1.3)
        .whitespace_normal()
        .overflow_hidden()
        .child(styled)
        .children(label.map(|label| {
            div()
                .absolute()
                .top_1()
                .right_2()
                .text_size(theme.ui_size * 0.85)
                .text_color(theme.text_muted)
                .child(label)
        }))
}

/// A muted box standing in for content that cannot be shown yet.
fn placeholder(theme: &Theme, label: &'static str, text: SharedString) -> AnyElement {
    div()
        .w_full()
        .px_3()
        .py_2()
        .rounded_md()
        .border_1()
        .border_dashed()
        .border_color(theme.chrome.border)
        .text_color(theme.text_muted)
        .flex()
        .flex_col()
        .child(
            div()
                .text_size(theme.ui_size * 0.85)
                .font(theme.ui.clone())
                .child(label),
        )
        .child(
            div()
                .font(theme.mono[0].clone())
                .text_size(theme.mono_size * 0.9)
                .child(text),
        )
        .into_any_element()
}

/// A typeset figure: its image once ready, else its source (and error) in a muted box.
fn figure_element(draw: &Draw, figure: Figure, source: &str) -> AnyElement {
    let theme = draw.theme;
    let label = match figure.kind {
        FigureKind::Math(_) => "math",
        FigureKind::Mermaid => "mermaid",
    };
    match draw.figures.get(figure) {
        Some(Render::Ready(svg)) => {
            let (width, height) = (svg.width_pt * PX_PER_PT, svg.height_pt * PX_PER_PT);
            div()
                .w_full()
                .flex()
                .justify_center()
                .child(
                    svg_canvas(draw.rasters, &draw.workspace, svg)
                        .w_full()
                        .max_w(px(width))
                        .aspect_ratio(width / height),
                )
                .into_any_element()
        }
        Some(Render::Failed(error)) => placeholder(
            theme,
            label,
            SharedString::from(format!("{error}\n{source}")),
        ),
        Some(Render::Pending) | None => {
            placeholder(theme, label, SharedString::from(source.to_owned()))
        }
    }
}

fn picture(theme: &Theme, file: &ImageFile, src: &str) -> AnyElement {
    match file {
        ImageFile::Local(path) => {
            let (missing, theme_muted) = (SharedString::from(format!("🖼 {src}")), theme.text_muted);
            img(path.clone())
                .max_w_full()
                .object_fit(ObjectFit::ScaleDown)
                .with_fallback(move || {
                    div()
                        .text_color(theme_muted)
                        .child(missing.clone())
                        .into_any_element()
                })
                .into_any_element()
        }
        ImageFile::Remote(url) => {
            placeholder(theme, "remote image", SharedString::from(url.clone()))
        }
        ImageFile::Missing => {
            placeholder(theme, "image not found", SharedString::from(src.to_owned()))
        }
    }
}

/// Drawn width of typst page `page` under `fit` and `zoom`, in a viewport measured at `measured`.
fn page_width(measured: &Measured, fit: Fit, zoom: Zoom, page: &Svg) -> Pixels {
    let room = (measured.width - PAGE_GAP * 2.).max(px(1.));
    let fitted = match fit {
        Fit::Width => room,
        Fit::Page => room
            .min((measured.height - PAGE_GAP * 2.).max(px(1.)) * (page.width_pt / page.height_pt)),
    };
    fitted * zoom.factor()
}
