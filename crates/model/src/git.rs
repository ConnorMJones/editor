/// Working-tree status of a path; ordered so the most urgent status wins when bubbling to parents.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum GitStatus {
    Ignored,
    Untracked,
    Added,
    Deleted,
    Modified,
    Conflicted,
}

impl GitStatus {
    pub const COUNT: usize = 6;
}
