use model::command::{
    Align, Answer, Case, ChangesCommand, Command, Cycle, Direction, Find, HunkOp, HunkTarget,
    Inclusion, Join, Motion, ObjectScope, Paste, Pending, PickerCommand, PromptCommand,
    PromptMotion, RegexOp, RenderedCommand, ScrollBy, SyntaxObject, Take, TreeCommand,
    WindowCommand, WordBounds, Yanking, ZoomBy,
};
use model::key::{Key, KeyCode, Mods};
use model::register::{Clipboard, Register};
use model::window::{Axis, PickerKind, Side};

/// Which binding table applies; derived from mode, overlay and focus each key press.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Context {
    Normal,
    Insert,
    Select,
    Prompt,
    Tree,
    TreeInput,
    Picker,
    /// The Changes sidebar has focus.
    Changes,
    /// An action such as a discard waits for `y` or `n`.
    Confirm,
    /// A rendered markdown or typst view has focus.
    Rendered,
}

impl Context {
    pub const COUNT: usize = 10;
    pub const ALL: [Context; Context::COUNT] = [
        Context::Normal,
        Context::Insert,
        Context::Select,
        Context::Prompt,
        Context::Tree,
        Context::TreeInput,
        Context::Picker,
        Context::Changes,
        Context::Confirm,
        Context::Rendered,
    ];
}

pub enum Trie {
    Leaf(Command),
    Node(Node),
}

/// `keys` holds `Key::packed` values sorted for binary search; `children` is parallel to it.
#[derive(Default)]
pub struct Node {
    pub keys: Vec<u32>,
    pub children: Vec<Trie>,
}

pub enum Resolved<'a> {
    Command(&'a Command),
    Pending,
    Unbound,
}

pub struct Keymap {
    pub tries: [Trie; Context::COUNT],
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum KeymapError {
    #[error("{context:?}: binding {keys:?} conflicts with an existing prefix or leaf")]
    Conflict { context: Context, keys: Vec<Key> },
    #[error("{context:?}: empty key sequence")]
    Empty { context: Context },
}

type Bindings = &'static [(&'static [Key], Command)];

const SAVE_ALL: [Key; 2] = [Key::ctrl('k'), Key::ctrl('s')];

const fn with(code: KeyCode, mods: Mods) -> Key {
    Key { code, mods }
}

const SYSTEM: Register = Register::Clipboard(Clipboard::System);

const fn find(direction: Direction, inclusion: Inclusion) -> Command {
    Command::Await(Pending::Find(Find {
        direction,
        inclusion,
    }))
}

