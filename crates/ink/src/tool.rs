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

/// The narrowest a text frame's side can wrap its text, in points.
pub const MIN_WRAP: f32 = 4.0;

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
    Rewrap {
        target: T,
        mark: TextMark,
        to: Rewrapped,
    },
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

/// The tools' settings, and buffers kept across strokes.
#[derive(Debug)]
pub struct Tools {
    pub brush: Brush,
    pub paint: Paint,
    pub eraser: Size,
    pub shape_width: Size,
    pub stroke: InProgress,
}

#[derive(Debug)]
pub enum Using<T> {
    Idle,
    /// `brush` and `paint` are the settings at `Down`.
    Brushing {
        target: T,
        brush: Brush,
        paint: Paint,
        modeler: Modeler,
    },
    /// `erased` holds the marks touched so far, which the host hides until `Up` removes them.
    Erasing {
        target: T,
        erased: SmallVec<[MarkId; 8]>,
    },
    Selecting(Selecting<T>),
    Shaping(Shaping<T>),
    Placing {
        target: T,
        from: Point<Surface>,
        to: Point<Surface>,
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

impl Tools {
    pub fn new(brush: Brush, paint: Paint, eraser: Size) -> Tools {
        Tools {
            brush,
            paint,
            eraser,
            shape_width: brush.size,
            stroke: InProgress::default(),
        }
    }

    /// A pen-down replaces whatever another tool was doing.
    pub fn input<T: Copy + PartialEq, D: Drawing<Target = T>>(
        &mut self,
        kind: ToolKind,
        input: PenInput<T>,
        using: &mut Using<T>,
        drawing: &D,
        reach: Size,
    ) -> Outcome<T> {
        let was = std::mem::replace(using, Using::Idle);
        match kind {
            ToolKind::Brush => self.brush(was, input, using),
            ToolKind::Eraser => self.erase(was, input, using, drawing),
            ToolKind::Select(gather) => select(was, gather, input, using, drawing, reach),
            ToolKind::Shape(kind) => self.shape(was, kind, input, using),
            ToolKind::Text => place(was, input, using, drawing, reach),
        }
    }

    fn brush<T>(&mut self, was: Using<T>, input: PenInput<T>, using: &mut Using<T>) -> Outcome<T> {
        match (was, input) {
            (_, PenInput::Down { target, sample }) => {
                let mut modeler = Modeler::new(self.brush.model, EPSILON);
                self.stroke.clear();
                self.stroke.extend(&mut modeler, &self.brush, sample);
                *using = Using::Brushing {
                    target,
                    brush: self.brush,
                    paint: self.paint,
                    modeler,
                };
                Outcome::Drawn
            }
            (
                Using::Brushing {
                    target,
                    brush,
                    paint,
                    mut modeler,
                },
                PenInput::Move(sample),
            ) => {
                self.stroke.extend(&mut modeler, &brush, sample);
                *using = Using::Brushing {
                    target,
                    brush,
                    paint,
                    modeler,
                };
                Outcome::Drawn
            }
            (
                Using::Brushing {
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
            (Using::Brushing { .. }, PenInput::Cancel) => Outcome::Drawn,
            (was, PenInput::Move(_) | PenInput::Up(_) | PenInput::Cancel) => {
                *using = was;
                Outcome::Ignored
            }
        }
    }

    /// A target that has left the drawing ends the stroke.
    fn erase<T: Copy, D: Drawing<Target = T>>(
        &mut self,
        was: Using<T>,
        input: PenInput<T>,
        using: &mut Using<T>,
        drawing: &D,
    ) -> Outcome<T> {
        let radius = self.eraser.get() * 0.5;
        match (was, input) {
            (_, PenInput::Down { target, sample }) => match drawing.marks(target) {
                Some(marks) => {
                    let mut erased = SmallVec::new();
                    touch(marks, sample, radius, &mut erased);
                    *using = Using::Erasing { target, erased };
                    Outcome::Drawn
                }
                None => Outcome::Ignored,
            },
            (Using::Erasing { target, mut erased }, PenInput::Move(sample)) => {
                match drawing.marks(target) {
                    Some(marks) => {
                        let outcome = match touch(marks, sample, radius, &mut erased) {
                            0 => Outcome::Ignored,
                            _ => Outcome::Drawn,
                        };
                        *using = Using::Erasing { target, erased };
                        outcome
                    }
                    None => Outcome::Drawn,
                }
            }
            (Using::Erasing { target, mut erased }, PenInput::Up(sample)) => {
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
            (Using::Erasing { .. }, PenInput::Cancel) => Outcome::Drawn,
            (was, PenInput::Move(_) | PenInput::Up(_) | PenInput::Cancel) => {
                *using = was;
                Outcome::Ignored
            }
        }
    }

    fn shape<T>(
        &mut self,
        was: Using<T>,
        kind: ShapeKind,
        input: PenInput<T>,
        using: &mut Using<T>,
    ) -> Outcome<T> {
        // A new shape's own space is aligned with its surface.
        let local = |at: Point<Surface>| Placement::ALIGNED.back().apply(at);
        match (was, input) {
            (_, PenInput::Down { target, sample }) => {
                *using = Using::Shaping(Shaping::Pressed {
                    target,
                    kind,
                    from: local(sample.at),
                });
                Outcome::Drawn
            }
            (
                Using::Shaping(
                    Shaping::Pressed { target, kind, from }
                    | Shaping::Dragging {
                        target, kind, from, ..
                    },
                ),
                PenInput::Move(sample),
            ) => {
                *using = Using::Shaping(Shaping::Dragging {
                    target,
                    kind,
                    from,
                    ink: ShapeInk::new(
                        Shape::dragged(kind, from, local(sample.at)),
                        self.shape_width,
                    ),
                });
                Outcome::Drawn
            }
            (
                Using::Shaping(Shaping::Dragging {
                    target, kind, from, ..
                }),
                PenInput::Up(sample),
            ) => {
                let shape = Shape::dragged(kind, from, local(sample.at));
                Outcome::Commit(MarkChange::Add(
                    target,
                    Mark {
                        placement: Placement::ALIGNED,
                        paint: self.paint,
                        ink: Ink::Shape(Arc::new(ShapeInk::new(shape, self.shape_width))),
                    },
                ))
            }
            (Using::Shaping(Shaping::Pressed { .. }), PenInput::Up(_) | PenInput::Cancel)
            | (Using::Shaping(Shaping::Dragging { .. }), PenInput::Cancel) => Outcome::Drawn,
            (was, PenInput::Move(_) | PenInput::Up(_) | PenInput::Cancel) => {
                *using = was;
                Outcome::Ignored
            }
        }
    }
}

impl<T> Using<T> {
    /// Removes the picked marks, leaving nothing picked.
    pub fn delete(&mut self) -> Option<MarkChange<T>> {
        match std::mem::replace(self, Using::Idle) {
            Using::Selecting(Selecting::Holding { target, picked, .. }) => Some(
                MarkChange::Remove(target, SmallVec::from_slice(picked.ids())),
            ),
            was => {
                *self = was;
                None
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
    /// Down but not yet moved past the reach: lifting here is a tap.
    Pressed { target: T, at: Point<Surface> },
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

#[derive(Clone, Debug)]
pub enum Picked {
    Text(TextMark),
    /// `ids` is never empty.
    Marks {
        ids: SmallVec<[MarkId; 8]>,
        frame: Frame,
    },
}

impl Picked {
    /// One mark is handled by its own rectangle, turned with it; several by an upright box.
    pub fn gather<'a>(found: impl Iterator<Item = (MarkId, &'a Arc<Mark>)>) -> Option<Picked> {
        let found: SmallVec<[(MarkId, &Arc<Mark>); 8]> = found.collect();
        match found.as_slice() {
            [] => None,
            [(id, mark)] => Some(TextMark::of(*id, mark).map_or_else(
                || Picked::Marks {
                    ids: smallvec::smallvec![*id],
                    frame: Frame::of(mark),
                },
                Picked::Text,
            )),
            [(_, first), rest @ ..] => {
                let bounds = (rest.iter()).fold(first.bounds(), |bounds, (_, mark)| {
                    bounds.union(&mark.bounds())
                });
                Some(Picked::Marks {
                    ids: found.iter().map(|(id, _)| *id).collect(),
                    frame: Frame::upright(bounds),
                })
            }
        }
    }

    pub fn frame(&self) -> Frame {
        match self {
            Picked::Text(mark) => mark.frame(),
            Picked::Marks { frame, .. } => *frame,
        }
    }

    pub fn ids(&self) -> &[MarkId] {
        match self {
            Picked::Text(mark) => std::slice::from_ref(&mark.id),
            Picked::Marks { ids, .. } => ids,
        }
    }

    pub fn sides(&self) -> Sides {
        match self {
            Picked::Text(_) => Sides::Wrap,
            Picked::Marks { .. } => Sides::Stretch,
        }
    }

    /// A `by` that flattens the marks leaves them as they were.
    pub fn moved(self, by: &Affine<Surface, Surface>) -> Picked {
        match self {
            Picked::Text(mark) => match Placement::new(mark.placement.forward().then(by)) {
                Some(placement) => Picked::Text(TextMark { placement, ..mark }),
                None => Picked::Text(mark),
            },
            Picked::Marks { ids, frame } => Picked::Marks {
                ids,
                frame: frame.moved(by).unwrap_or(frame),
            },
        }
    }
}

#[derive(Clone, Debug)]
pub struct TextMark {
    pub id: MarkId,
    pub text: Arc<TextInk>,
    pub placement: Placement,
    pub paint: Paint,
}

impl TextMark {
    pub fn of(id: MarkId, mark: &Mark) -> Option<TextMark> {
        match &mark.ink {
            Ink::Text(text) => Some(TextMark {
                id,
                text: text.clone(),
                placement: mark.placement,
                paint: mark.paint,
            }),
            Ink::Stroke(_) | Ink::Shape(_) | Ink::Image(_) => None,
        }
    }

    pub fn mark(&self) -> Mark {
        Mark {
            placement: self.placement,
            paint: self.paint,
            ink: Ink::Text(self.text.clone()),
        }
    }

    pub fn frame(&self) -> Frame {
        Frame {
            rect: self.text.area,
            placement: self.placement,
        }
    }
}

/// In the mark's own space.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rewrapped {
    pub left: f32,
    pub width: Size,
}

/// A rectangle in its own space, placed on the surface: the box a selection is handled by.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Frame {
    pub rect: Rect<Local>,
    pub placement: Placement,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sides {
    Stretch,
    Wrap,
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

    /// A side has a handle only once the frame is three reaches deep across it.
    pub fn handles(
        &self,
        sides: Sides,
        reach: Size,
    ) -> impl Iterator<Item = (Handle, Point<Surface>)> + '_ {
        let forward = self.placement.forward();
        (Handle::ALL.into_iter())
            .filter(move |handle| match (handle, sides) {
                (Handle::Edge(Edge::Top | Edge::Bottom), Sides::Wrap) => false,
                (Handle::Edge(edge), Sides::Stretch | Sides::Wrap) => {
                    let (near, far) = (edge.of(&self.rect), edge.opposite().of(&self.rect));
                    forward.apply(near).distance(forward.apply(far)) >= 3.0 * reach.get()
                }
                (Handle::Corner(_) | Handle::Rotate, Sides::Stretch | Sides::Wrap) => true,
            })
            .map(move |handle| (handle, handle.at(self, reach)))
    }

    /// `None` when `by` flattens it.
    pub fn moved(&self, by: &Affine<Surface, Surface>) -> Option<Frame> {
        Placement::new(self.placement.forward().then(by))
            .map(|placement| Frame { placement, ..*self })
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
        from: Point<Surface>,
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
            Grip::Resizing { resize, from, .. } => Grip::Resizing { resize, from, to },
        }
    }

    pub fn travel(&self) -> f32 {
        match *self {
            Grip::Still => 0.0,
            Grip::Moving { from, to }
            | Grip::Rotating { from, to, .. }
            | Grip::Resizing { from, to, .. } => from.distance(to),
        }
    }

    pub fn rewrapped(&self, mark: &TextMark) -> Option<Rewrapped> {
        let (min, max) = (mark.text.area.min(), mark.text.area.max());
        let (left, right) = match *self {
            Grip::Resizing {
                resize: Resize::Edge(Edge::Left),
                to,
                ..
            } => (
                mark.placement.back().apply(to).x.min(max.x - MIN_WRAP),
                max.x,
            ),
            Grip::Resizing {
                resize: Resize::Edge(Edge::Right),
                to,
                ..
            } => (
                min.x,
                mark.placement.back().apply(to).x.max(min.x + MIN_WRAP),
            ),
            Grip::Still | Grip::Moving { .. } | Grip::Rotating { .. } | Grip::Resizing { .. } => {
                return None;
            }
        };
        Some(Rewrapped {
            left,
            width: Size::new(right - left).ok()?,
        })
    }

    /// The frame as the grip shows it, before `transform` moves it.
    pub fn held(&self, picked: &Picked) -> Frame {
        match (picked, self) {
            (Picked::Text(mark), grip) => match grip.rewrapped(mark) {
                Some(to) => {
                    let (min, max) = (mark.text.area.min(), mark.text.area.max());
                    Frame {
                        rect: Rect::spanning(
                            Point::new(to.left, min.y),
                            Point::new(to.left + to.width.get(), max.y),
                        ),
                        placement: mark.placement,
                    }
                }
                None => mark.frame(),
            },
            (Picked::Marks { frame, .. }, _) => *frame,
        }
    }

    /// How the grip moves the picked marks. A resize scales in the frame's own space, so a turned
    /// frame keeps its shape; `frame.rect` is never flat, since marks have width.
    pub fn transform(&self, picked: &Picked) -> Affine<Surface, Surface> {
        let frame = picked.frame();
        match *self {
            Grip::Still => Affine::identity(),
            Grip::Moving { from, to } => Affine::translation(to.x - from.x, to.y - from.y),
            Grip::Rotating { center, from, to } => {
                let (a, b) = (from - center, to - center);
                Affine::rotation_about(center, a.cross(b).atan2(a.dot(b)))
            }
            Grip::Resizing { resize, to, .. } => {
                let (rect, back) = (&frame.rect, frame.placement.back());
                let to = back.apply(to);
                let local = match (resize, picked.sides()) {
                    // Text held by a side wraps again rather than stretching.
                    (Resize::Edge(_), Sides::Wrap) => return Affine::identity(),
                    // Scaled by how far the pen has gone along the corner's diagonal.
                    (Resize::Corner(corner), Sides::Stretch | Sides::Wrap) => {
                        let anchor = corner.opposite().of(rect);
                        let diagonal = corner.of(rect) - anchor;
                        let scale = (to - anchor).dot(diagonal) / diagonal.dot(diagonal);
                        Affine::scaling_about(anchor, scale, scale)
                    }
                    (Resize::Edge(edge), Sides::Stretch) => {
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

/// `reach` is how far a tap or handle reaches on the surface.
fn select<T: Copy + PartialEq, D: Drawing<Target = T>>(
    was: Using<T>,
    gather: Gather,
    input: PenInput<T>,
    using: &mut Using<T>,
    drawing: &D,
    reach: Size,
) -> Outcome<T> {
    match (was, input) {
        (
            Using::Selecting(Selecting::Holding {
                target: held,
                picked,
                grip: Grip::Still,
            }),
            PenInput::Down { target, sample },
        ) if held == target => {
            let at = sample.at;
            let frame = picked.frame();
            let center = frame.centre();
            let grip = (frame.handles(picked.sides(), reach))
                .map(|(handle, sits)| (handle, sits.distance(at)))
                .min_by(|(_, a), (_, b)| a.total_cmp(b))
                .filter(|(_, distance)| *distance <= reach.get())
                .map(|(handle, _)| match handle {
                    Handle::Corner(corner) => Grip::Resizing {
                        resize: Resize::Corner(corner),
                        from: at,
                        to: at,
                    },
                    Handle::Edge(edge) => Grip::Resizing {
                        resize: Resize::Edge(edge),
                        from: at,
                        to: at,
                    },
                    Handle::Rotate => Grip::Rotating {
                        center,
                        from: at,
                        to: at,
                    },
                })
                .or_else(|| (frame.holds(at)).then_some(Grip::Moving { from: at, to: at }));
            *using = Using::Selecting(match grip {
                Some(grip) => Selecting::Holding {
                    target,
                    picked,
                    grip,
                },
                None => Selecting::Pressed { target, at },
            });
            Outcome::Drawn
        }
        (_, PenInput::Down { target, sample }) => {
            *using = Using::Selecting(Selecting::Pressed {
                target,
                at: sample.at,
            });
            Outcome::Drawn
        }
        (Using::Selecting(Selecting::Pressed { target, at }), PenInput::Move(sample))
            if at.distance(sample.at) > reach.get() =>
        {
            *using = Using::Selecting(match gather {
                Gather::Box => Selecting::Boxing {
                    target,
                    from: at,
                    to: sample.at,
                },
                Gather::Lasso => Selecting::Lassoing {
                    target,
                    lasso: vec![at, sample.at],
                },
            });
            Outcome::Drawn
        }
        (Using::Selecting(Selecting::Pressed { target, at }), PenInput::Up(_)) => {
            let picked = (drawing.marks(target)).and_then(|marks| {
                Picked::gather(
                    marks
                        .iter()
                        .filter(|(_, mark)| mark.touches(at, reach.get()))
                        .last()
                        .into_iter(),
                )
            });
            *using = held(target, picked);
            Outcome::Drawn
        }
        (Using::Selecting(Selecting::Boxing { target, from, .. }), PenInput::Move(sample)) => {
            *using = Using::Selecting(Selecting::Boxing {
                target,
                from,
                to: sample.at,
            });
            Outcome::Drawn
        }
        (Using::Selecting(Selecting::Boxing { target, from, .. }), PenInput::Up(sample)) => {
            let around = Rect::spanning(from, sample.at);
            let picked = (drawing.marks(target)).and_then(|marks| {
                Picked::gather(marks.iter().filter(|(_, mark)| {
                    let bounds = mark.bounds();
                    around.contains(bounds.min()) && around.contains(bounds.max())
                }))
            });
            *using = held(target, picked);
            Outcome::Drawn
        }
        (Using::Selecting(Selecting::Lassoing { target, mut lasso }), PenInput::Move(sample)) => {
            lasso.push(sample.at);
            *using = Using::Selecting(Selecting::Lassoing { target, lasso });
            Outcome::Drawn
        }
        (Using::Selecting(Selecting::Lassoing { target, mut lasso }), PenInput::Up(sample)) => {
            lasso.push(sample.at);
            let around = (lasso.iter()).fold(Rect::spanning(lasso[0], lasso[0]), |rect, at| {
                rect.union(&Rect::spanning(*at, *at))
            });
            let picked = (drawing.marks(target)).and_then(|marks| {
                Picked::gather(marks.iter().filter(|(_, mark)| {
                    mark.bounds().intersects(&around) && enclosed(mark, &lasso)
                }))
            });
            *using = held(target, picked);
            Outcome::Drawn
        }
        (
            Using::Selecting(Selecting::Holding {
                target,
                picked,
                grip,
            }),
            PenInput::Move(sample),
        ) => {
            *using = Using::Selecting(Selecting::Holding {
                target,
                picked,
                grip: grip.to(sample.at),
            });
            Outcome::Drawn
        }
        (
            Using::Selecting(Selecting::Holding {
                target,
                picked,
                grip,
            }),
            PenInput::Up(sample),
        ) => match (picked, grip.to(sample.at)) {
            // A tap types in picked text, and leaves other marks as they were.
            (Picked::Text(mark), grip) if grip.travel() <= reach.get() => {
                Outcome::Type(TypeAt::Mark { target, mark })
            }
            (picked, grip) if grip.travel() <= reach.get() => {
                *using = still(target, picked);
                Outcome::Ignored
            }
            (Picked::Text(mark), grip) => match grip.rewrapped(&mark) {
                Some(to) => {
                    *using = still(target, Picked::Text(mark.clone()));
                    Outcome::Rewrap { target, mark, to }
                }
                None => transformed(using, target, Picked::Text(mark), grip),
            },
            (picked, grip) => transformed(using, target, picked, grip),
        },
        (Using::Selecting(Selecting::Holding { target, picked, .. }), PenInput::Cancel) => {
            *using = still(target, picked);
            Outcome::Drawn
        }
        (
            Using::Selecting(
                Selecting::Pressed { .. } | Selecting::Boxing { .. } | Selecting::Lassoing { .. },
            ),
            PenInput::Cancel,
        ) => Outcome::Drawn,
        (was, PenInput::Move(_) | PenInput::Up(_) | PenInput::Cancel) => {
            *using = was;
            Outcome::Ignored
        }
    }
}

fn transformed<T: Copy>(using: &mut Using<T>, target: T, picked: Picked, grip: Grip) -> Outcome<T> {
    let by = grip.transform(&picked);
    let ids = SmallVec::from_slice(picked.ids());
    *using = still(target, picked.moved(&by));
    Outcome::Commit(MarkChange::Transform(target, ids, by))
}

pub fn still<T>(target: T, picked: Picked) -> Using<T> {
    Using::Selecting(Selecting::Holding {
        target,
        picked,
        grip: Grip::Still,
    })
}

fn held<T>(target: T, picked: Option<Picked>) -> Using<T> {
    match picked {
        Some(picked) => still(target, picked),
        None => Using::Idle,
    }
}

/// A press on a text mark opens it; elsewhere a drag sets the width of new text.
fn place<T: Copy, D: Drawing<Target = T>>(
    was: Using<T>,
    input: PenInput<T>,
    using: &mut Using<T>,
    drawing: &D,
    reach: Size,
) -> Outcome<T> {
    match (was, input) {
        (_, PenInput::Down { target, sample }) => {
            let under = (drawing.marks(target).into_iter())
                .flat_map(|marks| marks.iter())
                .filter(|(_, mark)| mark.touches(sample.at, reach.get()))
                .filter_map(|(id, mark)| TextMark::of(id, mark))
                .map(|mark| TypeAt::Mark { target, mark })
                .last();
            match under {
                Some(at) => Outcome::Type(at),
                None => {
                    *using = Using::Placing {
                        target,
                        from: sample.at,
                        to: sample.at,
                    };
                    Outcome::Drawn
                }
            }
        }
        (Using::Placing { target, from, .. }, PenInput::Move(sample)) => {
            *using = Using::Placing {
                target,
                from,
                to: sample.at,
            };
            Outcome::Drawn
        }
        (Using::Placing { target, from, .. }, PenInput::Up(sample)) => {
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
        (Using::Placing { .. }, PenInput::Cancel) => Outcome::Drawn,
        (was, PenInput::Move(_) | PenInput::Up(_) | PenInput::Cancel) => {
            *using = was;
            Outcome::Ignored
        }
    }
}

#[derive(Debug)]
pub enum Shaping<T> {
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

/// Where the text tool asks for text.
#[derive(Clone, Debug)]
pub enum TypeAt<T> {
    /// New text with its top left at `at`.
    New {
        target: T,
        at: Point<Surface>,
        wrap: Wrap,
    },
    Mark {
        target: T,
        mark: TextMark,
    },
}
