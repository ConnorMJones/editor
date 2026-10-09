use std::num::NonZeroUsize;
use std::path::PathBuf;

use crate::register::Register;
use crate::window::{Axis, PickerKind, Side};

/// Everything the user can ask the editor to do, from keys or the `:` prompt.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Command {
    NormalMode,
    InsertMode,
    AppendMode,
    SelectMode,
    InsertAtLineStart,
    InsertAtLineEnd,
    OpenBelow,
    OpenAbove,

    /// Select mode extends the selection instead of moving it.
    Move(Motion),
    FindChar(Find, CharTarget),
    SelectTextObject(ObjectScope, Object),
    GotoSyntaxObject(SyntaxObject, Direction),
    RepeatMotion,

    CollapseSelection,
    FlipSelections,
    EnsureSelectionsForward,
    KeepPrimarySelection,
    RemovePrimarySelection,
    RotateSelections(Direction),
    RotateSelectionContents(Direction),
    SelectAll,
    ExtendLineBelow,
    ExtendToLineBounds,
    ShrinkToLineBounds,
    SplitSelectionOnNewline,
    MergeSelections,
    MergeConsecutiveSelections,
    CopySelectionOnLine(Direction),
    TrimSelections,
    AlignSelections,
    ExpandSelection,
    ShrinkSelection,
    SelectSibling(Direction),
    SelectAllSiblings,
    SelectAllChildren,
    SearchNext(Direction),
    SearchSelection(WordBounds),

    Scroll(Direction, ScrollBy),
    /// Scrolls so the primary cursor sits at this row of the view.
    AlignView(Align),
    /// `Z`: view-mode keys repeat until a key outside view mode.
    StickyView,

    InsertChar(char),
    InsertNewline,
    InsertTab,
    SmartTab,
    DeleteCharBackward,
    DeleteCharForward,
    DeleteWordBackward,
    DeleteWordForward,
    KillToLineStart,
    Delete(Yanking),
    Change(Yanking),
    Replace(CharTarget),
    ChangeCase(Case),
    Indent,
    Unindent,
    Join(Join),
    ToggleComments,
    SurroundAdd(CharTarget),
    SurroundReplace(SurroundPair, char),
    SurroundDelete(SurroundPair),
    AddNewline(Direction),
    Increment,
    Decrement,

    /// Uses the register chosen with `"`, else the default register.
    Yank,
    Paste(Paste),
    ReplaceWithYanked,
    YankTo(Register),
    YankMainTo(Register),
    PasteFrom(Register, Paste),
    ReplaceFrom(Register),

    Undo,
    Redo,
    Earlier,
    Later,
    CommitUndoCheckpoint,
    RepeatInsert,

    /// Waits for the next key to complete the command.
    Await(Pending),

    CommandMode,
    RegexPrompt(RegexOp),
    PromptInsert(char),
    Prompt(PromptCommand),
    PromptSubmit,
    PromptCancel,

    Open(PathBuf),
    Write,
    WriteAs(PathBuf, Dirs),
    ForceWrite,
    WriteAll,
    WriteQuit,
    WriteAllQuit,
    Quit,
    ForceQuit,
    CloseBuffer,
    ForceCloseBuffer,
    Reload,
    ReloadAll,
    GotoLine(NonZeroUsize),
    Sort(SortCase, SortOrder),
    /// `None` uses the buffer's configured text width.
    Reflow(Option<NonZeroUsize>),

    // Workspace chrome: panes, buffer cycling, file tree, pickers.
    Window(WindowCommand),
    CycleBuffer(Cycle),
    Tree(TreeCommand),
    Picker(PickerCommand),

    // Git: hunks against HEAD, staging and changed files.
    /// Select mode extends to the hunk; past the last or first hunk it opens the next changed file.
    GotoHunk(HunkTarget),
    /// Acts on the hunks touched by any selection.
    Hunk(HunkOp),
    /// Paths are relative to the root or absolute.
    File(FileOp),
    /// Answers the pending confirmation, such as a discard.
    Confirm(Answer),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HunkTarget {
    Next,
    Previous,
    First,
    Last,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HunkOp {
    Stage,
    Unstage,
    /// Restores HEAD's lines as an undoable edit.
    Revert,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FileOp {
    Stage(PathBuf),
    Unstage(PathBuf),
    /// Restores the index copy, or deletes an untracked file; asks for confirmation first.
    Discard(PathBuf),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Answer {
    Yes,
    No,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    Forward,
    Backward,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Motion {
    CharLeft,
    CharRight,
    VisualLineUp,
    VisualLineDown,
    LineUp,
    LineDown,
    NextWordStart,
    PrevWordStart,
    NextWordEnd,
    NextLongWordStart,
    PrevLongWordStart,
    NextLongWordEnd,
    LineStart,
    LineEnd,
    /// Past the last character, onto the line ending; used by insert mode `end`.
    LineEndNewline,
    FirstNonWhitespace,
    /// `gg`; with a count it goes to that line.
    FileStart,
    LastLine,
    /// `G`; only moves with a count.
    Line,
    Column,
    NextParagraph,
    PrevParagraph,
    MatchingBracket,
    ParentNodeEnd,
    ParentNodeStart,
    LastModification,
    /// `gt gc gb`: a row of the visible window.
    Window(Align),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Align {
    Top,
    Center,
    Bottom,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScrollBy {
    /// `count` rows; the cursor is dragged only when it would leave the view.
    Lines,
    /// A full view; the cursor is dragged only when it would leave the view.
    Page,
    /// Half a view, moving every cursor by the same amount.
    HalfPageCursor,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Find {
    pub direction: Direction,
    pub inclusion: Inclusion,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Inclusion {
    /// `f`/`F`: land on the character.
    Inclusive,
    /// `t`/`T`: stop next to it.
    Exclusive,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CharTarget {
    Char(char),
    /// The buffer's line ending, typed as `<ret>`.
    LineEnding,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ObjectScope {
    Inside,
    Around,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Object {
    Word,
    LongWord,
    Paragraph,
    ClosestPair,
    Pair(char),
    Syntax(SyntaxObject),
}

/// Tree-sitter textobject query captures.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SyntaxObject {
    Class,
    Function,
    Parameter,
    Comment,
    Test,
    Entry,
    XmlElement,
}

impl SyntaxObject {
    pub fn capture(self) -> &'static str {
        match self {
            SyntaxObject::Class => "class",
            SyntaxObject::Function => "function",
            SyntaxObject::Parameter => "parameter",
            SyntaxObject::Comment => "comment",
            SyntaxObject::Test => "test",
            SyntaxObject::Entry => "entry",
            SyntaxObject::XmlElement => "xml-element",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SurroundPair {
    /// `m`: the nearest enclosing pair of any kind.
    Closest,
    Char(char),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WordBounds {
    /// `*`: wrap in `\b` where the selection sits on word edges.
    Detect,
    /// `A-*`
    Ignore,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Yanking {
    Yank,
    NoYank,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Case {
    Switch,
    Lower,
    Upper,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Join {
    Plain,
    /// `A-J`: select the inserted spaces.
    SelectSpace,
}

/// Line editing in the `:` and regex prompts, as in helix's prompt.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PromptCommand {
    Move(PromptMotion),
    /// Deletes from the cursor to where the motion would move it.
    Delete(PromptMotion),
    /// `Backward` steps to older history entries.
    History(Direction),
    Complete(Direction),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PromptMotion {
    CharBackward,
    CharForward,
    WordBackward,
    WordForward,
    Start,
    End,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Paste {
    Before,
    After,
    Cursor,
}

/// Commands that read one more key before running.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pending {
    Find(Find),
    Replace,
    SelectRegister,
    InsertRegister,
    TextObject(ObjectScope),
    SurroundAdd,
    SurroundReplace,
    SurroundReplaceWith(SurroundPair),
    SurroundDelete,
}

/// What a regex prompt does with its pattern.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RegexOp {
    Search(Direction),
    Select,
    Split,
    Keep,
    Remove,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Dirs {
    Existing,
    /// `:w!` creates missing parent directories.
    Create,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SortCase {
    Sensitive,
    Insensitive,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SortOrder {
    Ascending,
    Descending,
}

/// Helix window mode (`C-w`, `space w`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WindowCommand {
    Split(Axis),
    Focus(Side),
    FocusNext,
    Close,
    Only,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cycle {
    Next,
    Previous,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TreeCommand {
    /// Shows and focuses the tree, or hides it when it already has focus.
    Toggle,
    FocusEditor,
    Up,
    Down,
    /// Expands a directory or opens a file.
    Open,
    /// Collapses a directory or moves to its parent.
    Close,
    Create,
    Rename,
    Delete,
    EditInsert(char),
    EditBackspace,
    EditSubmit,
    EditCancel,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PickerCommand {
    Open(PickerKind),
    Insert(char),
    Backspace,
    Next,
    Previous,
    Submit,
    Cancel,
}
