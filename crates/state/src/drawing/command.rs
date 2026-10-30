//! Keys on a focused drawing, and undo and redo there.

use ink::doc::ChangeError;
use ink::doc::{
    Board, Drawing, Layer, LayerChange, LayerIndex, LayerName, Mark, MarkId, PageIndex, Paged,
    PagedChange, ShapeKind, Visibility,
};
use ink::history::HistoryError;
use ink::text::Fonts;
use ink::tool::{Gather, ToolKind};
use model::command::{
    Annotate, Direction, DrawingCommand, InkCommand, InkTool, PageCommand, PageSide, PageStep,
    PdfCommand,
};
use model::mode::Mode;
use model::notice::Notice;
use model::register::Clipboard;
use model::sheet::{Measure, SheetView, Stack, StackEdit};

use crate::drawing::typing::{self, Ended};
use crate::drawing::{
    BoardTab, Doing, Found, Hand, InkFile, InkTabs, PagedId, PagedTab, PdfFile, PdfMode, PdfTab,
    Reading, TextSpan,
};
use crate::input::prompt;
use crate::view::layout::Layout;
use crate::view::tab::Tab;
use crate::{Commenting, Effect, Overlay, PromptKind, State};
use library::{Annotation, AnnotationId, Bookmark, Change, Highlight};

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
    Find,
    FindText(String),
    Comment(Commenting),
}

