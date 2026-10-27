//! What an export shows: a page, or a region of a board, as paper with the marks over it.

use ink::doc::{Background, Board, Drawing, LayerIndex, Mark, MarkId, PageIndex, Paged, Pattern};
use std::cmp::Ordering;

use ink::geom::{Local, Point, Rect, Surface};
use ink::mesh::Mesh;
use model::color::Rgb;
use model::sheet::GAP;
use smallvec::{SmallVec, smallvec};

/// The width of a ruled line, in points.
pub const RULE: f32 = 0.5;
/// The side of a dot on dotted paper, in points.
pub const DOT: f32 = 1.5;

pub struct Scene<'a> {
    pub area: Rect<Surface>,
    pub paper: Background,
    /// Bottom first.
    pub marks: Vec<&'a Mark>,
}

#[derive(Clone, Debug)]
pub enum Region {
    Everything,
    Picked {
        layer: LayerIndex,
        ids: SmallVec<[MarkId; 8]>,
    },
}

pub struct Fill {
    pub color: Rgb,
    pub rects: Vec<Rect<Surface>>,
}

pub fn page(paged: &Paged, index: PageIndex) -> Scene<'_> {
    let placed = paged.stack().place(index);
    let page = paged.page(index);
    Scene {
        area: Rect::spanning(
            Point::new(0.0, 0.0),
            Point::new(placed.size.width.get(), placed.size.height.get()),
        ),
        paper: page.background,
        marks: (page.marks().shown(paged.layers()))
            .flat_map(|marks| marks.iter().map(|(_, mark)| mark.as_ref()))
            .collect(),
    }
}

/// The marks' bounds and a margin around them; `None` when there is nothing to show.
pub fn region<'a>(board: &'a Board, region: &Region) -> Option<Scene<'a>> {
    let marks: Vec<&Mark> = match region {
        Region::Everything => (board.layer_marks().shown(board.layers()))
            .flat_map(|marks| marks.iter().map(|(_, mark)| mark.as_ref()))
            .collect(),
        Region::Picked { layer, ids } => (board.marks(*layer).into_iter())
            .flat_map(|marks| marks.iter())
            .filter(|(id, _)| ids.contains(id))
            .map(|(_, mark)| mark.as_ref())
            .collect(),
    };
    let (first, rest) = marks.split_first()?;
    let bounds = (rest.iter()).fold(first.bounds(), |bounds, mark| bounds.union(&mark.bounds()));
    let (min, max) = (bounds.min(), bounds.max());
    Some(Scene {
        area: Rect::spanning(
            Point::new(min.x - GAP, min.y - GAP),
            Point::new(max.x + GAP, max.y + GAP),
        ),
        paper: board.background,
        marks,
    })
}

impl Scene<'_> {
    /// The paper, then its pattern; a pattern follows the surface, so regions line up.
    pub fn paper(&self) -> SmallVec<[Fill; 2]> {
        let sheet = |color| Fill {
            color,
            rects: vec![self.area],
        };
        match self.paper {
            Background::Plain(paper) => smallvec![sheet(paper)],
            Background::Pattern {
                paper,
                ink,
                pattern,
                spacing,
            } => {
                let (min, max) = (self.area.min(), self.area.max());
                let step = spacing.get();
                let lines = |from: f32, to: f32| {
                    ((from / step).floor() as i64 + 1..)
                        .map(move |n| n as f32 * step)
                        .take_while(move |at| *at < to)
                };
                let rect = |x: f32, y: f32, width: f32, height: f32| {
                    Rect::spanning(Point::new(x, y), Point::new(x + width, y + height))
                };
                let across = |y: f32| rect(min.x, y - RULE * 0.5, max.x - min.x, RULE);
                let rects = match pattern {
                    Pattern::Lines => lines(min.y, max.y).map(across).collect(),
                    Pattern::Grid => (lines(min.y, max.y).map(across))
                        .chain(
                            lines(min.x, max.x)
                                .map(|x| rect(x - RULE * 0.5, min.y, RULE, max.y - min.y)),
                        )
                        .collect(),
                    Pattern::Dots => lines(min.y, max.y)
                        .flat_map(|y| {
                            lines(min.x, max.x)
                                .map(move |x| rect(x - DOT * 0.5, y - DOT * 0.5, DOT, DOT))
                        })
                        .collect(),
                };
                smallvec![sheet(paper), Fill { color: ink, rects }]
            }
        }
    }
}

/// `mesh`'s triangles all wound the same way, so a nonzero fill of them all is their union.
pub fn wound(mesh: &Mesh) -> impl Iterator<Item = [Point<Local>; 3]> + '_ {
    mesh.triangles.iter().map(|triangle| {
        let [a, b, c] = triangle.map(|index| mesh.vertices[index as usize]);
        match (b - a).cross(c - a).total_cmp(&0.0) {
            Ordering::Less => [a, c, b],
            Ordering::Equal | Ordering::Greater => [a, b, c],
        }
    })
}
