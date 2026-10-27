//! Reading a PDF: its pages' text, read in jobs as pages come into view.

use std::cmp::Ordering;

use ink::doc::{Paged, Underlay};
use model::notice::Notice;
use model::sheet::SheetView;

use crate::drawing::pen::{PenPhase, PenSample};
use crate::drawing::{Found, InkTabs, PdfId, PdfMode, Reading, TextSpan};
use crate::job::Job;
use crate::view::tab::Tab;
use crate::{Effect, State};

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
        let paged = pdfs[tab.file()].edited.drawing();
        let Underlay::Pdf(pdf) = paged.underlay() else {
            continue;
        };
        let pages = tab.view.frame(paged.stack(), measure.viewport).pages;
        out.extend(
            pages
                .filter_map(|page| pdf.document.claim_text(page))
                .map(Job::PdfText)
                .map(Effect::Job),
        );
    }
}

pub fn pen(
    reading: &mut Reading,
    view: &SheetView,
    paged: &Paged,
    phase: PenPhase,
    sample: PenSample,
) {
    let Underlay::Pdf(pdf) = paged.underlay() else {
        return;
    };
    let stack = paged.stack();
    let frame = view.frame(stack, sample.viewport);
    let nearest = |page: ink::doc::PageIndex| {
        let at = frame.page(stack, page).from_pane.apply(sample.at);
        pdf.document.text(page.0)?.nearest(at)
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
            Ordering::Equal => Reading::Idle,
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
        Some(Tab::Ink(InkTabs::Pdf(tab))) => match pdfs[tab.file()].edited.drawing().underlay() {
            Underlay::Pdf(pdf) => {
                tab.found = Found::Searching {
                    query: query.clone(),
                };
                out.push(Effect::Job(Job::PdfFind {
                    file: tab.file(),
                    document: pdf.document.clone(),
                    query,
                }));
            }
            Underlay::Blank => {}
        },
        Some(Tab::Ink(_) | Tab::Text(_) | Tab::Image(_)) | None => *notice = Some(Notice::NotAPdf),
    }
}

/// The first hit shown is the first at or after the page in view.
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
    let stack = pdf.edited.drawing().stack();
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
            tab.found = match hits
                .iter()
                .position(|hit| hit.page >= from)
                .or((!hits.is_empty()).then_some(0))
            {
                Some(at) => {
                    tab.view.go_to(
                        stack,
                        hits[at].page,
                        tab.hand.last.map(|measure| measure.viewport),
                    );
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
