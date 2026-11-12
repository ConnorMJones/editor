use helix_core::{Rope, Selection};

use model::command::DraftCommand;

use crate::drawing::{InkTabs, PdfMode, PdfTab};
use crate::json_canvas::{Doing, Place};
use crate::reading::Reading;
use crate::text::buffer::TextDoc;
use crate::view::tab::{Share, Tab};
use crate::view::text_view::Scroll;
use crate::{Effect, State};

pub struct Draft {
    pub doc: TextDoc,
    pub selection: Selection,
    pub scroll: Scroll,
    pub saved: Saved,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Saved {
    At(u64),
    Differs,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Edits {
    Saved,
    Unsaved,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Spread {
    Split,
    Full,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Beside {
    pub spread: Spread,
    pub share: Share,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NotWriting;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Start {
    AsKept,
    Unkept,
}

impl Draft {
    pub fn new(text: &str, start: Start) -> Draft {
        let doc = TextDoc::plain(Rope::from(text));
        let saved = match start {
            Start::AsKept => Saved::At(doc.version),
            Start::Unkept => Saved::Differs,
        };
        Draft {
            selection: Selection::point(doc.text.len_chars()),
            doc,
            scroll: Scroll::default(),
            saved,
        }
    }

    pub fn edits(&self) -> Edits {
        match self.saved {
            Saved::At(version) if version == self.doc.version => Edits::Saved,
            Saved::At(_) | Saved::Differs => Edits::Unsaved,
        }
    }

    pub fn kept(&mut self) {
        self.saved = Saved::At(self.doc.version);
    }
}

impl Beside {
    pub const SPLIT: Beside = Beside {
        spread: Spread::Split,
        share: Share::EVEN,
    };

    pub fn swapped(self) -> Beside {
        Beside {
            spread: match self.spread {
                Spread::Split => Spread::Full,
                Spread::Full => Spread::Split,
            },
            ..self
        }
    }
}

impl Tab {
    pub fn drafting(&self) -> Option<(&Draft, Option<Beside>)> {
        match self {
            Tab::Ink(InkTabs::Pdf(PdfTab {
                mode: PdfMode::Read(Reading::Writing(writing)),
                ..
            })) => Some((&writing.draft, Some(writing.beside))),
            Tab::Book(book) => match &book.reading {
                Reading::Writing(writing) => Some((&writing.draft, Some(writing.beside))),
                Reading::Idle
                | Reading::Selecting { .. }
                | Reading::Selected(_)
                | Reading::OnHighlight(_) => None,
            },
            Tab::JsonCanvas(canvas) => match &canvas.doing {
                Doing::Writing(writing) => Some((&writing.draft, writing.place.beside())),
                Doing::Arranging(_) => None,
            },
            Tab::Ink(_) | Tab::Text(_) | Tab::Image(_) => None,
        }
    }

    pub fn drafting_mut(&mut self) -> Option<(&mut Draft, Option<&mut Beside>)> {
        match self {
            Tab::Ink(InkTabs::Pdf(PdfTab {
                mode: PdfMode::Read(Reading::Writing(writing)),
                ..
            })) => Some((&mut writing.draft, Some(&mut writing.beside))),
            Tab::Book(book) => match &mut book.reading {
                Reading::Writing(writing) => Some((&mut writing.draft, Some(&mut writing.beside))),
                Reading::Idle
                | Reading::Selecting { .. }
                | Reading::Selected(_)
                | Reading::OnHighlight(_) => None,
            },
            Tab::JsonCanvas(canvas) => match &mut canvas.doing {
                Doing::Writing(writing) => {
                    let beside = match &mut writing.place {
                        Place::InPlace => None,
                        Place::Beside(beside) => Some(beside),
                    };
                    Some((&mut writing.draft, beside))
                }
                Doing::Arranging(_) => None,
            },
            Tab::Ink(_) | Tab::Text(_) | Tab::Image(_) => None,
        }
    }
}

pub fn command(state: &mut State, command: DraftCommand, out: &mut Vec<Effect>) {
    match state.layout.focused().active() {
        Some(Tab::JsonCanvas(_)) => crate::json_canvas::draft(state, command),
        Some(Tab::Text(_) | Tab::Ink(_) | Tab::Image(_) | Tab::Book(_)) | None => {
            crate::comment::command(state, command, out)
        }
    }
}

pub fn dropped(state: &mut State) {
    match state.layout.focused().active() {
        Some(Tab::JsonCanvas(_)) => crate::json_canvas::drop_draft(state),
        Some(Tab::Text(_) | Tab::Ink(_) | Tab::Image(_) | Tab::Book(_)) | None => {
            crate::comment::dropped(state)
        }
    }
}
