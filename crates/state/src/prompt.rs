use std::num::NonZeroUsize;
use std::path::PathBuf;

use model::command::{Command, Cycle, Dirs, SortCase, SortOrder, WindowCommand};
use model::notice::Notice;
use model::window::Axis;

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
        // `:<n>` jumps to line n, like helix.
        (name, None) if name.parse::<NonZeroUsize>().is_ok() => {
            number("goto", name).map(Command::GotoLine)
        }
        _ => Err(Notice::UnknownCommand {
            input: line.to_owned(),
        }),
    }
}
