//! Points, vectors, boxes and transforms, typed by the space they live in.

use std::fmt;
use std::marker::PhantomData;
use std::ops::{Add, AddAssign, Div, Mul, Neg, Sub};

/// A stroke's own space; its transform places it on the surface.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Local {}

/// A page, or the board, in points.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Surface {}

/// Pages stacked for display, in points; the same as the surface on a board.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sheet {}

/// A pane's logical pixels, from its top left.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pane {}

type Space<S> = PhantomData<fn() -> S>;

pub struct Point<S> {
    pub x: f32,
    pub y: f32,
    space: Space<S>,
}

pub struct Vector<S> {
    pub x: f32,
    pub y: f32,
    space: Space<S>,
}

/// An axis-aligned box with `min` at or before `max` on both axes.
pub struct Rect<S> {
    min: Point<S>, // private: min stays at or below max
    max: Point<S>, // private: min stays at or below max
}

/// Maps `From` to `To`: `x' = a x + c y + e`, `y' = b x + d y + f`.
pub struct Affine<From, To> {
    pub coefficients: [f32; 6],
    spaces: PhantomData<fn(From) -> To>,
}

impl<S> Point<S> {
    pub const fn new(x: f32, y: f32) -> Point<S> {
        Point {
            x,
            y,
            space: PhantomData,
        }
    }

    pub fn distance(self, other: Point<S>) -> f32 {
        (self - other).length()
    }

    pub fn lerp(self, other: Point<S>, t: f32) -> Point<S> {
        self + (other - self) * t
    }
}

impl<S> Vector<S> {
    pub const ZERO: Vector<S> = Vector::new(0.0, 0.0);

    pub const fn new(x: f32, y: f32) -> Vector<S> {
        Vector {
            x,
            y,
            space: PhantomData,
        }
    }

    pub fn length(self) -> f32 {
        self.x.hypot(self.y)
    }

    pub fn dot(self, other: Vector<S>) -> f32 {
        self.x * other.x + self.y * other.y
    }

    pub fn cross(self, other: Vector<S>) -> f32 {
        self.x * other.y - self.y * other.x
    }

    /// The vector turned a quarter turn, from +x towards +y.
    pub fn perpendicular(self) -> Vector<S> {
        Vector::new(-self.y, self.x)
    }

    pub fn rotated(self, angle: f32) -> Vector<S> {
        let (sin, cos) = angle.sin_cos();
        Vector::new(self.x * cos - self.y * sin, self.x * sin + self.y * cos)
    }
}

impl<S> Rect<S> {
    pub fn spanning(a: Point<S>, b: Point<S>) -> Rect<S> {
        Rect {
            min: Point::new(a.x.min(b.x), a.y.min(b.y)),
            max: Point::new(a.x.max(b.x), a.y.max(b.y)),
        }
    }

    pub fn around(center: Point<S>, radius: f32) -> Rect<S> {
        let half = Vector::new(radius.abs(), radius.abs());
        Rect {
            min: center - half,
            max: center + half,
        }
    }

    pub fn min(&self) -> Point<S> {
        self.min
    }

    pub fn max(&self) -> Point<S> {
        self.max
    }

    pub fn union(&self, other: &Rect<S>) -> Rect<S> {
        Rect {
            min: Point::new(self.min.x.min(other.min.x), self.min.y.min(other.min.y)),
            max: Point::new(self.max.x.max(other.max.x), self.max.y.max(other.max.y)),
        }
    }

    pub fn contains(&self, point: Point<S>) -> bool {
        (self.min.x..=self.max.x).contains(&point.x) && (self.min.y..=self.max.y).contains(&point.y)
    }

    pub fn intersects(&self, other: &Rect<S>) -> bool {
        self.min.x <= other.max.x
            && other.min.x <= self.max.x
            && self.min.y <= other.max.y
            && other.min.y <= self.max.y
    }

    /// The box mapped by `affine`, as the box around its four mapped corners.
    pub fn mapped<T>(&self, affine: &Affine<S, T>) -> Rect<T> {
        let corners = [
            Point::new(self.min.x, self.max.y),
            Point::new(self.max.x, self.min.y),
            self.max,
        ];
        corners.into_iter().fold(
            Rect::spanning(affine.apply(self.min), affine.apply(self.min)),
            |rect, corner| {
                let at = affine.apply(corner);
                rect.union(&Rect::spanning(at, at))
            },
        )
    }
}

