use std::path::PathBuf;
use std::time::SystemTime;

use helix_core::history::{self, History, UndoKind};
use helix_core::indent::{IndentStyle, auto_detect_indent_style};
use helix_core::syntax::config::LanguageConfiguration;
use helix_core::syntax::{HighlighterError, Loader};
use helix_core::{
    ChangeSet, Language, LineEnding, Range, Rope, RopeSlice, Selection, SmallVec, Syntax,
    Transaction,
};
use model::notice::Notice;
use slotmap::new_key_type;

use crate::git::conflict::{self, Conflicts};
use crate::git::diff::Diff;
use crate::git::navigate::Edge;
use crate::project::path::TextKind;
use crate::view::layout::PaneId;
use crate::view::panes::Arrive;
use model::config::OpenAs;

new_key_type! {
    pub struct BufferId;
    pub struct ReadId;
}

const DEFAULT_TAB_WIDTH: usize = 4;
const DEFAULT_TEXT_WIDTH: usize = 80;

pub struct Buffer {
    pub path: PathBuf,
    pub kind: TextKind,
    pub doc: TextDoc,
    pub disk: Disk,
    pub diff: Diff,
}

pub struct Read {
    pub path: PathBuf,
    pub waiting: SmallVec<[Waiter; 1]>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Waiter {
    pub pane: PaneId,
    pub arrive: Arrive,
    pub open: OpenAs,
    pub to: Option<Target>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Target {
    Position(Position),
    Hunk(Edge),
    Heading(String),
    Saved(Saved),
    Annotation(library::AnnotationId),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Saved {
    ranges: SmallVec<[Range; 1]>, // private: checked by `new`
    primary: usize,               // private: checked by `new`
    pub scroll: usize,
}

impl Saved {
    pub fn new(ranges: &[[usize; 2]], primary: usize, scroll: usize) -> Option<Saved> {
        ranges.get(primary)?;
        Some(Saved {
            ranges: (ranges.iter())
                .map(|&[anchor, head]| Range::new(anchor, head))
                .collect(),
            primary,
            scroll,
        })
    }

    pub fn selection(&self, len: usize) -> Selection {
        let ranges = (self.ranges.iter())
            .map(|range| Range::new(range.anchor.min(len), range.head.min(len)))
            .collect();
        Selection::new(ranges, self.primary)
    }
}

// Zero-based line and character column.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Position {
    pub line: usize,
    pub column: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Disk {
    Synced { mtime: SystemTime },
    New,
    ChangedExternally { mtime: SystemTime },
    Deleted,
}

pub struct TextDoc {
    pub text: Rope,
    pub version: u64,
    pub history: History,
    pub uncommitted: Uncommitted,
    pub saved_revision: usize,
    pub line_ending: LineEnding,
    pub highlight: Highlight,
    pub indent_style: IndentStyle,
    pub tab_width: usize,
    pub text_width: usize,
    pub conflicts: Conflicts,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParseJob {
    Wanted,
    Running { version: u64 },
}

pub enum Highlight {
    Plain,
    Parsing {
        language: Language,
        job: ParseJob,
    },
    Parsed(Syntax),
    Failed {
        language: Language,
        error: HighlighterError,
    },
}

impl Highlight {
    pub fn tree(&self) -> Result<&Syntax, Notice> {
        match self {
            Highlight::Parsed(syntax) => Ok(syntax),
            Highlight::Plain => Err(Notice::NoSyntax),
            Highlight::Parsing { .. } => Err(Notice::SyntaxPending),
            Highlight::Failed { .. } => Err(Notice::SyntaxFailed),
        }
    }

    pub fn parse(text: RopeSlice, language: Language, loader: &Loader) -> Highlight {
        Highlight::of(language, Syntax::new(text, language, loader))
    }

    pub fn of(language: Language, parsed: Result<Syntax, HighlighterError>) -> Highlight {
        match parsed {
            Ok(syntax) => Highlight::Parsed(syntax),
            Err(error) => {
                tracing::warn!(?error, "syntax parse failed");
                Highlight::Failed { language, error }
            }
        }
    }

    pub fn syntax(&self) -> Option<&Syntax> {
        match self {
            Highlight::Parsed(syntax) => Some(syntax),
            Highlight::Plain | Highlight::Parsing { .. } | Highlight::Failed { .. } => None,
        }
    }

    pub fn language(&self) -> Option<Language> {
        match self {
            Highlight::Plain => None,
            Highlight::Parsed(syntax) => Some(syntax.root_language()),
            Highlight::Parsing { language, .. } | Highlight::Failed { language, .. } => {
                Some(*language)
            }
        }
    }
}

pub enum Uncommitted {
    Clean,
    Pending {
        changes: Transaction,
        before: history::State,
    },
}

impl TextDoc {
    pub fn new(text: Rope, path: &std::path::Path, loader: &Loader) -> TextDoc {
        TextDoc::build(text, path, loader, |text, language| {
            tracing::info_span!("syntax parse")
                .in_scope(|| Highlight::parse(text, language, loader))
        })
    }

    pub fn unparsed(text: Rope, path: &std::path::Path, loader: &Loader) -> TextDoc {
        TextDoc::build(text, path, loader, |_, language| Highlight::Parsing {
            language,
            job: ParseJob::Wanted,
        })
    }

    pub fn plain(text: Rope) -> TextDoc {
        TextDoc {
            line_ending: LineEnding::LF,
            indent_style: IndentStyle::Spaces(4),
            tab_width: DEFAULT_TAB_WIDTH,
            text_width: DEFAULT_TEXT_WIDTH,
            text,
            version: 0,
            history: History::default(),
            uncommitted: Uncommitted::Clean,
            saved_revision: 0,
            highlight: Highlight::Plain,
            conflicts: Conflicts::EMPTY,
        }
    }

    fn build(
        text: Rope,
        path: &std::path::Path,
        loader: &Loader,
        highlight: impl FnOnce(RopeSlice, Language) -> Highlight,
    ) -> TextDoc {
        let language =
            tracing::info_span!("language lookup").in_scope(|| loader.language_for_filename(path));
        let highlight = match language {
            Some(language) => highlight(text.slice(..), language),
            None => Highlight::Plain,
        };
        let config = tracing::info_span!("language config").in_scope(|| {
            highlight
                .language()
                .map(|language| &**loader.language(language).config())
        });
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
            Highlight::Plain | Highlight::Parsing { .. } | Highlight::Failed { .. } => {
                (self.highlight.language()).map(|language| &**loader.language(language).config())
            }
        }
    }

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

    pub fn commit(&mut self, selection: &Selection) {
        match std::mem::replace(&mut self.uncommitted, Uncommitted::Clean) {
            Uncommitted::Clean => {}
            Uncommitted::Pending { changes, before } => {
                let tx = changes.with_selection(selection.clone());
                self.history.commit_revision(&tx, &before);
            }
        }
    }

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

    pub fn update_highlight(&mut self, old: &Rope, changes: &ChangeSet, loader: &Loader) {
        let text = self.text.slice(..);
        self.highlight = match std::mem::replace(&mut self.highlight, Highlight::Plain) {
            Highlight::Plain => Highlight::Plain,
            // `highlight::parsed` parses again when the text moved on meanwhile.
            parsing @ Highlight::Parsing { .. } => parsing,
            Highlight::Parsed(mut syntax) => {
                match syntax.update(old.slice(..), text, changes, loader) {
                    Ok(()) => Highlight::Parsed(syntax),
                    Err(error) => {
                        tracing::warn!(?error, "syntax update failed; parsing again");
                        Highlight::Parsing {
                            language: syntax.root_language(),
                            job: ParseJob::Wanted,
                        }
                    }
                }
            }
            Highlight::Failed { language, .. } => Highlight::Parsing {
                language,
                job: ParseJob::Wanted,
            },
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
