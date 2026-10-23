//! The binding tables, one per context, after helix's defaults.

use model::command::{
    Align, Answer, Case, ChangesCommand, Command, Cycle, Direction, DocumentCommand,
    DrawingCommand, Find, HunkOp, HunkTarget, Inclusion, InkCommand, InkTool, Join, Motion,
    ObjectScope, PageCommand, PageSide, PageStep, PagesCommand, Paste, Pending, PickerCommand,
    PickerKind, PromptCommand, PromptMotion, ReadCommand, ReadMove, RegexOp, Resize, ScrollBy,
    Shown, SyntaxObject, Take, TreeCommand, ViewCommand, WindowCommand, WordBounds, Yanking,
    ZoomBy,
};
use model::key::{Key, KeyCode, Mods};
use model::register::{Clipboard, Register};
use model::window::{Axis, Side};

use super::{Bindings, Context};

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

/// Modes of the focused tab; shared by every context that shows one.
const MODES: Bindings = &[
    (&[SPACE, Key::char('d')], Command::ToggleMode(Shown::Diff)),
    (&[SPACE, Key::char('v')], Command::ToggleMode(Shown::Split)),
    (&[SPACE, Key::char('V')], Command::ToggleMode(Shown::Render)),
    (
        &[SPACE, Key::code(KeyCode::Tab)],
        Command::CycleMode(Direction::Forward),
    ),
    (
        &[SPACE, with(KeyCode::Tab, Mods::SHIFT)],
        Command::CycleMode(Direction::Backward),
    ),
    (
        &[CTRL_W, Key::char('<')],
        Command::ResizeSplit(Resize::Narrow),
    ),
    (
        &[CTRL_W, Key::char('>')],
        Command::ResizeSplit(Resize::Widen),
    ),
    (
        &[CTRL_W, Key::char('=')],
        Command::ResizeSplit(Resize::Even),
    ),
];

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

/// Notes; shared by normal and select mode and rendered views.
const MARKDOWN: Bindings = &[(
    &[SPACE, Key::char('l')],
    Command::Picker(PickerCommand::Open(PickerKind::Backlinks)),
)];

const fn read(command: ReadCommand) -> Command {
    Command::Read(command)
}

const fn scroll(direction: Direction, by: ScrollBy) -> Command {
    read(ReadCommand::Move(ReadMove::Scroll(direction, by)))
}

/// Document and pages views alike.
const READ: Bindings = &[
    (
        &[Key::char('j')],
        scroll(Direction::Forward, ScrollBy::Lines),
    ),
    (
        &[Key::code(KeyCode::Down)],
        scroll(Direction::Forward, ScrollBy::Lines),
    ),
    (
        &[Key::char('k')],
        scroll(Direction::Backward, ScrollBy::Lines),
    ),
    (
        &[Key::code(KeyCode::Up)],
        scroll(Direction::Backward, ScrollBy::Lines),
    ),
    (
        &[Key::ctrl('d')],
        scroll(Direction::Forward, ScrollBy::HalfPageCursor),
    ),
    (
        &[Key::ctrl('u')],
        scroll(Direction::Backward, ScrollBy::HalfPageCursor),
    ),
    (
        &[Key::ctrl('f')],
        scroll(Direction::Forward, ScrollBy::Page),
    ),
    (
        &[Key::code(KeyCode::PageDown)],
        scroll(Direction::Forward, ScrollBy::Page),
    ),
    (
        &[Key::char(' '), Key::char(' ')],
        scroll(Direction::Forward, ScrollBy::Page),
    ),
    (
        &[Key::ctrl('b')],
        scroll(Direction::Backward, ScrollBy::Page),
    ),
    (
        &[Key::code(KeyCode::PageUp)],
        scroll(Direction::Backward, ScrollBy::Page),
    ),
    (
        &[Key::char('g'), Key::char('g')],
        read(ReadCommand::Move(ReadMove::Top)),
    ),
    (
        &[Key::code(KeyCode::Home)],
        read(ReadCommand::Move(ReadMove::Top)),
    ),
    (
        &[Key::char('g'), Key::char('e')],
        read(ReadCommand::Move(ReadMove::Bottom)),
    ),
    (&[Key::char('G')], read(ReadCommand::Move(ReadMove::Bottom))),
    (
        &[Key::code(KeyCode::End)],
        read(ReadCommand::Move(ReadMove::Bottom)),
    ),
    (&[Key::code(KeyCode::Esc)], read(ReadCommand::Leave)),
    (&[Key::char(':')], Command::CommandMode),
    (&SAVE_ALL, Command::WriteAll),
];

const DOCUMENT: Bindings = &[
    (
        &[Key::char(']'), Key::char(']')],
        Command::Document(DocumentCommand::Heading(Direction::Forward)),
    ),
    (
        &[Key::char('['), Key::char('[')],
        Command::Document(DocumentCommand::Heading(Direction::Backward)),
    ),
];