impl<From, To> Affine<From, To> {
    /// The identity, which also names a new space as aligned with an existing one.
    pub const fn identity() -> Affine<From, To> {
        Affine::new([1.0, 0.0, 0.0, 1.0, 0.0, 0.0])
    }

    pub const fn new(coefficients: [f32; 6]) -> Affine<From, To> {
        Affine {
            coefficients,
            spaces: PhantomData,
        }
    }

    pub const fn translation(x: f32, y: f32) -> Affine<From, To> {
        Affine::new([1.0, 0.0, 0.0, 1.0, x, y])
    }

    pub const fn scale(factor: f32) -> Affine<From, To> {
        Affine::new([factor, 0.0, 0.0, factor, 0.0, 0.0])
    }

    pub fn apply(&self, point: Point<From>) -> Point<To> {
        let [a, b, c, d, e, f] = self.coefficients;
        Point::new(a * point.x + c * point.y + e, b * point.x + d * point.y + f)
    }

    pub fn apply_vector(&self, vector: Vector<From>) -> Vector<To> {
        let [a, b, c, d, _, _] = self.coefficients;
        Vector::new(a * vector.x + c * vector.y, b * vector.x + d * vector.y)
    }

    /// This transform followed by `next`.
    pub fn then<Next>(&self, next: &Affine<To, Next>) -> Affine<From, Next> {
        let [a1, b1, c1, d1, e1, f1] = self.coefficients;
        let [a2, b2, c2, d2, e2, f2] = next.coefficients;
        Affine::new([
            a2 * a1 + c2 * b1,
            b2 * a1 + d2 * b1,
            a2 * c1 + c2 * d1,
            b2 * c1 + d2 * d1,
            a2 * e1 + c2 * f1 + e2,
            b2 * e1 + d2 * f1 + f2,
        ])
    }

    /// `None` when the transform collapses the plane onto a line or a point.
    pub fn inverse(&self) -> Option<Affine<To, From>> {
        let [a, b, c, d, e, f] = self.coefficients;
        let determinant = a * d - b * c;
        determinant.is_normal().then(|| {
            Affine::new([
                d / determinant,
                -b / determinant,
                -c / determinant,
                a / determinant,
                (c * f - d * e) / determinant,
                (b * e - a * f) / determinant,
            ])
        })
    }

    /// The largest factor by which the transform stretches a length.
    pub fn stretch(&self) -> f32 {
        let [a, b, c, d, _, _] = self.coefficients;
        a.hypot(b).max(c.hypot(d))
    }
}

/// The distance from `point` to the triangle `corners`, zero inside it.
pub fn triangle_distance<S>(point: Point<S>, corners: [Point<S>; 3]) -> f32 {
    let [a, b, c] = corners;
    let side = |from: Point<S>, to: Point<S>| (to - from).cross(point - from);
    let sides = [side(a, b), side(b, c), side(c, a)];
    let inside = sides.iter().all(|s| *s >= 0.0) || sides.iter().all(|s| *s <= 0.0);
    inside.then_some(0.0).unwrap_or_else(|| {
        [(a, b), (b, c), (c, a)]
            .into_iter()
            .map(|(from, to)| segment_distance(point, from, to))
            .fold(f32::INFINITY, f32::min)
    })
}

fn segment_distance<S>(point: Point<S>, from: Point<S>, to: Point<S>) -> f32 {
    let along = to - from;
    let length = along.dot(along);
    // A segment of no length is its one point.
    let t = (length > 0.0)
        .then(|| ((point - from).dot(along) / length).clamp(0.0, 1.0))
        .unwrap_or(0.0);
    point.distance(from + along * t)
}

impl<S> Sub for Point<S> {
    type Output = Vector<S>;

    fn sub(self, other: Point<S>) -> Vector<S> {
        Vector::new(self.x - other.x, self.y - other.y)
    }
}

impl<S> Add<Vector<S>> for Point<S> {
    type Output = Point<S>;

    fn add(self, vector: Vector<S>) -> Point<S> {
        Point::new(self.x + vector.x, self.y + vector.y)
    }
}

impl<S> Sub<Vector<S>> for Point<S> {
    type Output = Point<S>;

    fn sub(self, vector: Vector<S>) -> Point<S> {
        Point::new(self.x - vector.x, self.y - vector.y)
    }
}

