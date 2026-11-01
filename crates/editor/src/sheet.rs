//! The body of a tab shown as pages fitted to the pane: typst pages and image files, each page
//! drawn as its decoded picture where the shared sheet view puts it.

use std::sync::Arc;

use gpui::{
    App, Bounds, ContentMask, DispatchPhase, Element, ElementId, Entity, GlobalElementId, Hitbox,
    HitboxBehavior, InspectorElementId, IntoElement, LayoutId, MouseButton, MouseDownEvent, Pixels,
    RenderImage, ScrollWheelEvent, Style, Window, fill, point, px, relative,
};
use helix_core::SmallVec;
use model::geom::{Point, Vector};
use model::picture::Picture;
use model::sheet::{Measure, PageIndex, SheetView, Stack, Viewport};
use slotmap::Key as _;
use state::State;
use state::render::typeset::{Compiled, Outcome, PageSvg};
use state::view::layout::PaneId;
use state::view::sheet::Wheel;
use state::view::sheet::{Sheeted, Stacked};
use state::viewer::ImageFile;

use crate::fonts::hsla;
use crate::pane_space;
use crate::pictures::{Drawn, decoded, paint_picture};
use crate::rendered::update;
use crate::workspace::Workspace;

/// Wheel pixels per doubling of the zoom.
pub const WHEEL_PER_DOUBLING: f32 = 400.0;

pub struct SheetElement {
    pub workspace: Entity<Workspace>,
    pub pane: PaneId,
}

/// Where a sheet's pages come from, with what each kind adds.
pub enum Pages<'a> {
    /// A click on a page goes to its source.
    Typst(&'a Compiled, Look),
    Image(&'a ImageFile),
}

/// Whether the pages shown are from the last compile, or from before one that failed.
#[derive(Clone, Copy)]
pub enum Look {
    Current,
    Stale,
}

impl Pages<'_> {
    pub fn stack(&self) -> &Stack {
        match self {
            Pages::Typst(compiled, _) => &compiled.stack,
            Pages::Image(file) => &file.stack,
        }
    }

    pub fn picture(&self, page: PageIndex) -> Option<Picture> {
        match self {
            Pages::Typst(compiled, _) => match &compiled.pages.get(page.0)?.svg {
                PageSvg::Ready(svg) => Some(svg.picture()),
                PageSvg::Absent | PageSvg::Pending => None,
            },
            Pages::Image(file) => Some(file.picture()),
        }
    }
}

/// A pane's view of its pages, and the pane as last measured.
pub struct Sheet<'a> {
    pub view: &'a SheetView,
    pub pages: Pages<'a>,
    pub last: Option<Measure>,
}

pub fn sheet(state: &State, pane: PaneId) -> Option<Sheet<'_>> {
    let Sheeted::Pages { view, last, of } = state.layout.panes().get(pane)?.active()?.sheet()?
    else {
        return None;
    };
    let pages = match of {
        Stacked::Image(id) => Pages::Image(&state.images[id]),
        Stacked::Typst(buffer) => {
            let typst = state.compiles.get(buffer)?;
            let look = match typst.outcome {
                Outcome::Failed(_) => Look::Stale,
                Outcome::Pending | Outcome::Compiled => Look::Current,
            };
            Pages::Typst(typst.pages.as_ref()?, look)
        }
        Stacked::Paged(_) | Stacked::Pdf(_) => return None,
    };
    Some(Sheet { view, pages, last })
}

pub struct Prepared {
    pub hitbox: Hitbox,
    pub viewport: Viewport,
    pub pages: SmallVec<[PageShown; 4]>,
    pub look: Look,
    pub clicks: Clicks,
}

/// A page on screen and the picture to decode for it.
pub struct PageWanted {
    pub area: Bounds<Pixels>,
    pub picture: Option<Picture>,
}

/// A page on screen, with its picture once decoded.
pub struct PageShown {
    pub area: Bounds<Pixels>,
    pub image: Option<Arc<RenderImage>>,
}

/// What a click on a page does.
#[derive(Clone, Copy)]
pub enum Clicks {
    GoToSource,
    Nothing,
}

impl IntoElement for SheetElement {
    type Element = Self;

    fn into_element(self) -> Self {
        self
    }
}

impl Element for SheetElement {
    type RequestLayoutState = ();
    type PrepaintState = Option<Prepared>;

