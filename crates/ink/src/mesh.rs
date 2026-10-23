//! Triangle meshes extruded from a round tip swept along a stroke's modeled points.

use std::f32::consts::{FRAC_PI_2, TAU};

use crate::brush::{Brush, EPSILON};
use crate::geom::{Local, Point, Rect, Vector};
use crate::input::Modeled;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Mesh {
    pub vertices: Vec<Point<Local>>,
    pub triangles: Vec<[u32; 3]>,
}

impl Mesh {
    /// `None` for a mesh without vertices.
    pub fn bounds(&self) -> Option<Rect<Local>> {
        let (first, rest) = self.vertices.split_first()?;
        Some(
            rest.iter()
                .fold(Rect::spanning(*first, *first), |rect, at| {
                    rect.union(&Rect::spanning(*at, *at))
                }),
        )
    }
}

#[derive(Clone, Copy, Debug)]
struct Disc {
    center: Point<Local>,
    radius: f32,
}

/// Step `i` joins the turn at disc `i - 1` and bridges it to disc `i`, reading no later disc, so
/// the steps over final points are kept between updates.
#[derive(Clone, Debug, Default)]
pub struct MeshBuilder {
    mesh: Mesh,       // private: the kept steps match the mesh built so far
    discs: Vec<Disc>, // private: the kept steps match the mesh built so far
    ends: Ends,       // private: the kept steps match the mesh built so far
    kept: Kept,       // private: the kept steps match the mesh built so far
}

#[derive(Clone, Copy, Debug, Default)]
struct Kept {
    steps: usize,
    vertices: usize,
    triangles: usize,
    ends: Ends,
}

/// How the last step left the stroke, which decides how the next bridge starts.
#[derive(Clone, Copy, Debug, Default)]
enum Ends {
    /// Only the first disc, already capped.
    #[default]
    Start,
    /// The last bridge was skipped, one disc lying inside the other.
    Open,
    /// The last bridge's far corners, and its sides.
    Corners { left: u32, right: u32, sides: Sides },
}

impl MeshBuilder {
    pub fn mesh(&self) -> &Mesh {
        &self.mesh
    }

    pub fn into_mesh(self) -> Mesh {
        self.mesh
    }

    pub fn clear(&mut self) {
        self.mesh.vertices.clear();
        self.mesh.triangles.clear();
        self.discs.clear();
        self.ends = Ends::Start;
        self.kept = Kept::default();
    }

    /// Rebuilds the mesh after the kept steps from `modeled`, whose final points are the ones
    /// given before.
    pub fn update(&mut self, brush: &Brush, modeled: &Modeled) {
        let points = modeled.all();
        let stable = modeled.stable().len();
        self.mesh.vertices.truncate(self.kept.vertices);
        self.mesh.triangles.truncate(self.kept.triangles);
        self.discs.truncate(self.kept.steps);
        self.ends = self.kept.ends;
        self.discs
            .extend(points[self.kept.steps..].iter().map(|point| Disc {
                center: point.at,
                radius: brush.radius(point.pressure),
            }));
        for step in self.kept.steps..self.discs.len() {
            self.step(step);
            match step < stable {
                true => {
                    self.kept = Kept {
                        steps: step + 1,
                        vertices: self.mesh.vertices.len(),
                        triangles: self.mesh.triangles.len(),
                        ends: self.ends,
                    }
                }
                false => {}
            }
        }
        match self.discs.len() {
            0 | 1 => {}
            n => self.cap(self.discs[n - 1]),
        }
    }

    fn step(&mut self, step: usize) {
        match step {
            0 => self.cap(self.discs[0]),
            _ => {
                let (from, to) = (self.discs[step - 1], self.discs[step]);
                let Some(sides) = tangents(from, to) else {
                    match from.radius >= to.radius {
                        true => self.cap(from),
                        false => {}
                    }
                    self.ends = Ends::Open;
                    return;
                };
                let starts = match self.ends {
                    Ends::Start => self.corners(from, sides),
                    Ends::Open => {
                        self.cap(from);
                        self.corners(from, sides)
                    }
                    Ends::Corners {
                        left,
                        right,
                        sides: before,
                    } => self.join(from, before, sides, [left, right]),
                };
                let [left, right] = self.corners(to, sides);
                self.mesh
                    .triangles
                    .extend([[starts[0], left, right], [starts[0], right, starts[1]]]);
                self.ends = Ends::Corners { left, right, sides };
            }
        }
    }

    fn corners(&mut self, disc: Disc, sides: Sides) -> [u32; 2] {
        [
            self.vertex(disc.center + sides.left * disc.radius),
            self.vertex(disc.center + sides.right * disc.radius),
        ]
    }

