use std::path::PathBuf;
use std::time::SystemTime;

use helix_core::history::{self, History, UndoKind};
use helix_core::indent::{IndentStyle, auto_detect_indent_style};
use helix_core::syntax::config::LanguageConfiguration;
use helix_core::syntax::{HighlighterError, Loader};
use helix_core::{
    ChangeSet, Language, LineEnding, Rope, RopeSlice, Selection, Syntax, Transaction,
};
use model::notice::Notice;
use slotmap::new_key_type;

use crate::git::conflict::{self, Conflicts};
use crate::git::diff::Diff;
use crate::git::navigate::Arrive;
use crate::git::navigate::Edge;
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
    /// The text matches the file as modified at `mtime`.
    Synced {
        mtime: SystemTime,
    },
    /// The path does not exist yet; the first write creates it.
    New,
    /// The file as modified at `mtime` differs from the text, which kept its unsaved edits.
    ChangedExternally {
        mtime: SystemTime,
    },
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
    pub highlight: Highlight,
    pub indent_style: IndentStyle,
    pub tab_width: usize,
    pub text_width: usize,
    /// Parsed with the text, then again after edits by `conflict::sync`.
    pub conflicts: Conflicts,
}

/// What the buffer's grammar made of its text.
pub enum Highlight {
    /// No grammar matches the file.
    Plain,
    Parsed(Syntax),
    /// The grammar failed on this text; the next edit parses again from scratch.
    Failed {
        language: Language,
        error: HighlighterError,
    },
}

impl Highlight {
    /// The tree, or the notice for a command that needs one.
    pub fn tree(&self) -> Result<&Syntax, Notice> {
        match self {
            Highlight::Parsed(syntax) => Ok(syntax),
            Highlight::Plain => Err(Notice::NoSyntax),
            Highlight::Failed { .. } => Err(Notice::SyntaxFailed),
        }
    }

    pub fn parse(text: RopeSlice, language: Language, loader: &Loader) -> Highlight {
        match Syntax::new(text, language, loader) {
            Ok(syntax) => Highlight::Parsed(syntax),
            Err(error) => {
                tracing::warn!(?error, "syntax parse failed");
                Highlight::Failed { language, error }
            }
        }
    }

    /// The tree, for readers that fall back to plain text without one.
    pub fn syntax(&self) -> Option<&Syntax> {
        match self {
            Highlight::Parsed(syntax) => Some(syntax),
            Highlight::Plain | Highlight::Failed { .. } => None,
        }
    }

    pub fn language(&self) -> Option<Language> {
        match self {
            Highlight::Plain => None,
            Highlight::Parsed(syntax) => Some(syntax.root_language()),
            Highlight::Failed { language, .. } => Some(*language),
        }
    }
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
        let highlight = match loader.language_for_filename(path) {
            Some(language) => Highlight::parse(text.slice(..), language, loader),
            None => Highlight::Plain,
        };
        let config = highlight
            .language()
            .map(|language| &**loader.language(language).config());
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
            highlight,
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
        match &self.highlight {
            Highlight::Parsed(syntax) => {
                let layer = syntax.layer_for_byte_range(byte as u32, byte as u32);
                Some(&**loader.language(syntax.layer(layer).language).config())
            }
            Highlight::Plain | Highlight::Failed { .. } => {
                (self.highlight.language()).map(|language| &**loader.language(language).config())
            }
        }
    }

    /// Applies `tx` to the text and stages it for the next undo revision.
    pub fn apply(&mut self, tx: &Transaction, selection_before: &Selection, loader: &Loader) {
        let old = self.text.clone();
        if !tx.apply(&mut self.text) || tx.changes().is_empty() {
            return;
        }
        self.version += 1;
        self.update_highlight(&old, tx.changes(), loader);
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
            self.update_highlight(&old, tx.changes(), loader);
        }
        txs
    }

    /// Brings `highlight` up to the text `changes` made from `old`. A failed incremental update
    /// may leave the tree half-edited, so it parses again from scratch.
    pub fn update_highlight(&mut self, old: &Rope, changes: &ChangeSet, loader: &Loader) {
        let text = self.text.slice(..);
        self.highlight = match std::mem::replace(&mut self.highlight, Highlight::Plain) {
            Highlight::Plain => Highlight::Plain,
            Highlight::Parsed(mut syntax) => {
                match syntax.update(old.slice(..), text, changes, loader) {
                    Ok(()) => Highlight::Parsed(syntax),
                    Err(error) => {
                        tracing::warn!(?error, "syntax update failed; parsing again");
                        Highlight::parse(text, syntax.root_language(), loader)
                    }
                }
            }
            Highlight::Failed { language, .. } => Highlight::parse(text, language, loader),
        };
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

impl Buffer {
    /// `None` while the file is still being read.
    pub fn doc(&self) -> Option<&TextDoc> {
        match &self.content {
            Content::Text { doc, .. } => Some(doc),
            Content::Loading { .. } => None,
        }
    }

    pub fn doc_mut(&mut self) -> Option<&mut TextDoc> {
        match &mut self.content {
            Content::Text { doc, .. } => Some(doc),
            Content::Loading { .. } => None,
        }
    }
}
