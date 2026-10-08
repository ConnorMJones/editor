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

#[test]
fn parses_typed_editing_commands() {
    use model::command::{Dirs, SortCase, SortOrder};
    use std::num::NonZeroUsize;
    let line = |n| NonZeroUsize::new(n).unwrap();
    assert_eq!(
        parse("w! a/b.txt").unwrap(),
        Command::WriteAs(PathBuf::from("a/b.txt"), Dirs::Create)
    );
    assert_eq!(
        parse("w b.txt").unwrap(),
        Command::WriteAs(PathBuf::from("b.txt"), Dirs::Existing)
    );
    assert_eq!(parse("w!").unwrap(), Command::ForceWrite);
    assert_eq!(parse("xa").unwrap(), Command::WriteAllQuit);
    assert_eq!(parse("qa!").unwrap(), Command::ForceQuit);
    assert_eq!(parse("reload").unwrap(), Command::Reload);
    assert_eq!(parse("goto 12").unwrap(), Command::GotoLine(line(12)));
    assert_eq!(parse("12").unwrap(), Command::GotoLine(line(12)));
    assert_eq!(
        parse("sort -r -i").unwrap(),
        Command::Sort(SortCase::Insensitive, SortOrder::Descending)
    );
    assert_eq!(parse("reflow 40").unwrap(), Command::Reflow(Some(line(40))));
    assert!(matches!(
        parse("goto 0"),
        Err(Notice::InvalidArgument {
            command: "goto",
            ..
        })
    ));
    assert!(matches!(
        parse("sort -x"),
        Err(Notice::InvalidArgument {
            command: "sort",
            ..
        })
    ));
}
