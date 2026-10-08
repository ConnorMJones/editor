use std::path::PathBuf;

use helix_core::history::{self, History, UndoKind};
use helix_core::indent::{IndentStyle, auto_detect_indent_style};
use helix_core::syntax::Loader;
use helix_core::syntax::config::LanguageConfiguration;
use helix_core::{Language, LineEnding, Rope, Selection, Syntax, Transaction};
use slotmap::new_key_type;

new_key_type! {
    pub struct BufferId;
}

pub struct Buffer {
    /// Canonical when the file exists; root-joined and normalized for new files.
    pub path: PathBuf,
    pub content: Content,
    pub disk: Disk,
    /// Modification time of the disk copy the text last matched; `None` before the first write.
    pub mtime: Option<std::time::SystemTime>,
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

const DEFAULT_TAB_WIDTH: usize = 4;
const DEFAULT_TEXT_WIDTH: usize = 80;

pub struct TextDoc {
    pub text: Rope,
    pub history: History,
    pub uncommitted: Uncommitted,
    pub saved_revision: usize,
    pub line_ending: LineEnding,
    pub language: Option<Language>,
    pub syntax: Option<Syntax>,
    pub indent_style: IndentStyle,
    pub tab_width: usize,
    pub text_width: usize,
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
        let language = loader.language_for_filename(path);
        let syntax =
            language.and_then(|language| Syntax::new(text.slice(..), language, loader).ok());
        let config = language.map(|language| &**loader.language(language).config());
        let indent = config.and_then(|config| config.indent.as_ref());
        TextDoc {
            line_ending: helix_core::line_ending::auto_detect_line_ending(&text)
                .unwrap_or(LineEnding::LF),
            indent_style: auto_detect_indent_style(&text).unwrap_or_else(|| {
                indent.map_or(IndentStyle::Tabs, |indent| {
                    IndentStyle::from_str(&indent.unit)
                })
            }),
            tab_width: indent.map_or(DEFAULT_TAB_WIDTH, |indent| indent.tab_width),
            text_width: config
                .and_then(|config| config.text_width)
                .unwrap_or(DEFAULT_TEXT_WIDTH),
            text,
            history: History::default(),
            uncommitted: Uncommitted::Clean,
            saved_revision: 0,
            language,
            syntax,
        }
    }

    pub fn is_modified(&self) -> bool {
        match self.uncommitted {
            Uncommitted::Pending { .. } => true,
            Uncommitted::Clean => self.history.current_revision() != self.saved_revision,
        }
    }

    pub fn indent_width(&self) -> usize {
        self.indent_style.indent_width(self.tab_width)
    }

    /// Config of the injection layer at `byte`, else of the buffer's language.
    pub fn language_config_at<'a>(
        &self,
        loader: &'a Loader,
        byte: usize,
    ) -> Option<&'a LanguageConfiguration> {
        match &self.syntax {
            Some(syntax) => {
                let layer = syntax.layer_for_byte_range(byte as u32, byte as u32);
                Some(&**loader.language(syntax.layer(layer).language).config())
            }
            None => self
                .language
                .map(|language| &**loader.language(language).config()),
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

    /// Moves through history; returns the applied transactions in order so callers can map selections.
    pub fn step_history(&mut self, step: HistoryStep, loader: &Loader) -> Vec<Transaction> {
        let txs = match step {
            HistoryStep::Undo => self.history.undo().cloned().into_iter().collect(),
            HistoryStep::Redo => self.history.redo().cloned().into_iter().collect(),
            HistoryStep::Earlier => self.history.earlier(UndoKind::Steps(1)),
            HistoryStep::Later => self.history.later(UndoKind::Steps(1)),
        };
        for tx in &txs {
            let old = self.text.clone();
            tx.apply(&mut self.text);
            if let Some(syntax) = &mut self.syntax
                && syntax
                    .update(old.slice(..), self.text.slice(..), tx.changes(), loader)
                    .is_err()
            {
                self.syntax = None;
            }
        }
        txs
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HistoryStep {
    Undo,
    Redo,
    Earlier,
    Later,
}
