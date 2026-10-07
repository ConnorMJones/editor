use std::path::PathBuf;

use model::command::Command;
use model::notice::Notice;

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
    match name {
        "w" | "write" => bare(Command::Write, "write"),
        "wa" | "write-all" => bare(Command::WriteAll, "write-all"),
        "wq" | "x" | "write-quit" => bare(Command::WriteQuit, "write-quit"),
        "q" | "quit" => bare(Command::Quit, "quit"),
        "q!" | "quit!" => bare(Command::ForceQuit, "quit!"),
        "bc" | "bclose" | "buffer-close" => bare(Command::CloseBuffer, "buffer-close"),
        "bc!" | "bclose!" | "buffer-close!" => bare(Command::ForceCloseBuffer, "buffer-close!"),
        "o" | "open" | "e" | "edit" => match arg {
            Some(path) => Ok(Command::Open(PathBuf::from(path))),
            None => Err(Notice::MissingArgument { command: "open" }),
        },
        _ => Err(Notice::UnknownCommand {
            input: line.to_owned(),
        }),
    }
}