/// Bindings shared by normal and select mode; motions extend in select mode.
const COMMON: Bindings = &[
    (&[Key::char('h')], Command::Move(Motion::CharLeft)),
    (&[Key::code(KeyCode::Left)], Command::Move(Motion::CharLeft)),
    (&[Key::char('j')], Command::Move(Motion::VisualLineDown)),
    (
        &[Key::code(KeyCode::Down)],
        Command::Move(Motion::VisualLineDown),
    ),
    (&[Key::char('k')], Command::Move(Motion::VisualLineUp)),
    (
        &[Key::code(KeyCode::Up)],
        Command::Move(Motion::VisualLineUp),
    ),
    (&[Key::char('l')], Command::Move(Motion::CharRight)),
    (
        &[Key::code(KeyCode::Right)],
        Command::Move(Motion::CharRight),
    ),
    (
        &[Key::char('t')],
        find(Direction::Forward, Inclusion::Exclusive),
    ),
    (
        &[Key::char('f')],
        find(Direction::Forward, Inclusion::Inclusive),
    ),
    (
        &[Key::char('T')],
        find(Direction::Backward, Inclusion::Exclusive),
    ),
    (
        &[Key::char('F')],
        find(Direction::Backward, Inclusion::Inclusive),
    ),
    (&[Key::char('r')], Command::Await(Pending::Replace)),
    (&[Key::char('R')], Command::ReplaceWithYanked),
    (&[Key::alt('.')], Command::RepeatMotion),
    (&[Key::char('~')], Command::ChangeCase(Case::Switch)),
    (&[Key::char('`')], Command::ChangeCase(Case::Lower)),
    (&[Key::alt('`')], Command::ChangeCase(Case::Upper)),
    (
        &[Key::code(KeyCode::Home)],
        Command::Move(Motion::LineStart),
    ),
    (&[Key::code(KeyCode::End)], Command::Move(Motion::LineEnd)),
    (&[Key::char('w')], Command::Move(Motion::NextWordStart)),
    (&[Key::char('b')], Command::Move(Motion::PrevWordStart)),
    (&[Key::char('e')], Command::Move(Motion::NextWordEnd)),
    (&[Key::char('W')], Command::Move(Motion::NextLongWordStart)),
    (&[Key::char('B')], Command::Move(Motion::PrevLongWordStart)),
    (&[Key::char('E')], Command::Move(Motion::NextLongWordEnd)),
    (&[Key::char('G')], Command::Move(Motion::Line)),
    (
        &[Key::char('g'), Key::char('g')],
        Command::Move(Motion::FileStart),
    ),
    (
        &[Key::char('g'), Key::char('|')],
        Command::Move(Motion::Column),
    ),
    (
        &[Key::char('g'), Key::char('e')],
        Command::Move(Motion::LastLine),
    ),
    (
        &[Key::char('g'), Key::char('h')],
        Command::Move(Motion::LineStart),
    ),
    (
        &[Key::char('g'), Key::char('l')],
        Command::Move(Motion::LineEnd),
    ),
    (
        &[Key::char('g'), Key::char('s')],
        Command::Move(Motion::FirstNonWhitespace),
    ),
    (
        &[Key::char('g'), Key::char('k')],
        Command::Move(Motion::LineUp),
    ),
    (
        &[Key::char('g'), Key::char('j')],
        Command::Move(Motion::LineDown),
    ),
    (
        &[Key::char('g'), Key::char('.')],
        Command::Move(Motion::LastModification),
    ),
    (&[Key::char('g'), Key::char('f')], Command::GotoFile),
    // Arrow aliases for `gh` `gl` `gg` `ge`; not in helix.
    (
        &[Key::char('g'), Key::code(KeyCode::Left)],
        Command::Move(Motion::LineStart),
    ),
    (
        &[Key::char('g'), Key::code(KeyCode::Right)],
        Command::Move(Motion::LineEnd),
    ),
    (
        &[Key::char('g'), Key::code(KeyCode::Up)],
        Command::Move(Motion::FileStart),
    ),
    (
        &[Key::char('g'), Key::code(KeyCode::Down)],
        Command::Move(Motion::LastLine),
    ),
    (&[Key::char(':')], Command::CommandMode),
    (&[Key::char('i')], Command::InsertMode),
    (&[Key::char('I')], Command::InsertAtLineStart),
    (&[Key::char('a')], Command::AppendMode),
    (&[Key::char('A')], Command::InsertAtLineEnd),
    (&[Key::char('o')], Command::OpenBelow),
    (&[Key::char('O')], Command::OpenAbove),
    (&[Key::char('d')], Command::Delete(Yanking::Yank)),
    (&[Key::alt('d')], Command::Delete(Yanking::NoYank)),
    (&[Key::char('c')], Command::Change(Yanking::Yank)),
    (&[Key::alt('c')], Command::Change(Yanking::NoYank)),
    (
        &[Key::char('C')],
        Command::CopySelectionOnLine(Direction::Forward),
    ),
    (
        &[Key::alt('C')],
        Command::CopySelectionOnLine(Direction::Backward),
    ),
    (&[Key::char('s')], Command::RegexPrompt(RegexOp::Select)),
    (&[Key::alt('s')], Command::SplitSelectionOnNewline),
    (&[Key::alt('-')], Command::MergeSelections),
    (&[Key::alt('_')], Command::MergeConsecutiveSelections),
    (&[Key::char('S')], Command::RegexPrompt(RegexOp::Split)),
    (&[Key::char(';')], Command::CollapseSelection),
    (&[Key::alt(';')], Command::FlipSelections),
    (&[Key::alt('o')], Command::ExpandSelection),
    (&[with(KeyCode::Up, Mods::ALT)], Command::ExpandSelection),
    (&[Key::alt('i')], Command::ShrinkSelection),
    (&[with(KeyCode::Down, Mods::ALT)], Command::ShrinkSelection),
    (&[Key::alt('I')], Command::SelectAllChildren),
    (
        &[with(KeyCode::Down, Mods::ALT.union(Mods::SHIFT))],
        Command::SelectAllChildren,
    ),
    (
        &[Key::alt('p')],
        Command::SelectSibling(Direction::Backward),
    ),
    (
        &[with(KeyCode::Left, Mods::ALT)],
        Command::SelectSibling(Direction::Backward),
    ),
    (&[Key::alt('n')], Command::SelectSibling(Direction::Forward)),
    (
        &[with(KeyCode::Right, Mods::ALT)],
        Command::SelectSibling(Direction::Forward),
    ),
    (&[Key::alt('e')], Command::Move(Motion::ParentNodeEnd)),
    (&[Key::alt('b')], Command::Move(Motion::ParentNodeStart)),
    (&[Key::alt('a')], Command::SelectAllSiblings),
    (&[Key::char('%')], Command::SelectAll),
    (&[Key::char('x')], Command::ExtendLineBelow),
    (&[Key::char('X')], Command::ExtendToLineBounds),
    (&[Key::alt('x')], Command::ShrinkToLineBounds),
    (
        &[Key::char('m'), Key::char('m')],
        Command::Move(Motion::MatchingBracket),
    ),
    (
        &[Key::char('m'), Key::char('s')],
        Command::Await(Pending::SurroundAdd),
    ),
    (
        &[Key::char('m'), Key::char('r')],
        Command::Await(Pending::SurroundReplace),
    ),
    (
        &[Key::char('m'), Key::char('d')],
        Command::Await(Pending::SurroundDelete),
    ),
    (
        &[Key::char('m'), Key::char('a')],
        Command::Await(Pending::TextObject(ObjectScope::Around)),
    ),
    (
        &[Key::char('m'), Key::char('i')],
        Command::Await(Pending::TextObject(ObjectScope::Inside)),
    ),
    (
        &[Key::char('['), Key::char('f')],
        Command::GotoSyntaxObject(SyntaxObject::Function, Direction::Backward),
    ),
    (
        &[Key::char('['), Key::char('t')],
        Command::GotoSyntaxObject(SyntaxObject::Class, Direction::Backward),
    ),
    (
        &[Key::char('['), Key::char('a')],
        Command::GotoSyntaxObject(SyntaxObject::Parameter, Direction::Backward),
    ),
    (
        &[Key::char('['), Key::char('c')],
        Command::GotoSyntaxObject(SyntaxObject::Comment, Direction::Backward),
    ),
    (
        &[Key::char('['), Key::char('e')],
        Command::GotoSyntaxObject(SyntaxObject::Entry, Direction::Backward),
    ),
    (
        &[Key::char('['), Key::char('T')],
        Command::GotoSyntaxObject(SyntaxObject::Test, Direction::Backward),
    ),
    (
        &[Key::char('['), Key::char('x')],
        Command::GotoSyntaxObject(SyntaxObject::XmlElement, Direction::Backward),
    ),
    (
        &[Key::char('['), Key::char('p')],
        Command::Move(Motion::PrevParagraph),
    ),
    (
        &[Key::char('['), Key::char(' ')],
        Command::AddNewline(Direction::Backward),
    ),
    (
        &[Key::char(']'), Key::char('f')],
        Command::GotoSyntaxObject(SyntaxObject::Function, Direction::Forward),
    ),
    (
        &[Key::char(']'), Key::char('t')],
        Command::GotoSyntaxObject(SyntaxObject::Class, Direction::Forward),
    ),
    (
        &[Key::char(']'), Key::char('a')],
        Command::GotoSyntaxObject(SyntaxObject::Parameter, Direction::Forward),
    ),
    (
        &[Key::char(']'), Key::char('c')],
        Command::GotoSyntaxObject(SyntaxObject::Comment, Direction::Forward),
    ),
    (
        &[Key::char(']'), Key::char('e')],
        Command::GotoSyntaxObject(SyntaxObject::Entry, Direction::Forward),
    ),
    (
        &[Key::char(']'), Key::char('T')],
        Command::GotoSyntaxObject(SyntaxObject::Test, Direction::Forward),
    ),
    (
        &[Key::char(']'), Key::char('x')],
        Command::GotoSyntaxObject(SyntaxObject::XmlElement, Direction::Forward),
    ),
    (
        &[Key::char(']'), Key::char('p')],
        Command::Move(Motion::NextParagraph),
    ),
    (
        &[Key::char(']'), Key::char('g')],
        Command::GotoHunk(HunkTarget::Next),
    ),
    (
        &[Key::char('['), Key::char('g')],
        Command::GotoHunk(HunkTarget::Previous),
    ),
    (
        &[Key::char(']'), Key::char('G')],
        Command::GotoHunk(HunkTarget::Last),
    ),
    (
        &[Key::char('['), Key::char('G')],
        Command::GotoHunk(HunkTarget::First),
    ),
    (
        &[Key::char(' '), Key::char('h'), Key::char('s')],
        Command::Hunk(HunkOp::Stage),
    ),
    (
        &[Key::char(' '), Key::char('h'), Key::char('u')],
        Command::Hunk(HunkOp::Unstage),
    ),
    (
        &[Key::char(' '), Key::char('h'), Key::char('r')],
        Command::Hunk(HunkOp::Revert),
    ),
    (
        &[Key::char(']'), Key::char(' ')],
        Command::AddNewline(Direction::Forward),
    ),
    (
        &[Key::char('/')],
        Command::RegexPrompt(RegexOp::Search(Direction::Forward)),
    ),
    (
        &[Key::char('?')],
        Command::RegexPrompt(RegexOp::Search(Direction::Backward)),
    ),
    (&[Key::char('n')], Command::SearchNext(Direction::Forward)),
    (&[Key::char('N')], Command::SearchNext(Direction::Backward)),
    (
        &[Key::char('*')],
        Command::SearchSelection(WordBounds::Detect),
    ),
    (
        &[Key::alt('*')],
        Command::SearchSelection(WordBounds::Ignore),
    ),
    (&[Key::char('u')], Command::Undo),
    (&[Key::char('U')], Command::Redo),
    (&[Key::alt('u')], Command::Earlier),
    (&[Key::alt('U')], Command::Later),
    (&[Key::char('y')], Command::Yank),
    (&[Key::char('p')], Command::Paste(Paste::After)),
    (&[Key::char('P')], Command::Paste(Paste::Before)),
    (&[Key::char('>')], Command::Indent),
    (&[Key::char('<')], Command::Unindent),
    (&[Key::char('J')], Command::Join(Join::Plain)),
    (&[Key::alt('J')], Command::Join(Join::SelectSpace)),
    (&[Key::char('K')], Command::RegexPrompt(RegexOp::Keep)),
    (&[Key::alt('K')], Command::RegexPrompt(RegexOp::Remove)),
    (&[Key::char(',')], Command::KeepPrimarySelection),
    (&[Key::alt(',')], Command::RemovePrimarySelection),
    (&[Key::char('&')], Command::AlignSelections),
    (&[Key::char('_')], Command::TrimSelections),
    (
        &[Key::char('(')],
        Command::RotateSelections(Direction::Backward),
    ),
    (
        &[Key::char(')')],
        Command::RotateSelections(Direction::Forward),
    ),
    (
        &[Key::alt('(')],
        Command::RotateSelectionContents(Direction::Backward),
    ),
    (
        &[Key::alt(')')],
        Command::RotateSelectionContents(Direction::Forward),
    ),
    (&[Key::alt(':')], Command::EnsureSelectionsForward),
    (&[Key::code(KeyCode::Esc)], Command::NormalMode),
    (&[Key::ctrl('c')], Command::ToggleComments),
    (&[Key::char('"')], Command::Await(Pending::SelectRegister)),
    (&[Key::ctrl('a')], Command::Increment),
    (&[Key::ctrl('x')], Command::Decrement),
    (&[Key::char('.')], Command::RepeatInsert),
    (&[Key::char(' '), Key::char('y')], Command::YankTo(SYSTEM)),
    (
        &[Key::char(' '), Key::char('Y')],
        Command::YankMainTo(SYSTEM),
    ),
    (
        &[Key::char(' '), Key::char('p')],
        Command::PasteFrom(SYSTEM, Paste::After),
    ),
    (
        &[Key::char(' '), Key::char('P')],
        Command::PasteFrom(SYSTEM, Paste::Before),
    ),
    (
        &[Key::char(' '), Key::char('R')],
        Command::ReplaceFrom(SYSTEM),
    ),
    (&SAVE_ALL, Command::WriteAll),
];

