//! Keys on a focused drawing, and undo and redo there.

use ink::doc::{
    Board, Drawing, Layer, LayerChange, LayerIndex, LayerName, Mark, MarkId, PageIndex, Paged,
    PagedChange, ShapeKind, Visibility,
};
use ink::history::HistoryError;
use ink::text::Fonts;
use ink::tool::{Gather, ToolKind};
use model::command::{
    Direction, DrawingCommand, InkCommand, InkTool, PageCommand, PageSide, PageStep,
};
use model::mode::Mode;
use model::notice::Notice;
use model::sheet::StackEdit;

use crate::State;
use crate::drawing::typing::{self, Ended};
use crate::drawing::{BoardTab, Hand, InkFile, InkTabs, PagedId, PagedTab};
use crate::view::layout::Layout;
use crate::view::tab::Tab;

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
    /// The text being typed was finished, so the keys leave its modes.
    Typed,
}

pub fn command(state: &mut State, command: DrawingCommand) {
    let State {
        layout,
        pageds,
        boards,
        notice,
        input,
        fonts,
        ..
    } = state;
    let result = match layout.focused_mut().active_mut() {
        Some(Tab::Ink(InkTabs::Paged(tab))) => {
            let file = &mut pageds[tab.file()];
            paged(tab, file, command, fonts)
        }
        Some(Tab::Ink(InkTabs::Board(tab))) => {
            let file = &mut boards[tab.file()];
            board(tab, file, command, fonts)
        }
        Some(Tab::Text(_) | Tab::Image(_)) | None => Err(Notice::NotADrawing),
    };
    match result {
        Ok(After::Nothing) => {}
        Ok(After::Follow(file, edit)) => follow(layout, file, edit),
        Ok(After::Typed) => input.mode = Mode::Normal,
        Err(refused) => *notice = Some(refused),
    }
}

fn paged(
    tab: &mut PagedTab,
    file: &mut InkFile<Paged>,
    command: DrawingCommand,
    fonts: &Fonts,
) -> Result<After, Notice> {
    match command {
        DrawingCommand::Page(page) => pages(tab, file, page),
        DrawingCommand::Ink(command) => ink(&mut tab.hand, file, command, fonts),
    }
}

fn board(
    tab: &mut BoardTab,
    file: &mut InkFile<Board>,
    command: DrawingCommand,
    fonts: &Fonts,
) -> Result<After, Notice> {
    match command {
        DrawingCommand::Page(_) => Err(Notice::BoardHasNoPages),
        DrawingCommand::Ink(command) => ink(&mut tab.hand, file, command, fonts),
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
            tab.view
                .go_to(stack, page, tab.hand.last.map(|measure| measure.viewport));
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
    tab.view.go_to(
        file.edited.drawing().stack(),
        page,
        tab.hand.last.map(|measure| measure.viewport),
    );
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
                Tab::Ink(_) | Tab::Text(_) | Tab::Image(_) => {}
            }
        }
    }
}

fn ink<D: Drawing>(
    hand: &mut Hand<D::Target>,
    file: &mut InkFile<D>,
    command: InkCommand,
    fonts: &Fonts,
) -> Result<After, Notice> {
    match command {
        InkCommand::Tool(tool) => {
            // What the old tool was doing, a selection or text included, ends with it.
            hand.cancel_stroke();
            let ended = typing::finish(&mut hand.typing, file, fonts);
            hand.tip = match tool {
                InkTool::Brush => ToolKind::Brush,
                InkTool::Eraser => ToolKind::Eraser,
                InkTool::BoxSelect => ToolKind::Select(Gather::Box),
                InkTool::LassoSelect => ToolKind::Select(Gather::Lasso),
                InkTool::Line => ToolKind::Shape(ShapeKind::Line),
                InkTool::Arrow => ToolKind::Shape(ShapeKind::Arrow),
                InkTool::Rectangle => ToolKind::Shape(ShapeKind::Rectangle),
                InkTool::Ellipse => ToolKind::Shape(ShapeKind::Ellipse),
                InkTool::Text => ToolKind::Text,
            };
            ended.map(After::of)
        }
        InkCommand::FinishText => match typing::finish(&mut hand.typing, file, fonts)? {
            Ended::Kept { target, picked } => {
                hand.pick(target, picked);
                Ok(After::Typed)
            }
            ended @ (Ended::NothingOpen | Ended::Dropped) => Ok(After::of(ended)),
        },
        InkCommand::DeletePicked => {
            let change = hand.tools.select.delete().ok_or(Notice::NothingPicked)?;
            commit(file, change.into()).map(|()| After::Nothing)
        }
        InkCommand::Layer(direction) => {
            let count = file.edited.drawing().layers().len();
            hand.layer = LayerIndex(match direction {
                Direction::Forward => (hand.layer.0 + 1) % count,
                Direction::Backward => (hand.layer.0 + count - 1) % count,
            });
            Ok(After::Nothing)
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
            commit(file, LayerChange::Set(hand.layer, toggled).into()).map(|()| After::Nothing)
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
            Ok(After::Nothing)
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

pub fn add<D: Drawing>(
    file: &mut InkFile<D>,
    target: D::Target,
    mark: Mark,
) -> Result<MarkId, Notice> {
    file.edited
        .add(target, mark)
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
        Some(Tab::Text(_) | Tab::Image(_)) | None => Err(Notice::NotADrawing),
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

impl After {
    fn of<T>(ended: Ended<T>) -> After {
        match ended {
            Ended::NothingOpen => After::Nothing,
            Ended::Dropped | Ended::Kept { .. } => After::Typed,
        }
    }
}
