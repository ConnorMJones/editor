use std::path::PathBuf;
use std::time::SystemTime;

use helix_core::history::{self, History, UndoKind};
use helix_core::indent::{IndentStyle, auto_detect_indent_style};
use helix_core::syntax::config::LanguageConfiguration;
use helix_core::syntax::{HighlighterError, Loader};
use helix_core::{Language, LineEnding, Rope, Selection, Syntax, Transaction};
use slotmap::{SlotMap, new_key_type};

use crate::git::changes::Arrive;
use crate::git::conflict::{self, Conflicts};
use crate::git::diff::{Diff, Edge};
use crate::view::layout::PaneId;

new_key_type! {
    pub struct BufferId;
}

pub struct Buffer {
    /// Canonical when the file exists; root-joined and normalized for new files.
    pub path: PathBuf,
    pub content: Content,
}

pub enum Content {
    /// The first read and parse run as a job; commands skip the buffer and it cannot be written.
    Loading { jump: Option<Jump> },
    Text {
        doc: TextDoc,
        disk: Disk,
        /// Modification time of the disk copy the text last matched; `None` before the first write.
        mtime: Option<SystemTime>,
        diff: Diff,
    },
}

/// Cursor placement in `pane` waiting for a loading buffer's text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Jump {
    pub pane: PaneId,
    pub arrive: Arrive,
    pub to: Target,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Target {
    Position(Position),
    /// Waits further, for the buffer's first diff.
    Hunk(Edge),
    /// A markdown heading, by its text as a link names it.
    Heading(String),
}

/// Zero-based line and character column.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Position {
    pub line: usize,
    pub column: usize,
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
    /// Bumped by every change to `text`; diffs are stamped with it.
    pub version: u64,
    pub history: History,
    pub uncommitted: Uncommitted,
    pub saved_revision: usize,
    pub line_ending: LineEnding,
    pub language: Option<Language>,
    pub syntax: Option<Syntax>,
    pub indent_style: IndentStyle,
    pub tab_width: usize,
    pub text_width: usize,
    /// Parsed with the text, then again after edits by `conflict::sync`.
    pub conflicts: Conflicts,
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
        let mut conflicts = Conflicts::EMPTY;
        conflict::parse(text.slice(..), 0, &mut conflicts);
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
            version: 0,
            history: History::default(),
            uncommitted: Uncommitted::Clean,
            saved_revision: 0,
            language,
            syntax,
            conflicts,
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
        self.version += 1;
        let updated = self
            .syntax
            .as_mut()
            .map(|syntax| syntax.update(old.slice(..), self.text.slice(..), tx.changes(), loader));
        self.recover_syntax(updated, loader);
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
            self.version += 1;
            let updated = self.syntax.as_mut().map(|syntax| {
                syntax.update(old.slice(..), self.text.slice(..), tx.changes(), loader)
            });
            self.recover_syntax(updated, loader);
        }
        txs
    }

    /// Re-parses from scratch after a failed incremental update, which may leave the tree half-edited.
    pub fn recover_syntax(
        &mut self,
        updated: Option<Result<(), HighlighterError>>,
        loader: &Loader,
    ) {
        match (updated, &self.syntax) {
            (Some(Err(error)), Some(syntax)) => {
                let language = syntax.root_language();
                self.syntax = match Syntax::new(self.text.slice(..), language, loader) {
                    Ok(syntax) => {
                        tracing::warn!(?error, "syntax update failed; re-parsed");
                        Some(syntax)
                    }
                    Err(reparse) => {
                        tracing::warn!(?error, ?reparse, "syntax update and re-parse failed");
                        None
                    }
                };
            }
            (Some(Ok(())) | None, _) | (Some(Err(_)), None) => {}
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HistoryStep {
    Undo,
    Redo,
    Earlier,
    Later,
}

impl Content {
    pub fn is_modified(&self) -> bool {
        match self {
            Content::Loading { .. } => false,
            Content::Text { doc, .. } => doc.is_modified(),
        }
    }
}

/// The text of `buffer`, once loaded.
pub fn doc(buffers: &SlotMap<BufferId, Buffer>, buffer: BufferId) -> Option<&TextDoc> {
    match &buffers.get(buffer)?.content {
        Content::Text { doc, .. } => Some(doc),
        Content::Loading { .. } => None,
    }
}

pub fn doc_mut(buffers: &mut SlotMap<BufferId, Buffer>, buffer: BufferId) -> Option<&mut TextDoc> {
    match &mut buffers.get_mut(buffer)?.content {
        Content::Text { doc, .. } => Some(doc),
        Content::Loading { .. } => None,
    }
}
