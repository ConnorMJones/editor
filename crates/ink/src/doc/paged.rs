use model::color::Rgb;
pub use model::sheet::{InvalidPageSize, PageIndex, PageSize};
use model::sheet::{Stack, StackError};

use crate::doc::{
    ChangeError, Drawing, LayerChange, LayerIndex, LayerMarks, Layers, MarkChange, Marks,
    apply_marks,
};

/// Pages in reading order, each with its own size, background and marks; never empty.
#[derive(Clone, Debug)]
pub struct Paged {
    layers: Layers,   // private: pages, stack and layers stay in step
    pages: Vec<Page>, // private: pages, stack and layers stay in step
    stack: Stack,     // private: pages, stack and layers stay in step
}

#[derive(Clone, Debug)]
pub struct Page {
    pub background: Background,
    marks: LayerMarks, // private: one set of marks per document layer
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Background {
    Plain(Rgb),
    Pattern {
        paper: Rgb,
        ink: Rgb,
        pattern: Pattern,
        spacing: Spacing,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pattern {
    Lines,
    Grid,
    Dots,
}

/// Distance between pattern lines or dots, in points.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Spacing(f32); // private: checked by its constructor

#[derive(Debug, thiserror::Error, PartialEq)]
#[error("pattern spacing {0} is not positive")]
pub struct InvalidSpacing(pub f32);

impl Spacing {
    pub fn new(spacing: f32) -> Result<Spacing, InvalidSpacing> {
        (spacing.is_finite() && spacing > 0.0)
            .then_some(Spacing(spacing))
            .ok_or(InvalidSpacing(spacing))
    }

    pub fn get(self) -> f32 {
        self.0
    }
}

/// A layer on a page.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PageLayer {
    pub page: PageIndex,
    pub layer: LayerIndex,
}

#[derive(Clone, Debug)]
pub enum PagedChange {
    Marks(MarkChange<PageLayer>),
    /// A blank page placed before `at`; the page count appends.
    InsertPage {
        at: PageIndex,
        size: PageSize,
        background: Background,
    },
    /// Refused for the only page.
    RemovePage(PageIndex),
    Layers(LayerChange),
}

impl From<MarkChange<PageLayer>> for PagedChange {
    fn from(change: MarkChange<PageLayer>) -> PagedChange {
        PagedChange::Marks(change)
    }
}

impl From<LayerChange> for PagedChange {
    fn from(change: LayerChange) -> PagedChange {
        PagedChange::Layers(change)
    }
}

impl Page {
    pub fn marks(&self) -> &LayerMarks {
        &self.marks
    }
}

impl From<StackError> for ChangeError {
    fn from(error: StackError) -> ChangeError {
        match error {
            StackError::NoPage(page) => ChangeError::NoPage(page),
            StackError::LastPage => ChangeError::LastPage,
        }
    }
}

impl Paged {
    /// One blank page on one layer.
    pub fn new(size: PageSize, background: Background) -> Paged {
        let layers = Layers::new();
        let page = Page {
            background,
            marks: LayerMarks::empty(&layers),
        };
        Paged {
            layers,
            pages: vec![page],
            stack: Stack::one(size),
        }
    }

    /// `count` pages, each read with `page` and then `marks` once per layer; `Ok(None)` for no
    /// pages.
    pub fn build<R, E>(
        layers: Layers,
        count: usize,
        source: &mut R,
        page: fn(&mut R) -> Result<(PageSize, Background), E>,
        marks: fn(&mut R, LayerIndex) -> Result<Marks, E>,
    ) -> Result<Option<Paged>, E> {
        let mut pages = Vec::with_capacity(count.min(1024));
        let mut sizes = Vec::with_capacity(count.min(1024));
        for _ in 0..count {
            let (size, background) = page(source)?;
            sizes.push(size);
            pages.push(Page {
                background,
                marks: LayerMarks::build(&layers, |layer| marks(source, layer))?,
            });
        }
        Ok(Stack::new(sizes).map(|stack| Paged {
            layers,
            pages,
            stack,
        }))
    }

    pub fn pages(&self) -> &[Page] {
        &self.pages
    }

    /// The pages' sizes and where they sit stacked.
    pub fn stack(&self) -> &Stack {
        &self.stack
    }

    /// Page `index`, or the last page for an index past it, as a stack places pages.
    pub fn page(&self, index: PageIndex) -> &Page {
        &self.pages[self.stack.place(index).index.0]
    }
}

impl Drawing for Paged {
    type Target = PageLayer;
    type Surface = PageIndex;
    type Change = PagedChange;

    fn target(page: PageIndex, layer: LayerIndex) -> PageLayer {
        PageLayer { page, layer }
    }

    fn surface(target: PageLayer) -> PageIndex {
        target.page
    }

    fn layers(&self) -> &Layers {
        &self.layers
    }

    fn marks(&self, target: PageLayer) -> Option<&Marks> {
        self.pages.get(target.page.0)?.marks.get(target.layer)
    }

    fn apply(&mut self, change: PagedChange) -> Result<(), ChangeError> {
        match change {
            PagedChange::Marks(change) => {
                let target = change.target();
                let page = (self.pages.get_mut(target.page.0))
                    .ok_or(ChangeError::NoPage(target.page.0))?;
                let marks = (page.marks.get_mut(target.layer))
                    .ok_or(ChangeError::NoLayer(target.layer.0))?;
                apply_marks(marks, change)
            }
            PagedChange::InsertPage {
                at,
                size,
                background,
            } => {
                self.stack.insert(at, size)?;
                let page = Page {
                    background,
                    marks: LayerMarks::empty(&self.layers),
                };
                self.pages.insert(at.0, page);
                Ok(())
            }
            PagedChange::RemovePage(at) => {
                self.stack.remove(at)?;
                self.pages.remove(at.0);
                Ok(())
            }
            PagedChange::Layers(LayerChange::Add(layer)) => {
                self.layers.push(layer);
                for page in &mut self.pages {
                    page.marks.push_layer();
                }
                Ok(())
            }
            PagedChange::Layers(LayerChange::Set(index, layer)) => {
                *self
                    .layers
                    .get_mut(index)
                    .ok_or(ChangeError::NoLayer(index.0))? = layer;
                Ok(())
            }
        }
    }
}
