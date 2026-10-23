//! Keys on a focused drawing, and undo and redo there.

use ink::doc::{
    Board, Drawing, Layer, LayerChange, LayerIndex, LayerName, PageIndex, Paged, PagedChange,
    ShapeKind, Visibility,
};
use ink::history::HistoryError;
use ink::tool::{Gather, ToolKind};
use model::command::{
    Direction, DrawingCommand, InkCommand, InkTool, PageCommand, PageSide, PageStep, ViewCommand,
    ZoomBy,
};
use model::geom::{Pane, Vector};
use model::notice::Notice;
use model::sheet::{Fit, StackEdit, Viewport, Zoom};
use model::window::Side;

use crate::State;
use crate::drawing::{BoardTab, Hand, InkFile, InkTabs, PagedId, PagedTab};
use crate::view::layout::Layout;
use crate::view::tab::Tab;

/// One key's pan, as a part of the pane.
const PAN_STEP: f32 = 0.125;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    Undo,
    Redo,
}

/// What other tabs must do after a command.
enum After {
    Nothing,
    /// Tabs on the document stay on their pages across the edit.
    Follow(PagedId, StackEdit),
}

pub fn command(state: &mut State, command: DrawingCommand) {
    let State {
        layout,
        pageds,
        boards,
        notice,
        ..
    } = state;
    let result = match layout.focused_mut().active_mut() {
        Some(Tab::Ink(InkTabs::Paged(tab))) => {
            let file = &mut pageds[tab.file()];
            paged(tab, file, command)
        }
        Some(Tab::Ink(InkTabs::Board(tab))) => {
            let file = &mut boards[tab.file()];
            board(tab, file, command).map(|()| After::Nothing)
        }
        Some(Tab::Text(_)) | None => Err(Notice::NotADrawing),
    };
    match result {
        Ok(After::Nothing) => {}
        Ok(After::Follow(file, edit)) => follow(layout, file, edit),
        Err(refused) => *notice = Some(refused),
    }
}

fn paged(
    tab: &mut PagedTab,
    file: &mut InkFile<Paged>,
    command: DrawingCommand,
) -> Result<After, Notice> {
    match command {
        DrawingCommand::View(ViewCommand::Zoom(by)) => {
            tab.view.zoom = zoom(tab.view.zoom, by);
            Ok(After::Nothing)
        }
        DrawingCommand::View(ViewCommand::Pan(side)) => {
            match tab.hand.last {
                Some(viewport) => {
                    let stack = file.edited.drawing().stack();
                    tab.view.pan(stack, viewport, pan(side, viewport))
                }
                // Not drawn yet, so there is no pane to move by part of.
                None => {}
            }
            Ok(After::Nothing)
        }
        DrawingCommand::View(ViewCommand::ToggleFit) => {
            tab.view.fit = match tab.view.fit {
                Fit::Width => Fit::Height,
                Fit::Height => Fit::Width,
            };
            Ok(After::Nothing)
        }
        DrawingCommand::Page(page) => pages(tab, file, page),
        DrawingCommand::Ink(command) => ink(&mut tab.hand, file, command).map(|()| After::Nothing),
    }
}

fn board(
    tab: &mut BoardTab,
    file: &mut InkFile<Board>,
    command: DrawingCommand,
) -> Result<(), Notice> {
    match command {
        DrawingCommand::View(ViewCommand::Zoom(by)) => {
            tab.view.zoom = zoom(tab.view.zoom, by);
            Ok(())
        }
        DrawingCommand::View(ViewCommand::Pan(side)) => {
            match tab.hand.last {
                Some(viewport) => tab.view.pan(pan(side, viewport)),
                // Not drawn yet, so there is no pane to move by part of.
                None => {}
            }
            Ok(())
        }
        DrawingCommand::View(ViewCommand::ToggleFit) | DrawingCommand::Page(_) => {
            Err(Notice::BoardHasNoPages)
        }
        DrawingCommand::Ink(command) => ink(&mut tab.hand, file, command),
    }
}

fn zoom(zoom: Zoom, by: ZoomBy) -> Zoom {
    match by {
        ZoomBy::In => zoom.by(Zoom::STEP),
        ZoomBy::Out => zoom.by(1.0 / Zoom::STEP),
        ZoomBy::Reset => Zoom::ONE,
    }
}

fn pan(side: Side, viewport: Viewport) -> Vector<Pane> {
    let (x, y) = (
        viewport.width.get() * PAN_STEP,
        viewport.height.get() * PAN_STEP,
    );
    match side {
        Side::Left => Vector::new(-x, 0.0),
        Side::Right => Vector::new(x, 0.0),
        Side::Up => Vector::new(0.0, -y),
        Side::Down => Vector::new(0.0, y),
    }
}

fn pages(
    tab: &mut PagedTab,
    file: &mut InkFile<Paged>,
    command: PageCommand,
) -> Result<After, Notice> {
    let focus = tab.view.centre.page;
    match command {
        PageCommand::Go(step) => {
            let stack = file.edited.drawing().stack();
            let page = match step {
                PageStep::Next => PageIndex(focus.0 + 1),
                PageStep::Previous => PageIndex(focus.0.saturating_sub(1)),
                PageStep::First => PageIndex(0),
                PageStep::Last => stack.last(),
            };
            tab.view.go_to(stack, page, tab.hand.last);
            Ok(After::Nothing)
        }
        PageCommand::Insert(side) => {
            let at = match side {
                PageSide::Above => focus,
                PageSide::Below => PageIndex(focus.0 + 1),
            };
            let drawing = file.edited.drawing();
            let change = PagedChange::InsertPage {
                at,
                size: drawing.stack().place(focus).size,
                background: drawing.page(focus).background,
            };
            edit(tab, file, change, StackEdit::Inserted(at))
        }
        PageCommand::Remove => edit(
            tab,
            file,
            PagedChange::RemovePage(focus),
            StackEdit::Removed(focus),
        ),
    }
}

