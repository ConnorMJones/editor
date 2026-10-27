//! A tab shows one file, in one of the modes its kind of file offers.

use model::command::{Direction, Resize, Shown};

use crate::drawing::{self, BoardId, InkTabs, PagedId, PdfId};
use crate::git::diffview::DiffSides;
use crate::project::path::TextKind;
use crate::render::document::{DocumentSeen, DocumentState};
use crate::render::pages::{PagesSeen, PagesState};
use crate::render::{Opening, Rendering};
use crate::text::buffer::BufferId;
use crate::view::layout::TextView;
use crate::viewer::{ImageId, ImageTab};

pub trait Modes: Sized + Clone {
    const ORDER: &'static [Shown];

    fn first() -> Self;

    fn shown(&self) -> Shown;

    fn enter(shown: Shown, at: Opening) -> Option<Self>;
}

#[derive(Clone)]
pub enum TextModes {
    Source,
    Diff(DiffSides),
}

impl Modes for TextModes {
    const ORDER: &'static [Shown] = &[Shown::Source, Shown::Diff];

    fn first() -> TextModes {
        TextModes::Source
    }

    fn shown(&self) -> Shown {
        match self {
            TextModes::Source => Shown::Source,
            TextModes::Diff(_) => Shown::Diff,
        }
    }

    fn enter(shown: Shown, _: Opening) -> Option<TextModes> {
        match shown {
            Shown::Source => Some(TextModes::Source),
            Shown::Diff => Some(TextModes::Diff(DiffSides::new())),
            Shown::Render | Shown::Split => None,
        }
    }
}

#[derive(Clone)]
pub enum MarkupModes<R: Rendering> {
    Source,
    Diff(DiffSides),
    Render(R),
    Split {
        render: R,
        seen: Option<R::Seen>,
        share: Share,
    },
}

impl<R: Rendering> Modes for MarkupModes<R> {
    const ORDER: &'static [Shown] = &[Shown::Source, Shown::Split, Shown::Render, Shown::Diff];

    fn first() -> MarkupModes<R> {
        MarkupModes::Source
    }

    fn shown(&self) -> Shown {
        match self {
            MarkupModes::Source => Shown::Source,
            MarkupModes::Diff(_) => Shown::Diff,
            MarkupModes::Render(_) => Shown::Render,
            MarkupModes::Split { .. } => Shown::Split,
        }
    }

    fn enter(shown: Shown, at: Opening) -> Option<MarkupModes<R>> {
        match shown {
            Shown::Source => Some(MarkupModes::Source),
            Shown::Diff => Some(MarkupModes::Diff(DiffSides::new())),
            Shown::Render => Some(MarkupModes::Render(R::open(at))),
            Shown::Split => Some(MarkupModes::Split {
                render: R::open(at),
                seen: None,
                share: Share::EVEN,
            }),
        }
    }
}

#[derive(Clone)]
pub struct TextTab<M: Modes> {
    pub text: TextView,
    pub mode: M,
}

impl<M: Modes> TextTab<M> {
    fn new(buffer: BufferId) -> TextTab<M> {
        TextTab {
            text: TextView::new(buffer),
            mode: M::first(),
        }
    }

    fn enter(&mut self, shown: Shown, at: Opening) -> Result<(), NoMode> {
        self.mode = M::enter(shown, at).ok_or(NoMode)?;
        Ok(())
    }

