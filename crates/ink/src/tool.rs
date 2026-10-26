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
use crate::text::{TextInk, Wrap};

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
    /// The text tool asks for text to be typed there.
    Type(TypeAt<T>),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToolKind {
    Brush,
    Eraser,
    Select(Gather),
    Shape(ShapeKind),
    Text,
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
    pub text: Placing<T>,
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
            text: Placing::Idle,
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
            ToolKind::Text => self.text.input(input, drawing, reach),
        }
    }

    /// Drops any stroke in progress, as when the drawing under it is replaced.
    pub fn cancel(&mut self) {
        self.brush.state = Brushing::Idle;
        self.eraser.state = Erasing::Idle;
        self.select = Selecting::Idle;
        self.shape.state = Shaping::Idle;
        self.text = Placing::Idle;
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

/// Never empty; `frame` is the box the picked marks are handled by.
#[derive(Clone, Debug)]
pub struct Picked {
    pub ids: SmallVec<[MarkId; 8]>,
    pub frame: Frame,
}

impl Picked {
    /// One mark is handled by its own rectangle, turned with it; several by an upright box.
    pub fn gather<'a>(found: impl Iterator<Item = (MarkId, &'a Arc<Mark>)>) -> Option<Picked> {
        let found: SmallVec<[(MarkId, &Arc<Mark>); 8]> = found.collect();
        match found.as_slice() {
            [] => None,
            [(id, mark)] => Some(Picked {
                ids: smallvec::smallvec![*id],
                frame: Frame::of(mark),
            }),
            [(_, first), rest @ ..] => {
                let bounds = (rest.iter()).fold(first.bounds(), |bounds, (_, mark)| {
                    bounds.union(&mark.bounds())
                });
                Some(Picked {
                    ids: found.iter().map(|(id, _)| *id).collect(),
                    frame: Frame::upright(bounds),
                })
            }
        }
    }
}

/// A rectangle in its own space, placed on the surface: the box a selection is handled by.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Frame {
    pub rect: Rect<Local>,
    pub placement: Placement,
}

impl Frame {
    pub fn of(mark: &Mark) -> Frame {
        Frame {
            rect: mark.built().bounds,
            placement: mark.placement,
        }
    }

    pub fn upright(bounds: Rect<Surface>) -> Frame {
        Frame {
            rect: bounds.mapped(Placement::ALIGNED.back()),
            placement: Placement::ALIGNED,
        }
    }

    pub fn centre(&self) -> Point<Surface> {
        let rect = &self.rect;
        (self.placement.forward()).apply(rect.min().lerp(rect.max(), 0.5))
    }

    pub fn holds(&self, at: Point<Surface>) -> bool {
        self.rect.contains(self.placement.back().apply(at))
    }

