//! Pages stacked top to bottom and shown in a pane, fitted to its width or to a page's height,
//! and planes shown at a zoom: drawings, typeset and PDF pages, and an image as a stack of one.

use std::ops::Range;

use crate::geom::{Affine, Pane, Point, Sheet, Surface, Vector};
use crate::units::{Positive, Px};

/// Space between stacked pages, and the least space a fit leaves at the pane's edges, in points.
pub const GAP: f32 = 16.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PageIndex(pub usize);

/// Width and height in points.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PageSize {
    pub width: Positive,
    pub height: Positive,
}

#[derive(Debug, thiserror::Error, PartialEq)]
#[error("page size {0} by {1} is not positive")]
pub struct InvalidPageSize(pub f32, pub f32);

impl PageSize {
    pub const A4: PageSize = PageSize {
        width: Positive::new(595.28).unwrap(),
        height: Positive::new(841.89).unwrap(),
    };

    pub fn new(width: f32, height: f32) -> Result<PageSize, InvalidPageSize> {
        match (Positive::new(width), Positive::new(height)) {
            (Some(width), Some(height)) => Ok(PageSize { width, height }),
            (None, _) | (_, None) => Err(InvalidPageSize(width, height)),
        }
    }

    /// Whether `at`, in the page's own space, lies on the page.
    pub fn holds(self, at: Point<Surface>) -> bool {
        (0.0..=self.width.get()).contains(&at.x) && (0.0..=self.height.get()).contains(&at.y)
    }
}

/// Page sizes in reading order, each page centred on the widest; never empty.
#[derive(Clone, Debug, PartialEq)]
pub struct Stack {
    sizes: Vec<PageSize>, // private: tops and width are derived from sizes
    /// Where each page's top sits on the sheet.
    tops: Vec<f32>, // private: tops and width are derived from sizes
    width: f32,           // private: tops and width are derived from sizes
}

#[derive(Debug, thiserror::Error, PartialEq)]
pub enum StackError {
    #[error("there is no page {0}")]
    NoPage(usize),
    #[error("a stack keeps at least one page")]
    LastPage,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Placed {
    pub index: PageIndex,
    pub size: PageSize,
    pub place: Affine<Surface, Sheet>,
}

impl Stack {
    /// `None` for no pages.
    pub fn new(sizes: Vec<PageSize>) -> Option<Stack> {
        let mut stack = Stack {
            sizes,
            tops: Vec::new(),
            width: 0.0,
        };
        stack.lay_out();
        (!stack.sizes.is_empty()).then_some(stack)
    }

    pub fn one(size: PageSize) -> Stack {
        let mut stack = Stack {
            sizes: vec![size],
            tops: Vec::new(),
            width: 0.0,
        };
        stack.lay_out();
        stack
    }

    fn lay_out(&mut self) {
        self.tops.clear();
        let mut top = 0.0;
        for size in &self.sizes {
            self.tops.push(top);
            top += size.height.get() + GAP;
        }
        self.width = (self.sizes.iter())
            .map(|size| size.width.get())
            .fold(0.0, f32::max);
    }

    pub fn sizes(&self) -> &[PageSize] {
        &self.sizes
    }

    pub fn last(&self) -> PageIndex {
        PageIndex(self.sizes.len() - 1)
    }

    pub fn width(&self) -> f32 {
        self.width
    }

    pub fn height(&self) -> f32 {
        let last = self.sizes.len() - 1;
        self.tops[last] + self.sizes[last].height.get()
    }

    /// Page `index`, or the last page for an index past it.
    pub fn place(&self, index: PageIndex) -> Placed {
        let index = index.min(self.last());
        let size = self.sizes[index.0];
        Placed {
            index,
            size,
            place: Affine::translation((self.width - size.width.get()) * 0.5, self.tops[index.0]),
        }
    }

    /// The page whose band, the page and half the gap either side, holds sheet height `y`;
    /// heights past either end belong to the end page.
    pub fn band(&self, y: f32) -> PageIndex {
        let count = self.tops.partition_point(|top| *top - GAP * 0.5 <= y);
        PageIndex(count.clamp(1, self.sizes.len()) - 1)
    }

