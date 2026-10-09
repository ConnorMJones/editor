use std::io;
use std::path::PathBuf;

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
    /// The buffer's file is still being read.
    StillLoading {
        path: PathBuf,
    },
    NoBuffer,
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
    AlignMultiLine,
    SortNeedsSelections,
    Surround(SurroundError),

    // Workspace: disk sync and file operations.
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
    DeleteNotConfirmed,
    InvalidPath {
        input: String,
    },
    /// The saved session could not be read; the editor started fresh.
    SessionInvalid {
        path: PathBuf,
        reason: String,
    },

    // Git: hunks, staging and changed-file navigation.
    /// The buffer is outside a repository or not tracked.
    NotTracked {
        path: PathBuf,
    },
    /// The buffer changed since its diff; the command can run once the diff catches up.
    DiffPending,
    NoHunks,
    NoChangedFiles,
    /// Discard works on one file at a time.
    DiscardGroup,
    Unchanged {
        path: PathBuf,
    },
    /// The index entry moved since the diff was computed.
    IndexChanged {
        path: PathBuf,
    },
    GitFailed {
        path: PathBuf,
        error: String,
    },
    DiscardNotConfirmed,

    // Conflicts: merge markers in the buffer text.
    NoConflicts,
    NoConflictUnderSelection,
    /// A selected conflict has no `|||||||` section to take.
    NoConflictBase,
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
            Notice::StillLoading { path } => write!(f, "{} is still loading", path.display()),
            Notice::NoBuffer => f.write_str("no buffer open"),
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
        }
    }
}