/// Commits a change to the pages, then shows the page it touched.
fn edit(
    tab: &mut PagedTab,
    file: &mut InkFile<Paged>,
    change: PagedChange,
    edit: StackEdit,
) -> Result<After, Notice> {
    commit(file, change)?;
    let page = match edit {
        StackEdit::Inserted(page) | StackEdit::Removed(page) => page,
    };
    tab.view
        .go_to(file.edited.drawing().stack(), page, tab.hand.last);
    Ok(After::Follow(tab.file(), edit))
}

/// Tabs on `file` other than the focused one keep their pages across `edit`.
fn follow(layout: &mut Layout, file: PagedId, edit: StackEdit) {
    let focus = layout.focus();
    for (pane, shown) in layout.panes_mut() {
        for item in shown.items_mut() {
            match &mut item.tab {
                Tab::Ink(InkTabs::Paged(tab)) if tab.file() == file && pane != focus => {
                    tab.view.follow(edit)
                }
                Tab::Ink(_) | Tab::Text(_) => {}
            }
        }
    }
}

fn ink<D: Drawing>(
    hand: &mut Hand<D::Target>,
    file: &mut InkFile<D>,
    command: InkCommand,
) -> Result<(), Notice> {
    match command {
        InkCommand::Tool(tool) => {
            // What the old tool was doing, a selection included, ends with it.
            hand.cancel_stroke();
            hand.tip = match tool {
                InkTool::Brush => ToolKind::Brush,
                InkTool::Eraser => ToolKind::Eraser,
                InkTool::BoxSelect => ToolKind::Select(Gather::Box),
                InkTool::LassoSelect => ToolKind::Select(Gather::Lasso),
                InkTool::Line => ToolKind::Shape(ShapeKind::Line),
                InkTool::Arrow => ToolKind::Shape(ShapeKind::Arrow),
                InkTool::Rectangle => ToolKind::Shape(ShapeKind::Rectangle),
                InkTool::Ellipse => ToolKind::Shape(ShapeKind::Ellipse),
            };
            Ok(())
        }
        InkCommand::DeletePicked => {
            let change = hand.tools.select.delete().ok_or(Notice::NothingPicked)?;
            commit(file, change.into())
        }
        InkCommand::Layer(direction) => {
            let count = file.edited.drawing().layers().len();
            hand.layer = LayerIndex(match direction {
                Direction::Forward => (hand.layer.0 + 1) % count,
                Direction::Backward => (hand.layer.0 + count - 1) % count,
            });
            Ok(())
        }
        InkCommand::ToggleLayer => {
            let layers = file.edited.drawing().layers();
            let layer = layers.get(hand.layer).ok_or(Notice::NoLayer)?;
            let toggled = Layer {
                name: layer.name.clone(),
                visibility: match layer.visibility {
                    Visibility::Shown => Visibility::Hidden,
                    Visibility::Hidden => Visibility::Shown,
                },
            };
            commit(file, LayerChange::Set(hand.layer, toggled).into())
        }
        InkCommand::NewLayer => {
            let count = file.edited.drawing().layers().len();
            let name = LayerName::new(format!("Layer {}", count + 1)).expect("not blank");
            let layer = Layer {
                name,
                visibility: Visibility::Shown,
            };
            commit(file, LayerChange::Add(layer).into())?;
            hand.layer = LayerIndex(count);
            Ok(())
        }
    }
}

pub fn commit<D: Drawing>(file: &mut InkFile<D>, change: D::Change) -> Result<(), Notice> {
    file.edited
        .commit(change)
        .map(|_| ())
        .map_err(|error| Notice::DrawingRefused {
            reason: error.to_string(),
        })
}

/// Steps the focused drawing's history; the tab's stroke in progress is dropped first.
pub fn step(state: &mut State, step: Step) {
    let State {
        layout,
        pageds,
        boards,
        notice,
        ..
    } = state;
    let result = match layout.focused_mut().active_mut() {
        Some(Tab::Ink(InkTabs::Paged(tab))) => {
            tab.hand.cancel_stroke();
            history(&mut pageds[tab.file()], step)
        }
        Some(Tab::Ink(InkTabs::Board(tab))) => {
            tab.hand.cancel_stroke();
            history(&mut boards[tab.file()], step)
        }
        Some(Tab::Text(_)) | None => Err(Notice::NotADrawing),
    };
    match result {
        Ok(()) => {}
        Err(refused) => *notice = Some(refused),
    }
}

fn history<D: Drawing>(file: &mut InkFile<D>, step: Step) -> Result<(), Notice> {
    let stepped = match step {
        Step::Undo => file.edited.undo(),
        Step::Redo => file.edited.redo(),
    };
    match stepped {
        Ok(_) => Ok(()),
        Err(HistoryError::NothingToUndo) => Err(Notice::AlreadyOldest),
        Err(HistoryError::NothingToRedo) => Err(Notice::AlreadyNewest),
    }
}