const PAGES: Bindings = &[
    (&[Key::char('f')], Command::Pages(PagesCommand::Fit)),
    (
        &[Key::char('+')],
        Command::Pages(PagesCommand::Zoom(ZoomBy::In)),
    ),
    (
        &[Key::char('-')],
        Command::Pages(PagesCommand::Zoom(ZoomBy::Out)),
    ),
    (
        &[Key::char('=')],
        Command::Pages(PagesCommand::Zoom(ZoomBy::Reset)),
    ),
    (
        &[Key::char('h')],
        Command::Pages(PagesCommand::Pan(Direction::Backward)),
    ),
    (
        &[Key::code(KeyCode::Left)],
        Command::Pages(PagesCommand::Pan(Direction::Backward)),
    ),
    (
        &[Key::char('l')],
        Command::Pages(PagesCommand::Pan(Direction::Forward)),
    ),
    (
        &[Key::code(KeyCode::Right)],
        Command::Pages(PagesCommand::Pan(Direction::Forward)),
    ),
];

/// A paged document or board.
const fn tool(tool: InkTool) -> Command {
    Command::Drawing(DrawingCommand::Ink(InkCommand::Tool(tool)))
}

const fn view(view: ViewCommand) -> Command {
    Command::Drawing(DrawingCommand::View(view))
}

const fn page(page: PageCommand) -> Command {
    Command::Drawing(DrawingCommand::Page(page))
}

const fn ink(ink: InkCommand) -> Command {
    Command::Drawing(DrawingCommand::Ink(ink))
}

const DRAWING: Bindings = &[
    (&[Key::char('b')], tool(InkTool::Brush)),
    (&[Key::char('e')], tool(InkTool::Eraser)),
    (&[Key::char('s')], tool(InkTool::BoxSelect)),
    (&[Key::char('S')], tool(InkTool::LassoSelect)),
    (&[Key::char('t'), Key::char('l')], tool(InkTool::Line)),
    (&[Key::char('t'), Key::char('a')], tool(InkTool::Arrow)),
    (&[Key::char('t'), Key::char('r')], tool(InkTool::Rectangle)),
    (&[Key::char('t'), Key::char('o')], tool(InkTool::Ellipse)),
    (
        &[Key::char('d'), Key::char('d')],
        ink(InkCommand::DeletePicked),
    ),
    (&[Key::code(KeyCode::Delete)], ink(InkCommand::DeletePicked)),
    (&[Key::char('u')], Command::Undo),
    (&[Key::char('U')], Command::Redo),
    (&[Key::char('+')], view(ViewCommand::Zoom(ZoomBy::In))),
    (&[Key::char('-')], view(ViewCommand::Zoom(ZoomBy::Out))),
    (&[Key::char('=')], view(ViewCommand::Zoom(ZoomBy::Reset))),
    (
        &[Key::char('z'), Key::char('f')],
        view(ViewCommand::ToggleFit),
    ),
    (&[Key::char('h')], view(ViewCommand::Pan(Side::Left))),
    (
        &[Key::code(KeyCode::Left)],
        view(ViewCommand::Pan(Side::Left)),
    ),
    (&[Key::char('l')], view(ViewCommand::Pan(Side::Right))),
    (
        &[Key::code(KeyCode::Right)],
        view(ViewCommand::Pan(Side::Right)),
    ),
    (&[Key::char('k')], view(ViewCommand::Pan(Side::Up))),
    (&[Key::code(KeyCode::Up)], view(ViewCommand::Pan(Side::Up))),
    (&[Key::char('j')], view(ViewCommand::Pan(Side::Down))),
    (
        &[Key::code(KeyCode::Down)],
        view(ViewCommand::Pan(Side::Down)),
    ),
    (
        &[Key::char(']'), Key::char('p')],
        page(PageCommand::Go(PageStep::Next)),
    ),
    (
        &[Key::char('['), Key::char('p')],
        page(PageCommand::Go(PageStep::Previous)),
    ),
    (
        &[Key::char('g'), Key::char('g')],
        page(PageCommand::Go(PageStep::First)),
    ),
    (&[Key::char('G')], page(PageCommand::Go(PageStep::Last))),
    (
        &[Key::char('o')],
        page(PageCommand::Insert(PageSide::Below)),
    ),
    (
        &[Key::char('O')],
        page(PageCommand::Insert(PageSide::Above)),
    ),
    (&[Key::char('d'), Key::char('p')], page(PageCommand::Remove)),
    (
        &[Key::char(']'), Key::char('l')],
        ink(InkCommand::Layer(Direction::Forward)),
    ),
    (
        &[Key::char('['), Key::char('l')],
        ink(InkCommand::Layer(Direction::Backward)),
    ),
    (
        &[SPACE, Key::char('l'), Key::char('h')],
        ink(InkCommand::ToggleLayer),
    ),
    (
        &[SPACE, Key::char('l'), Key::char('n')],
        ink(InkCommand::NewLayer),
    ),
    (&[Key::char(':')], Command::CommandMode),
    (&SAVE_ALL, Command::WriteAll),
];

pub const TABLES: [&[Bindings]; Context::COUNT] = [
    &[
        COMMON,
        NORMAL,
        VIEW,
        WORKSPACE,
        WINDOW,
        SPACE_WINDOW,
        GOTO_BUFFER,
        MODES,
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
        MODES,
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
        READ,
        DOCUMENT,
        MODES,
        MARKDOWN,
        WORKSPACE,
        WINDOW,
        SPACE_WINDOW,
        GOTO_BUFFER,
    ],
    &[
        READ,
        PAGES,
        MODES,
        MARKDOWN,
        WORKSPACE,
        WINDOW,
        SPACE_WINDOW,
        GOTO_BUFFER,
    ],
    &[DRAWING, WORKSPACE, WINDOW, SPACE_WINDOW, GOTO_BUFFER],
];
