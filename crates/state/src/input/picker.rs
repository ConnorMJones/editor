use std::cmp::Reverse;
use std::io;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use helix_core::regex::bytes::{Regex, RegexBuilder};
use model::command::{
    Command, Cycle, Direction, HunkOp, HunkTarget, PickerCommand, PickerKind, Shown, Take,
    TreeCommand, WindowCommand,
};
use model::picture::Encoding;
use model::window::{Axis, Side};
use nucleo::pattern::{CaseMatching, Normalization, Pattern};
use nucleo::{Matcher, Utf32Str};

use crate::project::path::RelPath;
use crate::text::buffer::{BufferId, TextDoc};
use crate::{Effect, Overlay, State};

pub struct Picker {
    pub query: String,
    pub items: Items,
    /// Indices into `items`, best first.
    pub matches: Vec<u32>,
    pub cursor: usize,
    /// Scratch for ranking: (score, haystack length, index).
    pub scored: Vec<(Reverse<u32>, u32, u32)>,
    pub previews: Previews,
}

pub enum Items {
    Files(Arc<Vec<RelPath>>),
    /// The project's image files.
    Images(Vec<RelPath>),
    /// Display label per buffer, relative to the root where possible.
    Buffers(Vec<(BufferId, String)>),
    Commands,
    Grep(Grep),
    Backlinks(Referrers),
}

pub struct Grep {
    /// Matches the job generation in `State::grep`; stale results are dropped.
    pub generation: u64,
    pub search: Search,
    pub hits: Vec<Hit>,
}

