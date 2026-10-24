//! Lines, arrows, rectangles and ellipses, drawn with a constant width.

use smallvec::{SmallVec, smallvec};

use crate::brush::Size;
use crate::doc::GeometryId;
use crate::geom::{Local, Point, Rect, Vector};
use crate::mesh::{self, Mesh};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShapeKind {
    Line,
    Arrow,
    Rectangle,
    Ellipse,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Shape {
    Line {
        from: Point<Local>,
        to: Point<Local>,
    },
    Arrow {
        from: Point<Local>,
        to: Point<Local>,
    },
    Rectangle(Rect<Local>),
    Ellipse {
        center: Point<Local>,
        radii: Vector<Local>,
    },
}

/// A mesh with the box around it and an id the host keeps it under.
#[derive(Debug)]
pub struct Built {
    pub id: GeometryId,
    pub mesh: Mesh,
    pub bounds: Rect<Local>,
}

/// A shape and its width, with the mesh `ShapeInk::new` builds from them.
#[derive(Debug)]
pub struct ShapeInk {
    pub shape: Shape,
    pub width: Size,
    pub built: Built,
}

/// An arrow head's sides, in widths of its line.
pub const HEAD: f32 = 6.0;
/// The most of a short arrow's shaft its head takes.
pub const HEAD_SHARE: f32 = 0.5;
/// The widest an arrow head opens, in radians from the shaft.
pub const HEAD_ANGLE: f32 = 0.45;

impl Shape {
    pub fn dragged(kind: ShapeKind, from: Point<Local>, to: Point<Local>) -> Shape {
        match kind {
            ShapeKind::Line => Shape::Line { from, to },
            ShapeKind::Arrow => Shape::Arrow { from, to },
            ShapeKind::Rectangle => Shape::Rectangle(Rect::spanning(from, to)),
            ShapeKind::Ellipse => Shape::Ellipse {
                center: from.lerp(to, 0.5),
                radii: (to - from) * 0.5,
            },
        }
    }

    /// The lines a shape is swept along; an arrow's head is as long whatever the shaft's length.
    pub fn outline(&self, width: Size) -> SmallVec<[Vec<Point<Local>>; 3]> {
        match *self {
            Shape::Line { from, to } => smallvec![vec![from, to]],
            Shape::Arrow { from, to } => {
                let shaft = from - to;
                let length = shaft.length();
                let head = (width.get() * HEAD).min(length * HEAD_SHARE);
                // A shaft of no length has no direction, and so no head.
                let back = shaft * (head / length.max(f32::MIN_POSITIVE));
                smallvec![
                    vec![from, to],
                    vec![
                        to + back.rotated(HEAD_ANGLE),
                        to,
                        to + back.rotated(-HEAD_ANGLE)
                    ],
                ]
            }
            Shape::Rectangle(rect) => {
                let (min, max) = (rect.min(), rect.max());
                let corners = [
                    min,
                    Point::new(max.x, min.y),
                    max,
                    Point::new(min.x, max.y),
                    min,
                ];
                smallvec![corners.to_vec()]
            }
            Shape::Ellipse { center, radii } => smallvec![mesh::ellipse(center, radii)],
        }
    }
}

impl ShapeInk {
    pub fn new(shape: Shape, width: Size) -> ShapeInk {
        let radius = width.get() * 0.5;
        let mut mesh = Mesh::default();
        for line in shape.outline(width) {
            mesh::sweep(&mut mesh, &line, radius);
        }
        let on = match shape {
            Shape::Line { from, .. } | Shape::Arrow { from, .. } => from,
            Shape::Rectangle(rect) => rect.min(),
            Shape::Ellipse { center, radii } => Point::new(center.x + radii.x, center.y),
        };
        let bounds = (mesh.vertices.iter()).fold(Rect::around(on, radius), |rect, at| {
            rect.union(&Rect::spanning(*at, *at))
        });
        ShapeInk {
            shape,
            width,
            built: Built {
                id: GeometryId::next(),
                mesh,
                bounds,
            },
        }
    }
}
