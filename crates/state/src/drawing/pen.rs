//! Pen and mouse input on a drawing's pane, routed to the tab's tools.

use std::time::Duration;

use ink::doc::{Board, Drawing, LayerIndex, Paged, Visibility};
use ink::input::{Elapsed, Pressure, Sample, Tilt};
use ink::text::Fonts;
use ink::tool::{Outcome, PenInput, ToolKind, still};
use model::geom::{Pane, Point, Surface};
use model::mode::Mode;
use model::notice::Notice;
use model::sheet::Viewport;

use crate::State;
use crate::drawing::command::commit;
use crate::drawing::frame::{self, Landed};
use crate::drawing::typing::{self, Ended};
use crate::drawing::{Doing, Gesture, Hand, InkFile, InkTabs, PdfMode, Pen};
use crate::view::layout::PaneId;
use crate::view::tab::Tab;

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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stylus {
    Tip,
    Eraser,
}

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
        pdfs,
        notice,
        input,
        fonts,
        ..
    } = state;
    let (at, viewport) = (sample.at, sample.viewport);
    let result = match layout.find_pane(pane).and_then(|pane| pane.active_mut()) {
        Some(Tab::Ink(InkTabs::Paged(tab))) => {
            let view = tab.view;
            let file = &mut pageds[tab.file()];
            on(
                &mut tab.hand,
                file,
                phase,
                sample,
                fonts,
                &mut input.mode,
                |paged, layer| frame::paged(paged, &view, viewport).land::<Paged>(at, layer),
            )
        }
        Some(Tab::Ink(InkTabs::Pdf(tab))) => match tab.mode {
            PdfMode::Markup => {
                let view = tab.view;
                let file = &mut pdfs[tab.file()];
                on(
                    &mut tab.hand,
                    file,
                    phase,
                    sample,
                    fonts,
                    &mut input.mode,
                    |paged, layer| frame::paged(paged, &view, viewport).land::<Paged>(at, layer),
                )
            }
            PdfMode::Read => Ok(()),
        },
        Some(Tab::Ink(InkTabs::Board(tab))) => {
            let view = tab.view;
            let file = &mut boards[tab.file()];
            on(
                &mut tab.hand,
                file,
                phase,
                sample,
                fonts,
                &mut input.mode,
                |board, layer| frame::board(board, &view, viewport).land::<Board>(at, layer),
            )
        }
        Some(Tab::Text(_) | Tab::Image(_)) | None => Ok(()),
    };
    match result {
        Ok(()) => {}
        Err(refused) => *notice = Some(refused),
    }
}

fn on<D: Drawing>(
    hand: &mut Hand<D::Target>,
    file: &mut InkFile<D>,
    phase: PenPhase,
    sample: PenSample,
    fonts: &Fonts,
    mode: &mut Mode,
    land: impl FnOnce(&D, LayerIndex) -> Option<Landed<D::Target>>,
) -> Result<(), Notice> {
    let drawing = file.edited.drawing();
    let outcome = match (phase, &mut hand.doing) {
        (PenPhase::Down(_), Doing::Typing(open)) => {
            match land(drawing, hand.layer) {
                Some(landed)
                    if open.target == landed.target
                        && open.text.frame().holds(landed.onto.apply(sample.at)) =>
                {
                    open.text.cursor_to(landed.onto.apply(sample.at));
                    return Ok(());
                }
                Some(_) | None => {}
            }
            match typing::finish(&mut hand.doing, file, fonts)? {
                Ended::Kept { target, picked } => hand.pick(target, picked),
                Ended::Dropped | Ended::NothingOpen => {}
            }
            *mode = Mode::Normal;
            return Ok(());
        }
        (PenPhase::Down(stylus), Doing::Ink { using, pen }) => {
            match drawing
                .layers()
                .get(hand.layer)
                .map(|layer| layer.visibility)
            {
                Some(Visibility::Shown) => {}
                Some(Visibility::Hidden) => return Err(Notice::LayerHidden),
                None => return Err(Notice::NoLayer),
            }
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
            *pen = Pen::Down(gesture);
            let input = PenInput::Down {
                target: landed.target,
                sample: surface(&gesture, sample),
            };
            (hand.tools).input(gesture.tool, input, using, drawing, gesture.reach)
        }
        (
            PenPhase::Move,
            Doing::Ink {
                using,
                pen: Pen::Down(gesture),
            },
        ) => {
            let input = PenInput::Move(surface(gesture, sample));
            (hand.tools).input(gesture.tool, input, using, drawing, gesture.reach)
        }
        (PenPhase::Up, Doing::Ink { using, pen }) => match std::mem::replace(pen, Pen::Up) {
            Pen::Down(gesture) => {
                let input = PenInput::Up(surface(&gesture, sample));
                (hand.tools).input(gesture.tool, input, using, drawing, gesture.reach)
            }
            Pen::Up => return Ok(()),
        },
        (PenPhase::Cancel, Doing::Ink { using, pen }) => match std::mem::replace(pen, Pen::Up) {
            Pen::Down(gesture) => (hand.tools).input(
                gesture.tool,
                PenInput::Cancel,
                using,
                drawing,
                gesture.reach,
            ),
            Pen::Up => return Ok(()),
        },
        (PenPhase::Move, Doing::Ink { pen: Pen::Up, .. })
        | (PenPhase::Move | PenPhase::Up | PenPhase::Cancel, Doing::Typing(_)) => return Ok(()),
    };
    match outcome {
        Outcome::Commit(change) => commit(file, change.into()),
        Outcome::Failed(error) => Err(Notice::DrawingRefused {
            reason: error.to_string(),
        }),
        Outcome::Type(at) => {
            let open = typing::open(at, &hand.style, hand.tools.paint, fonts);
            hand.doing = Doing::Typing(Box::new(open));
            *mode = Mode::Insert;
            Ok(())
        }
        Outcome::Rewrap { target, mark, to } => {
            let picked = typing::rewrap(file, fonts, target, mark, to)?;
            hand.doing = Doing::Ink {
                using: still(target, picked),
                pen: Pen::Up,
            };
            Ok(())
        }
        Outcome::Ignored | Outcome::Drawn => Ok(()),
    }
}

fn surface(gesture: &Gesture, sample: PenSample) -> Sample<Surface> {
    Sample {
        at: gesture.onto.apply(sample.at),
        time: Elapsed::of(sample.time.saturating_sub(gesture.start)),
        pressure: sample.pressure,
        tilt: sample.tilt,
    }
}
