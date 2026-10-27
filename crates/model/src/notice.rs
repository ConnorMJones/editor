use std::io;
use std::path::PathBuf;

use crate::command::ExportAs;
use crate::typeset::{Diagnostic, SourceFile};

/// Status-line feedback produced by commands and finished jobs.
#[derive(Debug)]
pub enum Notice {
    Written {
        path: PathBuf,
        bytes: usize,
    },
    WriteFailed {
        path: PathBuf,
        error: io::Error,
    },
    OpenFailed {
        path: PathBuf,
        error: io::Error,
    },
    ReloadFailed {
        path: PathBuf,
        error: io::Error,
    },
    NotUtf8 {
        path: PathBuf,
    },
    NotAFile {
        path: PathBuf,
    },
    UnsavedBuffers {
        count: usize,
    },
    NoBuffer,
    ReadOnly,
    NotText,
    NothingToSplit,
    WrongExtension {
        path: PathBuf,
    },
    NotADrawing,
    NotAPdf,
    NoHits {
        query: String,
    },
    NoSearch,
    NoOutline,
    PageTextFailed {
        page: usize,
        reason: String,
    },
    /// Images go into drawings and notes only.
    NoImagesHere,
    NoImageOnClipboard,
    /// A drop landed off every page.
    OffThePages,
    NoteOutsideProject {
        path: PathBuf,
    },
    NotAnImage {
        path: PathBuf,
    },
    BoardHasNoPages,
    NoLayer,
    LayerHidden,
    NothingPicked,
    DrawingRefused {
        reason: String,
    },
    Unreadable {
        path: PathBuf,
        reason: String,
    },
    UnknownCommand {
        input: String,
    },
    MissingArgument {
        command: &'static str,
    },
    UnexpectedArgument {
        command: &'static str,
    },
    InvalidArgument {
        command: &'static str,
        input: String,
    },
    Yanked {
        count: usize,
        register: char,
    },
    RegisterSet {
        register: char,
        value: String,
    },
    RegisterReadOnly {
        register: char,
    },
    RegisterEmpty {
        register: char,
    },
    InvalidRegex {
        error: String,
    },
    NothingSelected,
    NoSelectionsRemaining,
    NoMoreMatches,
    WrappedAround,
    AlreadyOldest,
    AlreadyNewest,
    NoSyntax,
    SyntaxFailed,
    SyntaxPending,
    NotSplit,
    AlignMultiLine,
    SortNeedsSelections,
    Surround(SurroundError),

    ChangedOnDisk {
        path: PathBuf,
    },
    DeletedOnDisk {
        path: PathBuf,
    },
    FileOpFailed {
        path: PathBuf,
        error: io::Error,
    },
    MoveIntoItself,
    DeleteNotConfirmed,
    InvalidPath {
        input: String,
    },
    SessionInvalid {
        path: PathBuf,
        reason: String,
    },

    NotTracked {
        path: PathBuf,
    },
    DiffPending,
    NoHunks,
    NoChangedFiles,
    DiscardGroup,
    Unchanged {
        path: PathBuf,
    },
    IndexChanged {
        path: PathBuf,
    },
    GitFailed {
        path: PathBuf,
        error: String,
    },
    DiscardNotConfirmed,

    NoConflicts,
    NoConflictUnderSelection,
    NoConflictBase,

    NotRenderable {
        path: PathBuf,
    },
    LinkNotFound {
        target: String,
    },
    NoHeading,
    HeadingNotFound {
        heading: String,
    },
    NothingToFollow,
    NotANote {
        path: PathBuf,
    },
    /// The first walk of the project's files has not finished.
    IndexLoading,
    CreateNotConfirmed,
    TypstErrors {
        first: Diagnostic,
        count: usize,
    },
    Exported {
        path: PathBuf,
        bytes: usize,
    },
    MarkdownExport,
    NotTypst {
        path: PathBuf,
    },
    ExportFailed {
        path: PathBuf,
        reason: String,
    },
    NotExportable {
        to: ExportAs,
    },
    OutsideProject {
        file: SourceFile,
    },
    /// The details are in the log.
    JumpFailed,
}

/// Mirrors `helix_core::surround::Error`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SurroundError {
    PairNotFound,
    CursorOverlap,
    RangeExceedsText,
    CursorOnAmbiguousPair,
}

