use std::cmp::Ordering;
use std::path::Path;
use std::sync::Arc;

use library::{
    Annotation, AnnotationId, Bookmark, Change, Comment, DocumentId, FastHash, Highlight, Identity,
    Library, LibraryError, Notebook, Part, Place, TextSpan, Texts,
};
use model::command::{Annotate, Direction, ReadingCommand};
use model::notice::Notice;
use model::register::Clipboard;
use model::sheet::{Measure, PageIndex, SheetView, Stack};

use crate::book::{BookFiles, BookId};
use crate::drawing::{InkTabs, PdfFiles, PdfId, PdfMode};
use crate::input::prompt;
use crate::job::Job;
use crate::project::path::FileKind;
use crate::render::book::BookFlow;
use crate::render::document::Placed;
use crate::view::layout::PaneId;
use crate::view::tab::Tab;
use crate::{Effect, Overlay, PromptKind, State};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReaderId {
    Pdf(PdfId),
    Book(BookId),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReaderKind {
    Pdf,
    Book,
}

pub enum Notes {
    Listed(Listed),
    Unlisted(Unlisted),
}

pub struct Listed {
    pub library: Library,
    pub document: DocumentId,
    pub notebook: Notebook,
    pub position: Place,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Unlisted {
    Outside,
    Failed(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Found {
    Nothing,
    Searching {
        query: String,
    },
    Hits {
        query: String,
        hits: Vec<TextSpan>,
        at: usize,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Reading {
    Idle,
    Selecting {
        part: Part,
        anchor: usize,
        head: usize,
    },
    Selected(TextSpan),
    OnHighlight(Highlight),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Press {
    Down,
    Move,
    Up,
    Cancel,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Commenting {
    pub file: ReaderId,
    pub highlight: Highlight,
}

pub enum Viewing<'a> {
    Pages {
        view: &'a mut SheetView,
        pages: &'a Stack,
        last: Option<Measure>,
    },
    Flow {
        scroll: &'a mut Placed,
        flow: &'a BookFlow,
    },
}

pub struct Reader<'a> {
    pub id: ReaderId,
    pub texts: Arc<dyn Texts>,
    pub notes: &'a Notes,
    pub reading: &'a mut Reading,
    pub found: &'a mut Found,
    pub viewing: Viewing<'a>,
}

enum After {
    Nothing,
    Find,
    FindText(String),
    Comment(Commenting),
}

impl Notes {
    pub fn listed(&self) -> Result<&Listed, Notice> {
        match self {
            Notes::Listed(listed) => Ok(listed),
            Notes::Unlisted(Unlisted::Outside) => Err(Notice::NotInLibrary),
            Notes::Unlisted(Unlisted::Failed(reason)) => Err(Notice::LibraryFailed {
                reason: reason.clone(),
            }),
        }
    }
}

impl Listed {
    pub fn write(&self, file: ReaderId, change: Change) -> Effect {
        Effect::Job(Job::Annotate {
            library: self.library.clone(),
            file,
            document: self.document,
            change,
        })
    }
}

impl ReaderKind {
    pub fn of(path: &Path) -> ReaderKind {
        match FileKind::of(path) {
            FileKind::Book => ReaderKind::Book,
            FileKind::Pdf
            | FileKind::Text(_)
            | FileKind::Paged
            | FileKind::Board
            | FileKind::Image(_) => ReaderKind::Pdf,
        }
    }
}

impl ReaderId {
    pub fn of(tab: &Tab) -> Option<ReaderId> {
        match tab {
            Tab::Ink(InkTabs::Pdf(tab)) => Some(ReaderId::Pdf(tab.file())),
            Tab::Book(tab) => Some(ReaderId::Book(tab.file())),
            Tab::Ink(InkTabs::Paged(_) | InkTabs::Board(_)) | Tab::Text(_) | Tab::Image(_) => None,
        }
    }

    pub fn kind(self) -> ReaderKind {
        match self {
            ReaderId::Pdf(_) => ReaderKind::Pdf,
            ReaderId::Book(_) => ReaderKind::Book,
        }
    }

    pub fn notes<'a>(self, pdfs: &'a PdfFiles, books: &'a BookFiles) -> Option<&'a Notes> {
        match self {
            ReaderId::Pdf(id) => pdfs.get(id).map(|pdf| &pdf.notes),
            ReaderId::Book(id) => books.get(id).map(|book| &book.notes),
        }
    }

    pub fn notes_mut<'a>(
        self,
        pdfs: &'a mut PdfFiles,
        books: &'a mut BookFiles,
    ) -> Option<&'a mut Notes> {
        match self {
            ReaderId::Pdf(id) => pdfs.get_mut(id).map(|pdf| &mut pdf.notes),
            ReaderId::Book(id) => books.get_mut(id).map(|book| &mut book.notes),
        }
    }
}

impl Reading {
    pub fn shown(&self, texts: &dyn Texts) -> Option<TextSpan> {
        match self {
            Reading::Selected(span) => Some(span.clone()),
            Reading::Selecting { part, anchor, head } => Some(TextSpan {
                part: *part,
                chars: *anchor.min(head)..texts.part(*part)?.after(*anchor.max(head)),
            }),
            Reading::OnHighlight(highlight) => Some(highlight.span.clone()),
            Reading::Idle => None,
        }
    }

    pub fn step(
        self,
        press: Press,
        at: Option<Place>,
        texts: &dyn Texts,
        notes: &Notes,
    ) -> Reading {
        match (press, self) {
            (Press::Down, _) => match at {
                Some(at) => Reading::Selecting {
                    part: at.part,
                    anchor: at.char,
                    head: at.char,
                },
                None => Reading::Idle,
            },
            (Press::Move, Reading::Selecting { part, anchor, head }) => Reading::Selecting {
                part,
                anchor,
                head: (at.filter(|at| at.part == part)).map_or(head, |at| at.char),
            },
            (Press::Up, Reading::Selecting { part, anchor, head }) => {
                match (anchor.cmp(&head), texts.part(part)) {
                    (Ordering::Equal, _) => {
                        let tapped = match notes {
                            Notes::Listed(listed) => (listed.notebook.highlights.iter().rev())
                                .find(|highlight| {
                                    highlight.span.part == part
                                        && highlight.span.chars.contains(&anchor)
                                })
                                .cloned(),
                            Notes::Unlisted(_) => None,
                        };
                        match tapped {
                            Some(highlight) => Reading::OnHighlight(highlight),
                            None => Reading::Idle,
                        }
                    }
                    (Ordering::Less | Ordering::Greater, Some(text)) => {
                        Reading::Selected(TextSpan {
                            part,
                            chars: anchor.min(head)..text.after(anchor.max(head)),
                        })
                    }
                    (Ordering::Less | Ordering::Greater, None) => Reading::Idle,
                }
            }
            (Press::Cancel, _) => Reading::Idle,
            (Press::Move | Press::Up, kept) => kept,
        }
    }
}

impl<'a> Viewing<'a> {
    pub fn of(
        tab: &'a mut Tab,
        pdfs: &'a PdfFiles,
        books: &'a BookFiles,
    ) -> Option<(ReaderId, Viewing<'a>)> {
        match tab {
            Tab::Ink(InkTabs::Pdf(tab)) => {
                let id = tab.file();
                let viewing = Viewing::Pages {
                    view: &mut tab.view,
                    pages: pdfs[id].pages(),
                    last: tab.hand.last,
                };
                Some((ReaderId::Pdf(id), viewing))
            }
            Tab::Book(tab) => {
                let id = tab.file();
                let viewing = Viewing::Flow {
                    scroll: &mut tab.scroll,
                    flow: &books[id].flow,
                };
                Some((ReaderId::Book(id), viewing))
            }
            Tab::Ink(InkTabs::Paged(_) | InkTabs::Board(_)) | Tab::Text(_) | Tab::Image(_) => None,
        }
    }
}

#[derive(Clone, Copy)]
pub enum Seen<'a> {
    Pages(&'a SheetView),
    Flow { item: usize, flow: &'a BookFlow },
}

impl<'a> Seen<'a> {
    pub fn of(tab: &'a Tab, books: &'a BookFiles) -> Option<(ReaderId, Seen<'a>)> {
        match tab {
            Tab::Ink(InkTabs::Pdf(tab)) => {
                Some((ReaderId::Pdf(tab.file()), Seen::Pages(&tab.view)))
            }
            Tab::Book(tab) => Some((
                ReaderId::Book(tab.file()),
                Seen::Flow {
                    item: tab.scroll.at().item,
                    flow: &books[tab.file()].flow,
                },
            )),
            Tab::Ink(InkTabs::Paged(_) | InkTabs::Board(_)) | Tab::Text(_) | Tab::Image(_) => None,
        }
    }

    pub fn place(self) -> Place {
        match self {
            Seen::Pages(view) => Place {
                part: Part(view.centre.page.0),
                char: 0,
            },
            Seen::Flow { item, flow } => flow.place_of(item),
        }
    }

    pub fn holds(self, at: Place) -> bool {
        match self {
            Seen::Pages(view) => at.part == Part(view.centre.page.0),
            Seen::Flow { item, flow } => flow.item_at(at) == item,
        }
    }
}

pub fn bookmark_in_view<'a>(
    tab: &'a Tab,
    pdfs: &'a PdfFiles,
    books: &'a BookFiles,
) -> Option<&'a Bookmark> {
    let (file, seen) = Seen::of(tab, books)?;
    match file.notes(pdfs, books)? {
        Notes::Listed(listed) => {
            (listed.notebook.bookmarks.iter()).find(|mark| seen.holds(mark.at))
        }
        Notes::Unlisted(_) => None,
    }
}

impl Viewing<'_> {
    pub fn seen(&self) -> Seen<'_> {
        match self {
            Viewing::Pages { view, .. } => Seen::Pages(view),
            Viewing::Flow { scroll, flow } => Seen::Flow {
                item: scroll.at().item,
                flow,
            },
        }
    }

    pub fn place(&self) -> Place {
        self.seen().place()
    }

    pub fn holds(&self, at: Place) -> bool {
        self.seen().holds(at)
    }

    pub fn show(&mut self, at: Place) {
        match self {
            Viewing::Pages { view, pages, last } => view.go_to(
                pages,
                PageIndex(at.part.0),
                last.map(|measure| measure.viewport),
            ),
            Viewing::Flow { scroll, flow } => **scroll = Placed::item(flow.item_at(at)),
        }
    }
}

impl<'a> Reader<'a> {
    pub fn of(
        tab: &'a mut Tab,
        pdfs: &'a PdfFiles,
        books: &'a BookFiles,
    ) -> Result<Reader<'a>, Notice> {
        match tab {
            Tab::Ink(InkTabs::Pdf(tab)) => {
                let id = tab.file();
                let file = &pdfs[id];
                let texts: Arc<dyn Texts> = Arc::new(file.document().clone());
                match &mut tab.mode {
                    PdfMode::Read(reading) => Ok(Reader {
                        id: ReaderId::Pdf(id),
                        texts,
                        notes: &file.notes,
                        reading,
                        found: &mut tab.found,
                        viewing: Viewing::Pages {
                            view: &mut tab.view,
                            pages: file.pages(),
                            last: tab.hand.last,
                        },
                    }),
                    PdfMode::Markup => Err(Notice::NotReading),
                }
            }
            Tab::Book(tab) => {
                let id = tab.file();
                let file = &books[id];
                let texts: Arc<dyn Texts> = file.flow.texts.clone();
                Ok(Reader {
                    id: ReaderId::Book(id),
                    texts,
                    notes: &file.notes,
                    reading: &mut tab.reading,
                    found: &mut tab.found,
                    viewing: Viewing::Flow {
                        scroll: &mut tab.scroll,
                        flow: &file.flow,
                    },
                })
            }
            Tab::Ink(InkTabs::Paged(_) | InkTabs::Board(_)) | Tab::Text(_) | Tab::Image(_) => {
                Err(Notice::NotReading)
            }
        }
    }

    pub fn show(&mut self, annotation: Annotation) {
        self.viewing.show(annotation.place());
        *self.reading = match annotation {
            Annotation::Highlight(highlight) => Reading::OnHighlight(highlight),
            Annotation::Lost(_) | Annotation::Bookmark(_) => Reading::Idle,
        };
    }

    fn text(&self, span: &TextSpan) -> Result<String, Notice> {
        (self.texts.part(span.part))
            .map(|text| text.slice(span.chars.clone()).to_owned())
            .ok_or(Notice::NothingSelected)
    }

    fn selected(&self) -> Result<TextSpan, Notice> {
        match &*self.reading {
            Reading::Selected(span) => Ok(span.clone()),
            Reading::Idle | Reading::Selecting { .. } | Reading::OnHighlight(_) => {
                Err(Notice::NothingSelected)
            }
        }
    }

    fn command(mut self, command: ReadingCommand, out: &mut Vec<Effect>) -> Result<After, Notice> {
        match command {
            ReadingCommand::Copy => {
                let text = self.text(&self.selected()?)?;
                out.push(Effect::WriteClipboard {
                    clipboard: Clipboard::System,
                    text,
                });
                Ok(After::Nothing)
            }
            ReadingCommand::Clear => {
                *self.reading = Reading::Idle;
                Ok(After::Nothing)
            }
            ReadingCommand::Find => Ok(After::Find),
            ReadingCommand::FindSelection => Ok(After::FindText(self.text(&self.selected()?)?)),
            ReadingCommand::FindNext(direction) => match &mut *self.found {
                Found::Hits { hits, at, .. } => {
                    let count = hits.len();
                    *at = match direction {
                        Direction::Forward => (*at + 1) % count,
                        Direction::Backward => (*at + count - 1) % count,
                    };
                    let shown = hits[*at].start();
                    self.viewing.show(shown);
                    Ok(After::Nothing)
                }
                Found::Nothing | Found::Searching { .. } => Err(Notice::NoSearch),
            },
            ReadingCommand::Annotate(annotate) => {
                let listed = self.notes.listed()?;
                let file = self.id;
                match (annotate, &*self.reading) {
                    (Annotate::Highlight, Reading::Selected(span)) => {
                        let highlight = Highlight::of(&*self.texts, span.clone())
                            .ok_or(Notice::NothingSelected)?;
                        out.push(listed.write(file, Change::Put(Annotation::Highlight(highlight))));
                        Ok(After::Nothing)
                    }
                    (Annotate::Comment, Reading::Selected(span)) => {
                        Ok(After::Comment(Commenting {
                            file,
                            highlight: Highlight::of(&*self.texts, span.clone())
                                .ok_or(Notice::NothingSelected)?,
                        }))
                    }
                    (Annotate::Comment, Reading::OnHighlight(highlight)) => {
                        Ok(After::Comment(Commenting {
                            file,
                            highlight: highlight.clone(),
                        }))
                    }
                    (Annotate::Remove, Reading::OnHighlight(highlight)) => {
                        out.push(listed.write(file, Change::Remove(highlight.id)));
                        Ok(After::Nothing)
                    }
                    (Annotate::Bookmark, _) => {
                        let marked = (listed.notebook.bookmarks.iter())
                            .find(|mark| self.viewing.holds(mark.at));
                        let change = match marked {
                            Some(bookmark) => Change::Remove(bookmark.id),
                            None => Change::Put(Annotation::Bookmark(Bookmark {
                                id: AnnotationId::new(),
                                at: self.viewing.place(),
                            })),
                        };
                        out.push(listed.write(file, change));
                        Ok(After::Nothing)
                    }
                    (
                        Annotate::Highlight | Annotate::Comment,
                        Reading::Idle | Reading::Selecting { .. } | Reading::OnHighlight(_),
                    ) => Err(Notice::NothingSelected),
                    (
                        Annotate::Remove,
                        Reading::Idle | Reading::Selecting { .. } | Reading::Selected(_),
                    ) => Err(Notice::NoHighlightPicked),
                }
            }
        }
    }
}