const NORMAL: Bindings = &[(&[Key::char('v')], Command::SelectMode)];

const SELECT: Bindings = &[(&[Key::char('v')], Command::NormalMode)];

const INSERT: Bindings = &[
    (&[Key::code(KeyCode::Esc)], Command::NormalMode),
    (&[Key::ctrl('s')], Command::CommitUndoCheckpoint),
    (&[Key::ctrl('r')], Command::Await(Pending::InsertRegister)),
    (&[Key::ctrl('w')], Command::DeleteWordBackward),
    (
        &[with(KeyCode::Backspace, Mods::ALT)],
        Command::DeleteWordBackward,
    ),
    (&[Key::alt('d')], Command::DeleteWordForward),
    (
        &[with(KeyCode::Delete, Mods::ALT)],
        Command::DeleteWordForward,
    ),
    (&[Key::ctrl('u')], Command::KillToLineStart),
    (&[Key::ctrl('h')], Command::DeleteCharBackward),
    (
        &[Key::code(KeyCode::Backspace)],
        Command::DeleteCharBackward,
    ),
    (
        &[with(KeyCode::Backspace, Mods::SHIFT)],
        Command::DeleteCharBackward,
    ),
    (&[Key::ctrl('d')], Command::DeleteCharForward),
    (&[Key::code(KeyCode::Delete)], Command::DeleteCharForward),
    (&[Key::ctrl('j')], Command::InsertNewline),
    (&[Key::code(KeyCode::Enter)], Command::InsertNewline),
    (&[Key::code(KeyCode::Tab)], Command::SmartTab),
    (&[with(KeyCode::Tab, Mods::SHIFT)], Command::InsertTab),
    (
        &[Key::code(KeyCode::Up)],
        Command::Move(Motion::VisualLineUp),
    ),
    (
        &[Key::code(KeyCode::Down)],
        Command::Move(Motion::VisualLineDown),
    ),
    (&[Key::code(KeyCode::Left)], Command::Move(Motion::CharLeft)),
    (
        &[Key::code(KeyCode::Right)],
        Command::Move(Motion::CharRight),
    ),
    (
        &[Key::code(KeyCode::Home)],
        Command::Move(Motion::LineStart),
    ),
    (
        &[Key::code(KeyCode::End)],
        Command::Move(Motion::LineEndNewline),
    ),
    (
        &[Key::code(KeyCode::PageUp)],
        Command::Scroll(Direction::Backward, ScrollBy::Page),
    ),
    (
        &[Key::code(KeyCode::PageDown)],
        Command::Scroll(Direction::Forward, ScrollBy::Page),
    ),
    (&SAVE_ALL, Command::WriteAll),
];

const fn prompt_move(motion: PromptMotion) -> Command {
    Command::Prompt(PromptCommand::Move(motion))
}

const fn prompt_delete(motion: PromptMotion) -> Command {
    Command::Prompt(PromptCommand::Delete(motion))
}