    fn next(&self, direction: Direction) -> Shown {
        let order = M::ORDER;
        let at = (order.iter())
            .position(|shown| *shown == self.mode.shown())
            .expect("a mode is in its set's order");
        match direction {
            Direction::Forward => order[(at + 1) % order.len()],
            Direction::Backward => order[(at + order.len() - 1) % order.len()],
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NoMode;

#[derive(Clone)]
pub enum TextTabs {
    Code(TextTab<TextModes>),
    Markdown(TextTab<MarkupModes<DocumentState>>),
    Typst(TextTab<MarkupModes<PagesState>>),
}

#[derive(Clone, Copy)]
pub enum Beside<S> {
    Alone,
    Source(Option<S>, Share),
}

pub enum BesideMut<'a, S> {
    Alone,
    Source(&'a mut Option<S>, &'a mut Share),
}

#[derive(Clone, Copy)]
pub enum Showing<'a> {
    Source(&'a TextView),
    Diff(&'a TextView, &'a DiffSides),
    Document(&'a TextView, &'a DocumentState, Beside<DocumentSeen>),
    Pages(&'a TextView, &'a PagesState, Beside<PagesSeen>),
}

pub enum ShowingMut<'a> {
    Source(&'a mut TextView),
    Diff(&'a mut TextView, &'a mut DiffSides),
    Document(
        &'a mut TextView,
        &'a mut DocumentState,
        BesideMut<'a, DocumentSeen>,
    ),
    Pages(
        &'a mut TextView,
        &'a mut PagesState,
        BesideMut<'a, PagesSeen>,
    ),
}

impl<'a> Showing<'a> {
    pub fn source(&self) -> Option<&'a TextView> {
        match self {
            Showing::Source(text)
            | Showing::Document(text, _, Beside::Source(..))
            | Showing::Pages(text, _, Beside::Source(..)) => Some(text),
            Showing::Diff(..)
            | Showing::Document(_, _, Beside::Alone)
            | Showing::Pages(_, _, Beside::Alone) => None,
        }
    }
}

impl<'a> ShowingMut<'a> {
    pub fn source(self) -> Option<&'a mut TextView> {
        match self {
            ShowingMut::Source(text)
            | ShowingMut::Document(text, _, BesideMut::Source(..))
            | ShowingMut::Pages(text, _, BesideMut::Source(..)) => Some(text),
            ShowingMut::Diff(..)
            | ShowingMut::Document(_, _, BesideMut::Alone)
            | ShowingMut::Pages(_, _, BesideMut::Alone) => None,
        }
    }
}

impl TextTabs {
    pub fn open(buffer: BufferId, kind: TextKind) -> TextTabs {
        match kind {
            TextKind::Code => TextTabs::Code(TextTab::new(buffer)),
            TextKind::Markdown => TextTabs::Markdown(TextTab::new(buffer)),
            TextKind::Typst => TextTabs::Typst(TextTab::new(buffer)),
        }
    }

    pub fn buffer(&self) -> BufferId {
        self.text().buffer()
    }

    pub fn text(&self) -> &TextView {
        match self {
            TextTabs::Code(tab) => &tab.text,
            TextTabs::Markdown(tab) => &tab.text,
            TextTabs::Typst(tab) => &tab.text,
        }
    }

    pub fn text_mut(&mut self) -> &mut TextView {
        match self {
            TextTabs::Code(tab) => &mut tab.text,
            TextTabs::Markdown(tab) => &mut tab.text,
            TextTabs::Typst(tab) => &mut tab.text,
        }
    }

    pub fn shown(&self) -> Shown {
        match self {
            TextTabs::Code(tab) => tab.mode.shown(),
            TextTabs::Markdown(tab) => tab.mode.shown(),
            TextTabs::Typst(tab) => tab.mode.shown(),
        }
    }

    pub fn enter(&mut self, shown: Shown, at: Opening) -> Result<(), NoMode> {
        match self {
            TextTabs::Code(tab) => tab.enter(shown, at),
            TextTabs::Markdown(tab) => tab.enter(shown, at),
            TextTabs::Typst(tab) => tab.enter(shown, at),
        }
    }

    pub fn next(&self, direction: Direction) -> Shown {
        match self {
            TextTabs::Code(tab) => tab.next(direction),
            TextTabs::Markdown(tab) => tab.next(direction),
            TextTabs::Typst(tab) => tab.next(direction),
        }
    }

