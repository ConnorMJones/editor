//! What a drawing holds: marks on layers, on pages or on a board.

mod board;
mod image;
mod layer;
mod paged;
mod shape;

use std::sync::Arc;

use slotmap::SlotMap;
use smallvec::SmallVec;

use crate::brush::{Brush, EPSILON, Paint};
use crate::geom::{Affine, Local, Rect, Surface, Vector, triangle_distance};
use crate::input::{Modeler, Sample};
use crate::mesh::MeshBuilder;
use crate::text::TextInk;

pub use board::{Board, BoardChange};
pub use image::ImageInk;
pub use layer::{InvalidLayerName, Layer, LayerIndex, LayerMarks, LayerName, Layers, Visibility};
pub use paged::{
    Background, InvalidPageSize, InvalidSpacing, Page, PageIndex, PageLayer, PageSize, Paged,
    PagedChange, Pattern, Spacing,
};
pub use shape::{Built, HEAD, HEAD_ANGLE, HEAD_SHARE, Shape, ShapeInk, ShapeKind};

slotmap::new_key_type! {
    pub struct MarkId;
}

/// Ink placed on a surface: a stroke, a shape, text or an image, with where it sits and its
/// colour (which an image ignores).
#[derive(Clone, Debug)]
pub struct Mark {
    pub placement: Placement,
    pub paint: Paint,
    pub ink: Ink,
}

#[derive(Clone, Debug)]
pub enum Ink {
    Stroke(Arc<Geometry>),
    Shape(Arc<ShapeInk>),
    Text(Arc<TextInk>),
    Image(Arc<ImageInk>),
}

impl Mark {
    /// The mesh the mark is drawn as, in its own space.
    pub fn built(&self) -> &Built {
        match &self.ink {
            Ink::Stroke(geometry) => &geometry.built,
            Ink::Shape(shape) => &shape.built,
            Ink::Text(text) => &text.built,
            Ink::Image(image) => &image.built,
        }
    }

    pub fn bounds(&self) -> Rect<Surface> {
        self.built().bounds.mapped(&self.placement.forward)
    }

    pub fn touches(&self, at: crate::geom::Point<Surface>, radius: f32) -> bool {
        let back = &self.placement.back;
        let local = back.apply(at);
        let reach = radius * back.stretch();
        let Built { mesh, bounds, .. } = self.built();
        Rect::around(local, reach).intersects(bounds)
            && mesh.triangles.iter().any(|triangle| {
                let corners = triangle.map(|index| mesh.vertices[index as usize]);
                triangle_distance(local, corners) <= reach
            })
    }
}

/// Where a stroke sits on its surface: a transform that can be undone, held both ways.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Placement {
    forward: Affine<Local, Surface>, // private: `back` stays the inverse of `forward`
    back: Affine<Surface, Local>,    // private: `back` stays the inverse of `forward`
}

impl Placement {
    /// A new stroke's own space is aligned with its surface.
    pub const ALIGNED: Placement = Placement {
        forward: Affine::identity(),
        back: Affine::identity(),
    };

    /// The mark's own origin at `at`, unrotated and unscaled.
    pub fn at(at: crate::geom::Point<Surface>) -> Placement {
        Placement {
            forward: Affine::translation(at.x, at.y),
            back: Affine::translation(-at.x, -at.y),
        }
    }

    /// `None` for a transform that flattens the stroke.
    pub fn new(forward: Affine<Local, Surface>) -> Option<Placement> {
        Some(Placement {
            back: forward.inverse()?,
            forward,
        })
    }

    pub fn shifted(&self, by: Vector<Local>) -> Placement {
        Placement {
            forward: Affine::translation(by.x, by.y).then(&self.forward),
            back: self.back.then(&Affine::translation(-by.x, -by.y)),
        }
    }

    pub fn forward(&self) -> &Affine<Local, Surface> {
        &self.forward
    }

    pub fn back(&self) -> &Affine<Surface, Local> {
        &self.back
    }
}

/// Names a stroke's shape for as long as the process runs, so a host can keep its mesh.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct GeometryId(u64); // private: unique, made only by `next`

impl GeometryId {
    /// An id no shape has had before.
    pub fn next() -> GeometryId {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        GeometryId(NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed))
    }

    pub fn get(self) -> u64 {
        self.0
    }
}

