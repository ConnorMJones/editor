use std::io;
use std::path::PathBuf;

/// Status-line feedback produced by commands and finished jobs.
#[derive(Debug)]
pub enum Notice {
    Written { path: PathBuf, bytes: usize },
    WriteFailed { path: PathBuf, error: io::Error },
    OpenFailed { path: PathBuf, error: io::Error },
    NotUtf8 { path: PathBuf },
    NotAFile { path: PathBuf },
    UnsavedBuffers { count: usize },
    NoBuffer,
    UnknownCommand { input: String },
    MissingArgument { command: &'static str },
    UnexpectedArgument { command: &'static str },
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
            Notice::NotUtf8 { path } => write!(f, "{} is not valid UTF-8", path.display()),
            Notice::NotAFile { path } => write!(f, "{} is not a file", path.display()),
            Notice::UnsavedBuffers { count } => write!(f, "{count} buffer(s) have unsaved changes"),
            Notice::NoBuffer => f.write_str("no buffer open"),
            Notice::UnknownCommand { input } => write!(f, "unknown command: {input}"),
            Notice::MissingArgument { command } => write!(f, "{command}: missing argument"),
            Notice::UnexpectedArgument { command } => write!(f, "{command}: takes no argument"),
        }
    }
}
