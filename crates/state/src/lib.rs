pub mod apply;
pub mod backlinks;
pub mod buffer;
pub mod changes;
pub mod conflict;
pub mod diff;
pub mod diffview;
pub mod display;
pub mod edit;
pub mod files;
pub mod git;
pub mod input;
pub mod job;
pub mod keymap;
pub mod layout;
pub mod markdown;
pub mod motion;
pub mod path;
pub mod picker;
pub mod prompt;
pub mod register;
pub mod rendered;
pub mod scroll;
pub mod select;
pub mod session;
pub mod tree;
pub mod typeset;
pub mod vault;
pub mod watch;
pub mod workspace;

use std::io;
use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::AtomicU64;

use helix_core::Selection;
use helix_core::syntax::Loader;
use model::command::{Command, Pending, RegexOp};
use model::config::TabsConfig;
use model::git::Group;
use model::key::{Key, KeySeq};
use model::mode::Mode;
use model::notice::Notice;
use model::register::{Clipboard, Register};
use slotmap::SlotMap;

use crate::buffer::{Buffer, BufferId};
use crate::files::Files;
use crate::git::GitSnapshot;
use crate::job::Job;
use crate::keymap::{Keymap, KeymapError};
use crate::layout::Layout;
use crate::path::RelPath;
use crate::picker::Picker;
use crate::register::{ClipboardUse, Registers};
use crate::tree::{Dir, Fold, Listing, Tree};

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
    /// Shared with jobs that parse off the UI thread.
    pub languages: Arc<Loader>,
    pub keymap: Keymap,
    pub buffers: SlotMap<BufferId, Buffer>,
    pub layout: Layout,
    pub overlay: Overlay,
    pub input: InputState,
    pub registers: Registers,
    pub notice: Option<Notice>,
    pub exit: Exit,

    pub tab_config: TabsConfig,
    pub sidebar: Sidebar,
    pub sidebar_width: SidebarWidth,
    /// Kept while the sidebar is hidden so expansion survives toggling.
    pub tree: Tree,
    pub files: Files,
    pub git: Git,
    pub matcher: nucleo::Matcher,
    /// Generation of the newest grep; older grep jobs stop when it moves on.
    pub grep: Arc<AtomicU64>,
    /// Typeset math and diagrams of the parsed documents.
    pub figures: typeset::Figures,
    /// What the rendered tabs of each buffer show, while any is open.
    pub previews: rendered::Previews,
    pub vault: vault::Vault,
    pub backlinks: backlinks::Backlinks,
    /// An action waiting for `Command::Confirm`; the status line asks.
    pub confirm: Option<Confirm>,
}

impl State {
    pub fn new(
        root: Root,
        languages: Loader,
        tab_config: TabsConfig,
        look: typeset::Look,
    ) -> Result<State, KeymapError> {
        languages.set_scopes(
            model::theme::SCOPES
                .iter()
                .map(|(scope, _)| (*scope).to_owned())
                .collect(),
        );
        Ok(State {
            root,
            languages: Arc::new(languages),
            keymap: Keymap::helix()?,
            buffers: SlotMap::with_key(),
            layout: Layout::new(),
            overlay: Overlay::None,
            input: InputState {
                mode: Mode::Normal,
                pending: KeySeq::EMPTY,
                count: None,
                register: None,
                awaiting: None,
                sticky: Sticky::Off,
                insert_exit: InsertExit::Keep,
                last_insert: LastInsert {
                    entry: Command::NormalMode,
                    keys: Vec::new(),
                },
                last_motion: None,
            },
            registers: Registers::default(),
            notice: None,
            exit: Exit::Running,
            tab_config,
            sidebar: Sidebar::Hidden,
            sidebar_width: SidebarWidth::DEFAULT,
            tree: Tree::default(),
            files: Files::Loading,
            git: Git {
                repo: Repo::Unknown,
                refresh: Refresh::Idle,
                cursor: None,
                folds: [Fold::Expanded; Group::COUNT],
            },
            matcher: nucleo::Matcher::default(),
            grep: Arc::new(AtomicU64::new(0)),
            figures: typeset::Figures::new(look),
            previews: rendered::Previews::new(),
            vault: vault::Vault::new(),
            backlinks: backlinks::Backlinks::default(),
            confirm: None,
        })
    }
}

/// Starts the file index, the tree's root listing and the first git status.
pub fn boot(state: &mut State, out: &mut Vec<Effect>) {
    let root = state.root.path().to_owned();
    state.tree.dirs.insert(
        RelPath::ROOT,
        Dir {
            fold: Fold::Expanded,
            listing: Listing::Loading,
        },
    );
    out.push(Effect::Job(Job::Index { root: root.clone() }));
    out.push(Effect::Job(Job::ListDir {
        root,
        dir: RelPath::ROOT,
    }));
    request_git(state, out);
}

