use model::geom::{self, Point};
use model::sheet::Viewport;
use model::window::Side;

use crate::layout::PaneId;

pub const EDGE: f32 = 0.25;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum DropOn {
    Pane { pane: PaneId, zone: Zone },
    Tabs { pane: PaneId, at: usize },
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PanePoint {
    pub at: Point<geom::Pane>,
    pub viewport: Viewport,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Zone {
    Centre(PanePoint),
    Edge(Side),
}

impl Zone {
    pub fn at(point: PanePoint) -> Zone {
        let PanePoint { at, viewport } = point;
        let (x, y) = (at.x / viewport.width.get(), at.y / viewport.height.get());
        [
            (Side::Left, x),
            (Side::Right, 1.0 - x),
            (Side::Up, y),
            (Side::Down, 1.0 - y),
        ]
        .into_iter()
        .filter(|(_, distance)| *distance < EDGE)
        .min_by(|(_, a), (_, b)| a.total_cmp(b))
        .map_or(Zone::Centre(point), |(side, _)| Zone::Edge(side))
    }
}
