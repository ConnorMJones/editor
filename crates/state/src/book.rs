use std::collections::HashSet;
use std::fs::File;
use std::path::Path;
use std::sync::Arc;

use library::{FastHash, Library};
use model::command::ReadMove;
use model::notice::Notice;
use reader::epub::{Book, Contents, EpubError};
use slotmap::{SlotMap, new_key_type};

use crate::reading::{Found, Notes, Press, Reading};
use crate::render::book::{BookFlow, BookTarget, convert};
use crate::render::document::{Placed, TopItem};
use crate::render::flow::{ImagePlace, LocalImage};
use crate::render::typeset::{Probe, each_image};
use crate::text::buffer::Disk;
use crate::view::layout::{Pane, PaneId};
use crate::view::tab::Tab;
use crate::viewer;
use crate::{Effect, State};

new_key_type! {
    pub struct BookId;
}

pub struct BookFile {
    pub path: Arc<Path>,
    pub title: String,
    pub contents: Vec<Contents>,
    pub flow: BookFlow,
    pub notes: Notes,
    pub disk: Disk,
    pub version: u64,
}

pub type BookFiles = SlotMap<BookId, BookFile>;

pub struct BookRead {
    pub title: String,
    pub contents: Vec<Contents>,
    pub flow: BookFlow,
    pub images: Vec<(LocalImage, Probe)>,
    pub notes: Notes,
    pub said: Option<Notice>,
}

#[derive(Debug, thiserror::Error)]
pub enum BookError {
    #[error(transparent)]
    Epub(#[from] EpubError),
    #[error("the book has no text")]
    Blank,
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

pub struct BookTab {
    file: BookId, // private: a tab's file is its identity
    pub scroll: Placed,
    pub reading: Reading,
    pub found: Found,
}

impl BookTab {
    pub fn new(file: BookId, scroll: Placed) -> BookTab {
        BookTab {
            file,
            scroll,
            reading: Reading::Idle,
            found: Found::Nothing,
        }
    }

    pub fn file(&self) -> BookId {
        self.file
    }

    pub fn copy(&self) -> BookTab {
        BookTab::new(self.file, self.scroll)
    }
}

impl BookFile {
    pub fn new(path: &Path, read: BookRead, disk: Disk) -> BookFile {
        BookFile {
            path: path.into(),
            title: read.title,
            contents: read.contents,
            flow: read.flow,
            notes: read.notes,
            disk,
            version: 0,
        }
    }

    pub fn replace(&mut self, read: BookRead, disk: Disk) {
        self.title = read.title;
        self.contents = read.contents;
        self.flow = read.flow;
        self.notes = read.notes;
        self.disk = disk;
        self.version += 1;
    }
}

pub fn read(path: &Path, file: File, library: &Library) -> Result<BookRead, BookError> {
    let metadata = file.metadata()?;
    let modified = metadata.modified()?;
    let hash = FastHash::read(&file)?;
    let book = Book::open(file)?;
    let flow = convert(&book, &Arc::from(path)).ok_or(BookError::Blank)?;
    let mut seen = HashSet::new();
    let mut images = Vec::new();
    each_image(&flow.flow.blocks, |local| {
        match seen.insert(local.clone()) {
            true => {
                let probed = match &local.place {
                    ImagePlace::Zipped { name, .. } => {
                        viewer::probe_zipped(&book.archive, name, local.encoding, modified)
                    }
                    ImagePlace::File(path) => viewer::read(path, local.encoding),
                };
                let probe = match probed {
                    Ok(probed) => Probe::Read(probed),
                    Err(error) => Probe::Failed(Arc::from(error.to_string())),
                };
                images.push((local.clone(), probe))
            }
            false => {}
        }
    });
    let (notes, said) = crate::reading::notes(library, path, hash, metadata.len(), &*flow.texts);
    Ok(BookRead {
        title: book.title,
        contents: book.contents,
        flow,
        images,
        notes,
        said,
    })
}

pub fn moved(state: &mut State, motion: ReadMove) {
    let State { layout, books, .. } = state;
    match layout.focused_mut().active_mut() {
        Some(Tab::Book(tab)) => {
            let items = books[tab.file].flow.flow.roots.len();
            tab.scroll = tab.scroll.moved(motion, items)
        }
        Some(Tab::Text(_) | Tab::Ink(_) | Tab::Image(_)) | None => {}
    }
}

pub fn follow(
    state: &mut State,
    pane: PaneId,
    file: BookId,
    target: &BookTarget,
    out: &mut Vec<Effect>,
) {
    match target {
        BookTarget::Url(url) => out.push(Effect::OpenUrl(url.clone())),
        BookTarget::Spot(spot) => {
            let State { layout, books, .. } = state;
            match layout.find_pane(pane).and_then(Pane::active_mut) {
                Some(Tab::Book(tab)) if tab.file == file => {
                    tab.scroll = Placed::item(books[file].flow.item(spot))
                }
                Some(Tab::Book(_) | Tab::Text(_) | Tab::Ink(_) | Tab::Image(_)) | None => {}
            }
        }
    }
}

pub fn scrolled(state: &mut State, pane: PaneId, file: BookId, to: TopItem) {
    match state.layout.find_pane(pane).and_then(Pane::active_mut) {
        Some(Tab::Book(tab)) if tab.file == file => tab.scroll = Placed::At(to),
        Some(Tab::Book(_) | Tab::Text(_) | Tab::Ink(_) | Tab::Image(_)) | None => {}
    }
}

pub struct Pressed {
    pub block: usize,
    pub byte: usize,
}

pub fn press(state: &mut State, pane: PaneId, file: BookId, at: Option<Pressed>, press: Press) {
    let State { layout, books, .. } = state;
    match press {
        Press::Down => match layout.set_focus(pane) {
            Ok(()) => {}
            Err(_) => return,
        },
        Press::Move | Press::Up | Press::Cancel => {}
    }
    match (
        layout.find_pane(pane).and_then(Pane::active_mut),
        books.get(file),
    ) {
        (Some(Tab::Book(tab)), Some(book)) if tab.file == file => {
            let at = at.and_then(|at| book.flow.point(at.block, at.byte));
            let reading = std::mem::replace(&mut tab.reading, Reading::Idle);
            tab.reading = reading.step(press, at, &*book.flow.texts, &book.notes);
        }
        (Some(_), _) | (None, _) => {}
    }
}
