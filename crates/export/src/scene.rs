//! What an export shows: a page, or a region of a board, as paper with the marks over it.

use ink::doc::{Background, Board, Drawing, LayerIndex, Mark, MarkId, PageIndex, Paged, Underlay};
use std::cmp::Ordering;

use ink::geom::{Local, Point, Rect, Surface};
use ink::mesh::Mesh;
use model::color::Rgb;
use smallvec::{SmallVec, smallvec};

pub const MARGIN: f32 = 16.0;

pub static BLANK: Underlay = Underlay::Blank;

pub struct Scene<'a> {
    pub area: Rect<Surface>,
    pub paper: Background,
    pub underlay: &'a Underlay,
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
        underlay: paged.underlay(),
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
            Point::new(min.x - MARGIN, min.y - MARGIN),
            Point::new(max.x + MARGIN, max.y + MARGIN),
        ),
        paper: board.background,
        underlay: &BLANK,
        marks,
    })
}

impl Scene<'_> {
    pub fn paper(&self) -> SmallVec<[Fill; 2]> {
        let sheet = |color| Fill {
            color,
            rects: vec![self.area],
        };
        match self.paper {
            Background::Plain(paper) => smallvec![sheet(paper)],
            // The writers draw the PDF's own page.
            Background::Pdf { .. } => SmallVec::new(),
            Background::Pattern {
                paper,
                ink,
                pattern,
                spacing,
            } => smallvec![
                sheet(paper),
                Fill {
                    color: ink,
                    rects: pattern.rects(spacing, self.area),
                }
            ],
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