impl std::fmt::Display for Notice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Notice::Written { path, bytes } => write!(f, "written {} ({bytes} B)", path.display()),
            Notice::WriteFailed { path, error } => {
                write!(f, "failed to write {}: {error}", path.display())
            }
            Notice::OpenFailed { path, error } => {
                write!(f, "failed to open {}: {error}", path.display())
            }
            Notice::ReloadFailed { path, error } => {
                write!(f, "failed to reload {}: {error}", path.display())
            }
            Notice::NotUtf8 { path } => write!(f, "{} is not valid UTF-8", path.display()),
            Notice::NotAFile { path } => write!(f, "{} is not a file", path.display()),
            Notice::UnsavedBuffers { count } => write!(f, "{count} buffer(s) have unsaved changes"),
            Notice::NoBuffer => f.write_str("no buffer open"),
            Notice::ReadOnly => f.write_str("this view cannot be edited"),
            Notice::NotText => f.write_str("this tab has no text"),
            Notice::NothingToSplit => f.write_str("an empty pane has nothing to split"),
            Notice::WrongExtension { path } => {
                write!(f, "{} names another kind of file", path.display())
            }
            Notice::NoOutline => f.write_str("the PDF has no table of contents"),
            Notice::NoSearch => f.write_str("nothing searched for yet"),
            Notice::NoHits { query } => write!(f, "no {query:?} in the PDF"),
            Notice::PageTextFailed { page, reason } => {
                write!(f, "the text of page {page} could not be read: {reason}")
            }
            Notice::NotAPdf => f.write_str("only a PDF is read rather than drawn on"),
            Notice::NotADrawing => f.write_str("not a drawing"),
            Notice::NoImagesHere => f.write_str("images go into drawings and notes"),
            Notice::OffThePages => f.write_str("drop images onto a page"),
            Notice::NoImageOnClipboard => f.write_str("the clipboard holds no image"),
            Notice::NoteOutsideProject { path } => {
                write!(
                    f,
                    "{} is outside the project, so no image is kept beside it",
                    path.display()
                )
            }
            Notice::NotAnImage { path } => {
                write!(f, "{} is not an image this build reads", path.display())
            }
            Notice::BoardHasNoPages => f.write_str("a board has no pages"),
            Notice::NoLayer => f.write_str("the active layer is gone"),
            Notice::LayerHidden => f.write_str("the active layer is hidden"),
            Notice::NothingPicked => f.write_str("nothing is selected"),
            Notice::DrawingRefused { reason } => write!(f, "drawing refused: {reason}"),
            Notice::Unreadable { path, reason } => {
                write!(f, "cannot read {}: {reason}", path.display())
            }
            Notice::UnknownCommand { input } => write!(f, "unknown command: {input}"),
            Notice::MissingArgument { command } => write!(f, "{command}: missing argument"),
            Notice::UnexpectedArgument { command } => write!(f, "{command}: takes no argument"),
            Notice::InvalidArgument { command, input } => {
                write!(f, "{command}: invalid argument {input:?}")
            }
            Notice::Yanked { count, register } => {
                write!(f, "yanked {count} selection(s) to register {register}")
            }
            Notice::RegisterSet { register, value } => {
                write!(f, "register '{register}' set to '{value}'")
            }
            Notice::RegisterReadOnly { register } => {
                write!(f, "register {register} does not support writing")
            }
            Notice::RegisterEmpty { register } => write!(f, "register {register} is empty"),
            Notice::InvalidRegex { error } => write!(f, "invalid regex: {error}"),
            Notice::NothingSelected => f.write_str("nothing selected"),
            Notice::NoSelectionsRemaining => f.write_str("no selections remaining"),
            Notice::NoMoreMatches => f.write_str("no more matches"),
            Notice::WrappedAround => f.write_str("wrapped around document"),
            Notice::AlreadyOldest => f.write_str("already at oldest change"),
            Notice::AlreadyNewest => f.write_str("already at newest change"),
            Notice::NoSyntax => f.write_str("syntax tree is not available in current buffer"),
            Notice::SyntaxFailed => f.write_str("the grammar failed on this text; see the log"),
            Notice::SyntaxPending => f.write_str("still parsing; try again in a moment"),
            Notice::NotSplit => f.write_str("the tab is not split; space v splits it"),
            Notice::AlignMultiLine => f.write_str("align cannot work with multi line selections"),
            Notice::SortNeedsSelections => {
                f.write_str("sorting requires multiple selections; split selection first")
            }
            Notice::Surround(SurroundError::PairNotFound) => {
                f.write_str("surround pair not found around all cursors")
            }
            Notice::Surround(SurroundError::CursorOverlap) => {
                f.write_str("cursors overlap for a single surround pair range")
            }
            Notice::Surround(SurroundError::RangeExceedsText) => {
                f.write_str("cursor range exceeds text length")
            }
            Notice::Surround(SurroundError::CursorOnAmbiguousPair) => {
                f.write_str("cursor on ambiguous surround pair")
            }
            Notice::ChangedOnDisk { path } => {
                write!(
                    f,
                    "{} changed on disk; buffer has unsaved edits",
                    path.display()
                )
            }
            Notice::DeletedOnDisk { path } => write!(f, "{} was deleted on disk", path.display()),
            Notice::FileOpFailed { path, error } => write!(f, "{}: {error}", path.display()),
            Notice::DeleteNotConfirmed => f.write_str("delete cancelled"),
            Notice::InvalidPath { input } => write!(f, "invalid path: {input}"),
            Notice::SessionInvalid { path, reason } => {
                write!(f, "ignored session {}: {reason}", path.display())
            }
            Notice::NotTracked { path } => {
                write!(f, "{} is not tracked by git", path.display())
            }
            Notice::DiffPending => f.write_str("diff is still updating; try again"),
            Notice::NoHunks => f.write_str("no change under the selection"),
            Notice::NoChangedFiles => f.write_str("no changed files"),
            Notice::MoveIntoItself => f.write_str("cannot move a folder into itself"),
            Notice::DiscardGroup => f.write_str("discard files one at a time"),
            Notice::Unchanged { path } => write!(f, "{} has no changes", path.display()),
            Notice::IndexChanged { path } => {
                write!(
                    f,
                    "index of {} changed meanwhile; try again",
                    path.display()
                )
            }
            Notice::GitFailed { path, error } => write!(f, "git: {}: {error}", path.display()),
            Notice::DiscardNotConfirmed => f.write_str("discard cancelled"),
            Notice::NoConflicts => f.write_str("no more conflicts"),
            Notice::NoConflictUnderSelection => f.write_str("no conflict under the selection"),
            Notice::NoConflictBase => {
                f.write_str("conflict has no base section (needs merge.conflictStyle diff3)")
            }
            Notice::NotRenderable { path } => {
                write!(
                    f,
                    "{} has no rendered view (markdown and typst only)",
                    path.display()
                )
            }
            Notice::LinkNotFound { target } => write!(f, "link target not found: {target}"),
            Notice::NoHeading => f.write_str("no more headings"),
            Notice::HeadingNotFound { heading } => write!(f, "no heading {heading:?}"),
            Notice::NothingToFollow => f.write_str("no link or path under the cursor"),
            Notice::NotANote { path } => {
                write!(f, "{} is not a markdown note in the vault", path.display())
            }
            Notice::CreateNotConfirmed => f.write_str("note not created"),
            Notice::IndexLoading => f.write_str("the file index is still loading"),
            Notice::TypstErrors { first, count } => match count {
                1 => write!(f, "typst: {first}"),
                count => write!(f, "typst: {first} (+{} more)", count - 1),
            },
            Notice::Exported { path, bytes } => {
                write!(f, "exported {} ({bytes} B)", path.display())
            }
            Notice::ExportFailed { path, reason } => {
                write!(f, "exporting {}: {reason}", path.display())
            }
            Notice::NotExportable { to } => {
                write!(f, "export-{0}: nothing here exports as {0}", to.extension())
            }
            Notice::MarkdownExport => f.write_str("export-pdf: not supported for markdown yet"),
            Notice::NotTypst { path } => {
                write!(f, "export-pdf: {} is not a typst file", path.display())
            }
            Notice::OutsideProject { file } => write!(f, "defined in {file}, outside the project"),
            Notice::JumpFailed => f.write_str("typst could not resolve that point"),
        }
    }
}
