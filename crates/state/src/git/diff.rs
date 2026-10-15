//! Line diffs between HEAD, the index and a buffer, ported from helix-vcs (MPL-2.0) without its
//! worker thread: the line cache moves into each diff job and back.

use std::iter::Peekable;
use std::mem::transmute;
use std::ops::Range;

use git2::Oid;
use helix_core::{Rope, RopeSlice};
pub use imara_diff::Hunk;
use imara_diff::{Algorithm, IndentHeuristic, IndentLevel, Interner, Token};

use crate::Refresh;
use crate::git::changes::Arrive;
use crate::view::display::DiffSign;
use crate::view::layout::PaneId;

const ALGORITHM: Algorithm = Algorithm::Histogram;
const MAX_DIFF_LINES: usize = 64 * u16::MAX as usize;
// Caps the average line length at 128 bytes for files of `MAX_DIFF_LINES`.
const MAX_DIFF_BYTES: usize = MAX_DIFF_LINES * 128;

/// Git's view of a text buffer.
pub struct Diff {
    pub bases: Bases,
    /// Base reads are coalesced per buffer like git status refreshes.
    pub read: Refresh,
    /// Where `]g` puts the cursor once fresh hunks arrive.
    pub landing: Option<Landing>,
}

impl Diff {
    pub const LOADING: Diff = Diff {
        bases: Bases::Loading,
        read: Refresh::Idle,
        landing: None,
    };
}

pub enum Bases {
    /// Outside any repository's worktree.
    NoRepo,
    /// The first base read has not finished.
    Loading,
    /// In neither HEAD nor the index.
    Untracked,
    Tracked(Tracked),
}

/// A committed or staged copy of a file. Blobs are compared raw: no CRLF conversion or filters.
#[derive(Clone, Debug)]
pub enum Base {
    Absent,
    Blob { oid: Oid, text: Rope },
}

impl Base {
    pub fn oid(&self) -> Option<Oid> {
        match self {
            Base::Absent => None,
            Base::Blob { oid, .. } => Some(*oid),
        }
    }

    /// An absent base compares as empty text.
    pub fn text(&self) -> Rope {
        match self {
            Base::Absent => Rope::new(),
            Base::Blob { text, .. } => text.clone(),
        }
    }
}

/// One base side of a read: unchanged since the request, or read anew.
#[derive(Debug)]
pub enum BaseRead {
    Kept,
    Fresh(Base),
}

/// At least one of `head` and `index` is a blob; both absent is `Bases::Untracked`.
pub struct Tracked {
    pub head: Base,
    pub index: Base,
    /// HEAD to buffer text, sorted and non-overlapping in both texts.
    pub hunks: Vec<Hunk>,
    /// Index to buffer text; classifies `hunks` and drives staging.
    pub unstaged: Vec<Hunk>,
    /// HEAD to index; drives unstaging.
    pub staged: Vec<Hunk>,
    /// Inputs the hunks were computed from; `None` until the first diff lands.
    pub stamp: Option<Stamp>,
    pub compute: Compute,
}

impl Tracked {
    pub fn new(head: Base, index: Base) -> Tracked {
        Tracked {
            head,
            index,
            hunks: Vec::new(),
            unstaged: Vec::new(),
            staged: Vec::new(),
            stamp: None,
            compute: Compute::Idle(Box::new(Lines::new())),
        }
    }

    /// Whether the hunks describe `version` against the current bases.
    pub fn is_fresh(&self, version: u64) -> bool {
        self.stamp == Some(self.current(version))
    }

