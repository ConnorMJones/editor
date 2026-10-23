use ink::doc::{
    Background, ChangeError, Drawing, Geometry, LayerIndex, Mark, MarkChange, PageIndex, PageLayer,
    PageSize, Paged, Placement,
};
use ink::geom::Local;
use ink::history::{Edited, HistoryError};
use model::color::Rgb;

use crate::{brush, wave, window};

const FIRST: PageLayer = PageLayer {
    page: PageIndex(0),
    layer: LayerIndex(0),
};

fn mark() -> Mark {
    crate::mark(
        Geometry::new(wave::<Local>(10).into(), brush(window())).unwrap(),
        Placement::ALIGNED,
    )
}

fn add() -> MarkChange<PageLayer> {
    MarkChange::Add(FIRST, mark())
}

fn count(edited: &Edited<Paged>) -> usize {
    edited.drawing().marks(FIRST).unwrap().len()
}

#[test]
fn undo_returns_to_the_saved_revision() {
    let mut edited = Edited::new(Paged::new(
        PageSize::A4,
        Background::Plain(Rgb::hex(0xffffff)),
    ));
    let saved = edited.revision();
    let first = edited.commit(add().into()).unwrap();
    edited.commit(add().into()).unwrap();
    assert_eq!(count(&edited), 2);
    assert_eq!(edited.undo(), Ok(first));
    assert_eq!(edited.undo(), Ok(saved));
    assert_eq!(count(&edited), 0);
    assert_eq!(edited.undo(), Err(HistoryError::NothingToUndo));
    assert_eq!(edited.redo(), Ok(first));
    assert_eq!(count(&edited), 1);
}

#[test]
fn a_new_change_drops_what_could_be_redone() {
    let mut edited = Edited::new(Paged::new(
        PageSize::A4,
        Background::Plain(Rgb::hex(0xffffff)),
    ));
    edited.commit(add().into()).unwrap();
    edited.undo().unwrap();
    let again = edited.commit(add().into()).unwrap();
    assert_eq!(edited.redo(), Err(HistoryError::NothingToRedo));
    assert_eq!(edited.revision(), again);
}

#[test]
fn a_refused_change_leaves_the_drawing_and_revision_alone() {
    let mut edited = Edited::new(Paged::new(
        PageSize::A4,
        Background::Plain(Rgb::hex(0xffffff)),
    ));
    let before = edited.revision();
    let stray = MarkChange::Remove(FIRST, smallvec::smallvec![]);
    edited.commit(stray.into()).unwrap();
    let missing = PageLayer {
        page: PageIndex(9),
        layer: LayerIndex(0),
    };
    let refused = MarkChange::Add(missing, mark());
    let revision = edited.revision();
    assert_ne!(revision, before);
    assert_eq!(edited.commit(refused.into()), Err(ChangeError::NoPage(9)));
    assert_eq!(edited.revision(), revision);
    assert_eq!(edited.undo(), Ok(before));
}
