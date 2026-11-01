use library::Place;
use model::sheet::SheetView;

use crate::drawing::pen::{PenPhase, PenSample};
use crate::drawing::{InkTabs, PdfFile, PdfMode};
use crate::job::Job;
use crate::reading::{Press, ReaderKind, Reading};
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
            Some(Tab::Ink(_) | Tab::Text(_) | Tab::Image(_) | Tab::Book(_)) | None => continue,
        };
        let file = &pdfs[tab.file()];
        let document = file.document();
        let pages = tab.view.frame(file.pages(), measure.viewport).pages;
        out.extend(
            pages
                .filter_map(|page| document.claim_text(page))
                .map(Job::PdfText)
                .map(Effect::Job),
        );
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
    let stack = file.pages();
    let frame = view.frame(stack, sample.viewport);
    let press = match phase {
        PenPhase::Down(_, taps) => Press::Down(taps),
        PenPhase::Move => Press::Move,
        PenPhase::Up => Press::Up,
        PenPhase::Cancel => Press::Cancel,
    };
    let page = match (press, &*reading) {
        (Press::Down(_), _) | (Press::Move | Press::Up, Reading::Selecting { .. }) => {
            frame.hit(stack, sample.at).map(|(page, _)| page.index)
        }
        (Press::Cancel, _)
        | (
            Press::Move | Press::Up,
            Reading::Idle | Reading::Selected(_) | Reading::OnHighlight(_) | Reading::Writing(_),
        ) => None,
    };
    let at = page.and_then(|page| {
        let point = frame.page(stack, page).from_pane.apply(sample.at);
        Some(Place {
            part: library::Part(page.0),
            char: document.text(page.0)?.nearest(point)?,
        })
    });
    *reading = std::mem::replace(reading, Reading::Idle).step(
        press,
        at,
        ReaderKind::Pdf,
        document,
        &file.notes,
    );
}