    pub fn current(&self, version: u64) -> Stamp {
        Stamp {
            version,
            bases: [self.head.oid(), self.index.oid()],
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Stamp {
    /// `TextDoc::version` of the diffed text.
    pub version: u64,
    /// HEAD and index blob ids.
    pub bases: [Option<Oid>; 2],
}

/// At most one diff job runs per buffer; staleness against the stamp queues the next.
pub enum Compute {
    /// The line cache waits here between diffs.
    Idle(Box<Lines>),
    /// The line cache travels with the job.
    Running,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Landing {
    pub pane: PaneId,
    pub edge: Edge,
    pub arrive: Arrive,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Edge {
    First,
    Last,
}

/// How much of a HEAD-to-buffer hunk is already in the index.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stage {
    Staged,
    Unstaged,
    Partial,
}

/// Which text of a hunk line ranges refer to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    Before,
    After,
}

impl Side {
    pub fn of(self, hunk: &Hunk) -> Range<u32> {
        match self {
            Side::Before => hunk.before.clone(),
            Side::After => hunk.after.clone(),
        }
    }
}

const HEAD: usize = 0;
const INDEX: usize = 1;
const WORK: usize = 2;

/// Interned lines of HEAD, the index and the buffer, kept across diffs so an edit only
/// re-interns the buffer. Private: tokens borrow the boxed ropes, which stay put until cleared.
pub struct Lines {
    texts: [Box<Rope>; 3],
    interner: Interner<RopeSlice<'static>>,
    tokens: [Vec<Token>; 3],
    /// Tokens of both bases; the buffer's come after.
    base_tokens: u32,
    /// Blob ids of the interned bases; `None` before the first diff.
    bases: Option<[Option<Oid>; 2]>,
    diff: imara_diff::Diff,
    /// Filled by `compute`, then swapped with the buffer's lists so their allocations cycle.
    pub hunks: Vec<Hunk>,
    pub unstaged: Vec<Hunk>,
}

/// What a diff job found.
pub enum Outcome {
    /// `Lines::hunks` and `Lines::unstaged` hold the hunks; `staged` is new when the bases moved.
    Diffed { staged: Option<Vec<Hunk>> },
    /// Too big to diff interactively; shown without hunks.
    TooLarge,
}

impl Lines {
    pub fn new() -> Lines {
        Lines {
            texts: [
                Box::new(Rope::new()),
                Box::new(Rope::new()),
                Box::new(Rope::new()),
            ],
            interner: Interner::new(0),
            tokens: [Vec::new(), Vec::new(), Vec::new()],
            base_tokens: 0,
            bases: None,
            diff: imara_diff::Diff::default(),
            hunks: Vec::new(),
            unstaged: Vec::new(),
        }
    }

    /// Diffs `text` against both bases, re-interning the bases only when their ids moved.
    pub fn compute(&mut self, stamp: Stamp, head: &Base, index: &Base, text: Rope) -> Outcome {
        let too_large =
            |rope: &Rope| rope.len_lines() > MAX_DIFF_LINES || rope.len_bytes() > MAX_DIFF_BYTES;
        let bases_moved = self.bases != Some(stamp.bases);
        match bases_moved {
            // Every token may borrow a replaced rope, so the interner empties first.
            true => {
                self.interner.clear();
                self.tokens.iter_mut().for_each(Vec::clear);
                self.bases = None;
                *self.texts[HEAD] = head.text();
                *self.texts[INDEX] = index.text();
            }
            // Only the buffer's tokens borrow the rope about to be replaced.
            false => {
                self.interner.erase_tokens_after(Token(self.base_tokens));
                self.tokens[WORK].clear();
            }
        }
        *self.texts[WORK] = text;
        match self.texts.iter().any(|rope| too_large(rope)) {
            true => {
                self.interner.clear();
                self.tokens.iter_mut().for_each(Vec::clear);
                self.bases = None;
                self.hunks.clear();
                self.unstaged.clear();
                return Outcome::TooLarge;
            }
            false => {}
        }
        let mut staged = None;
        match bases_moved {
            true => {
                for side in [HEAD, INDEX] {
                    // Safety: only the lifetime changes; the rope is boxed in `self.texts` and
                    // replaced only after the interner forgets every token borrowing it.
                    let lines = self.texts[side].lines().map(|line| unsafe {
                        transmute::<RopeSlice<'_>, RopeSlice<'static>>(line)
                    });
                    let interner = &mut self.interner;
                    self.tokens[side].extend(lines.map(|line| interner.intern(line)));
                }
                self.base_tokens = self.interner.num_tokens();
                self.bases = Some(stamp.bases);
                let mut hunks = Vec::new();
                diff_into(
                    &mut self.diff,
                    &self.interner,
                    &self.tokens[HEAD],
                    &self.tokens[INDEX],
                    &mut hunks,
                );
                staged = Some(hunks);
            }
            false => {}
        }
        // Safety: as above, for the buffer's rope.
        let lines = self.texts[WORK]
            .lines()
            .map(|line| unsafe { transmute::<RopeSlice<'_>, RopeSlice<'static>>(line) });
        let interner = &mut self.interner;
        self.tokens[WORK].extend(lines.map(|line| interner.intern(line)));
        diff_into(
            &mut self.diff,
            &self.interner,
            &self.tokens[HEAD],
            &self.tokens[WORK],
            &mut self.hunks,
        );
        match stamp.bases[0] == stamp.bases[1] {
            true => {
                self.unstaged.clear();
                self.unstaged.extend_from_slice(&self.hunks);
            }
            false => diff_into(
                &mut self.diff,
                &self.interner,
                &self.tokens[INDEX],
                &self.tokens[WORK],
                &mut self.unstaged,
            ),
        }
        Outcome::Diffed { staged }
    }
}

impl Default for Lines {
    fn default() -> Lines {
        Lines::new()
    }
}

fn diff_into(
    diff: &mut imara_diff::Diff,
    interner: &Interner<RopeSlice<'static>>,
    before: &[Token],
    after: &[Token],
    out: &mut Vec<Hunk>,
) {
    diff.compute_with(ALGORITHM, before, after, interner.num_tokens());
    diff.postprocess_with(
        before,
        after,
        IndentHeuristic::new(|token| IndentLevel::for_ascii_line(interner[token].bytes(), 4)),
    );
    out.clear();
    out.extend(diff.hunks());
}

/// Index of the first hunk starting after `line` on `side`.
pub fn next_hunk(hunks: &[Hunk], side: Side, line: u32) -> Option<usize> {
    match hunks.binary_search_by_key(&line, |hunk| side.of(hunk).start) {
        Ok(pos) if pos + 1 == hunks.len() => None,
        Ok(pos) => Some(pos + 1),
        Err(pos) if pos == hunks.len() => None,
        Err(pos) => Some(pos),
    }
}

/// Index of the last hunk ending before `line` on `side`.
pub fn prev_hunk(hunks: &[Hunk], side: Side, line: u32) -> Option<usize> {
    match hunks.binary_search_by_key(&line, |hunk| side.of(hunk).end) {
        // Removals show on the line after them, so being on that line is inside the hunk.
        Ok(pos) if !side.of(&hunks[pos]).is_empty() => Some(pos),
        Err(0) | Ok(0) => None,
        Err(pos) | Ok(pos) => Some(pos - 1),
    }
}

/// Index of the hunk containing `line`; with `Removal::Include` a removal counts on its line.
pub fn hunk_at(hunks: &[Hunk], side: Side, line: u32, removal: Removal) -> Option<usize> {
    match hunks.binary_search_by_key(&line, |hunk| side.of(hunk).start) {
        Ok(pos) => Some(pos),
        Err(0) => None,
        Err(pos) => {
            let range = side.of(&hunks[pos - 1]);
            let removal_here = match removal {
                Removal::Include => range.start == line && range.is_empty(),
                Removal::Exclude => false,
            };
            match range.end > line || removal_here {
                true => Some(pos - 1),
                false => None,
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Removal {
    Include,
    Exclude,
}

/// Hunks touching any of the sorted, inclusive `(start, end)` line ranges, each at most once.
pub fn intersecting<I: Iterator<Item = (usize, usize)>>(
    hunks: &[Hunk],
    side: Side,
    ranges: I,
) -> Intersecting<'_, I> {
    Intersecting {
        hunks,
        side,
        ranges: ranges.peekable(),
        cursor: 0,
    }
}

pub struct Intersecting<'a, I: Iterator<Item = (usize, usize)>> {
    hunks: &'a [Hunk],
    side: Side,
    ranges: Peekable<I>,
    cursor: usize,
}

impl<'a, I: Iterator<Item = (usize, usize)>> Iterator for Intersecting<'a, I> {
    type Item = &'a Hunk;

    fn next(&mut self) -> Option<&'a Hunk> {
        let side = self.side;
        loop {
            let &(start, end) = self.ranges.peek()?;
            let hunk = self.hunks.get(self.cursor)?;
            let range = side.of(hunk);
            match ((range.end as usize) < start, (range.start as usize) <= end) {
                (true, _) => {
                    self.cursor += self.hunks[self.cursor..]
                        .partition_point(|hunk| (side.of(hunk).end as usize) < start)
                }
                (false, true) => {
                    self.cursor += 1;
                    return Some(hunk);
                }
                (false, false) => {
                    self.ranges.next();
                }
            }
        }
    }
}

/// Classifies a HEAD-to-buffer hunk by the index-to-buffer hunks over the same lines, as zed does.
pub fn stage(hunk: &Hunk, unstaged: &[Hunk]) -> Stage {
    let at = |range: &Range<u32>| range.start..range.end.max(range.start + 1);
    let lines = at(&hunk.after);
    let first = unstaged.partition_point(|other| at(&other.after).end <= lines.start);
    let mut overlapping = unstaged[first..]
        .iter()
        .take_while(|other| at(&other.after).start < lines.end);
    match (overlapping.next(), overlapping.next()) {
        (None, _) => Stage::Staged,
        (Some(only), None) if only.after == hunk.after => Stage::Unstaged,
        (Some(_), _) => Stage::Partial,
    }
}

/// Gutter sign of buffer `line`; a removal shows on the line after it.
pub fn sign_at(tracked: &Tracked, line: usize) -> Option<(DiffSign, Stage)> {
    let index = hunk_at(&tracked.hunks, Side::After, line as u32, Removal::Include)?;
    let hunk = &tracked.hunks[index];
    let sign = match (hunk.before.is_empty(), hunk.after.is_empty()) {
        (true, _) => DiffSign::Added,
        (false, true) => DiffSign::Removed,
        (false, false) => DiffSign::Modified,
    };
    Some((sign, stage(hunk, &tracked.unstaged)))
}

/// `target` with each hunk's `target_side` lines replaced by its other side's lines from `source`.
pub fn splice<'a>(
    target: &Rope,
    source: &Rope,
    hunks: impl DoubleEndedIterator<Item = &'a Hunk>,
    target_side: Side,
) -> Rope {
    let source_side = match target_side {
        Side::Before => Side::After,
        Side::After => Side::Before,
    };
    let mut out = target.clone();
    // From the end, so earlier line numbers stay valid.
    for hunk in hunks.rev() {
        let (to, from) = (target_side.of(hunk), source_side.of(hunk));
        let start = out.line_to_char(to.start as usize);
        let end = out.line_to_char(to.end as usize);
        out.remove(start..end);
        let slice = source.slice(
            source.line_to_char(from.start as usize)..source.line_to_char(from.end as usize),
        );
        let mut at = start;
        for chunk in slice.chunks() {
            out.insert(at, chunk);
            at += chunk.chars().count();
        }
    }
    out
}
