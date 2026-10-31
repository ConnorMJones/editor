//! Markdown and typst documents as the rendered view shows them.

mod highlight;
mod parse;
mod targets;

pub use highlight::{highlight, highlight_code};
pub use parse::OPTIONS;
pub use parse::parse;
pub use targets::link_target;
pub use targets::locate;

use std::path::Path;

use crate::project::links::Index;

pub use crate::render::flow::*;

pub struct Markdown {
    pub flow: Flow<Lines>,
    pub source: String,
}

impl std::ops::Deref for Markdown {
    type Target = Flow<Lines>;

    fn deref(&self) -> &Flow<Lines> {
        &self.flow
    }
}

impl std::ops::DerefMut for Markdown {
    fn deref_mut(&mut self) -> &mut Flow<Lines> {
        &mut self.flow
    }
}

/// Images resolve from the note's directory, then the vault root, then by vault resolution.
#[derive(Clone, Copy)]
pub struct Base<'a> {
    pub dir: &'a Path,
    pub root: &'a Path,
    /// The note's folder within the vault; empty at the root and for notes outside it.
    pub folder: &'a str,
    pub vault: &'a Index,
}
