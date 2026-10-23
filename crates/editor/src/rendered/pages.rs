//! Pages views: typst pages fitted to the pane by the shared sheet view, under a strip of its
//! diagnostics.

use std::sync::Arc;

use gpui::{
    AnyElement, App, Bounds, ClickEvent, ContentMask, Context, DispatchPhase, Element, ElementId,
    Entity, GlobalElementId, Hitbox, HitboxBehavior, InspectorElementId, IntoElement, LayoutId,
    MouseButton, MouseDownEvent, Pixels, RenderImage, ScrollWheelEvent, SharedString, Style,
    Window, div, fill, point, prelude::*, px, relative, white,
};
use helix_core::SmallVec;
use model::geom::{Point, Vector};
use model::sheet::Viewport;
use model::typeset::{Diagnostic, Origin, SourceFile};
use model::units::Px;
use slotmap::Key as _;
use state::render::pages::{Measure, Wheel};
use state::render::typeset::{Compiled, Outcome, Typst};
use state::view::layout::PaneId;

use super::raster::{paint_raster, raster};
use super::update;
use crate::drawing::WHEEL_PER_DOUBLING;
use crate::fonts::{PROSE_LINE, hsla};
use crate::workspace::Workspace;

/// The strip above the pages: the first error or warning, or that a first compile runs.
pub fn header(
    workspace: &Workspace,
    pane: PaneId,
    typst: &Typst,
    cx: &mut Context<Workspace>,
) -> Option<AnyElement> {
    let (state, theme, fonts) = (&workspace.state, &workspace.theme, &workspace.fonts);
    let strip = |color: gpui::Hsla| {
        div()
            .flex_none()
            .w_full()
            .px_3()
            .py_1()
            .font(fonts.ui.clone())
            .text_size(fonts.ui_size)
            .text_color(color)
            .bg(color.opacity(0.12))
            .border_b_1()
            .border_color(hsla(theme.chrome.border))
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
                path.strip_prefix(state.project.root.path())
                    .unwrap_or(path)
                    .display(),
                start.line + 1,
                start.column + 1
            ),
            Origin::Span {
                file: file @ (SourceFile::Embedded(_) | SourceFile::Package(_)),
                start,
                ..
            } => format!("{file}:{}:{}: ", start.line + 1, start.column + 1),
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
                    state::render::pages::goto_diagnostic(&mut this.state, pane, &mut this.effects)
                });
            })
            .child(SharedString::from(format!(
                "{mark} {place}{}{more}",
                first.message
            )))
            .into_any_element()
    };
    match (&typst.outcome, &typst.pages) {
        (Outcome::Failed(errors), _) => Some(diagnostics(errors, "✘", hsla(theme.prose.error))),
        (Outcome::Pending | Outcome::Compiled, Some(Compiled { warnings, .. })) => {
            match warnings.as_slice() {
                [] => None,
                warnings => Some(diagnostics(warnings, "⚠", hsla(theme.prose.warning))),
            }
        }
        (Outcome::Pending, None) => Some(
            strip(hsla(theme.muted))
                .child("compiling…")
                .into_any_element(),
        ),
        (Outcome::Compiled, None) => None,
    }
}

/// A pages view's body: the pages a frame of its view shows, each drawn as its raster.
pub struct PagesElement {
    pub workspace: Entity<Workspace>,
    pub pane: PaneId,
}

/// Whether the pages shown are from the last compile, or from before one that failed.
#[derive(Clone, Copy)]
enum Look {
    Current,
    Stale,
}

pub struct Prepared {
    hitbox: Hitbox,
    viewport: Viewport,
    /// Each page on screen, with its raster once made.
    pages: SmallVec<[(Bounds<Pixels>, Option<Arc<RenderImage>>); 4]>,
    look: Look,
}

impl IntoElement for PagesElement {
    type Element = Self;

    fn into_element(self) -> Self {
        self
    }
}

impl Element for PagesElement {
    type RequestLayoutState = ();
    type PrepaintState = Option<Prepared>;

