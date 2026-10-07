use std::path::PathBuf;

use model::command::Command;
use model::notice::Notice;
use state::prompt::parse;

#[test]
fn parses_write_family() {
    assert_eq!(parse("w").unwrap(), Command::Write);
    assert_eq!(parse("wa").unwrap(), Command::WriteAll);
    assert_eq!(parse(" write-all ").unwrap(), Command::WriteAll);
    assert_eq!(parse("wq").unwrap(), Command::WriteQuit);
    assert_eq!(parse("q!").unwrap(), Command::ForceQuit);
}

#[test]
fn open_requires_path() {
    assert_eq!(
        parse("o src/main.rs").unwrap(),
        Command::Open(PathBuf::from("src/main.rs"))
    );
    assert!(matches!(
        parse("open"),
        Err(Notice::MissingArgument { command: "open" })
    ));
}

#[test]
fn rejects_unknown_and_extra_args() {
    assert!(matches!(
        parse("frobnicate"),
        Err(Notice::UnknownCommand { .. })
    ));
    assert!(matches!(
        parse("q now"),
        Err(Notice::UnexpectedArgument { command: "quit" })
    ));
}
