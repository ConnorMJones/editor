use std::cmp::Ordering;

use library::{Annotation, AnnotationId, Change, Comment, DocumentId, Highlight, LibraryError};
use model::notice::Notice;
use model::sheet::{PageIndex, SheetView};

use crate::drawing::pen::{PenPhase, PenSample};
use crate::drawing::{Found, InkTabs, Notes, PdfFile, PdfId, PdfMode, Reading, TextSpan};
use crate::job::Job;
use crate::view::layout::PaneId;
use crate::view::tab::Tab;
use crate::{Commenting, Effect, State};

pub fn sync(state: &mut State, out: &mut Vec<Effect>) {
    let State { layout, pdfs, .. } = state;
    for (_, pane) in layout.panes() {
        let (tab, measure) = match pane.active() {
            Some(Tab::Ink(InkTabs::Pdf(tab))) => match (&tab.mode, tab.hand.last) {
                (PdfMode::Read(_), Some(measure)) => (tab, measure),
                (PdfMode::Read(_) | PdfMode::Markup, _) => continue,
            },
            Some(Tab::Ink(_) | Tab::Text(_) | Tab::Image(_)) | None => continue,
        };
        let document = pdfs[tab.file()].document();
        let pages = tab.view.frame(&document.pages, measure.viewport).pages;
        out.extend(
            pages
                .filter_map(|page| document.claim_text(page))
                .map(Job::PdfText)
                .map(Effect::Job),
        );
    }
    let Some(Tab::Ink(InkTabs::Pdf(tab))) = layout.focused().active() else {
        return;
    };
    let page = tab.view.centre.page;
    match &mut pdfs[tab.file()].notes {
        Notes::Listed(listed) if listed.position != page => {
            listed.position = page;
            out.push(Effect::Job(Job::KeepPlace {
                library: listed.library.clone(),
                document: listed.document,
                page,
            }));
        }
        Notes::Listed(_) | Notes::Unlisted(_) => {}
    }
}

pub fn pen(
    reading: &mut Reading,
    view: &SheetView,
    file: &PdfFile,
    phase: PenPhase,
    sample: PenSample,
) {
    let document = file.document();
    let stack = &document.pages;
    let frame = view.frame(stack, sample.viewport);
    let nearest = |page: PageIndex| {
        let at = frame.page(stack, page).from_pane.apply(sample.at);
        document.text(page.0)?.nearest(at)
    };
    *reading = match (phase, std::mem::replace(reading, Reading::Idle)) {
        (PenPhase::Down(_), _) => {
            let found = frame
                .hit(stack, sample.at)
                .and_then(|(page, _)| Some((page.index, nearest(page.index)?)));
            match found {
                Some((page, at)) => Reading::Selecting {
                    page,
                    anchor: at,
                    head: at,
                },
                None => Reading::Idle,
            }
        }
        (PenPhase::Move, Reading::Selecting { page, anchor, head }) => Reading::Selecting {
            page,
            anchor,
            head: nearest(page).unwrap_or(head),
        },
        (PenPhase::Up, Reading::Selecting { page, anchor, head }) => match anchor.cmp(&head) {
            Ordering::Equal => {
                let tapped = match &file.notes {
                    Notes::Listed(listed) => (listed.notebook.highlights.iter().rev())
                        .find(|highlight| {
                            highlight.span.page == page && highlight.span.chars.contains(&anchor)
                        })
                        .cloned(),
                    Notes::Unlisted(_) => None,
                };
                match tapped {
                    Some(highlight) => Reading::OnHighlight(highlight),
                    None => Reading::Idle,
                }
            }
            Ordering::Less => Reading::Selected(TextSpan {
                page,
                chars: anchor..head + 1,
            }),
            Ordering::Greater => Reading::Selected(TextSpan {
                page,
                chars: head..anchor + 1,
            }),
        },
        (PenPhase::Cancel, _) => Reading::Idle,
        (PenPhase::Move | PenPhase::Up, kept) => kept,
    };
}

pub fn find(state: &mut State, query: String, out: &mut Vec<Effect>) {
    let State {
        layout,
        pdfs,
        notice,
        ..
    } = state;
    match layout.focused_mut().active_mut() {
        Some(Tab::Ink(InkTabs::Pdf(tab))) => {
            tab.found = Found::Searching {
                query: query.clone(),
            };
            out.push(Effect::Job(Job::PdfFind {
                file: tab.file(),
                document: pdfs[tab.file()].document().clone(),
                query,
            }));
        }
        Some(Tab::Ink(_) | Tab::Text(_) | Tab::Image(_)) | None => *notice = Some(Notice::NotAPdf),
    }
}

pub fn found(state: &mut State, file: PdfId, query: String, hits: Vec<TextSpan>) {
    let State {
        layout,
        pdfs,
        notice,
        ..
    } = state;
    let Some(pdf) = pdfs.get(file) else {
        return;
    };
    let stack = &pdf.document().pages;
    for (_, pane) in layout.panes_mut() {
        for item in pane.items_mut() {
            let Tab::Ink(InkTabs::Pdf(tab)) = &mut item.tab else {
                continue;
            };
            match &tab.found {
                Found::Searching { query: asked } if tab.file() == file && *asked == query => {}
                Found::Searching { .. } | Found::Nothing | Found::Hits { .. } => continue,
            }
            let from = tab.view.centre.page;
            let first_after_view =
                (hits.iter().position(|hit| hit.page >= from)).or((!hits.is_empty()).then_some(0));
            tab.found = match first_after_view {
                Some(at) => {
                    tab.show_page(stack, hits[at].page);
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

pub fn comment(state: &mut State, commenting: Commenting, line: String, out: &mut Vec<Effect>) {
    let Some(file) = state.pdfs.get(commenting.file) else {
        return;
    };
    let highlight = Highlight {
        comment: Comment::new(line),
        ..commenting.highlight
    };
    match file.notes.listed() {
        Ok(listed) => out.push(listed.write(
            commenting.file,
            Change::Put(Annotation::Highlight(highlight)),
        )),
        Err(refused) => state.notice = Some(refused),
    }
}

pub fn arrive(state: &mut State, pane: PaneId, file: PdfId, id: AnnotationId) {
    let State { layout, pdfs, .. } = state;
    let (Some(pdf), Some(Tab::Ink(InkTabs::Pdf(tab)))) = (
        pdfs.get(file),
        layout.find_pane(pane).and_then(|pane| pane.active_mut()),
    ) else {
        return;
    };
    let annotation = match &pdf.notes {
        Notes::Listed(listed) => (listed.notebook.in_reading_order().into_iter())
            .find(|annotation| annotation.id() == id),
        Notes::Unlisted(_) => None,
    };
    match (tab.file() == file, annotation) {
        (true, Some(annotation)) => tab.show(&pdf.document().pages, annotation),
        (true, None) | (false, _) => {}
    }
}

pub fn annotated(
    state: &mut State,
    file: PdfId,
    document: DocumentId,
    change: Change,
    result: Result<(), LibraryError>,
) {
    let State {
        layout,
        pdfs,
        notice,
        ..
    } = state;
    let notebook = match (result, pdfs.get_mut(file).map(|pdf| &mut pdf.notes)) {
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
            let reading = match &mut item.tab {
                Tab::Ink(InkTabs::Pdf(tab)) if tab.file() == file => match &mut tab.mode {
                    PdfMode::Read(reading) => reading,
                    PdfMode::Markup => continue,
                },
                Tab::Ink(_) | Tab::Text(_) | Tab::Image(_) => continue,
            };
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