    /// The handles to grab at `reach`, and where they sit; a side has one only once the frame is
    /// three reaches deep across it, so it stays clear of the corners and the middle.
    pub fn handles(&self, reach: Size) -> impl Iterator<Item = (Handle, Point<Surface>)> + '_ {
        let forward = self.placement.forward();
        (Handle::ALL.into_iter())
            .filter(move |handle| match handle {
                Handle::Edge(edge) => {
                    let (near, far) = (edge.of(&self.rect), edge.opposite().of(&self.rect));
                    forward.apply(near).distance(forward.apply(far)) >= 3.0 * reach.get()
                }
                Handle::Corner(_) | Handle::Rotate => true,
            })
            .map(move |handle| (handle, handle.at(self, reach)))
    }

    /// The frame after `by`, or `None` when `by` flattens it.
    pub fn moved(&self, by: &Affine<Surface, Surface>) -> Option<Frame> {
        Placement::new(self.placement.forward().then(by)).map(|placement| Frame {
            rect: self.rect,
            placement,
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
    Resizing {
        resize: Resize,
        to: Point<Surface>,
    },
}

/// What a resize holds: a corner, which keeps the shape, or a side, which stretches one way.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Resize {
    Corner(Corner),
    Edge(Edge),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Corner {
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Edge {
    Top,
    Right,
    Bottom,
    Left,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Handle {
    Corner(Corner),
    Edge(Edge),
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

    pub fn of<S>(self, rect: &Rect<S>) -> Point<S> {
        let (min, max) = (rect.min(), rect.max());
        match self {
            Corner::TopLeft => min,
            Corner::TopRight => Point::new(max.x, min.y),
            Corner::BottomLeft => Point::new(min.x, max.y),
            Corner::BottomRight => max,
        }
    }

    pub fn opposite(self) -> Corner {
        match self {
            Corner::TopLeft => Corner::BottomRight,
            Corner::TopRight => Corner::BottomLeft,
            Corner::BottomLeft => Corner::TopRight,
            Corner::BottomRight => Corner::TopLeft,
        }
    }
}

impl Edge {
    /// The middle of the side.
    pub fn of<S>(self, rect: &Rect<S>) -> Point<S> {
        let (min, max) = (rect.min(), rect.max());
        let middle = min.lerp(max, 0.5);
        match self {
            Edge::Top => Point::new(middle.x, min.y),
            Edge::Right => Point::new(max.x, middle.y),
            Edge::Bottom => Point::new(middle.x, max.y),
            Edge::Left => Point::new(min.x, middle.y),
        }
    }

    pub fn opposite(self) -> Edge {
        match self {
            Edge::Top => Edge::Bottom,
            Edge::Right => Edge::Left,
            Edge::Bottom => Edge::Top,
            Edge::Left => Edge::Right,
        }
    }
}

impl Handle {
    pub const ALL: [Handle; 9] = [
        Handle::Corner(Corner::TopLeft),
        Handle::Corner(Corner::TopRight),
        Handle::Corner(Corner::BottomLeft),
        Handle::Corner(Corner::BottomRight),
        Handle::Edge(Edge::Top),
        Handle::Edge(Edge::Right),
        Handle::Edge(Edge::Bottom),
        Handle::Edge(Edge::Left),
        Handle::Rotate,
    ];

    /// Where the handle sits on `frame` when handles reach `reach`; the rotate handle stands off
    /// the top side, outward however the frame is turned.
    pub fn at(self, frame: &Frame, reach: Size) -> Point<Surface> {
        let forward = frame.placement.forward();
        match self {
            Handle::Corner(corner) => forward.apply(corner.of(&frame.rect)),
            Handle::Edge(edge) => forward.apply(edge.of(&frame.rect)),
            Handle::Rotate => {
                let top = forward.apply(Edge::Top.of(&frame.rect));
                let out = top - frame.centre();
                top + out * (4.0 * reach.get() / out.length().max(f32::MIN_POSITIVE))
            }
        }
    }
}

impl Grip {
    pub fn to(self, to: Point<Surface>) -> Grip {
        match self {
            Grip::Still => Grip::Still,
            Grip::Moving { from, .. } => Grip::Moving { from, to },
            Grip::Rotating { center, from, .. } => Grip::Rotating { center, from, to },
            Grip::Resizing { resize, .. } => Grip::Resizing { resize, to },
        }
    }

    /// How the grip moves the picked marks. A resize scales in the frame's own space, so a turned
    /// frame keeps its shape; `frame.rect` is never flat, since marks have width.
    pub fn transform(&self, frame: &Frame) -> Affine<Surface, Surface> {
        match *self {
            Grip::Still => Affine::identity(),
            Grip::Moving { from, to } => Affine::translation(to.x - from.x, to.y - from.y),
            Grip::Rotating { center, from, to } => {
                let (a, b) = (from - center, to - center);
                Affine::rotation_about(center, a.cross(b).atan2(a.dot(b)))
            }
            Grip::Resizing { resize, to } => {
                let (rect, back) = (&frame.rect, frame.placement.back());
                let to = back.apply(to);
                let local = match resize {
                    // Scaled by how far the pen has gone along the corner's diagonal.
                    Resize::Corner(corner) => {
                        let anchor = corner.opposite().of(rect);
                        let diagonal = corner.of(rect) - anchor;
                        let scale = (to - anchor).dot(diagonal) / diagonal.dot(diagonal);
                        Affine::scaling_about(anchor, scale, scale)
                    }
                    Resize::Edge(edge) => {
                        let anchor = edge.opposite().of(rect);
                        let grabbed = edge.of(rect);
                        match edge {
                            Edge::Top | Edge::Bottom => Affine::scaling_about(
                                anchor,
                                1.0,
                                (to.y - anchor.y) / (grabbed.y - anchor.y),
                            ),
                            Edge::Left | Edge::Right => Affine::scaling_about(
                                anchor,
                                (to.x - anchor.x) / (grabbed.x - anchor.x),
                                1.0,
                            ),
                        }
                    }
                };
                back.then(&local).then(frame.placement.forward())
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
                let center = picked.frame.centre();
                let grip = (picked.frame.handles(reach))
                    .map(|(handle, sits)| (handle, sits.distance(at)))
                    .min_by(|(_, a), (_, b)| a.total_cmp(b))
                    .filter(|(_, distance)| *distance <= reach.get())
                    .map(|(handle, _)| match handle {
                        Handle::Corner(corner) => Grip::Resizing {
                            resize: Resize::Corner(corner),
                            to: at,
                        },
                        Handle::Edge(edge) => Grip::Resizing {
                            resize: Resize::Edge(edge),
                            to: at,
                        },
                        Handle::Rotate => Grip::Rotating {
                            center,
                            from: at,
                            to: at,
                        },
                    })
                    .or_else(|| {
                        (picked.frame.holds(at)).then_some(Grip::Moving { from: at, to: at })
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
                let by = grip.transform(&picked.frame);
                // A refused transform leaves the marks, and so the frame, as they were.
                let frame = picked.frame.moved(&by).unwrap_or(picked.frame);
                *self = Selecting::Holding {
                    target,
                    picked: Picked {
                        ids: picked.ids.clone(),
                        frame,
                    },
                    grip: Grip::Still,
                };
                match grip {
                    Grip::Still => Outcome::Ignored,
                    Grip::Moving { .. } | Grip::Rotating { .. } | Grip::Resizing { .. } => {
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

/// Places text: a press on a text mark opens it, elsewhere a drag sets the width of new text.
#[derive(Debug)]
pub enum Placing<T> {
    Idle,
    Dragging {
        target: T,
        from: Point<Surface>,
        to: Point<Surface>,
    },
}

/// Where the text tool asks for text.
#[derive(Clone, Debug)]
pub enum TypeAt<T> {
    /// New text with its top left at `at`.
    New {
        target: T,
        at: Point<Surface>,
        wrap: Wrap,
    },
    /// The text mark `id`, as found under the pen, to edit.
    Mark {
        target: T,
        id: MarkId,
        text: Arc<TextInk>,
        placement: Placement,
        paint: Paint,
    },
}

impl<T: Copy> Placing<T> {
    pub fn input<D: Drawing<Target = T>>(
        &mut self,
        input: PenInput<T>,
        drawing: &D,
        reach: Size,
    ) -> Outcome<T> {
        match (std::mem::replace(self, Placing::Idle), input) {
            (_, PenInput::Down { target, sample }) => {
                let under = (drawing.marks(target).into_iter())
                    .flat_map(|marks| marks.iter())
                    .filter_map(|(id, mark)| match &mark.ink {
                        Ink::Text(text) => Some((id, mark, text)),
                        Ink::Stroke(_) | Ink::Shape(_) | Ink::Image(_) => None,
                    })
                    .filter(|(_, mark, _)| mark.touches(sample.at, reach.get()))
                    .last();
                match under {
                    Some((id, mark, text)) => Outcome::Type(TypeAt::Mark {
                        target,
                        id,
                        text: text.clone(),
                        placement: mark.placement,
                        paint: mark.paint,
                    }),
                    None => {
                        *self = Placing::Dragging {
                            target,
                            from: sample.at,
                            to: sample.at,
                        };
                        Outcome::Drawn
                    }
                }
            }
            (Placing::Dragging { target, from, .. }, PenInput::Move(sample)) => {
                *self = Placing::Dragging {
                    target,
                    from,
                    to: sample.at,
                };
                Outcome::Drawn
            }
            (Placing::Dragging { target, from, .. }, PenInput::Up(sample)) => {
                let area = Rect::spanning(from, sample.at);
                // A tap starts unwrapped text.
                let wrap = (Size::new(area.max().x - area.min().x).ok())
                    .filter(|width| width.get() > reach.get())
                    .map_or(Wrap::Lines, Wrap::At);
                Outcome::Type(TypeAt::New {
                    target,
                    at: area.min(),
                    wrap,
                })
            }
            (Placing::Dragging { .. }, PenInput::Cancel) => Outcome::Drawn,
            (Placing::Idle, PenInput::Move(_) | PenInput::Up(_) | PenInput::Cancel) => {
                Outcome::Ignored
            }
        }
    }
}
