//! Everything about the working root: its files, links between notes, the session and the watcher.

pub mod backlinks;
pub mod files;
pub mod links;
pub mod path;
pub mod session;
pub mod watch;

use crate::project::files::FileIndex;
use crate::{Refresh, Repo, Root};

/// Everything known about one working root.
pub struct Project {
    pub root: Root,
    pub index: FileIndex,
    pub git: Repo,
    pub refresh: Refresh,
}