pub fn command(state: &mut State, command: ReadingCommand, out: &mut Vec<Effect>) {
    let after = {
        let State {
            layout,
            pdfs,
            books,
            ..
        } = &mut *state;
        (layout.focused_mut().active_mut())
            .ok_or(Notice::NotReading)
            .and_then(|tab| Reader::of(tab, pdfs, books))
            .and_then(|reader| reader.command(command, out))
    };
    match after {
        Ok(After::Nothing) => {}
        Ok(After::Find) => {
            state.overlay = Overlay::Prompt(prompt::open(PromptKind::Find, &state.project.index))
        }
        Ok(After::FindText(query)) => find(state, query, out),
        Ok(After::Comment(commenting)) => {
            let line = (commenting.highlight.comment.as_ref())
                .map_or(String::new(), |comment| comment.as_str().to_owned());
            let mut prompt = prompt::open(PromptKind::Comment(commenting), &state.project.index);
            prompt.cursor = line.len();
            prompt.line = line;
            state.overlay = Overlay::Prompt(prompt);
        }
        Err(refused) => state.notice = Some(refused),
    }
}

pub fn find(state: &mut State, query: String, out: &mut Vec<Effect>) {
    let State {
        layout,
        pdfs,
        books,
        notice,
        ..
    } = state;
    let reader = (layout.focused_mut().active_mut())
        .ok_or(Notice::NotReading)
        .and_then(|tab| Reader::of(tab, pdfs, books));
    match reader {
        Ok(reader) => {
            *reader.found = Found::Searching {
                query: query.clone(),
            };
            out.push(Effect::Job(Job::Find {
                file: reader.id,
                texts: reader.texts,
                query,
            }));
        }
        Err(refused) => *notice = Some(refused),
    }
}

