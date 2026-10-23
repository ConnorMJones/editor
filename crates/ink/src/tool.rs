//! Pen tools as state machines over pen input; a tool reads the drawing but only proposes
//! changes to it.

use std::sync::Arc;

use smallvec::SmallVec;

use crate::brush::{Brush, EPSILON, Paint, Size};
use crate::doc::{
    Drawing, Geometry, Ink, Mark, MarkChange, MarkId, Marks, Placement, Shape, ShapeInk, ShapeKind,
    StrokeError,
};
use crate::geom::{Affine, Local, Point, Rect, Surface, inside_polygon};
use crate::input::{Modeled, Modeler, Sample};
use crate::mesh::MeshBuilder;

/// Pen input on the surface of the target fixed at `Down`.
#[derive(Clone, Copy, Debug)]
pub enum PenInput<T> {
    Down { target: T, sample: Sample<Surface> },
    Move(Sample<Surface>),
    Up(Sample<Surface>),
    Cancel,
}

#[derive(Debug)]
pub enum Outcome<T> {
    /// The input meant nothing in the tool's state.
    Ignored,
    /// What the tool shows changed.
    Drawn,
    Commit(MarkChange<T>),
    Failed(StrokeError),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToolKind {
    Brush,
    Eraser,
    Select(Gather),
    Shape(ShapeKind),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Gather {
    /// Marks wholly inside the box dragged out.
    Box,
    /// Marks mostly inside the loop drawn.
    Lasso,
}

#[derive(Debug)]
pub struct Tools<T> {
    pub brush: BrushTool<T>,
    pub eraser: EraserTool<T>,
    pub select: Selecting<T>,
    pub shape: ShapeTool<T>,
}

impl<T: Copy + PartialEq> Tools<T> {
    pub fn new(brush: Brush, paint: Paint, eraser: Size) -> Tools<T> {
        Tools {
            brush: BrushTool::new(brush, paint),
            eraser: EraserTool::new(eraser),
            select: Selecting::Idle,
            shape: ShapeTool {
                width: brush.size,
                paint,
                state: Shaping::Idle,
            },
        }
    }

    pub fn input<D: Drawing<Target = T>>(
        &mut self,
        kind: ToolKind,
        input: PenInput<T>,
        drawing: &D,
        reach: Size,
    ) -> Outcome<T> {
        match kind {
            ToolKind::Brush => self.brush.input(input),
            ToolKind::Eraser => self.eraser.input(input, drawing),
            ToolKind::Select(gather) => self.select.input(gather, input, drawing, reach),
            ToolKind::Shape(kind) => self.shape.input(kind, input),
        }
    }

    /// Drops any stroke in progress, as when the drawing under it is replaced.
    pub fn cancel(&mut self) {
        self.brush.state = Brushing::Idle;
        self.eraser.state = Erasing::Idle;
        self.select = Selecting::Idle;
        self.shape.state = Shaping::Idle;
    }
}

#[derive(Debug)]
pub struct BrushTool<T> {
    pub brush: Brush,
    pub paint: Paint,
    pub state: Brushing<T>,
    pub stroke: InProgress,
}

#[derive(Debug)]
pub enum Brushing<T> {
    Idle,
    /// `brush` and `paint` are the settings at `Down`.
    Drawing {
        target: T,
        brush: Brush,
        paint: Paint,
        modeler: Modeler,
    },
}

/// A stroke's samples and what is built from them so far; the buffers live across strokes.
#[derive(Debug, Default)]
pub struct InProgress {
    pub samples: Vec<Sample<Local>>,
    pub modeled: Modeled,
    pub mesh: MeshBuilder,
    /// Moves with every update of the mesh, so the host uploads it again.
    pub revision: u64,
}

impl InProgress {
    fn clear(&mut self) {
        self.samples.clear();
        self.modeled.clear();
        self.mesh.clear();
    }

    /// Adds `sample` and rebuilds what changed of the mesh.
    fn extend(&mut self, modeler: &mut Modeler, brush: &Brush, sample: Sample<Surface>) {
        let sample = Sample {
            at: Placement::ALIGNED.back().apply(sample.at),
            time: sample.time,
            pressure: sample.pressure,
            tilt: sample.tilt,
        };
        self.samples.push(sample);
        modeler.extend(&[sample], &[], &mut self.modeled);
        self.mesh.update(brush, &self.modeled);
        self.revision += 1;
    }
}

impl<T: Copy> BrushTool<T> {
    pub fn new(brush: Brush, paint: Paint) -> BrushTool<T> {
        BrushTool {
            brush,
            paint,
            state: Brushing::Idle,
            stroke: InProgress::default(),
        }
    }

    pub fn input(&mut self, input: PenInput<T>) -> Outcome<T> {
        match (std::mem::replace(&mut self.state, Brushing::Idle), input) {
            (Brushing::Idle | Brushing::Drawing { .. }, PenInput::Down { target, sample }) => {
                let mut modeler = Modeler::new(self.brush.model, EPSILON);
                self.stroke.clear();
                self.stroke.extend(&mut modeler, &self.brush, sample);
                self.state = Brushing::Drawing {
                    target,
                    brush: self.brush,
                    paint: self.paint,
                    modeler,
                };
                Outcome::Drawn
            }
            (
                Brushing::Drawing {
                    target,
                    brush,
                    paint,
                    mut modeler,
                },
                PenInput::Move(sample),
            ) => {
                self.stroke.extend(&mut modeler, &brush, sample);
                self.state = Brushing::Drawing {
                    target,
                    brush,
                    paint,
                    modeler,
                };
                Outcome::Drawn
            }
            (
                Brushing::Drawing {
                    target,
                    brush,
                    paint,
                    mut modeler,
                },
                PenInput::Up(sample),
            ) => {
                self.stroke.extend(&mut modeler, &brush, sample);
                match Geometry::new(self.stroke.samples.as_slice().into(), brush) {
                    Ok(geometry) => Outcome::Commit(MarkChange::Add(
                        target,
                        Mark {
                            placement: Placement::ALIGNED,
                            paint,
                            ink: Ink::Stroke(Arc::new(geometry)),
                        },
                    )),
                    Err(error) => Outcome::Failed(error),
                }
            }
            (Brushing::Drawing { .. }, PenInput::Cancel) => Outcome::Drawn,
            (Brushing::Idle, PenInput::Move(_) | PenInput::Up(_) | PenInput::Cancel) => {
                Outcome::Ignored
            }
        }
    }
}

#[derive(Debug)]
pub struct EraserTool<T> {
    pub size: Size,
    pub state: Erasing<T>,
}

#[derive(Debug)]
pub enum Erasing<T> {
    Idle,
    /// `erased` holds the marks touched so far, which the host hides until `Up` removes them.
    Erasing {
        target: T,
        erased: SmallVec<[MarkId; 8]>,
    },
}

impl<T: Copy> EraserTool<T> {
    pub fn new(size: Size) -> EraserTool<T> {
        EraserTool {
            size,
            state: Erasing::Idle,
        }
    }

    /// A target that has left the drawing ends the stroke.
    pub fn input<D: Drawing<Target = T>>(&mut self, input: PenInput<T>, drawing: &D) -> Outcome<T> {
        let radius = self.size.get() * 0.5;
        match (std::mem::replace(&mut self.state, Erasing::Idle), input) {
            (Erasing::Idle | Erasing::Erasing { .. }, PenInput::Down { target, sample }) => {
                match drawing.marks(target) {
                    Some(marks) => {
                        let mut erased = SmallVec::new();
                        touch(marks, sample, radius, &mut erased);
                        self.state = Erasing::Erasing { target, erased };
                        Outcome::Drawn
                    }
                    None => Outcome::Ignored,
                }
            }
            (Erasing::Erasing { target, mut erased }, PenInput::Move(sample)) => {
                match drawing.marks(target) {
                    Some(marks) => {
                        let outcome = match touch(marks, sample, radius, &mut erased) {
                            0 => Outcome::Ignored,
                            _ => Outcome::Drawn,
                        };
                        self.state = Erasing::Erasing { target, erased };
                        outcome
                    }
                    None => Outcome::Drawn,
                }
            }
            (Erasing::Erasing { target, mut erased }, PenInput::Up(sample)) => {
                match drawing.marks(target) {
                    Some(marks) => {
                        touch(marks, sample, radius, &mut erased);
                        match erased.len() {
                            0 => Outcome::Drawn,
                            _ => Outcome::Commit(MarkChange::Remove(target, erased)),
                        }
                    }
                    None => Outcome::Drawn,
                }
            }
            (Erasing::Erasing { .. }, PenInput::Cancel) => Outcome::Drawn,
            (Erasing::Idle, PenInput::Move(_) | PenInput::Up(_) | PenInput::Cancel) => {
                Outcome::Ignored
            }
        }
    }
}

/// Adds the marks within `radius` of `sample` that aren't in `erased` yet, returning how many.
fn touch(
    marks: &Marks,
    sample: Sample<Surface>,
    radius: f32,
    erased: &mut SmallVec<[MarkId; 8]>,
) -> usize {
    let fresh: SmallVec<[MarkId; 8]> = (marks.touching(sample.at, radius))
        .filter(|id| !erased.contains(id))
        .collect();
    erased.extend_from_slice(&fresh);
    fresh.len()
}

#[derive(Debug)]
pub enum Selecting<T> {
    Idle,
    /// Down but not yet moved past the reach: lifting here is a tap.
    Pressed {
        target: T,
        at: Point<Surface>,
    },
    Boxing {
        target: T,
        from: Point<Surface>,
        to: Point<Surface>,
    },
    Lassoing {
        target: T,
        lasso: Vec<Point<Surface>>,
    },
    /// Marks picked on one target, perhaps under a grip.
    Holding {
        target: T,
        picked: Picked,
        grip: Grip,
    },
}

/// Never empty; `bounds` holds the picked marks as they were when the grip began.
#[derive(Clone, Debug)]
pub struct Picked {
    pub ids: SmallVec<[MarkId; 8]>,
    pub bounds: Rect<Surface>,
}

impl Picked {
    fn gather<'a>(found: impl Iterator<Item = (MarkId, &'a Arc<Mark>)>) -> Option<Picked> {
        found.fold(None, |picked, (id, mark)| {
            let bounds = mark.bounds();
            Some(match picked {
                None => Picked {
                    ids: smallvec::smallvec![id],
                    bounds,
                },
                Some(mut picked) => {
                    picked.ids.push(id);
                    picked.bounds = picked.bounds.union(&bounds);
                    picked
                }
            })
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Grip {
    Still,
    Moving {
        from: Point<Surface>,
        to: Point<Surface>,
    },
    Rotating {
        center: Point<Surface>,
        from: Point<Surface>,
        to: Point<Surface>,
    },
    Scaling {
        corner: Corner,
        to: Point<Surface>,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Corner {
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Handle {
    Corner(Corner),
    Rotate,
}

impl Corner {
    /// Clockwise from the top left, as an outline visits them.
    pub const AROUND: [Corner; 4] = [
        Corner::TopLeft,
        Corner::TopRight,
        Corner::BottomRight,
        Corner::BottomLeft,
    ];

    pub fn of(self, rect: &Rect<Surface>) -> Point<Surface> {
        let (min, max) = (rect.min(), rect.max());
        match self {
            Corner::TopLeft => min,
            Corner::TopRight => Point::new(max.x, min.y),
            Corner::BottomLeft => Point::new(min.x, max.y),
            Corner::BottomRight => max,
        }
    }

    fn opposite(self) -> Corner {
        match self {
            Corner::TopLeft => Corner::BottomRight,
            Corner::TopRight => Corner::BottomLeft,
            Corner::BottomLeft => Corner::TopRight,
            Corner::BottomRight => Corner::TopLeft,
        }
    }
}

impl Handle {
    pub const ALL: [Handle; 5] = [
        Handle::Corner(Corner::TopLeft),
        Handle::Corner(Corner::TopRight),
        Handle::Corner(Corner::BottomLeft),
        Handle::Corner(Corner::BottomRight),
        Handle::Rotate,
    ];

    /// Where the handle sits on `bounds` when handles reach `reach`.
    pub fn at(self, bounds: &Rect<Surface>, reach: Size) -> Point<Surface> {
        match self {
            Handle::Corner(corner) => corner.of(bounds),
            Handle::Rotate => Point::new(
                (bounds.min().x + bounds.max().x) * 0.5,
                bounds.min().y - 4.0 * reach.get(),
            ),
        }
    }
}

impl Grip {
    pub fn to(self, to: Point<Surface>) -> Grip {
        match self {
            Grip::Still => Grip::Still,
            Grip::Moving { from, .. } => Grip::Moving { from, to },
            Grip::Rotating { center, from, .. } => Grip::Rotating { center, from, to },
            Grip::Scaling { corner, .. } => Grip::Scaling { corner, to },
        }
    }

    /// How the grip moves the picked marks; `bounds` is never flat, since marks have width.
    pub fn transform(&self, bounds: &Rect<Surface>) -> Affine<Surface, Surface> {
        match *self {
            Grip::Still => Affine::identity(),
            Grip::Moving { from, to } => Affine::translation(to.x - from.x, to.y - from.y),
            Grip::Rotating { center, from, to } => {
                let (a, b) = (from - center, to - center);
                Affine::rotation_about(center, a.cross(b).atan2(a.dot(b)))
            }
            Grip::Scaling { corner, to } => {
                let anchor = corner.opposite().of(bounds);
                let grabbed = corner.of(bounds);
                Affine::scaling_about(
                    anchor,
                    (to.x - anchor.x) / (grabbed.x - anchor.x),
                    (to.y - anchor.y) / (grabbed.y - anchor.y),
                )
            }
        }
    }
}

impl<T: Copy + PartialEq> Selecting<T> {
    /// `reach` is how far a tap or handle reaches on the surface.
    pub fn input<D: Drawing<Target = T>>(
        &mut self,
        gather: Gather,
        input: PenInput<T>,
        drawing: &D,
        reach: Size,
    ) -> Outcome<T> {
        match (std::mem::replace(self, Selecting::Idle), input) {
            (
                Selecting::Holding {
                    target: held,
                    picked,
                    grip: Grip::Still,
                },
                PenInput::Down { target, sample },
            ) if held == target => {
                let at = sample.at;
                let center = picked.bounds.min().lerp(picked.bounds.max(), 0.5);
                let grip = (Handle::ALL.into_iter())
                    .map(|handle| (handle, handle.at(&picked.bounds, reach).distance(at)))
                    .min_by(|(_, a), (_, b)| a.total_cmp(b))
                    .filter(|(_, distance)| *distance <= reach.get())
                    .map(|(handle, _)| match handle {
                        Handle::Corner(corner) => Grip::Scaling { corner, to: at },
                        Handle::Rotate => Grip::Rotating {
                            center,
                            from: at,
                            to: at,
                        },
                    })
                    .or_else(|| {
                        (picked.bounds.contains(at)).then_some(Grip::Moving { from: at, to: at })
                    });
                *self = match grip {
                    Some(grip) => Selecting::Holding {
                        target,
                        picked,
                        grip,
                    },
                    None => Selecting::Pressed { target, at },
                };
                Outcome::Drawn
            }
            (_, PenInput::Down { target, sample }) => {
                *self = Selecting::Pressed {
                    target,
                    at: sample.at,
                };
                Outcome::Drawn
            }
            (Selecting::Pressed { target, at }, PenInput::Move(sample))
                if at.distance(sample.at) > reach.get() =>
            {
                *self = match gather {
                    Gather::Box => Selecting::Boxing {
                        target,
                        from: at,
                        to: sample.at,
                    },
                    Gather::Lasso => Selecting::Lassoing {
                        target,
                        lasso: vec![at, sample.at],
                    },
                };
                Outcome::Drawn
            }
            (pressed @ Selecting::Pressed { .. }, PenInput::Move(_)) => {
                *self = pressed;
                Outcome::Ignored
            }
            (Selecting::Pressed { target, at }, PenInput::Up(_)) => {
                let picked = (drawing.marks(target)).and_then(|marks| {
                    Picked::gather(
                        marks
                            .iter()
                            .filter(|(_, mark)| mark.touches(at, reach.get()))
                            .last()
                            .into_iter(),
                    )
                });
                *self = held(target, picked);
                Outcome::Drawn
            }
            (Selecting::Boxing { target, from, .. }, PenInput::Move(sample)) => {
                *self = Selecting::Boxing {
                    target,
                    from,
                    to: sample.at,
                };
                Outcome::Drawn
            }
            (Selecting::Boxing { target, from, .. }, PenInput::Up(sample)) => {
                let around = Rect::spanning(from, sample.at);
                let picked = (drawing.marks(target)).and_then(|marks| {
                    Picked::gather(marks.iter().filter(|(_, mark)| {
                        let bounds = mark.bounds();
                        around.contains(bounds.min()) && around.contains(bounds.max())
                    }))
                });
                *self = held(target, picked);
                Outcome::Drawn
            }
            (Selecting::Lassoing { target, mut lasso }, PenInput::Move(sample)) => {
                lasso.push(sample.at);
                *self = Selecting::Lassoing { target, lasso };
                Outcome::Drawn
            }
            (Selecting::Lassoing { target, mut lasso }, PenInput::Up(sample)) => {
                lasso.push(sample.at);
                let around = (lasso.iter()).fold(Rect::spanning(lasso[0], lasso[0]), |rect, at| {
                    rect.union(&Rect::spanning(*at, *at))
                });
                let picked = (drawing.marks(target)).and_then(|marks| {
                    Picked::gather(marks.iter().filter(|(_, mark)| {
                        mark.bounds().intersects(&around) && enclosed(mark, &lasso)
                    }))
                });
                *self = held(target, picked);
                Outcome::Drawn
            }
            (
                Selecting::Holding {
                    target,
                    picked,
                    grip,
                },
                PenInput::Move(sample),
            ) => {
                *self = Selecting::Holding {
                    target,
                    picked,
                    grip: grip.to(sample.at),
                };
                Outcome::Drawn
            }
            (
                Selecting::Holding {
                    target,
                    picked,
                    grip,
                },
                PenInput::Up(sample),
            ) => {
                let grip = grip.to(sample.at);
                let by = grip.transform(&picked.bounds);
                *self = Selecting::Holding {
                    target,
                    picked: Picked {
                        ids: picked.ids.clone(),
                        bounds: picked.bounds.mapped(&by),
                    },
                    grip: Grip::Still,
                };
                match grip {
                    Grip::Still => Outcome::Ignored,
                    Grip::Moving { .. } | Grip::Rotating { .. } | Grip::Scaling { .. } => {
                        Outcome::Commit(MarkChange::Transform(target, picked.ids, by))
                    }
                }
            }
            (Selecting::Holding { target, picked, .. }, PenInput::Cancel) => {
                *self = Selecting::Holding {
                    target,
                    picked,
                    grip: Grip::Still,
                };
                Outcome::Drawn
            }
            (
                Selecting::Pressed { .. } | Selecting::Boxing { .. } | Selecting::Lassoing { .. },
                PenInput::Cancel,
            ) => Outcome::Drawn,
            (Selecting::Idle, PenInput::Move(_) | PenInput::Up(_) | PenInput::Cancel) => {
                Outcome::Ignored
            }
        }
    }

    /// Removes the picked marks, leaving nothing picked.
    pub fn delete(&mut self) -> Option<MarkChange<T>> {
        match std::mem::replace(self, Selecting::Idle) {
            Selecting::Holding { target, picked, .. } => {
                Some(MarkChange::Remove(target, picked.ids))
            }
            Selecting::Idle
            | Selecting::Pressed { .. }
            | Selecting::Boxing { .. }
            | Selecting::Lassoing { .. } => None,
        }
    }
}

fn held<T>(target: T, picked: Option<Picked>) -> Selecting<T> {
    match picked {
        Some(picked) => Selecting::Holding {
            target,
            picked,
            grip: Grip::Still,
        },
        None => Selecting::Idle,
    }
}

/// Whether most of a mark's outline lies inside the lasso, judged from up to 32 of its vertices.
fn enclosed(mark: &Mark, lasso: &[Point<Surface>]) -> bool {
    let vertices = &mark.built().mesh.vertices;
    let step = (vertices.len() / 32).max(1);
    let forward = mark.placement.forward();
    let inside = (vertices.iter().step_by(step))
        .filter(|at| inside_polygon(forward.apply(**at), lasso))
        .count();
    inside * 2 >= vertices.len().div_ceil(step)
}

/// Draws a shape by dragging from one corner or end to the other.
#[derive(Debug)]
pub struct ShapeTool<T> {
    pub width: Size,
    pub paint: Paint,
    pub state: Shaping<T>,
}

#[derive(Debug)]
pub enum Shaping<T> {
    Idle,
    /// Down but not moved: lifting here draws nothing.
    Pressed {
        target: T,
        kind: ShapeKind,
        from: Point<Local>,
    },
    Dragging {
        target: T,
        kind: ShapeKind,
        from: Point<Local>,
        ink: ShapeInk,
    },
}

impl<T: Copy> ShapeTool<T> {
    pub fn input(&mut self, kind: ShapeKind, input: PenInput<T>) -> Outcome<T> {
        // A new shape's own space is aligned with its surface.
        let local = |at: Point<Surface>| Placement::ALIGNED.back().apply(at);
        match (std::mem::replace(&mut self.state, Shaping::Idle), input) {
            (_, PenInput::Down { target, sample }) => {
                self.state = Shaping::Pressed {
                    target,
                    kind,
                    from: local(sample.at),
                };
                Outcome::Drawn
            }
            (
                Shaping::Pressed { target, kind, from }
                | Shaping::Dragging {
                    target, kind, from, ..
                },
                PenInput::Move(sample),
            ) => {
                self.state = Shaping::Dragging {
                    target,
                    kind,
                    from,
                    ink: ShapeInk::new(Shape::dragged(kind, from, local(sample.at)), self.width),
                };
                Outcome::Drawn
            }
            (
                Shaping::Dragging {
                    target, kind, from, ..
                },
                PenInput::Up(sample),
            ) => {
                let shape = Shape::dragged(kind, from, local(sample.at));
                Outcome::Commit(MarkChange::Add(
                    target,
                    Mark {
                        placement: Placement::ALIGNED,
                        paint: self.paint,
                        ink: Ink::Shape(Arc::new(ShapeInk::new(shape, self.width))),
                    },
                ))
            }
            (Shaping::Pressed { .. }, PenInput::Up(_) | PenInput::Cancel)
            | (Shaping::Dragging { .. }, PenInput::Cancel) => Outcome::Drawn,
            (Shaping::Idle, PenInput::Move(_) | PenInput::Up(_) | PenInput::Cancel) => {
                Outcome::Ignored
            }
        }
    }
}
