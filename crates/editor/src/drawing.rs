//! A drawing's pane body: paper, patterns and strokes, drawn through meshes the GPU keeps.

use std::borrow::Cow;
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

use gpui::{
    App, Bounds, ContentMask, Element, ElementId, Entity, GlobalElementId, Hsla,
    InspectorElementId, IntoElement, LayoutId, MeshData, MeshKey, MeshVersion, PathBuilder, Pixels,
    RenderImage, ScrollWheelEvent, Size, Style, TransformationMatrix, Window, fill, point, px,
    relative, size,
};
use helix_core::SmallVec;
use ink::doc::{
    Background, Board, Drawing, GeometryId, Ink, MarkId, PageIndex, Paged, Pattern, Underlay,
    Visibility,
};
use ink::input::{Pressure, Tilt};
use ink::mesh::Mesh;
use ink::text::{Caret, Inked};
use ink::tool::{Corner, Handle, Selecting, Shaping, Using};
use model::color::Rgb;
use model::geom::{Affine, Local, Pane, Point, Rect, Surface};
use model::picture::{Picture, Source};
use model::sheet::{Measure, Viewport};
use model::theme::Color;
use model::units::Px;
use slotmap::Key as _;
use state::drawing::frame::{self, InkFrame, Paper, REACH};
use state::drawing::pen::{self, PenPhase, PenSample, Stylus};
use state::drawing::typing::Writing;
use state::drawing::{Doing, Found, Hand, InkTabs, Notes, PdfMode, Reading, TextSpan};
use state::view::layout::PaneId;
use state::view::tab::Tab;

use crate::fonts::hsla;
use crate::page_tiles::{PageOnScreen, TileShown, prefetch, tiles_for};
use crate::pictures::{Drawn, decoded, paint_picture};
use crate::workspace::Workspace;

/// The window's pixels.
pub enum Screen {}

pub struct DrawingElement {
    pub workspace: Entity<Workspace>,
    pub pane: PaneId,
}

impl IntoElement for DrawingElement {
    type Element = Self;

    fn into_element(self) -> Self {
        self
    }
}

impl Element for DrawingElement {
    type RequestLayoutState = ();
    type PrepaintState = Pictured;