pub fn found(state: &mut State, file: ReaderId, query: String, hits: Vec<TextSpan>) {
    let State {
        layout,
        pdfs,
        books,
        notice,
        ..
    } = state;
    for (_, pane) in layout.panes_mut() {
        for item in pane.items_mut() {
            let Ok(mut reader) = Reader::of(&mut item.tab, pdfs, books) else {
                continue;
            };
            match &*reader.found {
                Found::Searching { query: asked } if reader.id == file && *asked == query => {}
                Found::Searching { .. } | Found::Nothing | Found::Hits { .. } => continue,
            }
            let from = reader.viewing.place();
            let first_after_view = (hits.iter().position(|hit| hit.start() >= from))
                .or((!hits.is_empty()).then_some(0));
            *reader.found = match first_after_view {
                Some(at) => {
                    reader.viewing.show(hits[at].start());
                    Found::Hits {
                        query: query.clone(),
                        hits: hits.clone(),
                        at,
                    }
                }
                None => {
                    *notice = Some(Notice::NoHits {
                        query: query.clone(),
                    });
                    Found::Nothing
                }
            };
        }
    }
}

pub fn sync(state: &mut State, out: &mut Vec<Effect>) {
    let State {
        layout,
        pdfs,
        books,
        ..
    } = state;
    let Some(Ok(reader)) =
        (layout.focused_mut().active_mut()).map(|tab| Reader::of(tab, pdfs, books))
    else {
        return;
    };
    let (file, at) = (reader.id, reader.viewing.place());
    match file.notes_mut(pdfs, books) {
        Some(Notes::Listed(listed)) if listed.position != at => {
            listed.position = at;
            out.push(Effect::Job(Job::KeepPlace {
                library: listed.library.clone(),
                document: listed.document,
                at,
            }));
        }
        Some(Notes::Listed(_) | Notes::Unlisted(_)) | None => {}
    }
}