pub fn command(state: &mut State, command: DrawingCommand, out: &mut Vec<Effect>) {
    let State {
        layout,
        pageds,
        boards,
        pdfs,
        notice,
        input,
        fonts,
        overlay,
        project,
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
        Some(Tab::Ink(InkTabs::Pdf(tab))) => {
            let file = &mut pdfs[tab.file()];
            pdf(tab, file, command, fonts, out)
        }
        Some(Tab::Text(_) | Tab::Image(_)) | None => Err(Notice::NotADrawing),
    };
    match result {
        Ok(After::Nothing) => {}
        Ok(After::Follow(file, edit)) => follow(layout, file, edit),
        Ok(After::Typed) => input.mode = Mode::Normal,
        Ok(After::Find) => {
            *overlay = Overlay::Prompt(prompt::open(PromptKind::FindInPdf, &project.index))
        }
        Ok(After::FindText(query)) => crate::drawing::reading::find(state, query, out),
        Ok(After::Comment(commenting)) => {
            let line = (commenting.highlight.comment.as_ref())
                .map_or(String::new(), |comment| comment.as_str().to_owned());
            let mut prompt = prompt::open(PromptKind::Comment(commenting), &project.index);
            prompt.cursor = line.len();
            prompt.line = line;
            *overlay = Overlay::Prompt(prompt);
        }
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
        DrawingCommand::Pdf(_) => Err(Notice::NotAPdf),
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
        DrawingCommand::Pdf(_) => Err(Notice::NotAPdf),
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
            go(
                &mut tab.view,
                file.edited.drawing().stack(),
                step,
                tab.hand.last,
            );
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

fn go(view: &mut SheetView, stack: &Stack, step: PageStep, last: Option<Measure>) {
    let focus = view.centre.page;
    let page = match step {
        PageStep::Next => PageIndex(focus.0 + 1),
        PageStep::Previous => PageIndex(focus.0.saturating_sub(1)),
        PageStep::First => PageIndex(0),
        PageStep::Last => stack.last(),
    };
    view.go_to(stack, page, last.map(|measure| measure.viewport));
}

fn pdf(
    tab: &mut PdfTab,
    file: &mut PdfFile,
    command: DrawingCommand,
    fonts: &Fonts,
    out: &mut Vec<Effect>,
) -> Result<After, Notice> {
    match command {
        DrawingCommand::Page(PageCommand::Go(step)) => {
            go(&mut tab.view, &file.document().pages, step, tab.hand.last);
            Ok(After::Nothing)
        }
        DrawingCommand::Page(PageCommand::Insert(_) | PageCommand::Remove) => {
            Err(Notice::DrawingRefused {
                reason: ChangeError::PdfPages.to_string(),
            })
        }
        DrawingCommand::Ink(command) => {
            tab.mode = PdfMode::Markup;
            ink(&mut tab.hand, &mut file.ink, command, fonts)
        }
        DrawingCommand::Pdf(PdfCommand::Copy) => match &tab.mode {
            PdfMode::Read(Reading::Selected(span)) => {
                let text = (file.document().text(span.page.0))
                    .map(|text| text.slice(span.chars.clone()).to_owned())
                    .ok_or(Notice::NothingSelected)?;
                out.push(Effect::WriteClipboard {
                    clipboard: Clipboard::System,
                    text,
                });
                Ok(After::Nothing)
            }
            _ => Err(Notice::NothingSelected),
        },
        DrawingCommand::Pdf(PdfCommand::Find) => Ok(After::Find),
        DrawingCommand::Pdf(PdfCommand::Annotate(annotate)) => {
            let listed = file.notes.listed()?;
            let selected = |span: &TextSpan| {
                Highlight::of(file.document(), span.clone()).ok_or(Notice::NothingSelected)
            };
            match (annotate, &tab.mode) {
                (Annotate::Highlight, PdfMode::Read(Reading::Selected(span))) => {
                    let highlight = selected(span)?;
                    out.push(
                        listed.write(tab.file(), Change::Put(Annotation::Highlight(highlight))),
                    );
                    Ok(After::Nothing)
                }
                (Annotate::Comment, PdfMode::Read(Reading::Selected(span))) => {
                    Ok(After::Comment(Commenting {
                        file: tab.file(),
                        highlight: selected(span)?,
                    }))
                }
                (Annotate::Comment, PdfMode::Read(Reading::OnHighlight(highlight))) => {
                    Ok(After::Comment(Commenting {
                        file: tab.file(),
                        highlight: highlight.clone(),
                    }))
                }
                (Annotate::Remove, PdfMode::Read(Reading::OnHighlight(highlight))) => {
                    out.push(listed.write(tab.file(), Change::Remove(highlight.id)));
                    Ok(After::Nothing)
                }
                (Annotate::Bookmark, _) => {
                    let page = tab.view.centre.page;
                    let marked = (listed.notebook.bookmarks.iter()).find(|mark| mark.page == page);
                    let change = match marked {
                        Some(bookmark) => Change::Remove(bookmark.id),
                        None => Change::Put(Annotation::Bookmark(Bookmark {
                            id: AnnotationId::new(),
                            page,
                        })),
                    };
                    out.push(listed.write(tab.file(), change));
                    Ok(After::Nothing)
                }
                (Annotate::Highlight | Annotate::Comment, _) => Err(Notice::NothingSelected),
                (Annotate::Remove, _) => Err(Notice::NoHighlightPicked),
            }
        }
        DrawingCommand::Pdf(PdfCommand::FindSelection) => match &tab.mode {
            PdfMode::Read(Reading::Selected(span)) => (file.document().text(span.page.0))
                .map(|text| After::FindText(text.slice(span.chars.clone()).to_owned()))
                .ok_or(Notice::NothingSelected),
            _ => Err(Notice::NothingSelected),
        },
        DrawingCommand::Pdf(PdfCommand::FindNext(direction)) => match &mut tab.found {
            Found::Hits { hits, at, .. } => {
                let count = hits.len();
                *at = match direction {
                    Direction::Forward => (*at + 1) % count,
                    Direction::Backward => (*at + count - 1) % count,
                };
                let viewport = tab.hand.last.map(|measure| measure.viewport);
                tab.view
                    .go_to(&file.document().pages, hits[*at].page, viewport);
                Ok(After::Nothing)
            }
            Found::Nothing | Found::Searching { .. } => Err(Notice::NoSearch),
        },
        DrawingCommand::Pdf(PdfCommand::Read) => {
            let ended = typing::finish(&mut tab.hand.doing, &mut file.ink, fonts);
            tab.hand.cancel_stroke();
            tab.mode = PdfMode::Read(Reading::Idle);
            ended.map(After::of)
        }
    }
}

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
            let ended = typing::finish(&mut hand.doing, file, fonts);
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
                InkTool::Text => ToolKind::Text,
            };
            ended.map(After::of)
        }
        InkCommand::FinishText => match typing::finish(&mut hand.doing, file, fonts)? {
            Ended::Kept { target, picked } => {
                hand.pick(target, picked);
                Ok(After::Typed)
            }
            ended @ (Ended::NothingOpen | Ended::Dropped) => Ok(After::of(ended)),
        },
        InkCommand::DeletePicked => {
            let change = match &mut hand.doing {
                Doing::Ink { using, .. } => using.delete(),
                Doing::Typing(_) => None,
            };
            let change = change.ok_or(Notice::NothingPicked)?;
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

pub fn step(state: &mut State, step: Step) {
    let State {
        layout,
        pageds,
        boards,
        pdfs,
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
        Some(Tab::Ink(InkTabs::Pdf(tab))) => {
            tab.hand.cancel_stroke();
            history(&mut pdfs[tab.file()].ink, step)
        }
        Some(Tab::Image(_)) => Err(Notice::ReadOnly),
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

impl After {
    fn of<T>(ended: Ended<T>) -> After {
        match ended {
            Ended::NothingOpen => After::Nothing,
            Ended::Dropped | Ended::Kept { .. } => After::Typed,
        }
    }
}