impl<S> Add for Vector<S> {
    type Output = Vector<S>;

    fn add(self, other: Vector<S>) -> Vector<S> {
        Vector::new(self.x + other.x, self.y + other.y)
    }
}

impl<S> AddAssign for Vector<S> {
    fn add_assign(&mut self, other: Vector<S>) {
        *self = *self + other;
    }
}

impl<S> Sub for Vector<S> {
    type Output = Vector<S>;

    fn sub(self, other: Vector<S>) -> Vector<S> {
        Vector::new(self.x - other.x, self.y - other.y)
    }
}

impl<S> Mul<f32> for Vector<S> {
    type Output = Vector<S>;

    fn mul(self, scale: f32) -> Vector<S> {
        Vector::new(self.x * scale, self.y * scale)
    }
}

impl<S> Div<f32> for Vector<S> {
    type Output = Vector<S>;

    fn div(self, scale: f32) -> Vector<S> {
        Vector::new(self.x / scale, self.y / scale)
    }
}

impl<S> Neg for Vector<S> {
    type Output = Vector<S>;

    fn neg(self) -> Vector<S> {
        Vector::new(-self.x, -self.y)
    }
}

impl<S> Clone for Point<S> {
    fn clone(&self) -> Point<S> {
        *self
    }
}

impl<S> Copy for Point<S> {}

impl<S> PartialEq for Point<S> {
    fn eq(&self, other: &Point<S>) -> bool {
        (self.x, self.y) == (other.x, other.y)
    }
}

impl<S> fmt::Debug for Point<S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "({}, {})", self.x, self.y)
    }
}

impl<S> Clone for Vector<S> {
    fn clone(&self) -> Vector<S> {
        *self
    }
}

impl<S> Copy for Vector<S> {}

impl<S> PartialEq for Vector<S> {
    fn eq(&self, other: &Vector<S>) -> bool {
        (self.x, self.y) == (other.x, other.y)
    }
}

impl<S> fmt::Debug for Vector<S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "<{}, {}>", self.x, self.y)
    }
}

impl<S> Clone for Rect<S> {
    fn clone(&self) -> Rect<S> {
        *self
    }
}

impl<S> Copy for Rect<S> {}

impl<S> PartialEq for Rect<S> {
    fn eq(&self, other: &Rect<S>) -> bool {
        (self.min, self.max) == (other.min, other.max)
    }
}

impl<S> fmt::Debug for Rect<S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}..{:?}", self.min, self.max)
    }
}

impl<F, T> Clone for Affine<F, T> {
    fn clone(&self) -> Affine<F, T> {
        *self
    }
}

impl<F, T> Copy for Affine<F, T> {}

impl<F, T> PartialEq for Affine<F, T> {
    fn eq(&self, other: &Affine<F, T>) -> bool {
        self.coefficients == other.coefficients
    }
}

impl<F, T> fmt::Debug for Affine<F, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Affine{:?}", self.coefficients)
    }
}

impl<S> Affine<S, S> {
    /// A turn by `angle` radians about `center`, from +x towards +y.
    pub fn rotation_about(center: Point<S>, angle: f32) -> Affine<S, S> {
        let (sin, cos) = angle.sin_cos();
        Affine::new([
            cos,
            sin,
            -sin,
            cos,
            center.x - cos * center.x + sin * center.y,
            center.y - sin * center.x - cos * center.y,
        ])
    }

    /// Scaling by `x` and `y` that leaves `anchor` where it is.
    pub fn scaling_about(anchor: Point<S>, x: f32, y: f32) -> Affine<S, S> {
        Affine::new([
            x,
            0.0,
            0.0,
            y,
            anchor.x - x * anchor.x,
            anchor.y - y * anchor.y,
        ])
    }
}

/// Whether `point` lies inside the polygon through `corners`, closed from the last back to the
/// first, by the even-odd rule.
pub fn inside_polygon<S>(point: Point<S>, corners: &[Point<S>]) -> bool {
    let edges = corners.iter().zip(corners.iter().cycle().skip(1));
    edges.fold(false, |inside, (a, b)| {
        let crosses = (a.y > point.y) != (b.y > point.y)
            && point.x < a.x + (point.y - a.y) * (b.x - a.x) / (b.y - a.y);
        inside != crosses
    })
}