/// Helix's prompt line editing, from `helix-term/src/ui/prompt.rs`.
const PROMPT: Bindings = &[
    (&[Key::code(KeyCode::Esc)], Command::PromptCancel),
    (&[Key::ctrl('c')], Command::PromptCancel),
    (&[Key::code(KeyCode::Enter)], Command::PromptSubmit),
    (&[Key::alt('b')], prompt_move(PromptMotion::WordBackward)),
    (
        &[with(KeyCode::Left, Mods::CTRL)],
        prompt_move(PromptMotion::WordBackward),
    ),
    (&[Key::alt('f')], prompt_move(PromptMotion::WordForward)),
    (
        &[with(KeyCode::Right, Mods::CTRL)],
        prompt_move(PromptMotion::WordForward),
    ),
    (&[Key::ctrl('b')], prompt_move(PromptMotion::CharBackward)),
    (
        &[Key::code(KeyCode::Left)],
        prompt_move(PromptMotion::CharBackward),
    ),
    (&[Key::ctrl('f')], prompt_move(PromptMotion::CharForward)),
    (
        &[Key::code(KeyCode::Right)],
        prompt_move(PromptMotion::CharForward),
    ),
    (&[Key::ctrl('e')], prompt_move(PromptMotion::End)),
    (&[Key::code(KeyCode::End)], prompt_move(PromptMotion::End)),
    (&[Key::ctrl('a')], prompt_move(PromptMotion::Start)),
    (
        &[Key::code(KeyCode::Home)],
        prompt_move(PromptMotion::Start),
    ),
    (&[Key::ctrl('w')], prompt_delete(PromptMotion::WordBackward)),
    (
        &[with(KeyCode::Backspace, Mods::ALT)],
        prompt_delete(PromptMotion::WordBackward),
    ),
    (
        &[with(KeyCode::Backspace, Mods::CTRL)],
        prompt_delete(PromptMotion::WordBackward),
    ),
    (&[Key::alt('d')], prompt_delete(PromptMotion::WordForward)),
    (
        &[with(KeyCode::Delete, Mods::ALT)],
        prompt_delete(PromptMotion::WordForward),
    ),
    (
        &[with(KeyCode::Delete, Mods::CTRL)],
        prompt_delete(PromptMotion::WordForward),
    ),
    (&[Key::ctrl('k')], prompt_delete(PromptMotion::End)),
    (&[Key::ctrl('u')], prompt_delete(PromptMotion::Start)),
    (&[Key::ctrl('h')], prompt_delete(PromptMotion::CharBackward)),
    (
        &[Key::code(KeyCode::Backspace)],
        prompt_delete(PromptMotion::CharBackward),
    ),
    (
        &[with(KeyCode::Backspace, Mods::SHIFT)],
        prompt_delete(PromptMotion::CharBackward),
    ),
    (&[Key::ctrl('d')], prompt_delete(PromptMotion::CharForward)),
    (
        &[Key::code(KeyCode::Delete)],
        prompt_delete(PromptMotion::CharForward),
    ),
    (
        &[Key::ctrl('p')],
        Command::Prompt(PromptCommand::History(Direction::Backward)),
    ),
    (
        &[Key::code(KeyCode::Up)],
        Command::Prompt(PromptCommand::History(Direction::Backward)),
    ),
    (
        &[Key::ctrl('n')],
        Command::Prompt(PromptCommand::History(Direction::Forward)),
    ),
    (
        &[Key::code(KeyCode::Down)],
        Command::Prompt(PromptCommand::History(Direction::Forward)),
    ),
    (
        &[Key::code(KeyCode::Tab)],
        Command::Prompt(PromptCommand::Complete(Direction::Forward)),
    ),
    (
        &[with(KeyCode::Tab, Mods::SHIFT)],
        Command::Prompt(PromptCommand::Complete(Direction::Backward)),
    ),
];

/// Helix view mode, reachable as `z x`; `Z` makes it sticky.
const VIEW: Bindings = &[
    (
        &[Key::char('z'), Key::char('z')],
        Command::AlignView(Align::Center),
    ),
    (
        &[Key::char('z'), Key::char('c')],
        Command::AlignView(Align::Center),
    ),
    (
        &[Key::char('z'), Key::char('t')],
        Command::AlignView(Align::Top),
    ),
    (
        &[Key::char('z'), Key::char('b')],
        Command::AlignView(Align::Bottom),
    ),
    (
        &[Key::char('z'), Key::char('k')],
        Command::Scroll(Direction::Backward, ScrollBy::Lines),
    ),
    (
        &[Key::char('z'), Key::code(KeyCode::Up)],
        Command::Scroll(Direction::Backward, ScrollBy::Lines),
    ),
    (
        &[Key::char('z'), Key::char('j')],
        Command::Scroll(Direction::Forward, ScrollBy::Lines),
    ),
    (
        &[Key::char('z'), Key::code(KeyCode::Down)],
        Command::Scroll(Direction::Forward, ScrollBy::Lines),
    ),
    (
        &[Key::char('z'), Key::ctrl('b')],
        Command::Scroll(Direction::Backward, ScrollBy::Page),
    ),
    (
        &[Key::char('z'), Key::code(KeyCode::PageUp)],
        Command::Scroll(Direction::Backward, ScrollBy::Page),
    ),
    (
        &[Key::char('z'), Key::ctrl('f')],
        Command::Scroll(Direction::Forward, ScrollBy::Page),
    ),
    (
        &[Key::char('z'), Key::code(KeyCode::PageDown)],
        Command::Scroll(Direction::Forward, ScrollBy::Page),
    ),
    (
        &[Key::char('z'), Key::ctrl('u')],
        Command::Scroll(Direction::Backward, ScrollBy::HalfPageCursor),
    ),
    (
        &[Key::char('z'), Key::code(KeyCode::Backspace)],
        Command::Scroll(Direction::Backward, ScrollBy::HalfPageCursor),
    ),
    (
        &[Key::char('z'), Key::ctrl('d')],
        Command::Scroll(Direction::Forward, ScrollBy::HalfPageCursor),
    ),
    (
        &[Key::char('z'), Key::char(' ')],
        Command::Scroll(Direction::Forward, ScrollBy::HalfPageCursor),
    ),
    (
        &[Key::char('z'), Key::char('/')],
        Command::RegexPrompt(RegexOp::Search(Direction::Forward)),
    ),
    (
        &[Key::char('z'), Key::char('?')],
        Command::RegexPrompt(RegexOp::Search(Direction::Backward)),
    ),
    (
        &[Key::char('z'), Key::char('n')],
        Command::SearchNext(Direction::Forward),
    ),
    (
        &[Key::char('z'), Key::char('N')],
        Command::SearchNext(Direction::Backward),
    ),
    (&[Key::char('Z')], Command::StickyView),
    (
        &[Key::ctrl('b')],
        Command::Scroll(Direction::Backward, ScrollBy::Page),
    ),
    (
        &[Key::code(KeyCode::PageUp)],
        Command::Scroll(Direction::Backward, ScrollBy::Page),
    ),
    (
        &[Key::ctrl('f')],
        Command::Scroll(Direction::Forward, ScrollBy::Page),
    ),
    (
        &[Key::code(KeyCode::PageDown)],
        Command::Scroll(Direction::Forward, ScrollBy::Page),
    ),
    (
        &[Key::ctrl('u')],
        Command::Scroll(Direction::Backward, ScrollBy::HalfPageCursor),
    ),
    (
        &[Key::ctrl('d')],
        Command::Scroll(Direction::Forward, ScrollBy::HalfPageCursor),
    ),
    (
        &[Key::char('g'), Key::char('t')],
        Command::Move(Motion::Window(Align::Top)),
    ),
    (
        &[Key::char('g'), Key::char('c')],
        Command::Move(Motion::Window(Align::Center)),
    ),
    (
        &[Key::char('g'), Key::char('b')],
        Command::Move(Motion::Window(Align::Bottom)),
    ),
];

