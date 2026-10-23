//! Pen and mouse input on a drawing's pane, routed to the tab's tools.

use std::time::Duration;

use ink::doc::{Board, Drawing, LayerIndex, Paged, Visibility};
use ink::input::{Elapsed, Pressure, Sample, Tilt};
use ink::tool::{Outcome, PenInput, ToolKind};
use model::geom::{Pane, Point, Surface};
use model::notice::Notice;
use model::sheet::Viewport;

use crate::State;
use crate::drawing::command::commit;
use crate::drawing::frame::{self, Landed};
use crate::drawing::{Gesture, Hand, InkFile, InkTabs, Pen};
use crate::view::layout::PaneId;
use crate::view::tab::Tab;

/// One reading as the host saw it, on the pane drawn at `viewport`; `time` is the host's clock.
#[derive(Clone, Copy, Debug)]
pub struct PenSample {
    pub at: Point<Pane>,
    pub viewport: Viewport,
    pub time: Duration,
    pub pressure: Option<Pressure>,
    pub tilt: Option<Tilt>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PenPhase {
    Down(Stylus),
    Move,
    Up,
    Cancel,
}

/// Which end of the pen; a mouse is the tip.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stylus {
    Tip,
    Eraser,
}

/// Pen input on `pane`; a stroke starting there focuses it.
pub fn pen(state: &mut State, pane: PaneId, phase: PenPhase, sample: PenSample) {
    match phase {
        PenPhase::Down(_) => match state.layout.set_focus(pane) {
            Ok(()) => {}
            // Closed since the host drew it.
            Err(_) => return,
        },
        PenPhase::Move | PenPhase::Up | PenPhase::Cancel => {}
    }
    let State {
        layout,
        pageds,
        boards,
        notice,
        ..
    } = state;
    let (at, viewport) = (sample.at, sample.viewport);
    let result = match layout.find_pane(pane).and_then(|pane| pane.active_mut()) {
        Some(Tab::Ink(InkTabs::Paged(tab))) => {
            let view = tab.view;
            let file = &mut pageds[tab.file()];
            on(&mut tab.hand, file, phase, sample, |paged, layer| {
                frame::paged(paged, &view, viewport).land::<Paged>(at, layer)
            })
        }
        Some(Tab::Ink(InkTabs::Board(tab))) => {
            let view = tab.view;
            let file = &mut boards[tab.file()];
            on(&mut tab.hand, file, phase, sample, |board, layer| {
                frame::board(board, &view, viewport).land::<Board>(at, layer)
            })
        }
        Some(Tab::Text(_)) | None => Ok(()),
    };
    match result {
        Ok(()) => {}
        Err(refused) => *notice = Some(refused),
    }
}

/// `land` finds where a pen-down lands; it runs only for `Down`.
fn on<D: Drawing>(
    hand: &mut Hand<D::Target>,
    file: &mut InkFile<D>,
    phase: PenPhase,
    sample: PenSample,
    land: impl FnOnce(&D, LayerIndex) -> Option<Landed<D::Target>>,
) -> Result<(), Notice> {
    let (gesture, input) = match (phase, hand.pen) {
        (PenPhase::Down(stylus), _) => {
            let drawing = file.edited.drawing();
            match drawing
                .layers()
                .get(hand.layer)
                .map(|layer| layer.visibility)
            {
                Some(Visibility::Shown) => {}
                Some(Visibility::Hidden) => return Err(Notice::LayerHidden),
                None => return Err(Notice::NoLayer),
            }
            // A stroke starting off every page draws nothing.
            let Some(landed) = land(drawing, hand.layer) else {
                return Ok(());
            };
            let gesture = Gesture {
                tool: match stylus {
                    Stylus::Tip => hand.tip,
                    Stylus::Eraser => ToolKind::Eraser,
                },
                onto: landed.onto,
                start: sample.time,
                reach: landed.reach,
            };
            hand.pen = Pen::Down(gesture);
            let input = PenInput::Down {
                target: landed.target,
                sample: surface(&gesture, sample),
            };
            (gesture, input)
        }
        (PenPhase::Move, Pen::Down(gesture)) => {
            (gesture, PenInput::Move(surface(&gesture, sample)))
        }
        (PenPhase::Up, Pen::Down(gesture)) => {
            hand.pen = Pen::Up;
            (gesture, PenInput::Up(surface(&gesture, sample)))
        }
        (PenPhase::Cancel, Pen::Down(gesture)) => {
            hand.pen = Pen::Up;
            (gesture, PenInput::Cancel)
        }
        (PenPhase::Move | PenPhase::Up | PenPhase::Cancel, Pen::Up) => return Ok(()),
    };
    let drawing = file.edited.drawing();
    match hand
        .tools
        .input(gesture.tool, input, drawing, gesture.reach)
    {
        Outcome::Commit(change) => commit(file, change.into()),
        Outcome::Failed(error) => Err(Notice::DrawingRefused {
            reason: error.to_string(),
        }),
        Outcome::Ignored | Outcome::Drawn => Ok(()),
    }
}

/// `sample` on the gesture's surface, timed from its start.
fn surface(gesture: &Gesture, sample: PenSample) -> Sample<Surface> {
    Sample {
        at: gesture.onto.apply(sample.at),
        time: Elapsed::of(sample.time.saturating_sub(gesture.start)),
        pressure: sample.pressure,
        tilt: sample.tilt,
    }
}
