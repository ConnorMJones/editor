//! A drawing as a pane shows it: each surface on screen, how it maps to the pane, and its marks.

use helix_core::{SmallVec, smallvec};
use ink::brush::Size;
use ink::doc::{Background, Board, Drawing, LayerIndex, LayerMarks, PageIndex, PageSize, Paged};
use model::geom::{Affine, Pane, Point, Surface};
use model::sheet::{PlaneView, SheetView, Viewport};
use model::units::Px;

/// How far a tap or a selection handle reaches on screen.
pub const REACH: Px = Px(8.0);

/// The least reach on a surface, kept above the brush's smallest size.
const LEAST_REACH: f32 = 0.1;

pub struct InkFrame<'a, S> {
    /// Pixels per point.
    pub scale: f32,
    /// In drawing order.
    pub shown: SmallVec<[Shown<'a, S>; 4]>,
}

pub struct Shown<'a, S> {
    pub surface: S,
    pub to_pane: Affine<Surface, Pane>,
    pub from_pane: Affine<Pane, Surface>,
    pub paper: Paper,
    pub marks: &'a LayerMarks,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Paper {
    Page {
        size: PageSize,
        background: Background,
    },
    Plane(Background),
}

/// Where a pen-down landed, with the map and reach its stroke keeps.
#[derive(Clone, Copy, Debug)]
pub struct Landed<T> {
    pub target: T,
    pub onto: Affine<Pane, Surface>,
    pub reach: Size,
}

/// How far a tap or a handle reaches on a surface mapped from the pane by `from_pane`.
pub fn reach(from_pane: &Affine<Pane, Surface>) -> Size {
    Size::new((REACH.0 * from_pane.stretch()).max(LEAST_REACH)).expect("at least the least reach")
}

pub fn paged<'a>(
    paged: &'a Paged,
    view: &SheetView,
    viewport: Viewport,
) -> InkFrame<'a, PageIndex> {
    let frame = view.frame(paged.stack(), viewport);
    InkFrame {
        scale: frame.scale,
        shown: (frame.shown(paged.stack()))
            .map(|page| Shown {
                surface: page.index,
                to_pane: page.to_pane,
                from_pane: page.from_pane,
                paper: Paper::Page {
                    size: page.size,
                    background: paged.page(page.index).background,
                },
                marks: paged.page(page.index).marks(),
            })
            .collect(),
    }
}

pub fn board<'a>(board: &'a Board, view: &PlaneView, viewport: Viewport) -> InkFrame<'a, ()> {
    let frame = view.frame(viewport);
    InkFrame {
        scale: frame.scale,
        shown: smallvec![Shown {
            surface: (),
            to_pane: frame.to_pane,
            from_pane: frame.from_pane,
            paper: Paper::Plane(board.background),
            marks: board.layer_marks(),
        }],
    }
}

impl<S: Copy + PartialEq> InkFrame<'_, S> {
    /// Where a pen-down at `at` lands on `layer`; `None` off every page.
    pub fn land<D: Drawing<Surface = S>>(
        &self,
        at: Point<Pane>,
        layer: LayerIndex,
    ) -> Option<Landed<D::Target>> {
        let shown = (self.shown.iter()).find(|shown| match shown.paper {
            Paper::Page { size, .. } => size.holds(shown.from_pane.apply(at)),
            Paper::Plane(_) => true,
        })?;
        Some(Landed {
            target: D::target(shown.surface, layer),
            onto: shown.from_pane,
            reach: reach(&shown.from_pane),
        })
    }

    pub fn on(&self, surface: S) -> Option<&Shown<'_, S>> {
        self.shown.iter().find(|shown| shown.surface == surface)
    }
}