// Workspace chrome bindings: windows, buffers, file tree, pickers.

const SPACE: Key = Key::char(' ');
const CTRL_W: Key = Key::ctrl('w');

/// Helix window mode, reachable as `C-w x` and `space w x`.
macro_rules! window_bindings {
    ($($prefix:expr),+) => {
        [
            (&[$($prefix,)+ Key::char('v')], Command::Window(WindowCommand::Split(Axis::Row))),
            (&[$($prefix,)+ Key::char('s')], Command::Window(WindowCommand::Split(Axis::Column))),
            (&[$($prefix,)+ Key::char('w')], Command::Window(WindowCommand::FocusNext)),
            (&[$($prefix,)+ Key::char('h')], Command::Window(WindowCommand::Focus(Side::Left))),
            (&[$($prefix,)+ Key::char('j')], Command::Window(WindowCommand::Focus(Side::Down))),
            (&[$($prefix,)+ Key::char('k')], Command::Window(WindowCommand::Focus(Side::Up))),
            (&[$($prefix,)+ Key::char('l')], Command::Window(WindowCommand::Focus(Side::Right))),
            (&[$($prefix,)+ Key::code(KeyCode::Left)], Command::Window(WindowCommand::Focus(Side::Left))),
            (&[$($prefix,)+ Key::code(KeyCode::Down)], Command::Window(WindowCommand::Focus(Side::Down))),
            (&[$($prefix,)+ Key::code(KeyCode::Up)], Command::Window(WindowCommand::Focus(Side::Up))),
            (&[$($prefix,)+ Key::code(KeyCode::Right)], Command::Window(WindowCommand::Focus(Side::Right))),
            (&[$($prefix,)+ Key::char('q')], Command::Window(WindowCommand::Close)),
            (&[$($prefix,)+ Key::char('o')], Command::Window(WindowCommand::Only)),
        ]
    };
}

/// Shared by normal and select mode and the file tree.
const WORKSPACE: Bindings = &[
    (&[SPACE, Key::char('e')], Command::Tree(TreeCommand::Toggle)),
    (
        &[SPACE, Key::char('f')],
        Command::Picker(PickerCommand::Open(PickerKind::Files)),
    ),
    (
        &[SPACE, Key::char('b')],
        Command::Picker(PickerCommand::Open(PickerKind::Buffers)),
    ),
    (
        &[SPACE, Key::char('/')],
        Command::Picker(PickerCommand::Open(PickerKind::Grep)),
    ),
    (
        &[SPACE, Key::char('?')],
        Command::Picker(PickerCommand::Open(PickerKind::Commands)),
    ),
    (
        &[SPACE, Key::char('g')],
        Command::Changes(ChangesCommand::Toggle),
    ),
];

/// Diff views; shared by normal and select mode.
const DIFF: Bindings = &[(&[SPACE, Key::char('d')], Command::ToggleDiff)];

const WINDOW: &[(&[Key], Command); 13] = &window_bindings!(CTRL_W);
const SPACE_WINDOW: &[(&[Key], Command); 13] = &window_bindings!(SPACE, Key::char('w'));

const GOTO_BUFFER: Bindings = &[
    (
        &[Key::char('g'), Key::char('n')],
        Command::CycleBuffer(Cycle::Next),
    ),
    (
        &[Key::char('g'), Key::char('p')],
        Command::CycleBuffer(Cycle::Previous),
    ),
];

const TREE: Bindings = &[
    (&[Key::char('j')], Command::Tree(TreeCommand::Down)),
    (
        &[Key::code(KeyCode::Down)],
        Command::Tree(TreeCommand::Down),
    ),
    (&[Key::char('k')], Command::Tree(TreeCommand::Up)),
    (&[Key::code(KeyCode::Up)], Command::Tree(TreeCommand::Up)),
    (&[Key::char('l')], Command::Tree(TreeCommand::Open)),
    (
        &[Key::code(KeyCode::Right)],
        Command::Tree(TreeCommand::Open),
    ),
    (
        &[Key::code(KeyCode::Enter)],
        Command::Tree(TreeCommand::Open),
    ),
    (&[Key::char('h')], Command::Tree(TreeCommand::Close)),
    (
        &[Key::code(KeyCode::Left)],
        Command::Tree(TreeCommand::Close),
    ),
    (&[Key::char('a')], Command::Tree(TreeCommand::Create)),
    (&[Key::char('r')], Command::Tree(TreeCommand::Rename)),
    (&[Key::char('d')], Command::Tree(TreeCommand::Delete)),
    (
        &[Key::code(KeyCode::Esc)],
        Command::Tree(TreeCommand::FocusEditor),
    ),
    (&[Key::char(':')], Command::CommandMode),
    (&SAVE_ALL, Command::WriteAll),
];

const TREE_INPUT: Bindings = &[
    (
        &[Key::code(KeyCode::Esc)],
        Command::Tree(TreeCommand::EditCancel),
    ),
    (&[Key::ctrl('c')], Command::Tree(TreeCommand::EditCancel)),
    (
        &[Key::code(KeyCode::Enter)],
        Command::Tree(TreeCommand::EditSubmit),
    ),
    (
        &[Key::code(KeyCode::Backspace)],
        Command::Tree(TreeCommand::EditBackspace),
    ),
];

const PICKER: Bindings = &[
    (
        &[Key::code(KeyCode::Esc)],
        Command::Picker(PickerCommand::Cancel),
    ),
    (&[Key::ctrl('c')], Command::Picker(PickerCommand::Cancel)),
    (
        &[Key::code(KeyCode::Enter)],
        Command::Picker(PickerCommand::Submit),
    ),
    (
        &[Key::code(KeyCode::Backspace)],
        Command::Picker(PickerCommand::Backspace),
    ),
    (&[Key::ctrl('n')], Command::Picker(PickerCommand::Next)),
    (
        &[Key::code(KeyCode::Down)],
        Command::Picker(PickerCommand::Next),
    ),
    (
        &[Key::code(KeyCode::Tab)],
        Command::Picker(PickerCommand::Next),
    ),
    (&[Key::ctrl('p')], Command::Picker(PickerCommand::Previous)),
    (
        &[Key::code(KeyCode::Up)],
        Command::Picker(PickerCommand::Previous),
    ),
    (
        &[with(KeyCode::Tab, Mods::SHIFT)],
        Command::Picker(PickerCommand::Previous),
    ),
];