/// A stroke's shape: its samples and brush, and the mesh `Geometry::new` builds from them.
#[derive(Debug)]
pub struct Geometry {
    pub samples: Box<[Sample<Local>]>,
    pub sensors: Sensors,
    pub brush: Brush,
    pub built: Built,
}

/// Which optional readings every sample of a stroke carries.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Sensors {
    pub pressure: Sensor,
    pub tilt: Sensor,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sensor {
    Read,
    Absent,
}

impl Sensors {
    fn of<S>(sample: &Sample<S>) -> Sensors {
        Sensors {
            pressure: match sample.pressure {
                Some(_) => Sensor::Read,
                None => Sensor::Absent,
            },
            tilt: match sample.tilt {
                Some(_) => Sensor::Read,
                None => Sensor::Absent,
            },
        }
    }
}

#[derive(Debug, thiserror::Error, PartialEq)]
pub enum StrokeError {
    #[error("a stroke needs at least one sample")]
    Empty,
    #[error("sample {0} is earlier than the one before it")]
    Backwards(usize),
    #[error("sample {index} reads {found:?} but the first reads {expected:?}")]
    Mixed {
        index: usize,
        expected: Sensors,
        found: Sensors,
    },
}

impl Geometry {
    /// The only way to build a stroke's shape, so the mesh always matches its samples and brush.
    pub fn new(samples: Box<[Sample<Local>]>, brush: Brush) -> Result<Geometry, StrokeError> {
        let sensors = Sensors::of(samples.first().ok_or(StrokeError::Empty)?);
        let backwards = (samples.windows(2)).position(|pair| pair[1].time < pair[0].time);
        let mixed =
            (samples.iter().map(Sensors::of).enumerate()).find(|(_, found)| *found != sensors);
        match (backwards, mixed) {
            (Some(index), _) => return Err(StrokeError::Backwards(index + 1)),
            (None, Some((index, found))) => {
                return Err(StrokeError::Mixed {
                    index,
                    expected: sensors,
                    found,
                });
            }
            (None, None) => {}
        }
        let modeled = Modeler::model(brush.model, EPSILON, &samples);
        let mut builder = MeshBuilder::default();
        builder.update(&brush, &modeled);
        let mesh = builder.into_mesh();
        Ok(Geometry {
            samples,
            sensors,
            brush,
            built: Built {
                id: GeometryId::next(),
                bounds: mesh.bounds().ok_or(StrokeError::Empty)?,
                mesh,
            },
        })
    }
}

/// One layer's marks on a page or board, in drawing order.
#[derive(Clone, Debug, Default)]
pub struct Marks {
    map: SlotMap<MarkId, Arc<Mark>>, // private: `order` holds exactly the map's keys
    order: Vec<MarkId>,              // private: `order` holds exactly the map's keys
}

impl Marks {
    pub fn insert(&mut self, mark: Mark) -> MarkId {
        let id = self.map.insert(Arc::new(mark));
        self.order.push(id);
        id
    }

    pub fn remove(&mut self, id: MarkId) -> Option<Arc<Mark>> {
        let mark = self.map.remove(id)?;
        self.order.retain(|kept| *kept != id);
        Some(mark)
    }

    pub fn get(&self, id: MarkId) -> Option<&Arc<Mark>> {
        self.map.get(id)
    }

    /// Puts `mark` where mark `id` was, keeping its id and place in the order.
    pub fn replace(&mut self, id: MarkId, mark: Mark) -> Option<Arc<Mark>> {
        let slot = self.map.get_mut(id)?;
        Some(std::mem::replace(slot, Arc::new(mark)))
    }

    /// Moves mark `id` to `placement`; an id not in the marks is left alone.
    pub fn place(&mut self, id: MarkId, placement: Placement) {
        match self.map.get_mut(id) {
            Some(mark) => Arc::make_mut(mark).placement = placement,
            None => {}
        }
    }

    /// Bottom first.
    pub fn iter(&self) -> impl Iterator<Item = (MarkId, &Arc<Mark>)> {
        self.order.iter().map(|id| (*id, &self.map[*id]))
    }

    pub fn len(&self) -> usize {
        self.order.len()
    }

