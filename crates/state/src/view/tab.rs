//! A tab shows one file, in one of the modes its kind of file offers.

use std::path::Path;

use model::command::{Direction, Resize, Shown};

use crate::git::diffview::DiffSides;
use crate::render::document::{DocumentSeen, DocumentState};
use crate::render::markdown::Format;
use crate::render::pages::{PagesSeen, PagesState};
use crate::render::{Opening, Rendering};
use crate::text::buffer::BufferId;
use crate::view::layout::TextView;

/// A set of modes, one enum per set; each kind of file points at one.
pub trait Modes: Sized + Clone {
    /// `space tab` order, holding every mode `shown` returns; it starts with `first`.
    const ORDER: &'static [Shown];

    /// The mode a tab opens in.
    fn first() -> Self;

    fn shown(&self) -> Shown;

    /// `shown`, a rendering opening `at`; `None` for a mode this set lacks.
    fn enter(shown: Shown, at: Opening) -> Option<Self>;
}

/// A kind of file, by the set of modes its tabs have.
pub trait FileKind {
    type Mode: Modes;
}

/// Text that only shows as text.
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

/// Text that renders, laid out as `R` describes.
#[derive(Clone)]
pub enum MarkupModes<R: Rendering> {
    Source,
    Diff(DiffSides),
    Render(R),
    /// `seen` is `None` until the first sync; `share` is the source side's part of the width.
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
pub struct Code;

impl FileKind for Code {
    type Mode = TextModes;
}

#[derive(Clone)]
pub struct Markdown;

impl FileKind for Markdown {
    type Mode = MarkupModes<DocumentState>;
}

#[derive(Clone)]
pub struct Typst;

impl FileKind for Typst {
    type Mode = MarkupModes<PagesState>;
}

/// A text file's tab: the text every mode shares, and the mode shown.
#[derive(Clone)]
pub struct TextTab<K: FileKind> {
    /// Selection and scroll, kept across modes.
    pub text: TextView,
    pub mode: K::Mode,
}

impl<K: FileKind> TextTab<K> {
    fn new(buffer: BufferId) -> TextTab<K> {
        TextTab {
            text: TextView::new(buffer),
            mode: K::Mode::first(),
        }
    }

    fn enter(&mut self, shown: Shown, at: Opening) -> Result<(), NoMode> {
        self.mode = K::Mode::enter(shown, at).ok_or(NoMode)?;
        Ok(())
    }

    /// The mode `direction` of the current one in `ORDER`, wrapping around.
    fn next(&self, direction: Direction) -> Shown {
        let order = K::Mode::ORDER;
        let at = (order.iter())
            .position(|shown| *shown == self.mode.shown())
            .expect("a mode is in its set's order");
        match direction {
            Direction::Forward => order[(at + 1) % order.len()],
            Direction::Backward => order[(at + order.len() - 1) % order.len()],
        }
    }
}

/// The kind of file has no such mode.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NoMode;

/// A tab, by the kind of file it holds; each kind can only be in its own modes.
#[derive(Clone)]
pub enum Tab {
    Code(TextTab<Code>),
    Markdown(TextTab<Markdown>),
    Typst(TextTab<Typst>),
}

/// Whether a rendering fills its tab, or sits beside the source it follows with where they last
/// agreed.
#[derive(Clone, Copy)]
pub enum Beside<S> {
    Alone,
    Source(Option<S>, Share),
}

pub enum BesideMut<'a, S> {
    Alone,
    Source(&'a mut Option<S>, &'a mut Share),
}

/// What a tab draws, with the state that mode keeps.
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
    /// The text drawn as editable source: in source mode, or beside a rendering.
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

impl Tab {
    /// `buffer`'s tab in its first mode; the kind comes from `path` and never changes.
    pub fn open(buffer: BufferId, path: &Path) -> Tab {
        match Format::of(path) {
            Some(Format::Markdown) => Tab::Markdown(TextTab::new(buffer)),
            Some(Format::Typst) => Tab::Typst(TextTab::new(buffer)),
            None => Tab::Code(TextTab::new(buffer)),
        }
    }

    pub fn buffer(&self) -> BufferId {
        self.text().buffer()
    }

    /// The selection and scroll every mode shares.
    pub fn text(&self) -> &TextView {
        match self {
            Tab::Code(tab) => &tab.text,
            Tab::Markdown(tab) => &tab.text,
            Tab::Typst(tab) => &tab.text,
        }
    }

    pub fn text_mut(&mut self) -> &mut TextView {
        match self {
            Tab::Code(tab) => &mut tab.text,
            Tab::Markdown(tab) => &mut tab.text,
            Tab::Typst(tab) => &mut tab.text,
        }
    }

    pub fn shown(&self) -> Shown {
        match self {
            Tab::Code(tab) => tab.mode.shown(),
            Tab::Markdown(tab) => tab.mode.shown(),
            Tab::Typst(tab) => tab.mode.shown(),
        }
    }

    /// Switches to `shown`, a rendering opening `at`.
    pub fn enter(&mut self, shown: Shown, at: Opening) -> Result<(), NoMode> {
        match self {
            Tab::Code(tab) => tab.enter(shown, at),
            Tab::Markdown(tab) => tab.enter(shown, at),
            Tab::Typst(tab) => tab.enter(shown, at),
        }
    }

    /// The mode `space tab` moves to.
    pub fn next(&self, direction: Direction) -> Shown {
        match self {
            Tab::Code(tab) => tab.next(direction),
            Tab::Markdown(tab) => tab.next(direction),
            Tab::Typst(tab) => tab.next(direction),
        }
    }

    pub fn showing(&self) -> Showing<'_> {
        match self {
            Tab::Code(TextTab { text, mode }) => match mode {
                TextModes::Source => Showing::Source(text),
                TextModes::Diff(sides) => Showing::Diff(text, sides),
            },
            Tab::Markdown(TextTab { text, mode }) => match mode {
                MarkupModes::Source => Showing::Source(text),
                MarkupModes::Diff(sides) => Showing::Diff(text, sides),
                MarkupModes::Render(state) => Showing::Document(text, state, Beside::Alone),
                MarkupModes::Split {
                    render,
                    seen,
                    share,
                } => Showing::Document(text, render, Beside::Source(*seen, *share)),
            },
            Tab::Typst(TextTab { text, mode }) => match mode {
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
            Tab::Code(TextTab { text, mode }) => match mode {
                TextModes::Source => ShowingMut::Source(text),
                TextModes::Diff(sides) => ShowingMut::Diff(text, sides),
            },
            Tab::Markdown(TextTab { text, mode }) => match mode {
                MarkupModes::Source => ShowingMut::Source(text),
                MarkupModes::Diff(sides) => ShowingMut::Diff(text, sides),
                MarkupModes::Render(state) => ShowingMut::Document(text, state, BesideMut::Alone),
                MarkupModes::Split {
                    render,
                    seen,
                    share,
                } => ShowingMut::Document(text, render, BesideMut::Source(seen, share)),
            },
            Tab::Typst(TextTab { text, mode }) => match mode {
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

/// The source side's part of a split's width, strictly between 0 and 1; the host draws each side
/// at least a few columns wide whatever it holds.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Share(f32);

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

    /// One key's resize, stopping a step short of either edge.
    pub fn resized(self, resize: Resize) -> Share {
        match resize {
            Resize::Narrow => Share((self.0 - Share::STEP).max(Share::STEP)),
            Resize::Widen => Share((self.0 + Share::STEP).min(1.0 - Share::STEP)),
            Resize::Even => Share::EVEN,
        }
    }
}
