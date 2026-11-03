use library::{Annotation, Change, Comment, Highlight};
use model::command::DraftCommand;
use model::mode::Mode;
use model::notice::Notice;

use crate::draft::{Beside, Draft, Edits, NotWriting, Start};
use crate::drawing::{InkTabs, PdfMode, PdfTab};
use crate::reading::{Notes, Reader, ReaderId, Reading};
use crate::view::tab::Tab;
use crate::{Confirm, Effect, Overlay, Prompt, PromptKind, State};

pub struct Writing {
    pub highlight: Highlight,
    pub kept: Kept,
    pub draft: Draft,
    pub beside: Beside,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kept {
    Library,
    New,
}

impl PartialEq for Writing {
    fn eq(&self, other: &Writing) -> bool {
        self.highlight == other.highlight && self.draft.doc.text == other.draft.doc.text
    }
}

impl Eq for Writing {}

impl std::fmt::Debug for Writing {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Writing")
            .field("highlight", &self.highlight.id)
            .field("text", &self.draft.doc.text)
            .field("saved", &self.draft.saved)
            .finish()
    }
}

impl Writing {
    pub fn new(highlight: Highlight, kept: Kept, text: &str) -> Writing {
        let stored = highlight.comment.as_ref().map_or("", Comment::as_str);
        let start = match (kept, stored == text) {
            (Kept::Library, true) => Start::AsKept,
            (Kept::Library, false) | (Kept::New, _) => Start::Unkept,
        };
        Writing {
            highlight,
            kept,
            draft: Draft::new(text, start),
            beside: Beside::SPLIT,
        }
    }

    pub fn save(&mut self, file: ReaderId, notes: &Notes) -> Result<Effect, Notice> {
        let listed = notes.listed()?;
        let text = self.draft.doc.text.to_string();
        let written = Highlight {
            comment: Comment::new(text.trim_end().to_owned()),
            ..self.highlight.clone()
        };
        self.highlight = written.clone();
        self.kept = Kept::Library;
        self.draft.kept();
        Ok(listed.write(file, Change::Put(Annotation::Highlight(written))))
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

pub fn command(state: &mut State, command: DraftCommand, out: &mut Vec<Effect>) {
    match command {
        DraftCommand::Expand => return expand(state),
        DraftCommand::Finish | DraftCommand::Drop | DraftCommand::Swap => {}
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
    let written = Comment::new(writing.draft.doc.text.to_string().trim_end().to_owned());
    match (command, writing.draft.edits(), writing.kept, written) {
        (DraftCommand::Swap, ..) => writing.beside = writing.beside.swapped(),
        (DraftCommand::Finish, Edits::Saved, Kept::Library, _)
        | (DraftCommand::Finish, _, Kept::New, None)
        | (DraftCommand::Drop, Edits::Saved, ..) => {
            *reading = leave(writing.highlight.clone(), writing.kept);
            input.mode = Mode::Normal;
        }
        (DraftCommand::Finish, ..) => match writing.save(file, notes) {
            Ok(effect) => {
                out.push(effect);
                *reading = leave(writing.highlight.clone(), Kept::Library);
                input.mode = Mode::Normal;
            }
            Err(refused) => *notice = Some(refused),
        },
        (DraftCommand::Drop, Edits::Unsaved, ..) => *overlay = Overlay::Confirm(Confirm::DropDraft),
        (DraftCommand::Expand, ..) => {}
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
        Tab::Ink(_) | Tab::Text(_) | Tab::Image(_) | Tab::JsonCanvas(_) => None,
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