    /// The pages reaching between sheet heights `top` and `bottom`.
    pub fn between(&self, top: f32, bottom: f32) -> Range<usize> {
        let first = (self.sizes.iter().zip(&self.tops))
            .position(|(size, at)| at + size.height.get() >= top)
            .unwrap_or(self.sizes.len());
        first..self.tops.partition_point(|at| *at <= bottom).max(first)
    }

    /// Places a page before `at`; the page count appends.
    pub fn insert(&mut self, at: PageIndex, size: PageSize) -> Result<StackEdit, StackError> {
        let index = (at.0 <= self.sizes.len())
            .then_some(at.0)
            .ok_or(StackError::NoPage(at.0))?;
        self.sizes.insert(index, size);
        self.lay_out();
        Ok(StackEdit::Inserted(at))
    }

    pub fn remove(&mut self, at: PageIndex) -> Result<StackEdit, StackError> {
        let index = (at.0 < self.sizes.len())
            .then_some(at.0)
            .ok_or(StackError::NoPage(at.0))?;
        match self.sizes.len() {
            1 => Err(StackError::LastPage),
            _ => {
                self.sizes.remove(index);
                self.lay_out();
                Ok(StackEdit::Removed(at))
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StackEdit {
    Inserted(PageIndex),
    Removed(PageIndex),
}

/// A pane's size in logical pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Viewport {
    pub width: Positive,
    pub height: Positive,
}

impl Viewport {
    /// `None` for a pane with no area.
    pub fn new(width: Px, height: Px) -> Option<Viewport> {
        Some(Viewport {
            width: Positive::new(width.0)?,
            height: Positive::new(height.0)?,
        })
    }

    fn centre(self) -> Vector<Pane> {
        Vector::new(self.width.get() * 0.5, self.height.get() * 0.5)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Fit {
    Width,
    Height,
    Page,
}

impl Fit {
    /// The fit after this one, for a key that steps through them.
    pub fn next(self) -> Fit {
        match self {
            Fit::Width => Fit::Height,
            Fit::Height => Fit::Page,
            Fit::Page => Fit::Width,
        }
    }
}

/// A factor on a view's base scale, within what a pane can usefully show.
#[derive(Clone, Copy, Debug, PartialEq, PartialOrd)]
pub struct Zoom(f32); // private: clamped to the zoom range

impl Zoom {
    pub const ONE: Zoom = Zoom(1.0);
    const MIN: f32 = 0.1;
    const MAX: f32 = 16.0;
    /// One key's step.
    pub const STEP: f32 = 1.25;

    pub fn clamped(factor: f32) -> Zoom {
        // `max` turns a NaN into the least zoom before the clamp.
        Zoom(factor.max(Zoom::MIN).clamp(Zoom::MIN, Zoom::MAX))
    }

    pub fn get(self) -> f32 {
        self.0
    }

    pub fn by(self, factor: f32) -> Zoom {
        Zoom::clamped(self.0 * factor)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Spot {
    pub page: PageIndex,
    pub at: Point<Surface>,
}

/// Where a pane looks on a stack: the page point at its centre, and its scale.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SheetView {
    /// Its page is the page in focus.
    pub centre: Spot,
    pub fit: Fit,
    pub zoom: Zoom,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SheetFrame {
    /// Pixels per point.
    pub scale: f32,
    pub to_pane: Affine<Sheet, Pane>,
    pub from_pane: Affine<Pane, Sheet>,
    pub pages: Range<usize>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PageFrame {
    pub index: PageIndex,
    pub size: PageSize,
    pub to_pane: Affine<Surface, Pane>,
    pub from_pane: Affine<Pane, Surface>,
}

impl SheetView {
    /// The middle of `page`, fitted to the width; a frame keeps it to the stack it is shown on.
    pub fn on(page: PageIndex) -> SheetView {
        SheetView {
            centre: Spot {
                page,
                at: Point::new(0.0, 0.0),
            },
            fit: Fit::Width,
            zoom: Zoom::ONE,
        }
    }

    /// The first page's top at the pane's top, fitted to the width.
    pub fn start(stack: &Stack) -> SheetView {
        SheetView {
            centre: Spot {
                page: PageIndex(0),
                at: Point::new(stack.place(PageIndex(0)).size.width.get() * 0.5, 0.0),
            },
            fit: Fit::Width,
            zoom: Zoom::ONE,
        }
    }

    pub fn scale(&self, stack: &Stack, viewport: Viewport) -> f32 {
        let page = stack.place(self.centre.page).size;
        let fitted = match self.fit {
            Fit::Width => viewport.width.get() / (stack.width() + 2.0 * GAP),
            Fit::Height => viewport.height.get() / (page.height.get() + 2.0 * GAP),
            Fit::Page => (viewport.width.get() / (page.width.get() + 2.0 * GAP))
                .min(viewport.height.get() / (page.height.get() + 2.0 * GAP)),
        };
        fitted * self.zoom.get()
    }

    /// The view at `viewport`. The centre keeps the stack on screen: a stack smaller than the
    /// pane along an axis is centred on it, a larger one moves only as far as its edges.
    pub fn frame(&self, stack: &Stack, viewport: Viewport) -> SheetFrame {
        let scale = self.scale(stack, viewport);
        let centre = clamp(stack, viewport, scale, self.sheet_centre(stack));
        let half = viewport.centre();
        let to_pane = Affine::new([
            scale,
            0.0,
            0.0,
            scale,
            half.x - centre.x * scale,
            half.y - centre.y * scale,
        ]);
        let from_pane = Affine::new([
            1.0 / scale,
            0.0,
            0.0,
            1.0 / scale,
            centre.x - half.x / scale,
            centre.y - half.y / scale,
        ]);
        SheetFrame {
            scale,
            to_pane,
            from_pane,
            pages: stack.between(centre.y - half.y / scale, centre.y + half.y / scale),
        }
    }

    fn sheet_centre(&self, stack: &Stack) -> Point<Sheet> {
        stack.place(self.centre.page).place.apply(self.centre.at)
    }

    pub fn pan(&mut self, stack: &Stack, viewport: Viewport, by: Vector<Pane>) {
        let scale = self.scale(stack, viewport);
        let from = clamp(stack, viewport, scale, self.sheet_centre(stack));
        let to = from + Vector::new(by.x / scale, by.y / scale);
        self.centre = spot(stack, clamp(stack, viewport, scale, to));
    }

    /// Zooms by `factor`, keeping the point under `at` there.
    pub fn zoom_about(&mut self, stack: &Stack, viewport: Viewport, at: Point<Pane>, factor: f32) {
        let frame = self.frame(stack, viewport);
        let under = frame.from_pane.apply(at);
        self.zoom = self.zoom.by(factor);
        let scale = self.scale(stack, viewport);
        let off = at - Point::new(0.0, 0.0) - viewport.centre();
        let centre = under - Vector::new(off.x / scale, off.y / scale);
        self.centre = spot(stack, clamp(stack, viewport, scale, centre));
    }

    /// Shows `page` from its top; without a measured pane, its middle.
    pub fn go_to(&mut self, stack: &Stack, page: PageIndex, viewport: Option<Viewport>) {
        let placed = stack.place(page);
        let middle = placed.size.width.get() * 0.5;
        // Set first, so a height fit scales to the new page.
        self.centre = Spot {
            page: placed.index,
            at: Point::new(middle, placed.size.height.get() * 0.5),
        };
        self.centre = match viewport {
            Some(viewport) => {
                let scale = self.scale(stack, viewport);
                let below = viewport.height.get() / scale * 0.5 - GAP;
                let centre = placed.place.apply(Point::new(middle, below));
                spot(stack, clamp(stack, viewport, scale, centre))
            }
            None => self.centre,
        };
    }

    /// Keeps the view on the same page across an edit to the stack.
    pub fn follow(&mut self, edit: StackEdit) {
        let page = &mut self.centre.page;
        match edit {
            StackEdit::Inserted(at) if at <= *page => page.0 += 1,
            StackEdit::Removed(at) if at < *page => page.0 -= 1,
            StackEdit::Inserted(_) | StackEdit::Removed(_) => {}
        }
    }
}

impl SheetFrame {
    pub fn page(&self, stack: &Stack, index: PageIndex) -> PageFrame {
        let placed = stack.place(index);
        let [_, _, _, _, x, y] = placed.place.coefficients;
        PageFrame {
            index: placed.index,
            size: placed.size,
            to_pane: placed.place.then(&self.to_pane),
            from_pane: self.from_pane.then(&Affine::translation(-x, -y)),
        }
    }

    /// The pages on screen, top first.
    pub fn shown<'a>(&'a self, stack: &'a Stack) -> impl Iterator<Item = PageFrame> + 'a {
        self.pages
            .clone()
            .map(|index| self.page(stack, PageIndex(index)))
    }

    pub fn hit(&self, stack: &Stack, at: Point<Pane>) -> Option<(PageFrame, Point<Surface>)> {
        self.shown(stack)
            .map(|page| (page, page.from_pane.apply(at)))
            .find(|(page, on)| page.size.holds(*on))
    }
}

/// `centre` held so the stack stays on screen at `scale`.
fn clamp(stack: &Stack, viewport: Viewport, scale: f32, centre: Point<Sheet>) -> Point<Sheet> {
    // The range is centred on the middle of the content and empty when the content fits.
    let axis = |at: f32, content: f32, visible: f32| {
        let reach = ((content + 2.0 * GAP - visible) * 0.5).max(0.0);
        at.clamp(content * 0.5 - reach, content * 0.5 + reach)
    };
    Point::new(
        axis(centre.x, stack.width(), viewport.width.get() / scale),
        axis(centre.y, stack.height(), viewport.height.get() / scale),
    )
}

/// The page point at sheet point `at`, on the page whose band holds it.
fn spot(stack: &Stack, at: Point<Sheet>) -> Spot {
    let placed = stack.place(stack.band(at.y));
    let [_, _, _, _, x, y] = placed.place.coefficients;
    Spot {
        page: placed.index,
        at: Point::new(at.x - x, at.y - y),
    }
}

/// Where a pane looks on an unbounded plane: the point at its centre, at one pixel per point
/// times the zoom.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlaneView {
    pub centre: Point<Surface>,
    pub zoom: Zoom,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlaneFrame {
    pub scale: f32,
    pub to_pane: Affine<Surface, Pane>,
    pub from_pane: Affine<Pane, Surface>,
}

impl PlaneView {
    pub const START: PlaneView = PlaneView {
        centre: Point::new(0.0, 0.0),
        zoom: Zoom::ONE,
    };

    pub fn frame(&self, viewport: Viewport) -> PlaneFrame {
        let scale = self.zoom.get();
        let (centre, half) = (self.centre, viewport.centre());
        PlaneFrame {
            scale,
            to_pane: Affine::new([
                scale,
                0.0,
                0.0,
                scale,
                half.x - centre.x * scale,
                half.y - centre.y * scale,
            ]),
            from_pane: Affine::new([
                1.0 / scale,
                0.0,
                0.0,
                1.0 / scale,
                centre.x - half.x / scale,
                centre.y - half.y / scale,
            ]),
        }
    }

    pub fn pan(&mut self, by: Vector<Pane>) {
        let scale = self.zoom.get();
        self.centre = self.centre + Vector::new(by.x / scale, by.y / scale);
    }

    /// Zooms by `factor`, keeping the point under `at` there.
    pub fn zoom_about(&mut self, viewport: Viewport, at: Point<Pane>, factor: f32) {
        let under = self.frame(viewport).from_pane.apply(at);
        self.zoom = self.zoom.by(factor);
        let scale = self.zoom.get();
        let off = at - Point::new(0.0, 0.0) - viewport.centre();
        self.centre = under - Vector::new(off.x / scale, off.y / scale);
    }
}

/// A pane's size and its line height, as the host drew it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Measure {
    pub viewport: Viewport,
    pub line: Px,
}