    /// Connects the bridge ending at `disc` with `ends` to the one leaving it along `after`,
    /// filling the outside of the turn, and returns where the next bridge starts.
    fn join(&mut self, disc: Disc, before: Sides, after: Sides, ends: [u32; 2]) -> [u32; 2] {
        let turn = before
            .left
            .cross(after.left)
            .atan2(before.left.dot(after.left));
        match turn.abs() * disc.radius > EPSILON {
            false => ends,
            true => {
                let starts = self.corners(disc, after);
                let steps = (turn.abs() / step_angle(disc.radius)).ceil().max(1.0) as u32;
                let center = self.vertex(disc.center);
                let (from, outer_end, outer_start) = match turn > 0.0 {
                    true => (before.right, ends[1], starts[1]),
                    false => (before.left, ends[0], starts[0]),
                };
                let rim = |step: u32| {
                    disc.center + from.rotated(turn * step as f32 / steps as f32) * disc.radius
                };
                let mut previous = outer_end;
                for step in 1..steps {
                    let next = self.vertex(rim(step));
                    self.mesh.triangles.push([center, previous, next]);
                    previous = next;
                }
                self.mesh.triangles.push([center, previous, outer_start]);
                starts
            }
        }
    }

    fn cap(&mut self, disc: Disc) {
        let steps = (TAU / step_angle(disc.radius)).ceil().max(4.0) as u32;
        self.fan(disc, Vector::new(1.0, 0.0), TAU, steps);
    }

    /// Triangles from the centre over the arc turning `angle` from `from`.
    fn fan(&mut self, disc: Disc, from: Vector<Local>, angle: f32, steps: u32) {
        let center = self.vertex(disc.center);
        let rim = |step: u32| {
            disc.center + from.rotated(angle * step as f32 / steps as f32) * disc.radius
        };
        let mut previous = self.vertex(rim(0));
        for step in 1..=steps {
            let next = self.vertex(rim(step));
            self.mesh.triangles.push([center, previous, next]);
            previous = next;
        }
    }

    fn vertex(&mut self, at: Point<Local>) -> u32 {
        self.mesh.vertices.push(at);
        (self.mesh.vertices.len() - 1) as u32
    }
}

/// Unit normals from the centres to where the outer tangent lines touch each side.
#[derive(Clone, Copy, Debug)]
struct Sides {
    left: Vector<Local>,
    right: Vector<Local>,
}

/// `None` when one disc lies inside the other, so no tangent lines run between them.
fn tangents(from: Disc, to: Disc) -> Option<Sides> {
    let between = to.center - from.center;
    let distance = between.length();
    match distance > (from.radius - to.radius).abs() && distance > 0.0 {
        true => {
            let along = between / distance;
            let sin = (from.radius - to.radius) / distance;
            let cos = (1.0 - sin * sin).sqrt();
            let across = along.perpendicular();
            Some(Sides {
                left: across * cos + along * sin,
                right: -across * cos + along * sin,
            })
        }
        false => None,
    }
}

/// The widest arc step whose chord stays within `EPSILON` of the circle.
fn step_angle(radius: f32) -> f32 {
    match radius > EPSILON {
        true => 2.0 * (1.0 - EPSILON / radius).acos(),
        false => FRAC_PI_2,
    }
}

/// Appends the polyline through `points` swept by a round tip of `radius`.
pub fn sweep(mesh: &mut Mesh, points: &[Point<Local>], radius: f32) {
    let mut builder = MeshBuilder {
        discs: (points.iter())
            .map(|&center| Disc { center, radius })
            .collect(),
        ..MeshBuilder::default()
    };
    for step in 0..builder.discs.len() {
        builder.step(step);
    }
    match builder.discs.len() {
        0 | 1 => {}
        n => builder.cap(builder.discs[n - 1]),
    }
    let offset = mesh.vertices.len() as u32;
    mesh.vertices.extend(builder.mesh.vertices);
    (mesh.triangles).extend(
        builder
            .mesh
            .triangles
            .iter()
            .map(|triangle| triangle.map(|index| index + offset)),
    );
}

/// Points around an ellipse close enough that its chords stay within `EPSILON` of it, the first
/// repeated at the end.
pub fn ellipse(center: Point<Local>, radii: Vector<Local>) -> Vec<Point<Local>> {
    let steps = (TAU / step_angle(radii.x.abs().max(radii.y.abs())))
        .ceil()
        .max(8.0) as u32;
    (0..=steps)
        .map(|step| {
            let (sin, cos) = (TAU * step as f32 / steps as f32).sin_cos();
            Point::new(center.x + radii.x * cos, center.y + radii.y * sin)
        })
        .collect()
}
