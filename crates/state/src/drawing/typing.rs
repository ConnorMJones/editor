//! Text marks being typed: the text is a plain `TextDoc`, edited by the same commands as any
//! text, and laid out into a mark when it changes. Where text goes is the text tool's, in ink.

use helix_core::{Rope, Selection, smallvec};
use ink::brush::Paint;
use ink::doc::{Drawing, Ink, Mark, MarkChange, MarkId, Placement};
use ink::text::{Fonts, Inked, Laid, TextStyle, Wrap};
use ink::tool::TypeAt;
use model::notice::Notice;

use crate::State;
use crate::drawing::command::commit;
use crate::drawing::{InkFile, InkTabs};
use crate::text::buffer::TextDoc;
use crate::view::tab::Tab;

pub struct Open<T> {
    pub target: T,
    pub writing: Writing,
    pub text: TextBox,
}

/// Whether finishing adds a mark or replaces the one being edited, which is hidden meanwhile.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Writing {
    New,
    Replacing(MarkId),
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
}

/// Text to type where the text tool asked: new text in the hand's style, or a text mark's.
pub fn open<T>(at: TypeAt<T>, style: &TextStyle, paint: Paint, fonts: &Fonts) -> Open<T> {
    match at {
        TypeAt::New { target, at, wrap } => Open {
            target,
            writing: Writing::New,
            text: TextBox::new("", Placement::at(at), style.clone(), wrap, paint, fonts),
        },
        TypeAt::Mark {
            target,
            id,
            text,
            placement,
            paint,
        } => Open {
            target,
            writing: Writing::Replacing(id),
            text: TextBox::new(
                &text.text,
                placement,
                text.style.clone(),
                text.wrap,
                paint,
                fonts,
            ),
        },
    }
}

/// Ends the text being typed, if any: blank text adds nothing and removes the mark it edited, and
/// changed text becomes one step of the drawing's history.
pub fn finish<D: Drawing>(
    typing: &mut Option<Box<Open<D::Target>>>,
    file: &mut InkFile<D>,
    fonts: &Fonts,
) -> Result<Ended, Notice> {
    let Some(open) = typing.take() else {
        return Ok(Ended::NothingOpen);
    };
    let Open {
        target,
        writing,
        text,
    } = *open;
    let typed = String::from(&text.doc.text);
    let mark = |ink| Mark {
        placement: text.placement,
        paint: text.paint,
        ink: Ink::Text(ink),
    };
    let marks = file.edited.drawing().marks(target);
    let was = |id| marks.and_then(|marks| marks.get(id)).map(|mark| &mark.ink);
    let change = match (writing, fonts.lay_out(&typed, &text.style, text.wrap).inked) {
        (Writing::New, Inked::Blank) => return Ok(Ended::Finished),
        (Writing::New, Inked::Ink(ink)) => MarkChange::Add(target, mark(ink)),
        (Writing::Replacing(id), Inked::Blank) => MarkChange::Remove(target, smallvec![id]),
        (Writing::Replacing(id), Inked::Ink(ink)) => match was(id) {
            Some(Ink::Text(was)) if was.text == ink.text => return Ok(Ended::Finished),
            Some(_) | None => MarkChange::Replace(target, id, mark(ink)),
        },
    };
    commit(file, change.into()).map(|()| Ended::Finished)
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

/// Whether ending typing closed some text, so the keys leave its modes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ended {
    NothingOpen,
    Finished,
}

impl InkTabs {
    pub fn typed(&self) -> Option<&TextBox> {
        match self {
            InkTabs::Paged(tab) => tab.hand.typing.as_ref().map(|open| &open.text),
            InkTabs::Board(tab) => tab.hand.typing.as_ref().map(|open| &open.text),
        }
    }
}