pub fn comment(state: &mut State, commenting: Commenting, line: String, out: &mut Vec<Effect>) {
    let Some(notes) = commenting.file.notes(&state.pdfs, &state.books) else {
        return;
    };
    let highlight = Highlight {
        comment: Comment::new(line),
        ..commenting.highlight
    };
    match notes.listed() {
        Ok(listed) => out.push(listed.write(
            commenting.file,
            Change::Put(Annotation::Highlight(highlight)),
        )),
        Err(refused) => state.notice = Some(refused),
    }
}

pub fn arrive(state: &mut State, pane: PaneId, file: ReaderId, id: AnnotationId) {
    let State {
        layout,
        pdfs,
        books,
        ..
    } = state;
    let Some(tab) = layout.find_pane(pane).and_then(|pane| pane.active_mut()) else {
        return;
    };
    let Ok(mut reader) = Reader::of(tab, pdfs, books) else {
        return;
    };
    let annotation = match reader.notes {
        Notes::Listed(listed) => (listed.notebook.in_reading_order().into_iter())
            .find(|annotation| annotation.id() == id),
        Notes::Unlisted(_) => None,
    };
    match (reader.id == file, annotation) {
        (true, Some(annotation)) => reader.show(annotation),
        (true, None) | (false, _) => {}
    }
}

