use std::io;
use std::path::{Path, PathBuf};

use helix_core::Transaction;
use library::{Annotation, Notebook, Part, Texts};
use model::notice::Notice;

use crate::apply::map_views;
use crate::job::Job;
use crate::reading::ReaderId;
use crate::view::tab::Opened;
use crate::{Effect, State};

pub const START: &str = "<!-- edt:annotations -->";
pub const END: &str = "<!-- /edt:annotations -->";

pub enum Places {
    Pages {
        chapters: Vec<usize>,
    },
    Chapters {
        starts: Vec<usize>,
        parts: Vec<usize>,
        total: usize,
    },
}

pub struct Source<'a> {
    pub title: &'a str,
    pub authors: &'a [String],
    pub file: &'a str,
    pub places: Places,
}

#[derive(Debug, thiserror::Error)]
pub enum SpliceError {
    #[error("the note has no {START} line to write under")]
    NoMarkers,
    #[error("the note opens {START} but never closes it with {END}")]
    Unclosed,
}

#[derive(Debug, thiserror::Error)]
pub enum ExportNotesError {
    #[error(transparent)]
    Splice(#[from] SpliceError),
    #[error(transparent)]
    Io(#[from] io::Error),
}

pub fn section(source: &Source, notebook: &Notebook) -> String {
    let mut out = format!("{START}\n# {}\n", source.title);
    match source.authors {
        [] => {}
        authors => out.push_str(&format!("{}\n", authors.join(", "))),
    }
    let block = |name: &str, text: &str| format!("<!-- {name} -->\n{text}\n<!-- /{name} -->\n");
    for annotation in notebook.in_reading_order() {
        let (label, at, body) = match &annotation {
            Annotation::Bookmark(bookmark) => ("bm", bookmark.at, String::new()),
            Annotation::Highlight(highlight) => match &highlight.comment {
                None => (
                    "hl",
                    highlight.place(),
                    block("quote", highlight.quote.exact()),
                ),
                Some(comment) => (
                    "qt",
                    highlight.place(),
                    format!(
                        "{}{}",
                        block("quote", highlight.quote.exact()),
                        block("comment", comment.as_str())
                    ),
                ),
            },
            Annotation::Lost(_) => continue,
        };
        let (pieces, locate) = match &source.places {
            Places::Pages { chapters } => {
                let page = at.part.0 + 1;
                let pieces = match chapters.partition_point(|&start| start <= at.part.0) {
                    0 => vec![format!("pg {page}")],
                    chapter => vec![format!("chp {chapter}"), format!("pg {page}")],
                };
                (pieces, format!("page={page}&"))
            }
            Places::Chapters {
                starts,
                parts,
                total,
            } => {
                let at = parts.get(at.part.0).copied().unwrap_or(*total) + at.char;
                let chapter = starts.partition_point(|&start| start <= at);
                let from = chapter.checked_sub(1).map_or(0, |before| starts[before]);
                let to = starts.get(chapter).copied().unwrap_or(*total);
                let percent = (at - from) * 100 / (to - from).max(1);
                match chapter {
                    0 => (vec![format!("{percent}%")], String::new()),
                    chapter => (
                        vec![format!("chp {chapter}"), format!("{percent}%")],
                        format!("chapter={chapter}&"),
                    ),
                }
            }
        };
        let place = (pieces.iter())
            .map(|piece| format!("**{piece}**"))
            .collect::<Vec<_>>()
            .join(" - ");
        let link = format!(
            "[[{}#{locate}note={}|{}]]",
            source.file,
            annotation.id().0,
            pieces.join(" - ")
        );
        out.push_str(&format!("\n[{label}] {place}\n{link}\n{body}"));
    }
    out.push_str(END);
    out.push('\n');
    out
}

pub fn splice(existing: Option<&str>, section: &str) -> Result<String, SpliceError> {
    let Some(existing) = existing else {
        return Ok(section.to_owned());
    };
    let start = existing.find(START).ok_or(SpliceError::NoMarkers)?;
    let end = (existing[start..].find(END))
        .map(|end| start + end + END.len())
        .ok_or(SpliceError::Unclosed)?;
    let after = existing[end..]
        .strip_prefix('\n')
        .unwrap_or(&existing[end..]);
    Ok(format!("{}{section}{after}", &existing[..start]))
}

pub fn export(state: &mut State, file: ReaderId, out: &mut Vec<Effect>) {
    let (document, title, authors, places, notes) = match file {
        ReaderId::Pdf(id) => {
            let pdf = &state.pdfs[id];
            let about = pdf.document().about();
            let mut chapters: Vec<usize> = (pdf.document().outline().into_iter())
                .filter(|heading| heading.depth == 0)
                .filter_map(|heading| heading.page)
                .collect();
            chapters.sort_unstable();
            (
                pdf.ink.path.clone(),
                about.title,
                about.authors,
                Places::Pages { chapters },
                &pdf.notes,
            )
        }
        ReaderId::Book(id) => {
            let book = &state.books[id];
            let texts = &book.flow.texts;
            let mut total = 0;
            let parts: Vec<usize> = (0..texts.parts())
                .map(|part| {
                    let start = total;
                    total += texts.part(Part(part)).map_or(0, |text| text.len());
                    start
                })
                .collect();
            let mut starts: Vec<usize> = (book.contents.iter())
                .filter(|entry| entry.depth == 0)
                .map(|entry| book.flow.place_of(book.flow.item(&entry.at)))
                .map(|place| parts.get(place.part.0).copied().unwrap_or(total) + place.char)
                .collect();
            starts.sort_unstable();
            starts.dedup();
            (
                book.path.to_path_buf(),
                Some(book.title.clone()),
                book.authors.clone(),
                Places::Chapters {
                    starts,
                    parts,
                    total,
                },
                &book.notes,
            )
        }
    };
    let notebook = match notes.listed() {
        Ok(listed) => &listed.notebook,
        Err(refused) => return state.notice = Some(refused),
    };
    match (
        notebook.highlights.as_slice(),
        notebook.bookmarks.as_slice(),
    ) {
        ([], []) => return state.notice = Some(Notice::NothingToExport),
        _ => {}
    }
    let name = document
        .file_name()
        .map_or(String::new(), |name| name.to_string_lossy().into());
    let stem = document
        .file_stem()
        .map_or(String::new(), |stem| stem.to_string_lossy().into());
    let section = section(
        &Source {
            title: title.as_deref().unwrap_or(&stem),
            authors: &authors,
            file: &name,
            places,
        },
        notebook,
    );
    let note = document.with_extension("md");
    let open = (state.buffers.iter())
        .find(|(_, buffer)| buffer.path == note)
        .map(|(id, _)| id);
    match open {
        Some(buffer) => {
            let State {
                layout,
                buffers,
                languages,
                notice,
                ..
            } = state;
            let doc = &mut buffers[buffer].doc;
            let text = match splice(Some(&doc.text.to_string()), &section) {
                Ok(text) => text,
                Err(error) => {
                    return *notice = Some(Notice::ExportFailed {
                        path: note,
                        reason: error.to_string(),
                    });
                }
            };
            let selection = layout.selection_of(buffer);
            doc.commit(&selection);
            let whole = doc.text.len_chars();
            let tx = Transaction::change(
                &doc.text,
                [(0, whole, Some(text.as_str().into()))].into_iter(),
            );
            doc.apply(&tx, &selection, languages);
            map_views(layout, buffer, tx.changes(), doc.text.slice(..));
            layout.keep(Opened::Text(buffer));
            doc.commit(&selection.map(tx.changes()));
            *notice = Some(Notice::Exported {
                path: note,
                bytes: text.len(),
            });
        }
        None => out.push(Effect::Job(Job::ExportNotes { note, section })),
    }
}

pub fn write(note: &Path, section: &str) -> Result<usize, ExportNotesError> {
    let existing = match std::fs::read_to_string(note) {
        Ok(text) => Some(text),
        Err(error) if error.kind() == io::ErrorKind::NotFound => None,
        Err(error) => return Err(error.into()),
    };
    let text = splice(existing.as_deref(), section)?;
    std::fs::write(note, &text)?;
    Ok(text.len())
}

pub fn exported(state: &mut State, note: PathBuf, result: Result<usize, ExportNotesError>) {
    state.notice = Some(match result {
        Ok(bytes) => Notice::Exported { path: note, bytes },
        Err(error) => Notice::ExportFailed {
            path: note,
            reason: error.to_string(),
        },
    });
}