    fn id(&self) -> Option<ElementId> {
        Some(ElementId::NamedInteger(
            "drawing".into(),
            self.pane.data().as_ffi(),
        ))
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, ()) {
        let mut style = Style::default();
        style.size.width = relative(1.).into();
        style.size.height = relative(1.).into();
        (window.request_layout(style, [], cx), ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) -> Pictured {
        let pane = self.pane;
        let workspace = self.workspace.read(cx);
        (workspace.areas.drawings.borrow_mut()).insert(pane, bounds);
        let Some(viewport) = viewport(bounds) else {
            return Pictured::default();
        };
        let state = &workspace.state;
        let Placed { to_screen, view } = placed(bounds, viewport);
        let Wants { last, marks, pages } =
            match state.layout.panes().get(pane).and_then(|p| p.active()) {
                Some(Tab::Ink(InkTabs::Paged(tab))) => {
                    let paged = state.pageds[tab.file()].edited.drawing();
                    let frame = frame::paged(paged, &tab.view, viewport);
                    Wants {
                        last: tab.hand.last,
                        marks: wanted::<Paged>(&frame, paged, &to_screen, &view),
                        pages: pages_wanted(&frame, paged, &to_screen, &view),
                    }
                }
                Some(Tab::Ink(InkTabs::Pdf(tab))) => {
                    let paged = state.pdfs[tab.file()].ink.edited.drawing();
                    let frame = frame::paged(paged, &tab.view, viewport);
                    Wants {
                        last: tab.hand.last,
                        marks: wanted::<Paged>(&frame, paged, &to_screen, &view),
                        pages: pages_wanted(&frame, paged, &to_screen, &view),
                    }
                }
                Some(Tab::Ink(InkTabs::Board(tab))) => {
                    let board = state.boards[tab.file()].edited.drawing();
                    let frame = frame::board(board, &tab.view, viewport);
                    Wants {
                        last: tab.hand.last,
                        marks: wanted::<Board>(&frame, board, &to_screen, &view),
                        pages: SmallVec::new(),
                    }
                }
                Some(Tab::Text(_) | Tab::Image(_)) | None => return Pictured::default(),
            };
        let measure = Measure {
            viewport,
            line: Px(f32::from(workspace.fonts.cell.height)),
        };
        let (pictures, weak) = (workspace.pictures.clone(), self.workspace.downgrade());
        let page_tiles = workspace.page_tiles.clone();
        // Keys that move by part of the pane hear of a new size after this frame.
        match last {
            Some(seen) if seen == measure => {}
            Some(_) | None => {
                let workspace = self.workspace.clone();
                window.defer(cx, move |_, cx| {
                    workspace.update(cx, |workspace, _| {
                        state::view::sheet::measured(&mut workspace.state, pane, measure);
                    })
                });
            }
        }
        let view_bounds = Bounds::from_corners(
            point(px(view.min().x), px(view.min().y)),
            point(px(view.max().x), px(view.max().y)),
        );
        let neighbours = (pages.first().zip(pages.last()))
            .map(|(first, last)| (first.document.clone(), first.page, last.page));
        let shown_pages = (pages.iter())
            .map(|shown| PageShown {
                page: shown.page as u32,
                tiles: tiles_for(&page_tiles, &weak, shown, view_bounds, window, cx),
            })
            .collect();
        match neighbours {
            Some((document, first, last)) => {
                for page in [first.checked_sub(1), Some(last + 1)].into_iter().flatten() {
                    prefetch(&page_tiles, &weak, &document, page, cx);
                }
            }
            None => {}
        }
        Pictured {
            marks: (marks.into_iter())
                .filter_map(|Wanted { id, picture, size }| {
                    let area = Bounds::new(point(px(0.0), px(0.0)), size);
                    let image =
                        decoded(&pictures, &weak, &Drawn::Picture(picture), area, window, cx)?;
                    Some(ImageShown { id, image })
                })
                .collect(),
            pages: shown_pages,
        }
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut (),
        pictured: &mut Pictured,
        window: &mut Window,
        cx: &mut App,
    ) {
        let _span = tracing::trace_span!("drawing paint").entered();
        let workspace = self.workspace.read(cx);
        let state = &workspace.state;
        let desk = hsla(workspace.theme.surface);
        let accent = hsla(workspace.theme.editor.cursor_primary);
        let Some(viewport) = viewport(bounds) else {
            return;
        };
        let Placed { to_screen, view } = placed(bounds, viewport);
        window.with_content_mask(Some(ContentMask { bounds }), |window| {
            window.paint_quad(fill(bounds, desk));
            let screen = Onscreen {
                to_screen,
                view,
                accent,
            };
            match state.layout.panes().get(self.pane).and_then(|p| p.active()) {
                Some(Tab::Ink(InkTabs::Paged(tab))) => {
                    let paged = state.pageds[tab.file()].edited.drawing();
                    let frame = frame::paged(paged, &tab.view, viewport);
                    paint_ink::<Paged>(&frame, &tab.hand, paged, &screen, pictured, window)
                }
                Some(Tab::Ink(InkTabs::Pdf(tab))) => {
                    let file = &state.pdfs[tab.file()];
                    let paged = file.ink.edited.drawing();
                    let frame = frame::paged(paged, &tab.view, viewport);
                    paint_ink::<Paged>(&frame, &tab.hand, paged, &screen, pictured, window);
                    let reader = &workspace.theme.reader;
                    let mut marked: SmallVec<[(&TextSpan, Hsla); 16]> = SmallVec::new();
                    match &file.notes {
                        Notes::Listed(listed) => marked.extend(
                            (listed.notebook.highlights.iter())
                                .filter(|highlight| frame.on(highlight.span.page).is_some())
                                .map(|highlight| (&highlight.span, hsla(reader.highlight))),
                        ),
                        Notes::Unlisted(_) => {}
                    }
                    match &tab.found {
                        Found::Hits { hits, at, .. } => marked.extend(
                            (hits.iter().enumerate())
                                .filter(|(_, hit)| frame.on(hit.page).is_some())
                                .map(|(index, hit)| match index == *at {
                                    true => (hit, hsla(reader.current_hit)),
                                    false => (hit, hsla(reader.hit)),
                                }),
                        ),
                        Found::Nothing | Found::Searching { .. } => {}
                    }
                    let selecting;
                    match &tab.mode {
                        PdfMode::Read(Reading::Selecting { page, anchor, head }) => {
                            selecting = TextSpan {
                                page: *page,
                                chars: *anchor.min(head)..*anchor.max(head) + 1,
                            };
                            marked.push((&selecting, hsla(reader.selection)))
                        }
                        PdfMode::Read(Reading::Selected(span)) => {
                            marked.push((span, hsla(reader.selection)))
                        }
                        PdfMode::Read(Reading::OnHighlight(highlight)) => {
                            marked.push((&highlight.span, hsla(reader.selection)))
                        }
                        PdfMode::Read(Reading::Idle) | PdfMode::Markup => {}
                    }
                    for (span, color) in marked {
                        let (Some(text), Some(shown)) =
                            (file.document().text(span.page.0), frame.on(span.page))
                        else {
                            continue;
                        };
                        let to_screen = shown.to_pane.then(&screen.to_screen);
                        let mut path = PathBuilder::fill();
                        for char in text.chars.get(span.chars.clone()).unwrap_or_default() {
                            for (index, corner) in char.quad.into_iter().enumerate() {
                                let at = to_screen.apply(corner);
                                match index {
                                    0 => path.move_to(point(px(at.x), px(at.y))),
                                    _ => path.line_to(point(px(at.x), px(at.y))),
                                }
                            }
                            path.close();
                        }
                        match path.build() {
                            Ok(path) => window.paint_path(path, color),
                            Err(error) => tracing::warn!(%error, "marked text"),
                        }
                    }
                }
                Some(Tab::Ink(InkTabs::Board(tab))) => {
                    let board = state.boards[tab.file()].edited.drawing();
                    let frame = frame::board(board, &tab.view, viewport);
                    paint_ink::<Board>(&frame, &tab.hand, board, &screen, pictured, window)
                }
                Some(Tab::Text(_) | Tab::Image(_)) | None => {}
            }
        });
    }
}

/// Where a pane sits in the window, and the part of the window it shows.
pub struct Placed {
    pub to_screen: Affine<Pane, Screen>,
    pub view: Rect<Screen>,
}

pub fn placed(bounds: Bounds<Pixels>, viewport: Viewport) -> Placed {
    let to_screen: Affine<Pane, Screen> =
        Affine::translation(f32::from(bounds.origin.x), f32::from(bounds.origin.y));
    let view = Rect::spanning(
        to_screen.apply(Point::new(0.0, 0.0)),
        to_screen.apply(Point::new(viewport.width.get(), viewport.height.get())),
    );
    Placed { to_screen, view }
}

/// The image marks and PDF pages a frame shows, decoded for this paint.
#[derive(Default)]
pub struct Pictured {
    pub marks: SmallVec<[ImageShown; 4]>,
    pub pages: SmallVec<[PageShown; 2]>,
}

pub struct PageShown {
    pub page: u32,
    pub tiles: SmallVec<[TileShown; 16]>,
}

pub struct ImageShown {
    pub id: GeometryId,
    pub image: Arc<RenderImage>,
}

/// What a frame asks the picture cache for, and the pane as last measured.
pub struct Wants {
    pub last: Option<Measure>,
    pub marks: SmallVec<[Wanted; 4]>,
    pub pages: SmallVec<[PageOnScreen; 2]>,
}

/// The PDF pages of a drawing over a PDF that reach into `view`.
pub fn pages_wanted(
    frame: &InkFrame<'_, PageIndex>,
    paged: &Paged,
    to_screen: &Affine<Pane, Screen>,
    view: &Rect<Screen>,
) -> SmallVec<[PageOnScreen; 2]> {
    let pdf = match paged.underlay() {
        Underlay::Pdf(pdf) => pdf,
        Underlay::Blank => return SmallVec::new(),
    };
    (frame.shown.iter())
        .filter_map(|shown| match shown.paper {
            Paper::Page {
                size: extent,
                background: Background::Pdf { page },
            } => {
                let corner = Point::<Surface>::new(extent.width.get(), extent.height.get());
                let area = Rect::spanning(Point::new(0.0, 0.0), corner)
                    .mapped(&shown.to_pane.then(to_screen));
                let (min, max) = (area.min(), area.max());
                area.intersects(view).then(|| PageOnScreen {
                    document: pdf.document.clone(),
                    page: page as usize,
                    area: Bounds::new(
                        point(px(min.x), px(min.y)),
                        size(px(max.x - min.x), px(max.y - min.y)),
                    ),
                })
            }
            Paper::Page { .. } | Paper::Plane(_) => None,
        })
        .collect()
}

pub struct Wanted {
    pub id: GeometryId,
    pub picture: Picture,
    pub size: Size<Pixels>,
}

pub fn wanted<D: Drawing>(
    frame: &InkFrame<'_, D::Surface>,
    drawing: &D,
    to_screen: &Affine<Pane, Screen>,
    view: &Rect<Screen>,
) -> SmallVec<[Wanted; 4]> {
    let layers = drawing.layers();
    let mut wanted = SmallVec::new();
    for shown in &frame.shown {
        let to_screen = shown.to_pane.then(to_screen);
        let visible = (layers.iter().zip(shown.marks.iter()))
            .filter(|((_, layer), _)| layer.visibility == Visibility::Shown);
        for (_, marks) in visible {
            wanted.extend(marks.iter().filter_map(|(_, mark)| {
                let Ink::Image(image) = &mark.ink else {
                    return None;
                };
                let transform = mark.placement.forward().then(&to_screen);
                let [a, b, c, d, _, _] = transform.coefficients;
                (image.built.bounds.mapped(&transform).intersects(view)).then(|| Wanted {
                    id: image.built.id,
                    picture: Picture {
                        source: Source::Bytes(image.encoded.clone()),
                        encoding: image.encoding.encoding(),
                    },
                    size: size(
                        px(image.extent.width.get() * a.hypot(b)),
                        px(image.extent.height.get() * c.hypot(d)),
                    ),
                })
            }));
        }
    }
    wanted
}
pub struct Onscreen {
    pub to_screen: Affine<Pane, Screen>,
    pub view: Rect<Screen>,
    pub accent: Hsla,
}

pub fn viewport(bounds: Bounds<Pixels>) -> Option<Viewport> {
    Viewport::new(
        Px(f32::from(bounds.size.width)),
        Px(f32::from(bounds.size.height)),
    )
}

pub fn paint_ink<D: Drawing>(
    frame: &InkFrame<'_, D::Surface>,
    hand: &Hand<D::Target>,
    drawing: &D,
    screen: &Onscreen,
    pictured: &Pictured,
    window: &mut Window,
) {
    let view = &screen.view;
    let layers = drawing.layers();
    for shown in &frame.shown {
        let to_screen = shown.to_pane.then(&screen.to_screen);
        let (area, background) = match shown.paper {
            Paper::Page { size, background } => {
                let page = Rect::spanning(
                    Point::<Surface>::new(0.0, 0.0),
                    Point::new(size.width.get(), size.height.get()),
                );
                (page.mapped(&to_screen), background)
            }
            Paper::Plane(background) => (*view, background),
        };
        let (min, max) = (area.min(), area.max());
        let rect = |left: f32, top: f32, width: f32, height: f32| {
            Bounds::new(point(px(left), px(top)), size(px(width), px(height)))
        };
        let sheet = rect(min.x, min.y, max.x - min.x, max.y - min.y);
        match background {
            Background::Pdf { page } => {
                window.paint_quad(fill(sheet, solid(Rgb::hex(0xffffff))));
                let tiles = pictured.pages.iter().find(|shown| shown.page == page);
                for tile in tiles.into_iter().flat_map(|shown| &shown.tiles) {
                    paint_picture(tile.bounds, tile.image.clone(), window);
                }
            }
            Background::Plain(paper) => window.paint_quad(fill(sheet, solid(paper))),
            Background::Pattern {
                paper,
                ink,
                pattern,
                spacing,
            } => {
                window.paint_quad(fill(sheet, solid(paper)));
                let step = spacing.get() * frame.scale;
                let ink = solid(ink);
                let (columns, rows) = (lines(min.x, max.x, step), lines(min.y, max.y, step));
                // Too dense to read at this zoom, so the paper shows plain.
                match (step < 4.0, pattern) {
                    (true, _) => {}
                    (false, Pattern::Lines) => {
                        for y in rows {
                            window.paint_quad(fill(rect(min.x, y, max.x - min.x, 1.0), ink));
                        }
                    }
                    (false, Pattern::Grid) => {
                        for y in rows {
                            window.paint_quad(fill(rect(min.x, y, max.x - min.x, 1.0), ink));
                        }
                        for x in columns {
                            window.paint_quad(fill(rect(x, min.y, 1.0, max.y - min.y), ink));
                        }
                    }
                    (false, Pattern::Dots) => {
                        for y in rows {
                            for x in columns.clone() {
                                window.paint_quad(fill(rect(x - 1.0, y - 1.0, 2.0, 2.0), ink));
                            }
                        }
                    }
                }
            }
        }
        for ((index, layer), marks) in layers.iter().zip(shown.marks.iter()) {
            match layer.visibility {
                Visibility::Shown => {}
                Visibility::Hidden => continue,
            }
            let target = D::target(shown.surface, index);
            let held = match &hand.doing {
                Doing::Ink {
                    using: Using::Erasing { target: on, erased },
                    ..
                } if *on == target => Held::Hidden(erased),
                Doing::Ink {
                    using:
                        Using::Selecting(Selecting::Holding {
                            target: on,
                            picked,
                            grip,
                        }),
                    ..
                } if *on == target => Held::Moved {
                    ids: picked.ids(),
                    by: grip.transform(picked),
                },
                // A text mark being edited is drawn from the text instead.
                Doing::Typing(open) if open.target == target => match &open.writing {
                    Writing::Replacing(was) => Held::Hidden(std::slice::from_ref(&was.id)),
                    Writing::New => Held::Nothing,
                },
                Doing::Ink { .. } | Doing::Typing(_) => Held::Nothing,
            };
            for (id, mark) in marks.iter() {
                let forward = match &held {
                    Held::Hidden(ids) if ids.contains(&id) => continue,
                    Held::Moved { ids, by } if ids.contains(&id) => {
                        mark.placement.forward().then(by)
                    }
                    Held::Nothing | Held::Hidden(_) | Held::Moved { .. } => {
                        *mark.placement.forward()
                    }
                };
                let transform = forward.then(&to_screen);
                let built = mark.built();
                match &mark.ink {
                    // An image not decoded yet leaves its place empty for a frame or two.
                    Ink::Image(image) => match pictured
                        .marks
                        .iter()
                        .find(|shown| shown.id == built.id)
                    {
                        Some(shown) => {
                            let [a, b, c, d, e, f] = transform.coefficients;
                            let painted = window.paint_image_transformed(
                                size(px(image.extent.width.get()), px(image.extent.height.get())),
                                TransformationMatrix {
                                    rotation_scale: [[a, c], [b, d]],
                                    translation: [e, f],
                                },
                                shown.image.clone(),
                                0,
                            );
                            match painted {
                                Ok(()) => {}
                                Err(error) => tracing::warn!(%error, "painting an image mark"),
                            }
                        }
                        None => {}
                    },
                    Ink::Stroke(_) | Ink::Shape(_) | Ink::Text(_) => paint_mesh(
                        MeshKey(built.id.get()),
                        MeshVersion(0),
                        &built.mesh,
                        built.bounds.mapped(&transform),
                        &transform,
                        mark.paint.color,
                        view,
                        window,
                    ),
                }
            }
        }
    }
    let on_screen = |target: D::Target| {
        (frame.on(D::surface(target))).map(|shown| (shown, shown.to_pane.then(&screen.to_screen)))
    };
    // Ink in progress is aligned with its surface.
    let aligned = |target| {
        on_screen(target)
            .map(|(_, to_screen)| Affine::<Local, Surface>::identity().then(&to_screen))
    };
    match &hand.doing {
        Doing::Ink {
            using: Using::Brushing { target, paint, .. },
            ..
        } => {
            let stroke = &hand.tools.stroke;
            let mesh = stroke.mesh.mesh();
            match (aligned(*target), mesh.bounds()) {
                (Some(transform), Some(local)) => paint_mesh(
                    MeshKey(hand.live.get()),
                    MeshVersion(stroke.revision),
                    mesh,
                    local.mapped(&transform),
                    &transform,
                    paint.color,
                    view,
                    window,
                ),
                (None, _) | (_, None) => {}
            }
        }
        Doing::Ink {
            using: Using::Shaping(Shaping::Dragging { target, ink, .. }),
            ..
        } => match aligned(*target) {
            Some(transform) => paint_mesh(
                MeshKey(ink.built.id.get()),
                MeshVersion(0),
                &ink.built.mesh,
                ink.built.bounds.mapped(&transform),
                &transform,
                hand.tools.paint.color,
                view,
                window,
            ),
            None => {}
        },
        Doing::Typing(open) => match on_screen(open.target) {
            Some((_, to_screen)) => {
                let text = &open.text;
                let to_screen = &to_screen;
                let transform = text.placement.forward().then(to_screen);
                match &text.laid.inked {
                    Inked::Ink(ink) => paint_mesh(
                        MeshKey(ink.built.id.get()),
                        MeshVersion(0),
                        &ink.built.mesh,
                        ink.built.bounds.mapped(&transform),
                        &transform,
                        text.paint.color,
                        &screen.view,
                        window,
                    ),
                    Inked::Blank => {}
                }
                let carets = &text.laid.carets;
                let rope = text.doc.text.slice(..);
                let quad = |corners: [Point<Local>; 4], color: Hsla, window: &mut Window| {
                    let mut path = PathBuilder::fill();
                    for (index, corner) in corners.into_iter().enumerate() {
                        let at = transform.apply(corner);
                        match index {
                            0 => path.move_to(point(px(at.x), px(at.y))),
                            _ => path.line_to(point(px(at.x), px(at.y))),
                        }
                    }
                    path.close();
                    match path.build() {
                        Ok(path) => window.paint_path(path, color),
                        Err(error) => tracing::warn!(%error, "text cursor"),
                    }
                };
                // A bar a tenth of the line high is wide enough to see at any zoom.
                let bar = |caret: Caret| caret.height * 0.1;
                let tint = screen.accent.opacity(0.3);
                for range in text.selection.iter() {
                    for index in range.from()..range.to().min(carets.len() - 1) {
                        let (here, next) = (carets[index], carets[index + 1]);
                        // A character ending its line, such as a newline, is shown as wide as the bar.
                        let right = match next.at.y == here.at.y {
                            true => next.at.x,
                            false => here.at.x + bar(here),
                        };
                        let (top, bottom) = (here.at.y, here.at.y + here.height);
                        quad(
                            [
                                Point::new(here.at.x, top),
                                Point::new(right, top),
                                Point::new(right, bottom),
                                Point::new(here.at.x, bottom),
                            ],
                            tint,
                            window,
                        );
                    }
                    let caret = carets[range.cursor(rope).min(carets.len() - 1)];
                    let (left, right) = (caret.at.x, caret.at.x + bar(caret));
                    let (top, bottom) = (caret.at.y, caret.at.y + caret.height);
                    quad(
                        [
                            Point::new(left, top),
                            Point::new(right, top),
                            Point::new(right, bottom),
                            Point::new(left, bottom),
                        ],
                        screen.accent,
                        window,
                    );
                }
            }
            None => {}
        },
        Doing::Ink {
            using:
                Using::Idle
                | Using::Erasing { .. }
                | Using::Selecting(_)
                | Using::Shaping(Shaping::Pressed { .. })
                | Using::Placing { .. },
            ..
        } => {}
    }
    let line = |points: &mut dyn Iterator<Item = Point<Screen>>, closed: Closed| {
        let mut path = PathBuilder::stroke(px(1.0));
        for (index, at) in points.enumerate() {
            match index {
                0 => path.move_to(point(px(at.x), px(at.y))),
                _ => path.line_to(point(px(at.x), px(at.y))),
            }
        }
        match closed {
            Closed::Yes => path.close(),
            Closed::No => {}
        }
        path.build()
    };
    let path = match &hand.doing {
        Doing::Ink { using, .. } => match using {
            Using::Selecting(Selecting::Boxing { target, from, to }) => {
                on_screen(*target).map(|(_, to_screen)| {
                    let around = Rect::spanning(*from, *to);
                    line(
                        &mut (Corner::AROUND.into_iter())
                            .map(|corner| to_screen.apply(corner.of(&around))),
                        Closed::Yes,
                    )
                })
            }
            Using::Selecting(Selecting::Lassoing { target, lasso }) => {
                on_screen(*target).map(|(_, to_screen)| {
                    line(&mut lasso.iter().map(|at| to_screen.apply(*at)), Closed::No)
                })
            }
            Using::Selecting(Selecting::Holding {
                target,
                picked,
                grip,
            }) => on_screen(*target).map(|(shown, to_screen)| {
                let held = grip.held(picked);
                let by = grip.transform(picked).then(&to_screen);
                let reach = frame::reach(&shown.from_pane);
                for (handle, sits) in held.handles(picked.sides(), reach) {
                    let at = by.apply(sits);
                    let side = REACH.0;
                    let square = Bounds::new(
                        point(px(at.x - side * 0.5), px(at.y - side * 0.5)),
                        size(px(side), px(side)),
                    );
                    let round = match handle {
                        Handle::Corner(_) | Handle::Edge(_) => px(0.0),
                        Handle::Rotate => px(side * 0.5),
                    };
                    window.paint_quad(fill(square, screen.accent).corner_radii(round));
                }
                line(
                    &mut (Corner::AROUND.into_iter()).map(|corner| {
                        by.apply(held.placement.forward().apply(corner.of(&held.rect)))
                    }),
                    Closed::Yes,
                )
            }),
            Using::Placing { target, from, to } => on_screen(*target).map(|(_, to_screen)| {
                let around = Rect::spanning(*from, *to);
                line(
                    &mut (Corner::AROUND.into_iter())
                        .map(|corner| to_screen.apply(corner.of(&around))),
                    Closed::Yes,
                )
            }),
            Using::Idle
            | Using::Brushing { .. }
            | Using::Erasing { .. }
            | Using::Shaping(_)
            | Using::Selecting(Selecting::Pressed { .. }) => None,
        },
        // Text being typed shows the box it wraps in.
        Doing::Typing(open) => on_screen(open.target).map(|(_, to_screen)| {
            let frame = open.text.frame();
            let to_screen = frame.placement.forward().then(&to_screen);
            line(
                &mut (Corner::AROUND.into_iter())
                    .map(|corner| to_screen.apply(corner.of(&frame.rect))),
                Closed::Yes,
            )
        }),
    };
    match path {
        Some(Ok(path)) => window.paint_path(path, screen.accent),
        Some(Err(error)) => tracing::warn!(%error, "selection outline"),
        None => {}
    }
}

pub enum Held<'a> {
    Nothing,
    Hidden(&'a [MarkId]),
    Moved {
        ids: &'a [MarkId],
        by: Affine<Surface, Surface>,
    },
}

#[derive(Clone, Copy)]
pub enum Closed {
    Yes,
    No,
}

pub fn paint_mesh(
    key: MeshKey,
    version: MeshVersion,
    shape: &Mesh,
    bounds: Rect<Screen>,
    transform: &Affine<Local, Screen>,
    color: Color,
    view: &Rect<Screen>,
    window: &mut Window,
) {
    match bounds.intersects(view) {
        true => {}
        false => return,
    }
    let [a, b, c, d, e, f] = transform.coefficients;
    window.paint_mesh(
        key,
        version,
        Bounds::from_corners(
            point(px(bounds.min().x), px(bounds.min().y)),
            point(px(bounds.max().x), px(bounds.max().y)),
        ),
        TransformationMatrix {
            rotation_scale: [[a, c], [b, d]],
            translation: [e, f],
        },
        hsla(color),
        &mut || MeshData {
            vertices: Cow::Owned(shape.vertices.iter().map(|at| [at.x, at.y]).collect()),
            indices: Cow::Owned(shape.triangles.iter().flatten().copied().collect()),
        },
    );
}

pub fn lines(from: f32, to: f32, step: f32) -> impl Iterator<Item = f32> + Clone {
    (1..)
        .map(move |n| from + n as f32 * step)
        .take_while(move |at| *at < to)
}

pub fn solid(rgb: Rgb) -> Hsla {
    hsla(Color::opaque(rgb))
}

pub fn now() -> Duration {
    static START: OnceLock<Instant> = OnceLock::new();
    START.get_or_init(Instant::now).elapsed()
}

pub fn on_pane(
    workspace: &Workspace,
    pane: PaneId,
    position: gpui::Point<Pixels>,
) -> Option<(Point<Pane>, Viewport)> {
    let bounds = *workspace.areas.drawings.borrow().get(pane)?;
    let at = Point::new(
        f32::from(position.x - bounds.origin.x),
        f32::from(position.y - bounds.origin.y),
    );
    Some((at, viewport(bounds)?))
}

pub fn sample(
    workspace: &Workspace,
    pane: PaneId,
    position: gpui::Point<Pixels>,
    time: Duration,
    pressure: Option<f32>,
    tilt: Option<gpui::Point<f32>>,
) -> Option<PenSample> {
    let (at, viewport) = on_pane(workspace, pane, position)?;
    Some(PenSample {
        at,
        viewport,
        time,
        pressure: pressure.and_then(|pressure| Pressure::new(pressure).ok()),
        tilt: tilt.and_then(|tilt| Tilt::new(tilt.x, tilt.y).ok()),
    })
}

pub fn input(
    workspace: &mut Workspace,
    pane: PaneId,
    phase: PenPhase,
    sample: PenSample,
    cx: &mut gpui::Context<Workspace>,
) {
    pen::pen(&mut workspace.state, pane, phase, sample);
    workspace.flush(cx);
    cx.notify();
}

pub fn wheel(
    workspace: &mut Workspace,
    pane: PaneId,
    event: &ScrollWheelEvent,
    cx: &mut gpui::Context<Workspace>,
) {
    let Some((at, viewport)) = on_pane(workspace, pane, event.position) else {
        return;
    };
    let wheel = crate::sheet::wheel_of(event, at, workspace.fonts.cell.height);
    state::view::sheet::wheeled(&mut workspace.state, pane, wheel, viewport);
    cx.notify();
}

pub fn stylus(tool: gpui::PenTool) -> Stylus {
    match tool {
        gpui::PenTool::Eraser => Stylus::Eraser,
        gpui::PenTool::Pen
        | gpui::PenTool::Brush
        | gpui::PenTool::Pencil
        | gpui::PenTool::Airbrush
        | gpui::PenTool::Finger
        | gpui::PenTool::Mouse
        | gpui::PenTool::Lens => Stylus::Tip,
    }
}