const CHANGES: Bindings = &[
    (&[Key::char('j')], Command::Changes(ChangesCommand::Down)),
    (
        &[Key::code(KeyCode::Down)],
        Command::Changes(ChangesCommand::Down),
    ),
    (&[Key::char('k')], Command::Changes(ChangesCommand::Up)),
    (
        &[Key::code(KeyCode::Up)],
        Command::Changes(ChangesCommand::Up),
    ),
    (
        &[Key::char('h')],
        Command::Changes(ChangesCommand::Collapse),
    ),
    (
        &[Key::code(KeyCode::Left)],
        Command::Changes(ChangesCommand::Collapse),
    ),
    (&[Key::char('l')], Command::Changes(ChangesCommand::Open)),
    (
        &[Key::code(KeyCode::Right)],
        Command::Changes(ChangesCommand::Open),
    ),
    (
        &[Key::code(KeyCode::Enter)],
        Command::Changes(ChangesCommand::Open),
    ),
    (&[Key::char('s')], Command::Changes(ChangesCommand::Stage)),
    (&[Key::char('u')], Command::Changes(ChangesCommand::Unstage)),
    (&[Key::char('d')], Command::Changes(ChangesCommand::Discard)),
    (
        &[Key::code(KeyCode::Esc)],
        Command::Changes(ChangesCommand::FocusEditor),
    ),
    (&[Key::char(':')], Command::CommandMode),
    (&SAVE_ALL, Command::WriteAll),
];

const CONFIRM: Bindings = &[
    (&[Key::char('y')], Command::Confirm(Answer::Yes)),
    (&[Key::char('n')], Command::Confirm(Answer::No)),
    (&[Key::code(KeyCode::Esc)], Command::Confirm(Answer::No)),
    (&[Key::ctrl('c')], Command::Confirm(Answer::No)),
];

/// Merge conflicts in the buffer text.
const CONFLICTS: Bindings = &[
    (
        &[Key::char(']'), Key::char('m')],
        Command::GotoConflict(Direction::Forward),
    ),
    (
        &[Key::char('['), Key::char('m')],
        Command::GotoConflict(Direction::Backward),
    ),
    (
        &[SPACE, Key::char('m'), Key::char('o')],
        Command::Resolve(Take::Ours),
    ),
    (
        &[SPACE, Key::char('m'), Key::char('t')],
        Command::Resolve(Take::Theirs),
    ),
    (
        &[SPACE, Key::char('m'), Key::char('b')],
        Command::Resolve(Take::Both),
    ),
    (
        &[SPACE, Key::char('m'), Key::char('r')],
        Command::Resolve(Take::Base),
    ),
];

/// Rendered markdown and typst views; shared by normal and select mode and rendered views.
const MARKDOWN: Bindings = &[
    (&[SPACE, Key::char('v')], Command::RenderedSplit),
    (&[SPACE, Key::char('V')], Command::ToggleRendered),
    (
        &[SPACE, Key::char('l')],
        Command::Picker(PickerCommand::Open(PickerKind::Backlinks)),
    ),
];

const fn rendered(command: RenderedCommand) -> Command {
    Command::Rendered(command)
}

const RENDERED: Bindings = &[
    (
        &[Key::char('j')],
        rendered(RenderedCommand::Scroll(Direction::Forward, ScrollBy::Lines)),
    ),
    (
        &[Key::code(KeyCode::Down)],
        rendered(RenderedCommand::Scroll(Direction::Forward, ScrollBy::Lines)),
    ),
    (
        &[Key::char('k')],
        rendered(RenderedCommand::Scroll(
            Direction::Backward,
            ScrollBy::Lines,
        )),
    ),
    (
        &[Key::code(KeyCode::Up)],
        rendered(RenderedCommand::Scroll(
            Direction::Backward,
            ScrollBy::Lines,
        )),
    ),
    (
        &[Key::ctrl('d')],
        rendered(RenderedCommand::Scroll(
            Direction::Forward,
            ScrollBy::HalfPageCursor,
        )),
    ),
    (
        &[Key::ctrl('u')],
        rendered(RenderedCommand::Scroll(
            Direction::Backward,
            ScrollBy::HalfPageCursor,
        )),
    ),
    (
        &[Key::ctrl('f')],
        rendered(RenderedCommand::Scroll(Direction::Forward, ScrollBy::Page)),
    ),
    (
        &[Key::code(KeyCode::PageDown)],
        rendered(RenderedCommand::Scroll(Direction::Forward, ScrollBy::Page)),
    ),
    (
        &[Key::char(' '), Key::char(' ')],
        rendered(RenderedCommand::Scroll(Direction::Forward, ScrollBy::Page)),
    ),
    (
        &[Key::ctrl('b')],
        rendered(RenderedCommand::Scroll(Direction::Backward, ScrollBy::Page)),
    ),
    (
        &[Key::code(KeyCode::PageUp)],
        rendered(RenderedCommand::Scroll(Direction::Backward, ScrollBy::Page)),
    ),
    (
        &[Key::char('g'), Key::char('g')],
        rendered(RenderedCommand::Top),
    ),
    (&[Key::code(KeyCode::Home)], rendered(RenderedCommand::Top)),
    (
        &[Key::char('g'), Key::char('e')],
        rendered(RenderedCommand::Bottom),
    ),
    (&[Key::char('G')], rendered(RenderedCommand::Bottom)),
    (
        &[Key::code(KeyCode::End)],
        rendered(RenderedCommand::Bottom),
    ),
    (
        &[Key::char(']'), Key::char(']')],
        rendered(RenderedCommand::Heading(Direction::Forward)),
    ),
    (
        &[Key::char('['), Key::char('[')],
        rendered(RenderedCommand::Heading(Direction::Backward)),
    ),
    (&[Key::code(KeyCode::Esc)], rendered(RenderedCommand::Leave)),
    (&[Key::char('f')], rendered(RenderedCommand::Fit)),
    (
        &[Key::char('+')],
        rendered(RenderedCommand::Zoom(ZoomBy::In)),
    ),
    (
        &[Key::char('-')],
        rendered(RenderedCommand::Zoom(ZoomBy::Out)),
    ),
    (
        &[Key::char('=')],
        rendered(RenderedCommand::Zoom(ZoomBy::Reset)),
    ),
    (
        &[Key::char('h')],
        rendered(RenderedCommand::Pan(Direction::Backward)),
    ),
    (
        &[Key::code(KeyCode::Left)],
        rendered(RenderedCommand::Pan(Direction::Backward)),
    ),
    (
        &[Key::char('l')],
        rendered(RenderedCommand::Pan(Direction::Forward)),
    ),
    (
        &[Key::code(KeyCode::Right)],
        rendered(RenderedCommand::Pan(Direction::Forward)),
    ),
    (&[Key::char(':')], Command::CommandMode),
    (&SAVE_ALL, Command::WriteAll),
];

