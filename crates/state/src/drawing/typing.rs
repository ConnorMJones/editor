//! Text marks being typed: the text is a plain `TextDoc`, edited by the same commands as any
//! text, and laid out into a mark when it changes. Where text goes is the text tool's, in ink.

use helix_core::{Rope, Selection, smallvec};
use ink::brush::Paint;
use ink::doc::{Drawing, Ink, Mark, MarkChange, Placement};
use ink::geom::{Point, Surface, Vector};
use ink::text::{Fonts, Inked, Laid, TextStyle, Wrap};
use ink::tool::{Frame, Picked, Rewrapped, TextMark, TypeAt};
use model::notice::Notice;

use crate::State;
use crate::drawing::command::{add, commit};
use crate::drawing::{InkFile, InkTabs};
use crate::text::buffer::TextDoc;
use crate::view::tab::Tab;

pub struct Open<T> {
    pub target: T,
    pub writing: Writing,
    pub text: TextBox,
}

/// Whether finishing adds a mark or replaces the one being edited, which is hidden meanwhile.
#[derive(Clone, Debug)]
pub enum Writing {
    New,
    Replacing(TextMark),
}

/// Text being typed, where it sits and how it looks, and its layout as of `laid_version`.
pub struct TextBox {
    pub doc: TextDoc,
    pub selection: Selection,
    pub placement: Placement,
    pub style: TextStyle,
    pub wrap: Wrap,
    pub paint: Paint,
    pub laid: Laid,
    /// The `TextDoc::version` `laid` was laid out from.
    pub laid_version: u64,
}

impl TextBox {
    pub fn new(
        text: &str,
        placement: Placement,
        style: TextStyle,
        wrap: Wrap,
        paint: Paint,
        fonts: &Fonts,
    ) -> TextBox {
        let doc = TextDoc::plain(Rope::from(text));
        TextBox {
            laid: fonts.lay_out(text, &style, wrap),
            laid_version: doc.version,
            selection: Selection::point(doc.text.len_chars()),
            doc,
            placement,
            style,
            wrap,
            paint,
        }
    }

    pub fn frame(&self) -> Frame {
        Frame {
            rect: self.laid.area,
            placement: self.placement,
        }
    }

    pub fn cursor_to(&mut self, at: Point<Surface>) {
        let at = self.placement.back().apply(at);
        let off_line = |y: f32, height: f32| (y - at.y).max(at.y - y - height).max(0.0);
        let nearest = (self.laid.carets.iter().enumerate())
            .min_by(|(_, a), (_, b)| {
                (off_line(a.at.y, a.height).total_cmp(&off_line(b.at.y, b.height)))
                    .then((a.at.x - at.x).abs().total_cmp(&(b.at.x - at.x).abs()))
            })
            .map_or(0, |(index, _)| index);
        self.selection = Selection::point(nearest);
    }
}

/// Text to type where the text tool asked: new text in the hand's style, or a text mark's.
pub fn open<T>(at: TypeAt<T>, style: &TextStyle, paint: Paint, fonts: &Fonts) -> Open<T> {
    match at {
        TypeAt::New { target, at, wrap } => Open {
            target,
            writing: Writing::New,
            text: TextBox::new("", Placement::at(at), style.clone(), wrap, paint, fonts),
        },
        TypeAt::Mark { target, mark } => Open {
            target,
            text: TextBox::new(
                &mark.text.text,
                mark.placement,
                mark.text.style.clone(),
                mark.text.wrap,
                mark.paint,
                fonts,
            ),
            writing: Writing::Replacing(mark),
        },
    }
}

/// Ends the text being typed, if any: blank text adds nothing and removes the mark it edited, and
/// changed text becomes one step of the drawing's history.
pub fn finish<D: Drawing>(
    typing: &mut Option<Box<Open<D::Target>>>,
    file: &mut InkFile<D>,
    fonts: &Fonts,
) -> Result<Ended<D::Target>, Notice> {
    let Some(open) = typing.take() else {
        return Ok(Ended::NothingOpen);
    };
    let Open {
        target,
        writing,
        text,
    } = *open;
    let typed = String::from(&text.doc.text);
    let typed_mark = |id, ink| TextMark {
        id,
        text: ink,
        placement: text.placement,
        paint: text.paint,
    };
    let kept = |mark| Ended::Kept {
        target,
        picked: Picked::Text(mark),
    };
    match (writing, fonts.lay_out(&typed, &text.style, text.wrap).inked) {
        (Writing::New, Inked::Blank) => Ok(Ended::Dropped),
        (Writing::New, Inked::Ink(ink)) => {
            let mark = Mark {
                placement: text.placement,
                paint: text.paint,
                ink: Ink::Text(ink.clone()),
            };
            let id = add(file, target, mark)?;
            Ok(kept(typed_mark(id, ink)))
        }
        (Writing::Replacing(was), Inked::Blank) => {
            commit(file, MarkChange::Remove(target, smallvec![was.id]).into())
                .map(|()| Ended::Dropped)
        }
        (Writing::Replacing(was), Inked::Ink(ink)) if was.text.text == ink.text => Ok(kept(was)),
        (Writing::Replacing(was), Inked::Ink(ink)) => {
            let mark = typed_mark(was.id, ink);
            commit(
                file,
                MarkChange::Replace(target, mark.id, mark.mark()).into(),
            )?;
            Ok(kept(mark))
        }
    }
}

pub fn rewrap<D: Drawing>(
    file: &mut InkFile<D>,
    fonts: &Fonts,
    target: D::Target,
    mark: TextMark,
    to: Rewrapped,
) -> Result<Picked, Notice> {
    let rewrapped = TextMark {
        text: fonts.rewrap(&mark.text, Wrap::At(to.width)),
        placement: mark.placement.shifted(Vector::new(to.left, 0.0)),
        ..mark
    };
    commit(
        file,
        MarkChange::Replace(target, rewrapped.id, rewrapped.mark()).into(),
    )?;
    Ok(Picked::Text(rewrapped))
}

/// Lays out again each text being typed whose doc changed since.
pub fn sync(state: &mut State) {
    let State { layout, fonts, .. } = state;
    let boxes = (layout.panes_mut())
        .flat_map(|(_, pane)| pane.items_mut().iter_mut())
        .filter_map(|item| match &mut item.tab {
            Tab::Ink(ink) => ink.text_box(),
            Tab::Text(_) | Tab::Image(_) => None,
        });
    for text in boxes.filter(|text| text.laid_version != text.doc.version) {
        text.laid = fonts.lay_out(&String::from(&text.doc.text), &text.style, text.wrap);
        text.laid_version = text.doc.version;
    }
}

impl InkTabs {
    /// The text the tab is typing, if any.
    pub fn text_box(&mut self) -> Option<&mut TextBox> {
        match self {
            InkTabs::Paged(tab) => tab.hand.typing.as_mut().map(|open| &mut open.text),
            InkTabs::Board(tab) => tab.hand.typing.as_mut().map(|open| &mut open.text),
        }
    }
}

#[derive(Clone, Debug)]
pub enum Ended<T> {
    NothingOpen,
    /// The text was blank: nothing was added, and the mark it edited is gone.
    Dropped,
    /// The text stands as the mark `picked` holds.
    Kept {
        target: T,
        picked: Picked,
    },
}

impl InkTabs {
    pub fn typed(&self) -> Option<&TextBox> {
        match self {
            InkTabs::Paged(tab) => tab.hand.typing.as_ref().map(|open| &open.text),
            InkTabs::Board(tab) => tab.hand.typing.as_ref().map(|open| &open.text),
        }
    }
}