    pub fn showing(&self) -> Showing<'_> {
        match self {
            TextTabs::Code(TextTab { text, mode }) => match mode {
                TextModes::Source => Showing::Source(text),
                TextModes::Diff(sides) => Showing::Diff(text, sides),
            },
            TextTabs::Markdown(TextTab { text, mode }) => match mode {
                MarkupModes::Source => Showing::Source(text),
                MarkupModes::Diff(sides) => Showing::Diff(text, sides),
                MarkupModes::Render(state) => Showing::Document(text, state, Beside::Alone),
                MarkupModes::Split {
                    render,
                    seen,
                    share,
                } => Showing::Document(text, render, Beside::Source(*seen, *share)),
            },
            TextTabs::Typst(TextTab { text, mode }) => match mode {
                MarkupModes::Source => Showing::Source(text),
                MarkupModes::Diff(sides) => Showing::Diff(text, sides),
                MarkupModes::Render(state) => Showing::Pages(text, state, Beside::Alone),
                MarkupModes::Split {
                    render,
                    seen,
                    share,
                } => Showing::Pages(text, render, Beside::Source(*seen, *share)),
            },
        }
    }

    pub fn showing_mut(&mut self) -> ShowingMut<'_> {
        match self {
            TextTabs::Code(TextTab { text, mode }) => match mode {
                TextModes::Source => ShowingMut::Source(text),
                TextModes::Diff(sides) => ShowingMut::Diff(text, sides),
            },
            TextTabs::Markdown(TextTab { text, mode }) => match mode {
                MarkupModes::Source => ShowingMut::Source(text),
                MarkupModes::Diff(sides) => ShowingMut::Diff(text, sides),
                MarkupModes::Render(state) => ShowingMut::Document(text, state, BesideMut::Alone),
                MarkupModes::Split {
                    render,
                    seen,
                    share,
                } => ShowingMut::Document(text, render, BesideMut::Source(seen, share)),
            },
            TextTabs::Typst(TextTab { text, mode }) => match mode {
                MarkupModes::Source => ShowingMut::Source(text),
                MarkupModes::Diff(sides) => ShowingMut::Diff(text, sides),
                MarkupModes::Render(state) => ShowingMut::Pages(text, state, BesideMut::Alone),
                MarkupModes::Split {
                    render,
                    seen,
                    share,
                } => ShowingMut::Pages(text, render, BesideMut::Source(seen, share)),
            },
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Share(f32); // private: checked by its constructor

#[derive(Debug, thiserror::Error)]
#[error("split share {0} is not strictly between 0 and 1")]
pub struct InvalidShare(pub f32);

impl Share {
    pub const EVEN: Share = Share(0.5);
    const STEP: f32 = 0.05;

    pub fn new(share: f32) -> Result<Share, InvalidShare> {
        match share > 0.0 && share < 1.0 {
            true => Ok(Share(share)),
            false => Err(InvalidShare(share)),
        }
    }

    pub fn get(self) -> f32 {
        self.0
    }

    pub fn resized(self, resize: Resize) -> Share {
        match resize {
            Resize::Narrow => Share((self.0 - Share::STEP).max(Share::STEP)),
            Resize::Widen => Share((self.0 + Share::STEP).min(1.0 - Share::STEP)),
            Resize::Even => Share::EVEN,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Opened {
    Text(BufferId),
    Paged(PagedId),
    Board(BoardId),
    Pdf(PdfId),
    Image(ImageId),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Writable {
    Text(BufferId),
    Paged(PagedId),
    Board(BoardId),
}

impl Opened {
    pub fn writable(self) -> Option<Writable> {
        match self {
            Opened::Text(buffer) => Some(Writable::Text(buffer)),
            Opened::Paged(id) => Some(Writable::Paged(id)),
            Opened::Board(id) => Some(Writable::Board(id)),
            // Until markup is written into the PDF itself.
            Opened::Pdf(_) => None,
            Opened::Image(_) => None,
        }
    }
}

impl From<Writable> for Opened {
    fn from(file: Writable) -> Opened {
        match file {
            Writable::Text(buffer) => Opened::Text(buffer),
            Writable::Paged(id) => Opened::Paged(id),
            Writable::Board(id) => Opened::Board(id),
        }
    }
}

pub enum Tab {
    Text(TextTabs),
    Ink(InkTabs),
    Image(ImageTab),
}

impl Tab {
    pub fn opened(&self) -> Opened {
        match self {
            Tab::Text(text) => Opened::Text(text.buffer()),
            Tab::Ink(ink) => ink.opened(),
            Tab::Image(image) => Opened::Image(image.file()),
        }
    }

    pub fn as_text(&self) -> Option<&TextTabs> {
        match self {
            Tab::Text(text) => Some(text),
            Tab::Ink(_) | Tab::Image(_) => None,
        }
    }

    pub fn as_text_mut(&mut self) -> Option<&mut TextTabs> {
        match self {
            Tab::Text(text) => Some(text),
            Tab::Ink(_) | Tab::Image(_) => None,
        }
    }
}

impl Tab {
    pub fn open(opened: Opened, state: &crate::State) -> Tab {
        match opened {
            Opened::Text(buffer) => Tab::Text(TextTabs::open(buffer, state.buffers[buffer].kind)),
            Opened::Paged(id) => Tab::Ink(drawing::open_paged(state, id)),
            Opened::Board(id) => Tab::Ink(drawing::open_board(state, id)),
            Opened::Pdf(id) => Tab::Ink(drawing::open_pdf(state, id)),
            Opened::Image(id) => Tab::Image(ImageTab::open(id)),
        }
    }

    pub fn split_copy(&self) -> Tab {
        match self {
            Tab::Text(text) => Tab::Text(text.clone()),
            Tab::Ink(ink) => Tab::Ink(ink.copy()),
            Tab::Image(image) => Tab::Image(image.copy()),
        }
    }
}