const TABLES: [&[Bindings]; Context::COUNT] = [
    &[
        COMMON,
        NORMAL,
        VIEW,
        WORKSPACE,
        WINDOW,
        SPACE_WINDOW,
        GOTO_BUFFER,
        DIFF,
        CONFLICTS,
        MARKDOWN,
    ],
    &[INSERT],
    &[
        COMMON,
        SELECT,
        VIEW,
        WORKSPACE,
        WINDOW,
        SPACE_WINDOW,
        GOTO_BUFFER,
        DIFF,
        CONFLICTS,
        MARKDOWN,
    ],
    &[PROMPT],
    &[TREE, WORKSPACE, WINDOW, SPACE_WINDOW],
    &[TREE_INPUT],
    &[PICKER],
    &[CHANGES, WORKSPACE, WINDOW, SPACE_WINDOW],
    &[CONFIRM],
    &[
        RENDERED,
        MARKDOWN,
        WORKSPACE,
        WINDOW,
        SPACE_WINDOW,
        GOTO_BUFFER,
    ],
];

fn tables(context: Context) -> &'static [Bindings] {
    TABLES[context as usize]
}

/// First binding of `command` in `context`, for display.
pub fn keys_for(context: Context, command: &Command) -> Option<&'static [Key]> {
    tables(context)
        .iter()
        .copied()
        .flatten()
        .find(|(_, bound)| bound == command)
        .map(|(keys, _)| *keys)
}

fn insert(node: &mut Node, keys: &[Key], command: &Command) -> Result<(), ()> {
    match keys {
        [] => Err(()),
        [key, rest @ ..] => match (node.keys.binary_search(&key.packed()), rest) {
            (Err(slot), []) => {
                node.keys.insert(slot, key.packed());
                node.children.insert(slot, Trie::Leaf(command.clone()));
                Ok(())
            }
            (Err(slot), _) => {
                let mut child = Node::default();
                insert(&mut child, rest, command)?;
                node.keys.insert(slot, key.packed());
                node.children.insert(slot, Trie::Node(child));
                Ok(())
            }
            (Ok(slot), _) => match (&mut node.children[slot], rest) {
                (Trie::Node(child), [_, ..]) => insert(child, rest, command),
                _ => Err(()),
            },
        },
    }
}

fn build_trie(context: Context) -> Result<Trie, KeymapError> {
    let mut root = Node::default();
    for (keys, command) in tables(context).iter().flat_map(|table| table.iter()) {
        match keys.is_empty() {
            true => return Err(KeymapError::Empty { context }),
            false => insert(&mut root, keys, command).map_err(|()| KeymapError::Conflict {
                context,
                keys: keys.to_vec(),
            })?,
        }
    }
    Ok(Trie::Node(root))
}

impl Keymap {
    pub fn helix() -> Result<Keymap, KeymapError> {
        let [
            normal,
            insert,
            select,
            prompt,
            tree,
            tree_input,
            picker,
            changes,
            confirm,
            rendered,
        ] = Context::ALL;
        Ok(Keymap {
            tries: [
                build_trie(normal)?,
                build_trie(insert)?,
                build_trie(select)?,
                build_trie(prompt)?,
                build_trie(tree)?,
                build_trie(tree_input)?,
                build_trie(picker)?,
                build_trie(changes)?,
                build_trie(confirm)?,
                build_trie(rendered)?,
            ],
        })
    }

    pub fn resolve(&self, context: Context, keys: &[Key]) -> Resolved<'_> {
        let mut trie = &self.tries[context as usize];
        for key in keys {
            trie = match trie {
                Trie::Node(node) => match node.keys.binary_search(&key.packed()) {
                    Ok(i) => &node.children[i],
                    Err(_) => return Resolved::Unbound,
                },
                Trie::Leaf(_) => return Resolved::Unbound,
            };
        }
        match trie {
            Trie::Leaf(command) => Resolved::Command(command),
            Trie::Node(_) => Resolved::Pending,
        }
    }
}

/// Bindings this editor adds or changes relative to helix; kept by hand next to the tables.
const CUSTOM: &str = "\
## Custom bindings (not helix defaults)

Source view keys are listed in their own section below.

| Keys | Context | Action |
| --- | --- | --- |
| `C-k C-s` | normal, insert, select, tree | Save all modified buffers. Shadows helix's insert-mode `C-k` (kill to line end). |
| `g left` / `g right` | normal, select | Line start / end (aliases of `gh` / `gl`). |
| `g up` / `g down` | normal, select | File start / last line (aliases of `gg` / `ge`). |
| `space e` | normal, select, tree | Toggle the file tree sidebar (helix opens an explorer picker). |
| `j k l h a r d esc` | tree | Move, open or expand, collapse, add, rename, delete (type `y`), back to the editor. |
| mouse | tree | Click to open, double-click to keep the tab. Right-click a row (or the empty area for the root): New File, New Folder, Rename, Delete. Drag a file or folder onto a folder (or onto a file, for its folder; the empty area for the root) to move it; open buffers follow. |
| `g n` / `g p` | normal, select | Next / previous tab in the focused pane (helix: buffers). |
";

/// The git source view, kept by hand next to the tables.
const SOURCE_VIEW: &str = "\
## Source view (git)

Diffs and gutter marks compare against HEAD; staged changes are dimmed. Colours are set under
`[theme.diff]` in the config (`added`, `removed`, `modified`, `ours`, `theirs`, `base`).

