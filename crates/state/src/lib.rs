pub mod apply;
pub mod git;
pub mod input;
pub mod job;
pub mod project;
pub mod render;
pub mod sidebar;
pub mod text;
pub mod view;

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
use model::units::Px;
use slotmap::SlotMap;

use crate::git::stage;
use crate::input::prompt;
use crate::render::document::Documents;
use crate::render::pages::Compiles;
use crate::render::typeset;

use crate::git::GitSnapshot;
use crate::input::keymap::{Keymap, KeymapError};
use crate::input::picker::Picker;
use crate::job::Job;
use crate::project::Project;
use crate::project::files::{FileIndex, OnIndexed};
use crate::project::path::{Folder, RelPath};
use crate::sidebar::changes::ChangesList;
use crate::sidebar::tree::{Dir, Fold, Listing, Tree, TreeEdit};
use crate::text::buffer::{Buffer, BufferId};
use crate::text::registers::{ClipboardUse, Registers};
use crate::view::layout::Layout;

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
    pub project: Project,
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
    pub changes: ChangesList,
    pub matcher: nucleo::Matcher,
    /// Generation of the newest grep; older grep jobs stop when it moves on.
    pub grep: Arc<AtomicU64>,
    /// Typeset math and diagrams of the parsed documents.
    pub figures: typeset::Figures,
    /// Parses of the buffers document views show, while any is open.
    pub documents: Documents,
    /// Compiles of the buffers pages views show, while any is open.
    pub compiles: Compiles,
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
            project: Project {
                root,
                index: FileIndex::Loading {
                    then: OnIndexed::Nothing,
                },
                git: Repo::Unknown,
                refresh: Refresh::Idle,
            },
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
            sidebar_width: SidebarWidth::Default,
            tree: Tree::default(),
            changes: ChangesList {
                cursor: None,
                folds: [Fold::Expanded; Group::COUNT],
            },
            matcher: nucleo::Matcher::default(),
            grep: Arc::new(AtomicU64::new(0)),
            figures: typeset::Figures::new(look),
            documents: Documents::new(),
            compiles: Compiles::new(),
        })
    }
}

/// Starts the file index, the tree's root listing and the first git status.
pub fn boot(state: &mut State, out: &mut Vec<Effect>) {
    let root = state.project.root.path().to_owned();
    state.tree.dirs.insert(
        Folder::Root,
        Dir {
            fold: Fold::Expanded,
            listing: Listing::Loading,
        },
    );
    out.push(Effect::Job(Job::Index { root: root.clone() }));
    out.push(Effect::Job(Job::ListDir {
        root,
        dir: Folder::Root,
    }));
    request_git(state, out);
}

/// Coalesces refreshes: at most one status job runs, with one more queued behind it.
pub fn request_git(state: &mut State, out: &mut Vec<Effect>) {
    state.project.refresh = match state.project.refresh {
        Refresh::Idle => {
            out.push(Effect::Job(Job::GitStatus {
                root: state.project.root.path().to_owned(),
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
    /// An action such as a discard waits for `Command::Confirm`; the status line asks.
    Confirm(Confirm),
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Sidebar {
    Hidden,
    Files(Files),
    Changes(Focus),
}

/// Where keys go while the file tree is shown.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Files {
    Editor,
    Tree,
    /// A tree line being typed; focusing the editor drops it.
    Edit(TreeEdit),
}

/// The width the user chose; the host draws it narrower when the window lacks room.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SidebarWidth {
    /// Sized by the host from the interface font.
    Default,
    Dragged(Dragged),
}

#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(try_from = "Px", into = "Px")]
pub struct Dragged(Px);

#[derive(Debug, thiserror::Error)]
#[error("sidebar width {0:?} is not a finite positive width")]
pub struct InvalidWidth(Px);

impl Dragged {
    pub fn new(width: Px) -> Result<Dragged, InvalidWidth> {
        match width.0.is_finite() && width.0 > 0.0 {
            true => Ok(Dragged(width)),
            false => Err(InvalidWidth(width)),
        }
    }

    pub fn get(self) -> Px {
        self.0
    }
}

impl TryFrom<Px> for Dragged {
    type Error = InvalidWidth;

    fn try_from(width: Px) -> Result<Dragged, InvalidWidth> {
        Dragged::new(width)
    }
}

impl From<Dragged> for Px {
    fn from(width: Dragged) -> Px {
        width.0
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
    Discard { path: PathBuf, how: stage::Discard },
    /// Creates the empty note an unresolved link names, and opens it.
    Create(RelPath),
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
    /// The host answers with `registers::clipboard_contents`.
    ReadClipboard {
        clipboard: Clipboard,
        then: ClipboardUse,
    },
    /// Opens a URL in the browser.
    OpenUrl(String),
    /// The host answers with `JobResult::Typeset`.
    Typeset(typeset::Request),
}

/// Brings everything derived from buffer text up to date; runs once after every event.
pub fn sync(state: &mut State, out: &mut Vec<Effect>) {
    git::conflict::sync(state);
    render::sync(state, out);
    git::track::sync(state, out);
    git::diffview::refresh(state, out);
}