    fn id(&self) -> Option<ElementId> {
        Some(ElementId::NamedInteger(
            "pages".into(),
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
        let viewport = Viewport::new(
            Px(f32::from(bounds.size.width)),
            Px(f32::from(bounds.size.height)),
        )?;
        let workspace = self.workspace.read(cx);
        let (text, view) = workspace.state.layout.pages(pane)?;
        let typst = workspace.state.compiles.get(text.buffer())?;
        let compiled = typst.pages.as_ref()?;
        let measure = Measure {
            viewport,
            line: Px(f32::from(workspace.fonts.prose_size) * PROSE_LINE),
        };
        let unreported = match view.last {
            Some(seen) if seen == measure => None,
            Some(_) | None => Some(measure),
        };
        let frame = view.view.frame(&compiled.stack, viewport);
        let shown: SmallVec<[(Bounds<Pixels>, Arc<[u8]>); 4]> = (frame.shown(&compiled.stack))
            .map(|page| {
                let top = page.to_pane.apply(Point::new(0.0, 0.0));
                let bottom = page
                    .to_pane
                    .apply(Point::new(page.size.width.get(), page.size.height.get()));
                let area = Bounds::from_corners(
                    bounds.origin + point(px(top.x), px(top.y)),
                    bounds.origin + point(px(bottom.x), px(bottom.y)),
                );
                (area, compiled.pages[page.index.0].bytes.clone())
            })
            .collect();
        let look = match typst.outcome {
            Outcome::Failed(_) => Look::Stale,
            Outcome::Pending | Outcome::Compiled => Look::Current,
        };
        let (rasters, entity) = (workspace.rasters.clone(), self.workspace.downgrade());
        // Keys that move by lines or part of the pane hear of a new size after this frame.
        match unreported {
            Some(measure) => {
                let entity = self.workspace.clone();
                window.defer(cx, move |_, cx| {
                    entity.update(cx, |workspace, _| {
                        state::render::pages::measured(&mut workspace.state, pane, measure)
                    })
                });
            }
            None => {}
        }
        let pages = (shown.into_iter())
            .map(|(area, bytes)| (area, raster(&rasters, &entity, &bytes, area, window, cx)))
            .collect();
        Some(Prepared {
            hitbox: window.insert_hitbox(bounds, HitboxBehavior::Normal),
            viewport,
            pages,
            look,
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
            for (area, image) in &prepared.pages {
                window.paint_quad(fill(*area, white()));
                match image {
                    Some(image) => paint_raster(*area, image.clone(), window),
                    None => {}
                }
                match prepared.look {
                    Look::Stale => window.paint_quad(fill(*area, surface.opacity(0.55))),
                    Look::Current => {}
                }
            }
        });
        let (pane, viewport, origin) = (self.pane, prepared.viewport, bounds.origin);
        let on_pane = move |position: gpui::Point<Pixels>| {
            Point::new(
                f32::from(position.x - origin.x),
                f32::from(position.y - origin.y),
            )
        };
        window.on_mouse_event({
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
        });
        window.on_mouse_event({
            let (workspace, hitbox) = (self.workspace.downgrade(), prepared.hitbox);
            move |event: &ScrollWheelEvent, phase, window, cx| match (
                phase,
                hitbox.is_hovered(window),
            ) {
                (DispatchPhase::Bubble, true) => {
                    cx.stop_propagation();
                    let _ = workspace.update(cx, |this, cx| {
                        let delta = event.delta.pixel_delta(this.fonts.cell.height);
                        let (x, y) = (f32::from(delta.x), f32::from(delta.y));
                        let wheel = match event.modifiers.control {
                            true => Wheel::Zoom {
                                at: on_pane(event.position),
                                factor: (y / WHEEL_PER_DOUBLING).exp2(),
                            },
                            false => Wheel::Pan(Vector::new(-x, -y)),
                        };
                        state::render::pages::wheeled(&mut this.state, pane, wheel, viewport);
                        cx.notify();
                    });
                }
                (DispatchPhase::Bubble | DispatchPhase::Capture, _) => {}
            }
        });
    }
}
