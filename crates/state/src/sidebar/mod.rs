//! The file tree and Changes sidebars.

pub mod changes;
pub mod tree;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pick {
    Only,
    Toggle,
    Extend,
}
