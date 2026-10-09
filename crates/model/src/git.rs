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

/// What changed on one side of the index.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Kind {
    Added,
    Modified,
    Deleted,
    Renamed,
    TypeChange,
}

/// A path's change between HEAD, the index and the worktree.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Change {
    Ignored,
    /// In neither HEAD nor the index.
    Untracked,
    Conflicted,
    /// HEAD to index only.
    Staged(Kind),
    /// Index to worktree only.
    Unstaged(Kind),
    Both {
        staged: Kind,
        unstaged: Kind,
    },
}

/// A group of the Changes sidebar; also the order `]g` walks files in.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Group {
    Conflicts,
    Staged,
    /// Unstaged and untracked files.
    Changes,
}

impl Change {
    pub fn staged(self) -> Option<Kind> {
        match self {
            Change::Staged(kind) | Change::Both { staged: kind, .. } => Some(kind),
            Change::Ignored | Change::Untracked | Change::Conflicted | Change::Unstaged(_) => None,
        }
    }

    pub fn unstaged(self) -> Option<Kind> {
        match self {
            Change::Unstaged(kind) | Change::Both { unstaged: kind, .. } => Some(kind),
            Change::Ignored | Change::Untracked | Change::Conflicted | Change::Staged(_) => None,
        }
    }

    /// Tree colour: deletions outrank modifications, which outrank additions.
    pub fn status(self) -> GitStatus {
        let rank = |kind: Kind| match kind {
            Kind::Added => GitStatus::Added,
            Kind::Deleted => GitStatus::Deleted,
            Kind::Modified | Kind::Renamed | Kind::TypeChange => GitStatus::Modified,
        };
        match self {
            Change::Ignored => GitStatus::Ignored,
            Change::Untracked => GitStatus::Untracked,
            Change::Conflicted => GitStatus::Conflicted,
            Change::Staged(kind) | Change::Unstaged(kind) => rank(kind),
            Change::Both { staged, unstaged } => match (rank(staged), rank(unstaged)) {
                (GitStatus::Deleted, _) | (_, GitStatus::Deleted) => GitStatus::Deleted,
                (a, b) => a.max(b),
            },
        }
    }

    /// Sidebar groups listing this path; a partly staged file sits in two.
    pub fn groups(self) -> &'static [Group] {
        match self {
            Change::Ignored => &[],
            Change::Conflicted => &[Group::Conflicts],
            Change::Staged(_) => &[Group::Staged],
            Change::Untracked | Change::Unstaged(_) => &[Group::Changes],
            Change::Both { .. } => &[Group::Staged, Group::Changes],
        }
    }
}
