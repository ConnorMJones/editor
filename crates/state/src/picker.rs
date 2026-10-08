use std::cmp::Reverse;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use helix_core::regex::bytes::{Regex, RegexBuilder};
use model::command::{Command, Cycle, PickerCommand, TreeCommand, WindowCommand};
use model::window::{Axis, PickerKind, Side};
use nucleo::pattern::{CaseMatching, Normalization, Pattern};
use nucleo::{Matcher, Utf32Str};

use crate::buffer::BufferId;
use crate::path::RelPath;

pub struct Picker {
    pub query: String,
    pub items: Items,
    /// Indices into `items`, best first.
    pub matches: Vec<u32>,
    pub cursor: usize,
    /// Scratch for ranking: (score, haystack length, index).
    pub scored: Vec<(Reverse<u32>, u32, u32)>,
}

pub enum Items {
    Files(Arc<Vec<RelPath>>),
    /// Display label per buffer, relative to the root where possible.
    Buffers(Vec<(BufferId, String)>),
    Commands,
    Grep(Grep),
}

pub struct Grep {
    /// Matches the job generation in `State::grep`; stale results are dropped.
    pub generation: u64,
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
            Items::Buffers(_) => PickerKind::Buffers,
            Items::Commands => PickerKind::Commands,
            Items::Grep(_) => PickerKind::Grep,
        }
    }

    pub fn len(&self) -> usize {
        match self {
            Items::Files(files) => files.len(),
            Items::Buffers(buffers) => buffers.len(),
            Items::Commands => PALETTE.len(),
            Items::Grep(grep) => grep.hits.len(),
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
            Items::Buffers(buffers) => &buffers[index].1,
            Items::Commands => PALETTE[index].0,
            Items::Grep(grep) => &grep.hits[index].text,
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
        }
    }
}

/// Re-ranks `picker.matches` for its query; narrows from the current matches when
/// `narrowing`, which holds when the query only grew and has no negations.
pub fn filter(picker: &mut Picker, matcher: &mut Matcher, narrowing: Narrowing) {
    matcher.config = nucleo::Config::DEFAULT;
    match picker.items.kind() {
        PickerKind::Files => matcher.config.set_match_paths(),
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

/// Smart case: any uppercase character makes the search case-sensitive; falls back to a
/// literal search when `query` is not a valid regex.
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
        "buffer-picker",
        Command::Picker(PickerCommand::Open(PickerKind::Buffers)),
    ),
    (
        "global-search",
        Command::Picker(PickerCommand::Open(PickerKind::Grep)),
    ),
    ("undo", Command::Undo),
    ("redo", Command::Redo),
];