pub fn annotated(
    state: &mut State,
    file: ReaderId,
    document: DocumentId,
    change: Change,
    result: Result<(), LibraryError>,
) {
    let State {
        layout,
        pdfs,
        books,
        notice,
        ..
    } = state;
    let notebook = match (result, file.notes_mut(pdfs, books)) {
        (Err(error), _) => {
            return *notice = Some(Notice::LibraryFailed {
                reason: error.to_string(),
            });
        }
        (Ok(()), Some(Notes::Listed(listed))) if listed.document == document => {
            &mut listed.notebook
        }
        (Ok(()), Some(Notes::Listed(_) | Notes::Unlisted(_)) | None) => return,
    };
    notebook.apply(&change);
    for (_, pane) in layout.panes_mut() {
        for item in pane.items_mut() {
            let reader = match Reader::of(&mut item.tab, pdfs, books) {
                Ok(reader) if reader.id == file => reader,
                Ok(_) | Err(_) => continue,
            };
            let reading = reader.reading;
            *reading = match (std::mem::replace(reading, Reading::Idle), &change) {
                (Reading::OnHighlight(shown), Change::Put(Annotation::Highlight(written)))
                    if shown.id == written.id =>
                {
                    Reading::OnHighlight(written.clone())
                }
                (Reading::Selected(span), Change::Put(Annotation::Highlight(written)))
                    if span == written.span =>
                {
                    Reading::OnHighlight(written.clone())
                }
                (Reading::OnHighlight(shown), Change::Remove(removed)) if shown.id == *removed => {
                    Reading::Idle
                }
                (kept, _) => kept,
            };
        }
    }
}

pub fn notes(
    library: &Library,
    path: &Path,
    hash: FastHash,
    size: u64,
    texts: &dyn Texts,
) -> (Notes, Option<Notice>) {
    match (library.place(path)).map(|place| library.identify(&place, hash, size, texts)) {
        None => (Notes::Unlisted(Unlisted::Outside), None),
        Some(Ok(found)) => {
            let lost = found.notebook.lost.len();
            let said = match (found.identity, lost) {
                (Identity::Changed, 1..) => Some(Notice::HighlightsLost { lost }),
                (
                    Identity::Changed | Identity::Known | Identity::Moved { .. } | Identity::New,
                    _,
                ) => None,
            };
            let listed = Listed {
                library: library.clone(),
                document: found.id,
                notebook: found.notebook,
                position: found.position,
            };
            (Notes::Listed(listed), said)
        }
        Some(Err(error)) => {
            tracing::warn!(?path, %error, "library");
            let reason = error.to_string();
            (
                Notes::Unlisted(Unlisted::Failed(reason.clone())),
                Some(Notice::LibraryFailed { reason }),
            )
        }
    }
}