| Keys | Context | Action |
| --- | --- | --- |
| `space g` | normal, select, tree, changes | Toggle the Changes sidebar: Conflicts / Staged Changes / Changes (helix: changed-file picker). |
| `j` / `k` | changes | Move between files and group headers. |
| `l` / enter | changes | On a file: open its diff as a preview tab (conflicted files open as text; focus stays in the sidebar). On a header: collapse or expand the group. |
| `h` / left | changes | Collapse the group (from a file, move to its header). |
| `s` / `u` | changes | Stage / unstage the file, or the whole group on a header. |
| `d` | changes | Discard the file's changes (asks `y` / `n`; untracked files are deleted). |
| `esc` | changes | Back to the editor. |
| mouse | changes | Click a file to open its diff, double-click to keep the tab, click a header to collapse it. Hover a row for `+` stage, `−` unstage and `↶` discard. |
| `y` / `n` / `esc` | confirm | Answer a pending discard (or note creation, below). |
| `space d` | normal, select | Toggle the tab between the file and its side-by-side diff (helix: diagnostics picker). The right side is the live buffer and takes every key. |
| `]g` / `[g` | normal, select | Next / previous change; past the file's last / first change, continue into the next / previous changed file (its diff when in a diff; conflicted files open as text). Helix stops at the file's ends. |
| `]G` / `[G` | normal, select | Last / first change of the file, as helix. |
| `space h s` | normal, select | Stage the changes touched by the selections. |
| `space h u` | normal, select | Unstage the staged changes under the selections. |
| `space h r` | normal, select | Revert the changes under the selections to HEAD, as one undoable edit. |
| `]m` / `[m` | normal, select | Next / previous merge conflict, selecting it whole; select mode extends; takes a count. |
| `space m o` / `space m t` | normal, select | Resolve the conflicts touched by the selections with ours (HEAD) / theirs (incoming), as one undoable edit. Also `:conflict-ours` / `:conflict-theirs`. |
| `space m b` | normal, select | Resolve with both sides, ours then theirs. Also `:conflict-both`. |
| `space m r` | normal, select | Resolve with the base section of a diff3 conflict. Also `:conflict-base`. |
";

/// Rendered markdown and typst views, kept by hand next to the tables.
const MARKDOWN_VIEW: &str = "\
## Markdown and typst

`.md` and `.typ` files have a rendered view: an Obsidian-like reading view of headings, lists,
task checkboxes, callouts (`> [!note]`, folded with `[!tip]-`, foldable with `[!tip]+`),
highlighted code, tables, images, wikilinks, math and mermaid. LaTeX math (`$inline$`,
`$$display$$`, ```` ```math ````) and ```` ```mermaid ```` diagrams are typeset in the
background in the theme's colours; until then, or when they fail, a muted box shows the source
(and the error). Inline math sits on the text baseline; its paragraph wraps between words.
Typst files show their pages, recompiled on every edit (imports and images resolve from the
project root). Errors show in a strip above the pages with the first error's
`file:line:col` and a count, and in the status line when they appear; the last good pages stay,
dimmed. Clicking the strip puts the text cursor on the error.

| Keys | Context | Action |
| --- | --- | --- |
| `space v` | normal, select, rendered | Open a rendered view of the file in a split to the right that follows the text pane; again (from either side) closes it. Focus stays in the text. |
| `space V` | normal, select, rendered | Toggle the tab between the file's text and its rendered view in place, keeping the position. |
| `j` / `k`, down / up | rendered | Scroll a little. |
| `C-d` / `C-u` | rendered | Scroll half a page. |
| `C-f` / `C-b`, pagedown / pageup, `space space` | rendered | Scroll a page. |
| `gg` / `ge` / `G`, home / end | rendered | Top / bottom. |
| `]]` / `[[` | rendered | Next / previous top-level heading. |
| `esc` | rendered | Back to the text pane it follows; a standalone view toggles back to text. |
| `f` | rendered | Typst preview: fit pages to the pane width, or each page whole. |
| `+` / `-` / `=` | rendered | Typst preview: zoom pages in / out from the fit, or back to it. |
| `h` / `l`, left / right | rendered | Typst preview: pan sideways across pages wider than the pane. |
| `:export-pdf` | typst file | Compile the focused typst file to `<file>.pdf` beside it (also in the `space ?` palette). Markdown export is not supported yet. |
| mouse | rendered | Wheel scrolls. Click a block, or a spot on a typst page, to put the text cursor there (in the text pane it follows). Click a checkbox to toggle `[ ]` / `[x]` in the source (one undo step), a callout header to fold it, a link or a `![[note]]` embed to follow it (see links below). |
| `gf` | normal, select | Follow the link under the cursor: a wikilink, a markdown link or a URL in a markdown file; elsewhere the URL or file path under the cursor (or the selected text), relative to the file's folder, then the root, as in helix. |
| `space l` | normal, select, rendered | Backlinks: notes linking to the focused note, with the line of each link. Enter opens the note at that line; the preview shows it. Also `backlinks` in the `space ?` palette. |
| `y` / `n` / `esc` | confirm | Create the note an unresolved link names, or not. |

A following view scrolls to the block under the text cursor, or under the text's top line
when it scrolls; scrolling the rendered view scrolls the text (not its cursor). A typst
preview scrolls to where the text cursor landed on the pages, and its strip shows the
first error or warning; clicking the strip opens it.

Links resolve the way Obsidian does, with the project root as the vault. `[[name]]` names
`name.md`, or a file named exactly `name` (`[[photo.png]]`), anywhere in the vault, ignoring
case; `[[folder/name]]` matches a path ending in it. When several files match, an exact-case
match wins, then the shallowest path, then one in the note's folder. Markdown links
(`[text](path.md)`) resolve against the note's folder, then the root, then by name.
`#heading` puts the cursor (or the rendered view) on that heading; `#^block` just opens the
note; `|alias` changes only the text shown. Links are followed into a rendered view when
followed from one, else into the text. Unresolved links render dimmed and in italics;
following one asks `create <path>? y/n` and creates an empty note next to the linking note
(or at the vault path the link names). `![[image.png]]` embeds images from anywhere in the
vault; `![[note]]` shows a box naming the resolved note that opens it when clicked.
";

/// Markdown listing of every binding, generated from the tables; `docs/keymap.md` holds a copy.
pub fn reference() -> String {
    let mut out = String::from(
        "# Keymap\n\nGenerated from `crates/state/src/keymap.rs`; regenerate with \
         `UPDATE_DOCS=1 cargo test -p state --test unit keymap_reference`.\n\n",
    );
    out.push_str(CUSTOM);
    out.push_str("\n");
    out.push_str(SOURCE_VIEW);
    out.push_str("\n");
    out.push_str(MARKDOWN_VIEW);
    out.push_str("\n## Typed commands (`:`)\n\n| Names |\n| --- |\n");
    for (names, _) in crate::prompt::COMMANDS {
        let names: Vec<String> = names.iter().map(|name| format!("`:{name}`")).collect();
        out.push_str(&format!("| {} |\n", names.join(", ")));
    }
    for context in Context::ALL {
        out.push_str(&format!(
            "\n## {context:?}\n\n| Keys | Command |\n| --- | --- |\n"
        ));
        for (keys, command) in tables(context).iter().copied().flatten() {
            let keys: Vec<String> = keys.iter().map(|key| key.to_string()).collect();
            out.push_str(&format!("| `{}` | `{command:?}` |\n", keys.join(" ")));
        }
    }
    out
}
