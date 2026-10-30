use std::num::NonZeroUsize;
use std::path::PathBuf;

use crate::register::Register;
use crate::window::{Axis, Side};

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
    Image(PathBuf),
    PasteImage,
    /// `gf`: follows the link, URL or file path under the primary cursor.
    GotoFile,
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

    Window(WindowCommand),
    CycleBuffer(Cycle),
    Tree(TreeCommand),
    Picker(PickerCommand),

    GotoHunk(HunkTarget),
    Hunk(HunkOp),
    File(FileOp),
    Confirm(Answer),

    ToggleMode(Shown),
    CycleMode(Direction),
    ResizeSplit(Resize),
    Changes(ChangesCommand),

    GotoConflict(Direction),
    Resolve(Take),

    Read(ReadCommand),
    Document(DocumentCommand),
    Drawing(DrawingCommand),
    View(ViewCommand),
    Export(ExportAs),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExportAs {
    Pdf,
    Svg,
    Png,
}

impl ExportAs {
    pub fn extension(self) -> &'static str {
        match self {
            ExportAs::Pdf => "pdf",
            ExportAs::Svg => "svg",
            ExportAs::Png => "png",
        }
    }
}

/// Keys of a focused document or pages view.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReadCommand {
    Move(ReadMove),
    /// Back to the tab's source.
    Leave,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReadMove {
    Scroll(Direction, ScrollBy),
    Top,
    Bottom,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DocumentCommand {
    /// The next or previous top-level heading.
    Heading(Direction),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ZoomBy {
    In,
    Out,
    Reset,
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
    Plain(PlainObject),
    /// Fails without a syntax tree.
    Syntax(SyntaxObject),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlainObject {
    Word,
    LongWord,
    Paragraph,
    ClosestPair,
    Pair(char),
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
    /// Like `Create`, but the typed name is always a folder.
    CreateFolder,
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

/// The Changes sidebar: files changed against HEAD, grouped as git shows them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChangesCommand {
    /// Shows and focuses the sidebar, or hides it when it already has focus.
    Toggle,
    FocusEditor,
    Up,
    Down,
    /// Opens the file's diff, or its text when it has conflicts.
    Open,
    Stage,
    Unstage,
    /// Asks for confirmation first.
    Discard,
    /// Collapses a group, or moves from an entry to its group.
    Collapse,
}

/// Which sections of a conflict replace it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Take {
    Ours,
    Theirs,
    /// Ours, then theirs.
    Both,
    /// The common ancestor of a diff3 conflict.
    Base,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PickerKind {
    Files,
    Buffers,
    Grep,
    Commands,
    /// Image files, to put into the focused drawing or note.
    Images,
    /// Notes linking to the focused note.
    Backlinks,
    Outline,
    Notebook,
    Annotations,
}

/// What a tab shows of its file; each kind of file offers some of these.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shown {
    Source,
    /// Side by side with the file at HEAD.
    Diff,
    Render,
    /// The source beside its rendering, which follows the source cursor.
    Split,
}

/// How a split's source side changes width.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Resize {
    Narrow,
    Widen,
    Even,
}

/// Keys on a focused drawing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DrawingCommand {
    Page(PageCommand),
    Ink(InkCommand),
    Pdf(PdfCommand),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PdfCommand {
    Read,
    Markup,
    Copy,
    Find,
    FindSelection,
    FindNext(Direction),
    Annotate(Annotate),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Annotate {
    Highlight,
    Comment,
    Remove,
    Bookmark,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ViewCommand {
    Zoom(ZoomBy),
    Pan(Side),
    /// Steps the fit: the width, the height, the whole page.
    Fit,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PageCommand {
    Go(PageStep),
    /// A blank page like the one in focus, beside it.
    Insert(PageSide),
    Remove,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PageStep {
    Next,
    Previous,
    First,
    Last,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PageSide {
    Above,
    Below,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InkCommand {
    Tool(InkTool),
    Layer(Direction),
    ToggleLayer,
    NewLayer,
    FinishText,
    DeletePicked,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InkTool {
    Brush,
    Eraser,
    BoxSelect,
    LassoSelect,
    Line,
    Arrow,
    Rectangle,
    Ellipse,
    Text,
}
