//! Everything about the working root: files, note links, the session and the watcher.

pub mod backlinks;
pub mod files;
pub mod links;
pub mod path;
pub mod session;
pub mod watch;

use crate::project::files::FileIndex;
use crate::{Repo, Root};
use model::job::InFlight;

/// Everything known about one working root.
pub struct Project {
    pub root: Root,
    pub index: FileIndex,
    pub git: Repo,
    pub refresh: InFlight,
}