/// Notes linking to `note`, filled once the backlinks index has its first read.
pub struct Referrers {
    pub note: RelPath,
    /// `Running` until the first read of every note finishes.
    pub search: Search,
    pub hits: Vec<Hit>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Search {
    Idle,
    Running,
    Done,
    /// The hit cap was reached; later files were not searched.
    Truncated,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Hit {
    pub path: RelPath,
    /// Zero-based.
    pub line: u32,
    /// Zero-based character offset within the line.
    pub column: u32,
    pub text: String,
}

impl Items {
    pub fn kind(&self) -> PickerKind {
        match self {
            Items::Files(_) => PickerKind::Files,
            Items::Images(_) => PickerKind::Images,
            Items::Buffers(_) => PickerKind::Buffers,
            Items::Commands => PickerKind::Commands,
            Items::Grep(_) => PickerKind::Grep,
            Items::Backlinks(_) => PickerKind::Backlinks,
        }
    }

    pub fn len(&self) -> usize {
        match self {
            Items::Files(files) => files.len(),
            Items::Images(images) => images.len(),
            Items::Buffers(buffers) => buffers.len(),
            Items::Commands => PALETTE.len(),
            Items::Grep(grep) => grep.hits.len(),
            Items::Backlinks(referrers) => referrers.hits.len(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Text matched against and shown for item `index`.
    pub fn label(&self, index: u32) -> &str {
        let index = index as usize;
        match self {
            Items::Files(files) => files[index].as_str(),
            Items::Images(images) => images[index].as_str(),
            Items::Buffers(buffers) => &buffers[index].1,
            Items::Commands => PALETTE[index].0,
            Items::Grep(grep) => &grep.hits[index].text,
            Items::Backlinks(referrers) => referrers.hits[index].path.as_str(),
        }
    }
}

impl Picker {
    pub fn new(items: Items) -> Picker {
        let matches = (0..items.len() as u32).collect();
        Picker {
            query: String::new(),
            items,
            matches,
            cursor: 0,
            scored: Vec::new(),
            previews: Previews::default(),
        }
    }
}

/// Narrows from the current matches when the query only grew and has no negations.
pub fn filter(picker: &mut Picker, matcher: &mut Matcher, narrowing: Narrowing) {
    matcher.config = nucleo::Config::DEFAULT;
    match picker.items.kind() {
        PickerKind::Files | PickerKind::Images | PickerKind::Backlinks => {
            matcher.config.set_match_paths()
        }
        PickerKind::Buffers | PickerKind::Commands | PickerKind::Grep => {}
    }
    picker.cursor = 0;
    match picker.query.is_empty() {
        true => {
            picker.matches.clear();
            picker.matches.extend(0..picker.items.len() as u32);
            return;
        }
        false => {}
    }
    let pattern = Pattern::parse(&picker.query, CaseMatching::Smart, Normalization::Smart);
    let mut chars = Vec::new();
    picker.scored.clear();
    let mut score = |index: u32| {
        let label = picker.items.label(index);
        pattern
            .score(Utf32Str::new(label, &mut chars), matcher)
            .map(|score| (Reverse(score), label.len() as u32, index))
    };
    match narrowing {
        Narrowing::FromMatches => picker
            .scored
            .extend(picker.matches.iter().filter_map(|&i| score(i))),
        Narrowing::All => picker
            .scored
            .extend((0..picker.items.len() as u32).filter_map(score)),
    }
    picker.scored.sort_unstable();
    picker.matches.clear();
    picker
        .matches
        .extend(picker.scored.iter().map(|&(_, _, index)| index));
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Narrowing {
    FromMatches,
    All,
}

impl Narrowing {
    pub fn between(previous: &str, next: &str) -> Narrowing {
        match next.starts_with(previous) && !next.contains('!') && !previous.is_empty() {
            true => Narrowing::FromMatches,
            false => Narrowing::All,
        }
    }
}

/// Smart case, falling back to a literal search when `query` is not a valid regex.
pub fn grep_regex(query: &str) -> Regex {
    let insensitive = !query.chars().any(char::is_uppercase);
    RegexBuilder::new(query)
        .case_insensitive(insensitive)
        .build()
        .or_else(|_| {
            RegexBuilder::new(&helix_core::regex::escape(query))
                .case_insensitive(insensitive)
                .build()
        })
        .expect("escaped pattern is valid")
}

pub const MAX_HITS: usize = 10_000;
const MAX_LINE: usize = 240;

/// Searches `files` in order, stopping early once `current` moves past `generation`.
pub fn grep(
    root: &Path,
    files: &[RelPath],
    regex: &Regex,
    generation: u64,
    current: &AtomicU64,
) -> (Vec<Hit>, Search) {
    let mut hits = Vec::new();
    let mut bytes = Vec::new();
    for file in files {
        match current.load(Ordering::Relaxed) == generation {
            true => {}
            false => return (hits, Search::Done),
        }
        bytes.clear();
        let read = std::fs::File::open(file.to_path(root))
            .and_then(|mut f| std::io::Read::read_to_end(&mut f, &mut bytes));
        match read {
            Ok(_) => {}
            Err(_) => continue,
        }
        // Binary files are skipped, as ripgrep does by default.
        match bytes[..bytes.len().min(8192)].contains(&0) {
            true => continue,
            false => {}
        }
        let mut line = 0u32;
        let mut scanned = 0usize;
        let mut start = 0usize;
        let mut reported = usize::MAX;
        for found in regex.find_iter(&bytes) {
            for (offset, &byte) in bytes[scanned..found.start()].iter().enumerate() {
                match byte {
                    b'\n' => {
                        line += 1;
                        start = scanned + offset + 1;
                    }
                    _ => {}
                }
            }
            scanned = found.start();
            // One hit per line.
            match start == reported {
                true => continue,
                false => reported = start,
            }
            let end = bytes[found.start()..]
                .iter()
                .position(|&b| b == b'\n')
                .map_or(bytes.len(), |i| found.start() + i);
            let text = String::from_utf8_lossy(&bytes[start..end]);
            let column = String::from_utf8_lossy(&bytes[start..found.start()])
                .chars()
                .count() as u32;
            hits.push(Hit {
                path: file.clone(),
                line,
                column,
                text: text.trim().chars().take(MAX_LINE).collect(),
            });
            match hits.len() >= MAX_HITS {
                true => return (hits, Search::Truncated),
                false => {}
            }
        }
    }
    (hits, Search::Done)
}

/// Commands listed by the `space ?` palette.
pub static PALETTE: &[(&str, Command)] = &[
    ("write", Command::Write),
    ("write-all", Command::WriteAll),
    ("write-quit", Command::WriteQuit),
    ("quit", Command::Quit),
    ("quit!", Command::ForceQuit),
    ("buffer-close", Command::CloseBuffer),
    ("buffer-close!", Command::ForceCloseBuffer),
    ("buffer-next", Command::CycleBuffer(Cycle::Next)),
    ("buffer-previous", Command::CycleBuffer(Cycle::Previous)),
    ("vsplit", Command::Window(WindowCommand::Split(Axis::Row))),
    (
        "hsplit",
        Command::Window(WindowCommand::Split(Axis::Column)),
    ),
    ("window-next", Command::Window(WindowCommand::FocusNext)),
    (
        "window-left",
        Command::Window(WindowCommand::Focus(Side::Left)),
    ),
    (
        "window-down",
        Command::Window(WindowCommand::Focus(Side::Down)),
    ),
    ("window-up", Command::Window(WindowCommand::Focus(Side::Up))),
    (
        "window-right",
        Command::Window(WindowCommand::Focus(Side::Right)),
    ),
    ("window-close", Command::Window(WindowCommand::Close)),
    ("window-only", Command::Window(WindowCommand::Only)),
    ("file-explorer", Command::Tree(TreeCommand::Toggle)),
    (
        "file-picker",
        Command::Picker(PickerCommand::Open(PickerKind::Files)),
    ),
    (
        "image-picker",
        Command::Picker(PickerCommand::Open(PickerKind::Images)),
    ),
    ("paste-image", Command::PasteImage),
    (
        "buffer-picker",
        Command::Picker(PickerCommand::Open(PickerKind::Buffers)),
    ),
    (
        "global-search",
        Command::Picker(PickerCommand::Open(PickerKind::Grep)),
    ),
    ("goto-next-change", Command::GotoHunk(HunkTarget::Next)),
    ("goto-prev-change", Command::GotoHunk(HunkTarget::Previous)),
    ("stage-hunk", Command::Hunk(HunkOp::Stage)),
    ("unstage-hunk", Command::Hunk(HunkOp::Unstage)),
    ("revert-hunk", Command::Hunk(HunkOp::Revert)),
    ("undo", Command::Undo),
    ("redo", Command::Redo),
    (
        "goto-next-conflict",
        Command::GotoConflict(Direction::Forward),
    ),
    (
        "goto-prev-conflict",
        Command::GotoConflict(Direction::Backward),
    ),
    ("conflict-ours", Command::Resolve(Take::Ours)),
    ("conflict-theirs", Command::Resolve(Take::Theirs)),
    ("conflict-both", Command::Resolve(Take::Both)),
    ("conflict-base", Command::Resolve(Take::Base)),
    ("diff-toggle", Command::ToggleMode(Shown::Diff)),
    ("split-toggle", Command::ToggleMode(Shown::Split)),
    ("render-toggle", Command::ToggleMode(Shown::Render)),
    ("mode-next", Command::CycleMode(Direction::Forward)),
    ("export-pdf", Command::ExportPdf),
    (
        "backlinks",
        Command::Picker(PickerCommand::Open(PickerKind::Backlinks)),
    ),
];

/// Previews kept per picker; loads beyond this evict the oldest.
pub const PREVIEW_SLOTS: usize = 8;
/// Larger files are not previewed, as in helix.
pub const MAX_PREVIEW_BYTES: u64 = 10 * 1024 * 1024;

pub struct Previews {
    pub slots: [Option<(RelPath, Preview)>; PREVIEW_SLOTS],
    /// Slot the next load replaces.
    pub next: usize,
}

impl Default for Previews {
    fn default() -> Previews {
        Previews {
            slots: [const { None }; PREVIEW_SLOTS],
            next: 0,
        }
    }
}

impl Previews {
    pub fn get(&self, path: &RelPath) -> Option<&Preview> {
        self.slots.iter().find_map(|slot| match slot {
            Some((cached, preview)) if cached == path => Some(preview),
            Some(_) | None => None,
        })
    }
}

/// Contents of a file that is not open, read for the picker preview.
pub enum Preview {
    Loading,
    Text(TextDoc),
    Binary,
    TooLarge,
    Failed(io::Error),
}

/// What the picker's highlighted item previews.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PreviewSource<'a> {
    Buffer(BufferId),
    File { path: &'a RelPath, line: usize },
}

pub fn preview_source(picker: &Picker) -> Option<PreviewSource<'_>> {
    let &index = picker.matches.get(picker.cursor)?;
    let index = index as usize;
    match &picker.items {
        Items::Files(files) => Some(PreviewSource::File {
            path: &files[index],
            line: 0,
        }),
        Items::Buffers(buffers) => Some(PreviewSource::Buffer(buffers[index].0)),
        // Images have no text to preview.
        Items::Images(_) => None,
        Items::Grep(Grep { hits, .. }) | Items::Backlinks(Referrers { hits, .. }) => {
            hits.get(index).map(|hit| PreviewSource::File {
                path: &hit.path,
                line: hit.line as usize,
            })
        }
        Items::Commands => None,
    }
}

/// Fills the preview slot still waiting for `path`.
pub fn previewed(state: &mut State, path: RelPath, preview: Preview) {
    match &mut state.overlay {
        Overlay::Picker(picker) => {
            let waiting = picker.previews.slots.iter_mut().find(|slot| match slot {
                Some((cached, Preview::Loading)) => *cached == path,
                Some(_) | None => false,
            });
            match waiting {
                Some(slot) => *slot = Some((path, preview)),
                None => {}
            }
        }
        Overlay::None | Overlay::Prompt(_) | Overlay::Confirm(_) => {}
    }
}

/// Takes the hits of the current grep; older searches are dropped.
pub fn grepped(
    state: &mut State,
    generation: u64,
    hits: Vec<Hit>,
    search: Search,
    out: &mut Vec<Effect>,
) {
    match &mut state.overlay {
        Overlay::Picker(picker) => match &mut picker.items {
            Items::Grep(grep) if grep.generation == generation => {
                grep.hits = hits;
                grep.search = search;
                picker.matches.clear();
                picker.matches.extend(0..grep.hits.len() as u32);
                picker.cursor = 0;
            }
            Items::Grep(_)
            | Items::Files(_)
            | Items::Images(_)
            | Items::Buffers(_)
            | Items::Commands
            | Items::Backlinks(_) => {}
        },
        Overlay::None | Overlay::Prompt(_) | Overlay::Confirm(_) => {}
    }
    crate::view::panes::request_preview(state, out);
}

/// The files whose names are an image encoding.
pub fn images(files: &[RelPath]) -> Vec<RelPath> {
    (files.iter())
        .filter(|file| Encoding::of_path(std::path::Path::new(file.as_str())).is_some())
        .cloned()
        .collect()
}