/// Coalesces refreshes: at most one status job runs, with one more queued behind it.
pub fn request_git(state: &mut State, out: &mut Vec<Effect>) {
    state.git.refresh = match state.git.refresh {
        Refresh::Idle => {
            out.push(Effect::Job(Job::GitStatus {
                root: state.root.path().to_owned(),
            }));
            Refresh::Running
        }
        Refresh::Running | Refresh::Queued => Refresh::Queued,
    };
}

pub enum Overlay {
    None,
    Prompt(Prompt),
    Picker(Picker),
}

pub struct Prompt {
    pub kind: PromptKind,
    pub line: String,
    /// Byte offset into `line`, on a char boundary.
    pub cursor: usize,
    pub history: prompt::History,
    pub completion: prompt::Completion,
}

pub enum PromptKind {
    Command,
    /// Previews on every edit from `snapshot`, which cancel restores.
    Regex {
        op: RegexOp,
        /// Receives the submitted pattern as history; `n` searches it afterwards.
        register: Register,
        snapshot: Selection,
    },
}

impl PromptKind {
    /// Register keeping submitted lines, browsed with up and down.
    pub fn history(&self) -> Register {
        match self {
            PromptKind::Command => Register::COMMAND,
            PromptKind::Regex { register, .. } => *register,
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            PromptKind::Command => ":",
            PromptKind::Regex { op, .. } => match op {
                RegexOp::Search(_) => "search:",
                RegexOp::Select => "select:",
                RegexOp::Split => "split:",
                RegexOp::Keep => "keep:",
                RegexOp::Remove => "remove:",
            },
        }
    }
}

pub struct InputState {
    pub mode: Mode,
    pub pending: KeySeq,
    /// Numeric prefix typed in normal or select mode.
    pub count: Option<NonZeroUsize>,
    /// Register chosen with `"`; consumed by the next command.
    pub register: Option<Register>,
    pub awaiting: Option<Pending>,
    pub sticky: Sticky,
    pub insert_exit: InsertExit,
    /// Replayed by `.`.
    pub last_insert: LastInsert,
    /// Replayed by `A-.`.
    pub last_motion: Option<LastMotion>,
}

/// A key mode that stays active across keys, like helix's `Z`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sticky {
    Off,
    /// Keys resolve as if prefixed by `z`.
    View,
}

/// What leaving insert mode does to the cursor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InsertExit {
    Keep,
    /// Append mode moved the cursor past the selection; step back onto it.
    RestoreCursor,
}

pub struct LastInsert {
    /// The command that entered insert mode.
    pub entry: Command,
    pub keys: Vec<Key>,
}

pub struct LastMotion {
    pub command: Command,
    pub count: Option<NonZeroUsize>,
}

/// Quit progress; `:wq` and `:wqa` wait for their writes before exiting.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Exit {
    Running,
    AfterWrite { buffers: Vec<BufferId> },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sidebar {
    Hidden,
    Files(Focus),
    Changes(Focus),
}

/// Sidebar width in logical pixels, kept within `MIN..=MAX`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SidebarWidth(f32);

impl SidebarWidth {
    pub const MIN: f32 = 120.0;
    pub const MAX: f32 = 800.0;
    pub const DEFAULT: SidebarWidth = SidebarWidth(280.0);

    /// Clamps into range; a non-finite width falls back to the default.
    pub fn new(width: f32) -> SidebarWidth {
        match width.is_finite() {
            true => SidebarWidth(width.clamp(Self::MIN, Self::MAX)),
            false => Self::DEFAULT,
        }
    }

    pub fn get(self) -> f32 {
        self.0
    }
}

/// Which side of the window receives keys while the sidebar is shown.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Focus {
    Sidebar,
    Editor,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Confirm {
    /// Discards a file's changes, as `how` says.
    Discard {
        path: PathBuf,
        how: changes::Discard,
    },
    /// Creates the empty note an unresolved link names, and opens it.
    Create(RelPath),
}

pub struct Git {
    pub repo: Repo,
    pub refresh: Refresh,
    /// Row selected in the Changes sidebar, kept by value so it survives refreshes.
    pub cursor: Option<changes::ListCursor>,
    /// Per `Group`, whether the sidebar shows its entries.
    pub folds: [Fold; Group::COUNT],
}

pub enum Repo {
    /// No status has completed yet.
    Unknown,
    None,
    Found {
        git_dir: PathBuf,
        /// Canonical worktree root; buffer paths strip it to get repository paths.
        workdir: PathBuf,
        snapshot: GitSnapshot,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refresh {
    Idle,
    Running,
    /// Another refresh starts when the running one finishes.
    Queued,
}

/// Work for the host to perform after a state change.
pub enum Effect {
    Job(Job),
    Quit,
    /// Adds non-recursive watches on these directories.
    Watch(Vec<PathBuf>),
    WriteClipboard {
        clipboard: Clipboard,
        text: String,
    },
    /// The host answers with `register::clipboard_contents`.
    ReadClipboard {
        clipboard: Clipboard,
        then: ClipboardUse,
    },
    /// Opens a URL in the browser.
    OpenUrl(String),
    /// The host answers with `JobResult::Typeset`.
    Typeset(typeset::Request),
}
