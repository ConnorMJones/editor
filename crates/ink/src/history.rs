//! A drawing with its undo history; every change goes through it.

use std::collections::VecDeque;

use crate::doc::{ChangeError, Drawing};

/// Names one state of a drawing; a file is unsaved while its state isn't the one written.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Revision(u64); // private: named only by `Edited`

/// The states kept to undo into.
const KEPT: usize = 100;

#[derive(Clone, Debug)]
pub struct Edited<D> {
    drawing: D,                    // private: the history and revisions move together
    revision: Revision,            // private: the history and revisions move together
    next: u64,                     // private: the history and revisions move together
    past: VecDeque<(D, Revision)>, // private: the history and revisions move together
    future: Vec<(D, Revision)>,    // private: the history and revisions move together
}

#[derive(Debug, thiserror::Error, PartialEq)]
pub enum HistoryError {
    #[error("nothing to undo")]
    NothingToUndo,
    #[error("nothing to redo")]
    NothingToRedo,
}

impl<D: Drawing> Edited<D> {
    pub fn new(drawing: D) -> Edited<D> {
        Edited {
            drawing,
            revision: Revision(0),
            next: 1,
            past: VecDeque::new(),
            future: Vec::new(),
        }
    }

    pub fn drawing(&self) -> &D {
        &self.drawing
    }

    pub fn revision(&self) -> Revision {
        self.revision
    }

    /// Applies `change` as one undoable step; a refused change leaves everything as it was.
    pub fn commit(&mut self, change: D::Change) -> Result<Revision, ChangeError> {
        let before = self.drawing.clone();
        match self.drawing.apply(change) {
            Ok(()) => {}
            Err(error) => {
                self.drawing = before;
                return Err(error);
            }
        }
        Ok(self.advance(before))
    }

    /// Puts `drawing` in place of the current one as one undoable step, as when the file is read
    /// again.
    pub fn replace(&mut self, drawing: D) -> Revision {
        let before = std::mem::replace(&mut self.drawing, drawing);
        self.advance(before)
    }

    /// Keeps `before` to undo into and names the state now held.
    fn advance(&mut self, before: D) -> Revision {
        self.past.push_back((before, self.revision));
        while self.past.len() > KEPT {
            self.past.pop_front();
        }
        self.future.clear();
        self.revision = Revision(self.next);
        self.next += 1;
        self.revision
    }

    pub fn undo(&mut self) -> Result<Revision, HistoryError> {
        let (drawing, revision) = self.past.pop_back().ok_or(HistoryError::NothingToUndo)?;
        let current = std::mem::replace(&mut self.drawing, drawing);
        self.future.push((current, self.revision));
        self.revision = revision;
        Ok(revision)
    }

    pub fn redo(&mut self) -> Result<Revision, HistoryError> {
        let (drawing, revision) = self.future.pop().ok_or(HistoryError::NothingToRedo)?;
        let current = std::mem::replace(&mut self.drawing, drawing);
        self.past.push_back((current, self.revision));
        self.revision = revision;
        Ok(revision)
    }
}