    fn id(&self) -> Option<ElementId> {
        Some(ElementId::NamedInteger(
            "sheet".into(),
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
    ) -> Option<Prepared> {
        let pane = self.pane;
        let viewport = pane_space::viewport(bounds)?;
        let last = sheet(&self.workspace.read(cx).state, pane)?.last;
        pane_space::report_measure(&self.workspace, pane, viewport, last, window, cx);
        let workspace = self.workspace.read(cx);
        let Sheet { view, pages, .. } = sheet(&workspace.state, pane)?;
        let frame = view.frame(pages.stack(), viewport);
        let shown: SmallVec<[PageWanted; 4]> = (frame.shown(pages.stack()))
            .map(|page| {
                let top = page.to_pane.apply(Point::new(0.0, 0.0));
                let bottom = page
                    .to_pane
                    .apply(Point::new(page.size.width.get(), page.size.height.get()));
                let area = Bounds::from_corners(
                    bounds.origin + point(px(top.x), px(top.y)),
                    bounds.origin + point(px(bottom.x), px(bottom.y)),
                );
                PageWanted {
                    area,
                    picture: pages.picture(page.index),
                }
            })
            .collect();
        let (look, clicks) = match pages {
            Pages::Typst(_, look) => (look, Clicks::GoToSource),
            Pages::Image(_) => (Look::Current, Clicks::Nothing),
        };
        let (pictures, entity) = (workspace.pictures.clone(), self.workspace.downgrade());
        let pages = (shown.into_iter())
            .map(|PageWanted { area, picture }| PageShown {
                area,
                image: picture.and_then(|picture| {
                    decoded(
                        &pictures,
                        &entity,
                        &Drawn::Picture(picture),
                        area,
                        window,
                        cx,
                    )
                }),
            })
            .collect();
        Some(Prepared {
            hitbox: window.insert_hitbox(bounds, HitboxBehavior::Normal),
            viewport,
            pages,
            look,
            clicks,
        })
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut (),
        prepared: &mut Option<Prepared>,
        window: &mut Window,
        cx: &mut App,
    ) {
        let Some(prepared) = prepared.take() else {
            return;
        };
        let surface = hsla(self.workspace.read(cx).theme.surface);
        window.with_content_mask(Some(ContentMask { bounds }), |window| {
            for PageShown { area, image } in &prepared.pages {
                window.paint_quad(fill(*area, crate::drawing::solid(model::color::Rgb::WHITE)));
                match image {
                    Some(image) => paint_picture(*area, image.clone(), window),
                    None => {}
                }
                match prepared.look {
                    Look::Stale => window.paint_quad(fill(*area, surface.opacity(0.55))),
                    Look::Current => {}
                }
            }
        });
        let (pane, viewport) = (self.pane, prepared.viewport);
        let on_pane = move |position: gpui::Point<Pixels>| pane_space::pane_point(bounds, position);
        match prepared.clicks {
            Clicks::GoToSource => window.on_mouse_event({
                let (workspace, hitbox) = (self.workspace.downgrade(), prepared.hitbox.clone());
                move |event: &MouseDownEvent, phase, window, cx| match (
                    phase,
                    event.button,
                    hitbox.is_hovered(window),
                ) {
                    (DispatchPhase::Bubble, MouseButton::Left, true) => {
                        cx.stop_propagation();
                        let at = on_pane(event.position);
                        update(&workspace, window, cx, |this| {
                            state::render::pages::click(
                                &mut this.state,
                                pane,
                                at,
                                viewport,
                                &mut this.effects,
                            )
                        });
                    }
                    (DispatchPhase::Bubble | DispatchPhase::Capture, ..) => {}
                }
            }),
            Clicks::Nothing => {}
        }
        window.on_mouse_event({
            let (workspace, hitbox) = (self.workspace.downgrade(), prepared.hitbox);
            move |event: &ScrollWheelEvent, phase, window, cx| match (
                phase,
                hitbox.is_hovered(window),
            ) {
                (DispatchPhase::Bubble, true) => {
                    cx.stop_propagation();
                    let _ = workspace.update(cx, |this, cx| {
                        let wheel =
                            wheel_of(event, on_pane(event.position), this.fonts.cell.height);
                        state::view::sheet::wheeled(&mut this.state, pane, wheel, viewport);
                        cx.notify();
                    });
                }
                (DispatchPhase::Bubble | DispatchPhase::Capture, _) => {}
            }
        });
    }
}

/// The wheel pans the view, or zooms it about `at` with Ctrl held.
pub fn wheel_of(event: &ScrollWheelEvent, at: Point<model::geom::Pane>, line: Pixels) -> Wheel {
    let delta = event.delta.pixel_delta(line);
    let (x, y) = (f32::from(delta.x), f32::from(delta.y));
    match event.modifiers.control {
        true => Wheel::Zoom {
            at,
            factor: (y / WHEEL_PER_DOUBLING).exp2(),
        },
        false => Wheel::Pan(Vector::new(-x, -y)),
    }
}
