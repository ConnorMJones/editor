use std::num::NonZeroUsize;
use std::path::PathBuf;

use helix_core::unicode::segmentation::GraphemeCursor;
use model::command::{
    Command, Cycle, Direction, Dirs, PromptCommand, PromptMotion, SortCase, SortOrder, Take,
    WindowCommand,
};
use model::notice::Notice;
use model::register::Register;
use model::window::Axis;

use crate::apply::apply;
use crate::files::Files;
use crate::{Effect, Overlay, Prompt, PromptKind, State, register, select};

/// Where up/down browsing of the prompt's history register stands.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum History {
    Editing,
    /// Stepping newer than entry 0 restores `draft`, the line typed before browsing.
    Browsing {
        index: usize,
        draft: String,
    },
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Completion {
    /// Byte offset in the line from which a chosen candidate replaces the rest.
    pub start: usize,
    pub candidates: Vec<Candidate>,
    pub chosen: Chosen,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Chosen {
    #[default]
    Nothing,
    At(usize),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Candidate {
    Command(&'static str),
    File(String),
    /// Ends in `/`.
    Dir(String),
}

impl Candidate {
    pub fn text(&self) -> &str {
        match self {
            Candidate::Command(name) => name,
            Candidate::File(name) | Candidate::Dir(name) => name,
        }
    }
}

/// What a typed command's argument completes to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Completes {
    Nothing,
    Paths,
}

/// Typed command names, canonical first, as `parse` accepts them.
pub const COMMANDS: [(&[&str], Completes); 25] = [
    (&["write", "w"], Completes::Paths),
    (&["write!", "w!"], Completes::Paths),
    (&["write-all", "wa"], Completes::Nothing),
    (&["write-quit", "wq", "x"], Completes::Nothing),
    (&["write-quit-all", "wqa", "xa"], Completes::Nothing),
    (&["quit", "q"], Completes::Nothing),
    (&["quit-all", "qa"], Completes::Nothing),
    (&["quit!", "q!"], Completes::Nothing),
    (&["quit-all!", "qa!"], Completes::Nothing),
    (&["buffer-close", "bc", "bclose"], Completes::Nothing),
    (&["buffer-close!", "bc!", "bclose!"], Completes::Nothing),
    (&["reload", "rl"], Completes::Nothing),
    (&["reload-all", "rla"], Completes::Nothing),
    (&["buffer-next", "bn", "bnext"], Completes::Nothing),
    (&["buffer-previous", "bp", "bprev"], Completes::Nothing),
    (&["vsplit", "vs"], Completes::Nothing),
    (&["hsplit", "hs", "sp"], Completes::Nothing),
    (&["open", "o", "e", "edit"], Completes::Paths),
    (&["goto", "g"], Completes::Nothing),
    (&["reflow"], Completes::Nothing),
    (&["sort"], Completes::Nothing),
    (&["conflict-ours"], Completes::Nothing),
    (&["conflict-theirs"], Completes::Nothing),
    (&["conflict-both"], Completes::Nothing),
    (&["conflict-base"], Completes::Nothing),
];

/// A fresh prompt, with completions listed up front like helix.
pub fn open(kind: PromptKind, files: &Files) -> Prompt {
    let mut prompt = Prompt {
        kind,
        line: String::new(),
        cursor: 0,
        history: History::Editing,
        completion: Completion::default(),
    };
    complete(&mut prompt, files);
    prompt
}

/// Recomputes the candidates for the line; paths come from the file index, one directory level at a time.
pub fn complete(prompt: &mut Prompt, files: &Files) {
    let completion = &mut prompt.completion;
    completion.candidates.clear();
    completion.chosen = Chosen::Nothing;
    completion.start = 0;
    match prompt.kind {
        PromptKind::Command => {}
        PromptKind::Regex { .. } => return,
    }
    let line = prompt.line.as_str();
    let (name, arg) = match line.split_once(char::is_whitespace) {
        Some((name, arg)) => (name, Some(arg)),
        None => (line, None),
    };
    let completes = COMMANDS
        .iter()
        .find(|(names, _)| names.contains(&name))
        .map_or(Completes::Nothing, |(_, completes)| *completes);
    match (arg, completes) {
        (None, _) => completion.candidates.extend(
            COMMANDS
                .iter()
                .filter(|(names, _)| names.iter().any(|n| n.starts_with(name)))
                .map(|(names, _)| Candidate::Command(names[0])),
        ),
        (Some(_), Completes::Nothing) => {}
        (Some(arg), Completes::Paths) => {
            let files = match files {
                Files::Ready(files) => files,
                Files::Loading => return,
            };
            let arg = arg.trim_start();
            let arg_start = line.len() - arg.len();
            let dir_len = arg.rfind('/').map_or(0, |slash| slash + 1);
            completion.start = arg_start + dir_len;
            let dir_ok = arg[..dir_len]
                .split('/')
                .all(|part| !matches!(part, "." | ".."));
            match (arg.starts_with('/'), dir_ok) {
                (false, true) => {}
                (true, _) | (_, false) => return,
            }
            let mut at = files.partition_point(|file| file.as_str() < arg);
            while let Some(file) = files.get(at).map(|file| file.as_str())
                && file.starts_with(arg)
            {
                let rest = &file[dir_len..];
                match rest.split_once('/') {
                    Some((name, _)) => {
                        let dir = &file[..dir_len + name.len() + 1];
                        completion
                            .candidates
                            .push(Candidate::Dir(format!("{name}/")));
                        at = files.partition_point(|file| {
                            file.as_str() < dir || file.as_str().starts_with(dir)
                        });
                    }
                    None => {
                        completion.candidates.push(Candidate::File(rest.to_owned()));
                        at += 1;
                    }
                }
            }
        }
    }
}

/// Inserts typed text at the cursor.
pub fn insert(state: &mut State, c: char) {
    match &mut state.overlay {
        Overlay::Prompt(prompt) => {
            prompt.line.insert(prompt.cursor, c);
            prompt.cursor += c.len_utf8();
            prompt.history = History::Editing;
            complete(prompt, &state.files);
            select::preview_regex(state);
        }
        Overlay::None | Overlay::Picker(_) => {}
    }
}

fn is_word_sep(c: char) -> bool {
    c == '/' || c.is_whitespace()
}

/// Byte offset `motion` moves the cursor to, as helix's prompt moves it.
pub fn target(line: &str, cursor: usize, motion: PromptMotion) -> usize {
    match motion {
        PromptMotion::CharBackward => GraphemeCursor::new(cursor, line.len(), true)
            .prev_boundary(line, 0)
            .ok()
            .flatten()
            .unwrap_or(cursor),
        PromptMotion::CharForward => GraphemeCursor::new(cursor, line.len(), true)
            .next_boundary(line, 0)
            .ok()
            .flatten()
            .unwrap_or(cursor),
        // Skips the char just before the cursor, then stops after the previous separator.
        PromptMotion::WordBackward => {
            let mut before = line[..cursor].char_indices().rev().skip(1);
            before
                .find(|(_, c)| is_word_sep(*c))
                .map_or(0, |(at, c)| at + c.len_utf8())
        }
        PromptMotion::WordForward => line[cursor..]
            .char_indices()
            .skip_while(|(_, c)| !is_word_sep(*c))
            .find(|(_, c)| !is_word_sep(*c))
            .map_or(line.len(), |(at, _)| cursor + at),
        PromptMotion::Start => 0,
        PromptMotion::End => line.len(),
    }
}

/// How a prompt command changed the prompt.
enum Change {
    Nothing,
    Typed,
    /// The line was replaced by a history entry.
    Replaced,
    /// The only candidate was a directory; list its contents, as helix does.
    Descend,
    Close,
}

pub fn command(state: &mut State, command: PromptCommand, out: &mut Vec<Effect>) {
    let State {
        overlay,
        registers,
        files,
        ..
    } = state;
    let Overlay::Prompt(prompt) = overlay else {
        return;
    };
    let change = match command {
        PromptCommand::Move(motion) => {
            prompt.cursor = target(&prompt.line, prompt.cursor, motion);
            Change::Nothing
        }
        PromptCommand::Delete(PromptMotion::CharBackward) if prompt.line.is_empty() => {
            Change::Close
        }
        PromptCommand::Delete(motion) => {
            let to = target(&prompt.line, prompt.cursor, motion);
            let range = prompt.cursor.min(to)..prompt.cursor.max(to);
            prompt.cursor = range.start;
            prompt.line.replace_range(range, "");
            Change::Typed
        }
        PromptCommand::History(direction) => {
            let entries = registers
                .values
                .get(&prompt.kind.history())
                .map_or(&[][..], Vec::as_slice);
            let history = std::mem::replace(&mut prompt.history, History::Editing);
            let (index, draft) = match (history, direction, entries.len()) {
                (history, _, 0) => {
                    prompt.history = history;
                    return;
                }
                (History::Editing, Direction::Backward, _) => {
                    (Some(0), std::mem::take(&mut prompt.line))
                }
                (History::Editing, Direction::Forward, _) => return,
                (History::Browsing { index, draft }, Direction::Backward, len) => {
                    (Some((index + 1).min(len - 1)), draft)
                }
                (History::Browsing { index: 0, draft }, Direction::Forward, _) => (None, draft),
                (History::Browsing { index, draft }, Direction::Forward, _) => {
                    (Some(index - 1), draft)
                }
            };
            match index {
                Some(index) => {
                    prompt.line.clone_from(&entries[index]);
                    prompt.history = History::Browsing { index, draft };
                }
                None => prompt.line = draft,
            }
            prompt.cursor = prompt.line.len();
            Change::Replaced
        }
        PromptCommand::Complete(direction) => {
            let completion = &mut prompt.completion;
            let len = completion.candidates.len();
            let index = match (len, completion.chosen, direction) {
                (0, ..) => return,
                (_, Chosen::Nothing, Direction::Forward) => 0,
                (_, Chosen::Nothing, Direction::Backward) => len - 1,
                (_, Chosen::At(i), Direction::Forward) => (i + 1) % len,
                (_, Chosen::At(i), Direction::Backward) => (i + len - 1) % len,
            };
            completion.chosen = Chosen::At(index);
            prompt.line.truncate(completion.start);
            prompt.line.push_str(completion.candidates[index].text());
            prompt.cursor = prompt.line.len();
            match (len, &completion.candidates[index]) {
                (1, Candidate::Dir(_)) => Change::Descend,
                _ => Change::Nothing,
            }
        }
    };
    match change {
        Change::Nothing => {}
        Change::Descend => complete(prompt, files),
        Change::Typed => {
            prompt.history = History::Editing;
            complete(prompt, files);
            select::preview_regex(state);
        }
        Change::Replaced => {
            complete(prompt, files);
            select::preview_regex(state);
        }
        Change::Close => apply(state, Command::PromptCancel, out),
    }
}

/// The line to run on submit; an empty line reruns the newest entry, a new line is recorded.
pub fn remember(
    state: &mut State,
    register: Register,
    line: String,
    out: &mut Vec<Effect>,
) -> Option<String> {
    let last = register::first(state, register);
    match line.is_empty() {
        true => last,
        false => {
            if last.as_deref() != Some(line.as_str())
                && let Err(notice) = state.registers.push(register, line.clone(), out)
            {
                state.notice = Some(notice);
            }
            Some(line)
        }
    }
}

/// Parses a `:` command line into a command.
pub fn parse(line: &str) -> Result<Command, Notice> {
    let line = line.trim();
    let (name, arg) = match line.split_once(char::is_whitespace) {
        Some((name, arg)) => (name, Some(arg.trim()).filter(|arg| !arg.is_empty())),
        None => (line, None),
    };
    let bare = |command: Command, label: &'static str| match arg {
        None => Ok(command),
        Some(_) => Err(Notice::UnexpectedArgument { command: label }),
    };
    let number = |label: &'static str, input: &str| {
        input
            .parse::<NonZeroUsize>()
            .map_err(|_| Notice::InvalidArgument {
                command: label,
                input: input.to_owned(),
            })
    };
    match (name, arg) {
        ("w" | "write", None) => Ok(Command::Write),
        ("w" | "write", Some(path)) => Ok(Command::WriteAs(PathBuf::from(path), Dirs::Existing)),
        ("w!" | "write!", None) => Ok(Command::ForceWrite),
        ("w!" | "write!", Some(path)) => Ok(Command::WriteAs(PathBuf::from(path), Dirs::Create)),
        ("wa" | "write-all", _) => bare(Command::WriteAll, "write-all"),
        ("wq" | "x" | "write-quit", _) => bare(Command::WriteQuit, "write-quit"),
        ("wqa" | "xa" | "write-quit-all", _) => bare(Command::WriteAllQuit, "write-quit-all"),
        ("q" | "quit", _) => bare(Command::Quit, "quit"),
        ("qa" | "quit-all", _) => bare(Command::Quit, "quit-all"),
        ("q!" | "quit!", _) => bare(Command::ForceQuit, "quit!"),
        ("qa!" | "quit-all!", _) => bare(Command::ForceQuit, "quit-all!"),
        ("bc" | "bclose" | "buffer-close", _) => bare(Command::CloseBuffer, "buffer-close"),
        ("bc!" | "bclose!" | "buffer-close!", _) => {
            bare(Command::ForceCloseBuffer, "buffer-close!")
        }
        ("rl" | "reload", _) => bare(Command::Reload, "reload"),
        ("rla" | "reload-all", _) => bare(Command::ReloadAll, "reload-all"),
        ("bn" | "bnext" | "buffer-next", _) => {
            bare(Command::CycleBuffer(Cycle::Next), "buffer-next")
        }
        ("bp" | "bprev" | "buffer-previous", _) => {
            bare(Command::CycleBuffer(Cycle::Previous), "buffer-previous")
        }
        ("vs" | "vsplit", _) => bare(Command::Window(WindowCommand::Split(Axis::Row)), "vsplit"),
        ("hs" | "hsplit" | "sp", _) => bare(
            Command::Window(WindowCommand::Split(Axis::Column)),
            "hsplit",
        ),
        ("o" | "open" | "e" | "edit", Some(path)) => Ok(Command::Open(PathBuf::from(path))),
        ("o" | "open" | "e" | "edit", None) => Err(Notice::MissingArgument { command: "open" }),
        ("g" | "goto", Some(line)) => number("goto", line).map(Command::GotoLine),
        ("g" | "goto", None) => Err(Notice::MissingArgument { command: "goto" }),
        ("reflow", None) => Ok(Command::Reflow(None)),
        ("reflow", Some(width)) => number("reflow", width).map(|w| Command::Reflow(Some(w))),
        ("sort", flags) => {
            let (mut case, mut order) = (SortCase::Sensitive, SortOrder::Ascending);
            for flag in flags.into_iter().flat_map(str::split_whitespace) {
                match flag {
                    "-i" | "--insensitive" => case = SortCase::Insensitive,
                    "-r" | "--reverse" => order = SortOrder::Descending,
                    other => {
                        return Err(Notice::InvalidArgument {
                            command: "sort",
                            input: other.to_owned(),
                        });
                    }
                }
            }
            Ok(Command::Sort(case, order))
        }
        ("conflict-ours", _) => bare(Command::Resolve(Take::Ours), "conflict-ours"),
        ("conflict-theirs", _) => bare(Command::Resolve(Take::Theirs), "conflict-theirs"),
        ("conflict-both", _) => bare(Command::Resolve(Take::Both), "conflict-both"),
        ("conflict-base", _) => bare(Command::Resolve(Take::Base), "conflict-base"),
        // `:<n>` jumps to line n, like helix.
        (name, None) if name.parse::<NonZeroUsize>().is_ok() => {
            number("goto", name).map(Command::GotoLine)
        }
        _ => Err(Notice::UnknownCommand {
            input: line.to_owned(),
        }),
    }
}