    pub fn is_empty(&self) -> bool {
        self.order.is_empty()
    }

    /// Marks reaching within `radius` of `at`, bottom first.
    pub fn touching(
        &self,
        at: crate::geom::Point<Surface>,
        radius: f32,
    ) -> impl Iterator<Item = MarkId> + '_ {
        self.iter()
            .filter(move |(_, mark)| mark.touches(at, radius))
            .map(|(id, _)| id)
    }
}

/// A change to a drawing's marks, on the layer of a page or board that `T` names.
#[derive(Clone, Debug)]
pub enum MarkChange<T> {
    Add(T, Mark),
    Remove(T, SmallVec<[MarkId; 8]>),
    /// Mark `id` becomes the mark given, as when its text is edited.
    Replace(T, MarkId, Mark),
    /// The marks moved by the transform, as one step.
    Transform(T, SmallVec<[MarkId; 8]>, Affine<Surface, Surface>),
}

impl<T: Copy> MarkChange<T> {
    pub fn target(&self) -> T {
        match self {
            MarkChange::Add(target, _)
            | MarkChange::Remove(target, _)
            | MarkChange::Replace(target, ..)
            | MarkChange::Transform(target, ..) => *target,
        }
    }
}

#[derive(Clone, Debug)]
pub enum LayerChange {
    Add(Layer),
    Set(LayerIndex, Layer),
}

/// A change refused because what it names isn't in the drawing.
#[derive(Debug, thiserror::Error, PartialEq)]
pub enum ChangeError {
    #[error("there is no page {0}")]
    NoPage(usize),
    #[error("there is no layer {0}")]
    NoLayer(usize),
    #[error("a mark to change is not on its layer")]
    NoMark,
    #[error("the transform would flatten a mark")]
    Flattened,
    #[error("a document keeps at least one page")]
    LastPage,
}

/// A paged document or a board: where marks go, and the changes it takes.
pub trait Drawing: Clone {
    /// A layer on a page, or on the board.
    type Target: Copy + std::fmt::Debug + PartialEq;
    /// A page, or the board's one plane.
    type Surface: Copy + std::fmt::Debug + PartialEq;
    type Change: From<MarkChange<Self::Target>> + From<LayerChange>;

    fn target(surface: Self::Surface, layer: LayerIndex) -> Self::Target;

    fn surface(target: Self::Target) -> Self::Surface;

    fn layers(&self) -> &Layers;

    fn marks(&self, target: Self::Target) -> Option<&Marks>;

    fn apply(&mut self, change: Self::Change) -> Result<(), ChangeError>;

    /// Adds `mark` on `target`, returning the id it is kept under.
    fn add(&mut self, target: Self::Target, mark: Mark) -> Result<MarkId, ChangeError>;
}

/// Applies `change` to `marks`, which belong to the target the change names.
pub fn apply_marks<T>(marks: &mut Arc<Marks>, change: MarkChange<T>) -> Result<(), ChangeError> {
    match change {
        MarkChange::Add(_, mark) => {
            Arc::make_mut(marks).insert(mark);
            Ok(())
        }
        MarkChange::Remove(_, ids) => {
            (ids.iter())
                .try_for_each(|id| marks.get(*id).map(|_| ()).ok_or(ChangeError::NoMark))?;
            let marks = Arc::make_mut(marks);
            for id in ids {
                marks.remove(id);
            }
            Ok(())
        }
        MarkChange::Replace(_, id, mark) => {
            marks.get(id).ok_or(ChangeError::NoMark)?;
            Arc::make_mut(marks).replace(id, mark);
            Ok(())
        }
        MarkChange::Transform(_, ids, by) => {
            let moved = (ids.iter())
                .map(|id| {
                    let mark = marks.get(*id).ok_or(ChangeError::NoMark)?;
                    let placement = Placement::new(mark.placement.forward.then(&by))
                        .ok_or(ChangeError::Flattened)?;
                    Ok((*id, placement))
                })
                .collect::<Result<SmallVec<[(MarkId, Placement); 8]>, ChangeError>>()?;
            let marks = Arc::make_mut(marks);
            for (id, placement) in moved {
                marks.place(id, placement);
            }
            Ok(())
        }
    }
}
