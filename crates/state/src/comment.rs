use helix_core::{Rope, Selection};
use library::{Annotation, Change, Comment, Highlight};
use model::command::CommentCommand;
use model::mode::Mode;
use model::notice::Notice;

use crate::drawing::{InkTabs, PdfMode, PdfTab};
use crate::reading::{Notes, Reader, ReaderId, Reading};
use crate::text::buffer::TextDoc;
use crate::view::layout::Scroll;
use crate::view::tab::{Share, Tab};
use crate::{Confirm, Effect, Overlay, Prompt, PromptKind, State};

pub struct Writing {
    pub highlight: Highlight,
    pub kept: Kept,
    pub doc: TextDoc,
    pub selection: Selection,
    pub scroll: Scroll,
    pub saved: Saved,
    pub share: Share,
    pub spread: Spread,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kept {
    Library,
    New,
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NotWriting;

impl PartialEq for Writing {
    fn eq(&self, other: &Writing) -> bool {
        self.highlight == other.highlight && self.doc.text == other.doc.text
    }
}

impl Eq for Writing {}

impl std::fmt::Debug for Writing {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Writing")
            .field("highlight", &self.highlight.id)
            .field("text", &self.doc.text)
            .field("saved", &self.saved)
            .finish()
    }
}

impl Writing {
    pub fn new(highlight: Highlight, kept: Kept, text: &str) -> Writing {
        let doc = TextDoc::plain(Rope::from(text));
        let stored = highlight.comment.as_ref().map_or("", Comment::as_str);
        let saved = match (kept, stored == text) {
            (Kept::Library, true) => Saved::At(doc.version),
            (Kept::Library, false) | (Kept::New, _) => Saved::Differs,
        };
        Writing {
            selection: Selection::point(doc.text.len_chars()),
            highlight,
            kept,
            doc,
            scroll: Scroll::default(),
            saved,
            share: Share::EVEN,
            spread: Spread::Split,
        }
    }

    pub fn edits(&self) -> Edits {
        match self.saved {
            Saved::At(version) if version == self.doc.version => Edits::Saved,
            Saved::At(_) | Saved::Differs => Edits::Unsaved,
        }
    }

    pub fn save(&mut self, file: ReaderId, notes: &Notes) -> Result<Effect, Notice> {
        let listed = notes.listed()?;
        let text = self.doc.text.to_string();
        let written = Highlight {
            comment: Comment::new(text.trim_end().to_owned()),
            ..self.highlight.clone()
        };
        self.highlight = written.clone();
        self.kept = Kept::Library;
        self.saved = Saved::At(self.doc.version);
        Ok(listed.write(file, Change::Put(Annotation::Highlight(written))))
    }
}

impl Tab {
    pub fn writing(&self) -> Option<&Writing> {
        match self {
            Tab::Ink(InkTabs::Pdf(PdfTab {
                mode: PdfMode::Read(Reading::Writing(writing)),
                ..
            })) => Some(writing),
            Tab::Book(book) => match &book.reading {
                Reading::Writing(writing) => Some(writing),
                Reading::Idle
                | Reading::Selecting { .. }
                | Reading::Selected(_)
                | Reading::OnHighlight(_) => None,
            },
            Tab::Ink(_) | Tab::Text(_) | Tab::Image(_) => None,
        }
    }

