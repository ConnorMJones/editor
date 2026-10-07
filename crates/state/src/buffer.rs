use std::path::PathBuf;

use helix_core::history::{self, History};
use helix_core::syntax::{Loader, Syntax};
use helix_core::{LineEnding, Rope, Selection, Transaction};
use slotmap::new_key_type;

new_key_type! {
    pub struct BufferId;
}

pub struct Buffer {
    /// Canonical when the file exists; root-joined and normalized for new files.
    pub path: PathBuf,
    pub content: Content,
    pub disk: Disk,
}

pub enum Content {
    Text(TextDoc),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Disk {
    Synced,
    /// Path does not exist yet; the first write creates it.
    New,
    ChangedExternally,
    Deleted,
}

pub struct TextDoc {
    pub text: Rope,
    pub history: History,
    pub uncommitted: Uncommitted,
    pub saved_revision: usize,
    pub line_ending: LineEnding,
    pub syntax: Option<Syntax>,
}

/// Edits applied to `text` but not yet grouped into an undo revision.
pub enum Uncommitted {
    Clean,
    Pending {
        changes: Transaction,
        before: history::State,
    },
}

impl TextDoc {
    pub fn new(text: Rope, path: &std::path::Path, loader: &Loader) -> TextDoc {
        let syntax = loader
            .language_for_filename(path)
            .and_then(|language| Syntax::new(text.slice(..), language, loader).ok());
        TextDoc {
            line_ending: helix_core::line_ending::auto_detect_line_ending(&text)
                .unwrap_or(LineEnding::LF),
            text,
            history: History::default(),
            uncommitted: Uncommitted::Clean,
            saved_revision: 0,
            syntax,
        }
    }

    pub fn is_modified(&self) -> bool {
        match self.uncommitted {
            Uncommitted::Pending { .. } => true,
            Uncommitted::Clean => self.history.current_revision() != self.saved_revision,
        }
    }

    /// Applies `tx` to the text and stages it for the next undo revision.
    pub fn apply(&mut self, tx: &Transaction, selection_before: &Selection, loader: &Loader) {
        let old = self.text.clone();
        if !tx.apply(&mut self.text) || tx.changes().is_empty() {
            return;
        }
        if let Some(syntax) = &mut self.syntax
            && let Err(error) =
                syntax.update(old.slice(..), self.text.slice(..), tx.changes(), loader)
        {
            tracing::warn!(?error, "syntax update failed; dropping highlights");
            self.syntax = None;
        }
        self.uncommitted = match std::mem::replace(&mut self.uncommitted, Uncommitted::Clean) {
            Uncommitted::Clean => Uncommitted::Pending {
                changes: tx.clone(),
                before: history::State {
                    doc: old,
                    selection: selection_before.clone(),
                },
            },
            Uncommitted::Pending { changes, before } => Uncommitted::Pending {
                changes: changes.compose(tx.clone()),
                before,
            },
        };
    }

    /// Groups staged edits into one undo revision.
    pub fn commit(&mut self, selection: &Selection) {
        match std::mem::replace(&mut self.uncommitted, Uncommitted::Clean) {
            Uncommitted::Clean => {}
            Uncommitted::Pending { changes, before } => {
                let tx = changes.with_selection(selection.clone());
                self.history.commit_revision(&tx, &before);
            }
        }
    }

    /// Moves through history; returns the applied transaction so callers can map selections.
    pub fn step_history(&mut self, step: HistoryStep, loader: &Loader) -> Option<Transaction> {
        let tx = match step {
            HistoryStep::Undo => self.history.undo(),
            HistoryStep::Redo => self.history.redo(),
        }?
        .clone();
        let old = self.text.clone();
        tx.apply(&mut self.text);
        if let Some(syntax) = &mut self.syntax
            && syntax
                .update(old.slice(..), self.text.slice(..), tx.changes(), loader)
                .is_err()
        {
            self.syntax = None;
        }
        Some(tx)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HistoryStep {
    Undo,
    Redo,
}
