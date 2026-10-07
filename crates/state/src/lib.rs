pub mod apply;
pub mod buffer;
pub mod input;
pub mod job;
pub mod keymap;
pub mod layout;
pub mod prompt;

use std::io;
use std::path::{Path, PathBuf};

use helix_core::syntax::Loader;
use model::key::KeySeq;
use model::mode::Mode;
use model::notice::Notice;
use slotmap::SlotMap;

use crate::buffer::{Buffer, BufferId};
use crate::job::Job;
use crate::keymap::{Keymap, KeymapError};
use crate::layout::Layout;

/// Canonical path of an existing directory the editor was opened on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Root(PathBuf);

#[derive(Debug, thiserror::Error)]
pub enum RootError {
    #[error("cannot resolve {path:?}: {source}")]
    Resolve { path: PathBuf, source: io::Error },
    #[error("{0:?} is not a directory")]
    NotDirectory(PathBuf),
}

impl Root {
    pub fn open(path: &Path) -> Result<Root, RootError> {
        let canonical = path.canonicalize().map_err(|source| RootError::Resolve {
            path: path.to_owned(),
            source,
        })?;
        match canonical.is_dir() {
            true => Ok(Root(canonical)),
            false => Err(RootError::NotDirectory(canonical)),
        }
    }

    pub fn path(&self) -> &Path {
        &self.0
    }
}

pub struct State {
    pub root: Root,
    pub languages: Loader,
    pub keymap: Keymap,
    pub buffers: SlotMap<BufferId, Buffer>,
    pub layout: Layout,
    pub overlay: Overlay,
    pub input: InputState,
    pub notice: Option<Notice>,
    pub exit: Exit,
}

impl State {
    pub fn new(root: Root, languages: Loader) -> Result<State, KeymapError> {
        Ok(State {
            root,
            languages,
            keymap: Keymap::helix()?,
            buffers: SlotMap::with_key(),
            layout: Layout::new(),
            overlay: Overlay::None,
            input: InputState {
                mode: Mode::Normal,
                pending: KeySeq::EMPTY,
            },
            notice: None,
            exit: Exit::Running,
        })
    }
}

pub enum Overlay {
    None,
    Prompt(Prompt),
}

pub struct Prompt {
    pub line: String,
}

pub struct InputState {
    pub mode: Mode,
    pub pending: KeySeq,
}

/// Quit progress; `:wq` waits for its write before exiting.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Exit {
    Running,
    AfterWrite { buffer: BufferId },
}

/// Work for the host to perform after a state change.
pub enum Effect {
    Job(Job),
    Quit,
}