    pub fn writing_mut(&mut self) -> Option<&mut Writing> {
        match self {
            Tab::Ink(InkTabs::Pdf(PdfTab {
                mode: PdfMode::Read(Reading::Writing(writing)),
                ..
            })) => Some(writing),
            Tab::Book(book) => match &mut book.reading {
                Reading::Writing(writing) => Some(writing),
                Reading::Idle
                | Reading::Selecting { .. }
                | Reading::Selected(_)
                | Reading::OnHighlight(_) => None,
            },
            Tab::Ink(_) | Tab::Text(_) | Tab::Image(_) => None,
        }
    }
}

pub fn write(state: &mut State, out: &mut Vec<Effect>) -> Result<(), NotWriting> {
    let State {
        layout,
        pdfs,
        books,
        notice,
        ..
    } = state;
    let tab = layout.focused_mut().active_mut().ok_or(NotWriting)?;
    let Ok(reader) = Reader::of(tab, pdfs, books) else {
        return Err(NotWriting);
    };
    let (file, notes) = (reader.id, reader.notes);
    let Reading::Writing(writing) = reader.reading else {
        return Err(NotWriting);
    };
    match writing.save(file, notes) {
        Ok(effect) => out.push(effect),
        Err(refused) => *notice = Some(refused),
    }
    Ok(())
}

pub fn command(state: &mut State, command: CommentCommand, out: &mut Vec<Effect>) {
    match command {
        CommentCommand::Expand => return expand(state),
        CommentCommand::Finish | CommentCommand::Drop | CommentCommand::Swap => {}
    }
    let State {
        layout,
        pdfs,
        books,
        notice,
        overlay,
        input,
        ..
    } = state;
    let reader = (layout.focused_mut().active_mut())
        .ok_or(Notice::NotReading)
        .and_then(|tab| Reader::of(tab, pdfs, books));
    let Ok(reader) = reader else {
        return *notice = Some(Notice::NotWriting);
    };
    let (file, notes, reading) = (reader.id, reader.notes, reader.reading);
    let Reading::Writing(writing) = reading else {
        return *notice = Some(Notice::NotWriting);
    };
    let written = Comment::new(writing.doc.text.to_string().trim_end().to_owned());
    match (command, writing.edits(), writing.kept, written) {
        (CommentCommand::Swap, ..) => {
            writing.spread = match writing.spread {
                Spread::Split => Spread::Full,
                Spread::Full => Spread::Split,
            }
        }
        (CommentCommand::Finish, Edits::Saved, Kept::Library, _)
        | (CommentCommand::Finish, _, Kept::New, None)
        | (CommentCommand::Drop, Edits::Saved, ..) => {
            *reading = leave(writing.highlight.clone(), writing.kept);
            input.mode = Mode::Normal;
        }
        (CommentCommand::Finish, ..) => match writing.save(file, notes) {
            Ok(effect) => {
                out.push(effect);
                *reading = leave(writing.highlight.clone(), Kept::Library);
                input.mode = Mode::Normal;
            }
            Err(refused) => *notice = Some(refused),
        },
        (CommentCommand::Drop, Edits::Unsaved, ..) => {
            *overlay = Overlay::Confirm(Confirm::DropComment)
        }
        (CommentCommand::Expand, ..) => {}
    }
}

fn expand(state: &mut State) {
    let (commenting, line) = match std::mem::replace(&mut state.overlay, Overlay::None) {
        Overlay::Prompt(Prompt {
            kind: PromptKind::Comment(commenting),
            line,
            ..
        }) => (commenting, line),
        other => return state.overlay = other,
    };
    let State {
        layout,
        pdfs,
        books,
        input,
        ..
    } = state;
    let Some(tab) = layout.focused_mut().active_mut() else {
        return;
    };
    let reader = match Reader::of(tab, pdfs, books) {
        Ok(reader) if reader.id == commenting.file => reader,
        Ok(_) | Err(_) => return,
    };
    let kept = match reader.notes {
        Notes::Listed(listed)
            if (listed.notebook.highlights.iter())
                .any(|kept| kept.id == commenting.highlight.id) =>
        {
            Kept::Library
        }
        Notes::Listed(_) | Notes::Unlisted(_) => Kept::New,
    };
    let writing = Writing::new(commenting.highlight, kept, &line);
    *reader.reading = Reading::Writing(Box::new(writing));
    input.mode = Mode::Insert;
}

pub fn dropped(state: &mut State) {
    let reading = (state.layout.focused_mut().active_mut()).and_then(|tab| match tab {
        Tab::Ink(InkTabs::Pdf(PdfTab {
            mode: PdfMode::Read(reading),
            ..
        })) => Some(reading),
        Tab::Book(book) => Some(&mut book.reading),
        Tab::Ink(_) | Tab::Text(_) | Tab::Image(_) => None,
    });
    let Some(reading) = reading else {
        return;
    };
    *reading = match std::mem::replace(reading, Reading::Idle) {
        Reading::Writing(writing) => leave(writing.highlight, writing.kept),
        other => other,
    };
    state.input.mode = Mode::Normal;
}

fn leave(highlight: Highlight, kept: Kept) -> Reading {
    match kept {
        Kept::Library => Reading::OnHighlight(highlight),
        Kept::New => Reading::Idle,
    }
}
