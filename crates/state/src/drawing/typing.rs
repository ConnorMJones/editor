//! Text marks being typed: the text is a plain `TextDoc`, edited by the same commands as any
//! text, and laid out into a mark when it changes. Where text goes is the text tool's, in ink.

use helix_core::{Rope, Selection, smallvec};
use ink::brush::Paint;
use ink::brush::Size;
use ink::doc::{Drawing, Ink, Mark, MarkChange, MarkId, Placement};
use ink::geom::{Affine, Local, Point, Rect, Surface};
use ink::text::{Fonts, Inked, Laid, TextStyle, Wrap};
use ink::tool::{Frame, Picked, Sides, TypeAt};
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

    pub fn frame(&self) -> Frame {
        Frame {
            rect: self.laid.area,
            placement: self.placement,
            sides: Sides::Wrap,
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
    let mark = |ink| Mark {
        placement: text.placement,
        paint: text.paint,
        ink: Ink::Text(ink),
    };
    let kept = |id, mark: &Mark| Ended::Kept {
        target,
        picked: Picked {
            ids: smallvec![id],
            frame: Frame::of(mark),
        },
    };
    match (writing, fonts.lay_out(&typed, &text.style, text.wrap).inked) {
        (Writing::New, Inked::Blank) => Ok(Ended::Dropped),
        (Writing::New, Inked::Ink(ink)) => {
            let mark = mark(ink);
            let id = (file.edited.add(target, mark.clone())).map_err(refused)?;
            Ok(kept(id, &mark))
        }
        (Writing::Replacing(id), Inked::Blank) => {
            commit(file, MarkChange::Remove(target, smallvec![id]).into()).map(|()| Ended::Dropped)
        }
        (Writing::Replacing(id), Inked::Ink(ink)) => {
            let was = (file.edited.drawing().marks(target))
                .and_then(|marks| marks.get(id))
                .map(|was| &was.ink);
            match was {
                Some(Ink::Text(was)) if was.text == ink.text => {}
                Some(_) | None => commit(
                    file,
                    MarkChange::Replace(target, id, mark(ink.clone())).into(),
                )?,
            }
            Ok(kept(id, &mark(ink)))
        }
    }
}

pub fn rewrap<D: Drawing>(
    file: &mut InkFile<D>,
    fonts: &Fonts,
    target: D::Target,
    id: MarkId,
    area: Rect<Local>,
) -> Result<Picked, Notice> {
    let marks = file.edited.drawing().marks(target);
    let mark = marks
        .and_then(|marks| marks.get(id))
        .ok_or(Notice::NothingPicked)?;
    let text = match &mark.ink {
        Ink::Text(text) => text,
        Ink::Stroke(_) | Ink::Shape(_) | Ink::Image(_) => return Err(Notice::NothingPicked),
    };
    let width = Size::new(area.max().x - area.min().x).map_err(refused)?;
    let ink = match fonts
        .lay_out(&text.text, &text.style, Wrap::At(width))
        .inked
    {
        Inked::Ink(ink) => ink,
        // The same text as before, which was not blank.
        Inked::Blank => return Err(Notice::NothingPicked),
    };
    let shifted = Affine::translation(area.min().x, 0.0).then(&mark.placement.forward());
    let rewrapped = Mark {
        placement: Placement::new(shifted).ok_or(Notice::NothingPicked)?,
        paint: mark.paint,
        ink: Ink::Text(ink),
    };
    let frame = Frame::of(&rewrapped);
    commit(file, MarkChange::Replace(target, id, rewrapped).into())?;
    Ok(Picked {
        ids: smallvec![id],
        frame,
    })
}

fn refused(error: impl std::fmt::Display) -> Notice {
    Notice::DrawingRefused {
        reason: error.to_string(),
    }
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
