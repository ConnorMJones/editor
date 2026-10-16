use std::path::PathBuf;

use model::command::Command;
use model::notice::Notice;
use state::input::prompt::parse;

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

#[test]
fn every_completed_command_parses() {
    for (names, _) in state::input::prompt::COMMANDS {
        for name in names {
            assert!(
                !matches!(parse(name), Err(Notice::UnknownCommand { .. })),
                "{name} is offered but unknown"
            );
        }
    }
}

mod editing {
    use model::register::Register;
    use state::input::prompt::{Candidate, Chosen, History};
    use state::{Overlay, Prompt};

    use crate::harness::Harness;

    fn open(h: &Harness) -> &Prompt {
        match &h.state.overlay {
            Overlay::Prompt(prompt) => prompt,
            Overlay::None | Overlay::Picker(_) | Overlay::Confirm(_) => panic!("no prompt"),
        }
    }

    /// The prompt line with `|` at the cursor.
    fn line(h: &Harness) -> String {
        let prompt = open(h);
        format!(
            "{}|{}",
            &prompt.line[..prompt.cursor],
            &prompt.line[prompt.cursor..]
        )
    }

    /// A root holding `src/{main,lib}.rs`, `src/ui/view.rs` and `README.md`.
    fn harness(dir: &tempfile::TempDir) -> Harness {
        for file in ["src/main.rs", "src/lib.rs", "src/ui/view.rs", "README.md"] {
            let path = dir.path().join(file);
            std::fs::create_dir_all(path.parent().expect("has parent")).expect("mkdir");
            std::fs::write(path, "x\n").expect("write fixture");
        }
        Harness::new(dir.path())
    }

    #[test]
    fn cursor_moves_and_inserts_mid_line() {
        let dir = tempfile::tempdir().expect("temp dir");
        let mut h = harness(&dir);
        h.keys(":open file");
        assert_eq!(line(&h), "open file|");
        h.keys("<left><left>");
        assert_eq!(line(&h), "open fi|le");
        h.keys("X");
        assert_eq!(line(&h), "open fiX|le");
        h.keys("<C-a>");
        assert_eq!(line(&h), "|open fiXle");
        h.keys("<C-f><right>");
        assert_eq!(line(&h), "op|en fiXle");
        h.keys("<C-e><C-b>");
        assert_eq!(line(&h), "open fiXl|e");
        h.keys("<home><A-f>");
        assert_eq!(line(&h), "open |fiXle");
        h.keys("<end><A-b>");
        assert_eq!(line(&h), "open |fiXle");
        h.keys("<A-b>");
        assert_eq!(line(&h), "|open fiXle");
    }

    #[test]
    fn deletions_follow_helix() {
        let dir = tempfile::tempdir().expect("temp dir");
        let mut h = harness(&dir);
        h.keys(":one two/three four");
        h.keys("<C-w>");
        assert_eq!(line(&h), "one two/three |");
        h.keys("<bs><C-w>");
        assert_eq!(line(&h), "one two/|");
        h.keys("<C-a><A-d>");
        assert_eq!(line(&h), "|two/");
        h.keys("<del>");
        assert_eq!(line(&h), "|wo/");
        h.keys("<right><C-k>");
        assert_eq!(line(&h), "w|");
        h.keys("<C-u>");
        assert_eq!(line(&h), "|");
        // Backspace on an empty line closes the prompt.
        h.keys("<bs>");
        assert!(matches!(h.state.overlay, Overlay::None));
    }

    #[test]
    fn history_browses_the_colon_register() {
        let dir = tempfile::tempdir().expect("temp dir");
        let mut h = harness(&dir);
        h.keys(":vsplit<ret>:hsplit<ret>");
        assert_eq!(
            h.state.registers.values.get(&Register::COMMAND),
            Some(&vec!["hsplit".to_owned(), "vsplit".to_owned()])
        );
        h.keys(":draft<up>");
        assert_eq!(line(&h), "hsplit|");
        h.keys("<C-p>");
        assert_eq!(line(&h), "vsplit|");
        // Clamped at the oldest entry.
        h.keys("<up>");
        assert_eq!(line(&h), "vsplit|");
        h.keys("<C-n>");
        assert_eq!(line(&h), "hsplit|");
        h.keys("<down>");
        assert_eq!(line(&h), "draft|");
        assert_eq!(open(&h).history, History::Editing);
    }

    #[test]
    fn empty_submit_reruns_the_last_command() {
        let dir = tempfile::tempdir().expect("temp dir");
        let mut h = harness(&dir);
        h.keys(":vsplit<ret>");
        let panes = h.state.layout.panes().len();
        h.keys(":<ret>");
        assert_eq!(h.state.layout.panes().len(), panes + 1);
    }

    #[test]
    fn regex_prompts_keep_search_history() {
        let mut h = Harness::with("t.txt", "#[a|]#bc abc\n");
        h.keys("/bc<ret>/<up>");
        assert_eq!(line(&h), "bc|");
    }

    #[test]
    fn completes_command_names() {
        let dir = tempfile::tempdir().expect("temp dir");
        let mut h = harness(&dir);
        h.keys(":wr");
        let candidates: Vec<&str> = open(&h)
            .completion
            .candidates
            .iter()
            .map(Candidate::text)
            .collect();
        assert_eq!(
            candidates,
            [
                "write",
                "write!",
                "write-all",
                "write-quit",
                "write-quit-all"
            ]
        );
        h.keys("<tab><tab>");
        assert_eq!(line(&h), "write!|");
        assert_eq!(open(&h).completion.chosen, Chosen::At(1));
        h.keys("<S-tab>");
        assert_eq!(line(&h), "write|");
        // Typing recomputes from the new line.
        h.keys("-q");
        assert_eq!(open(&h).completion.candidates.len(), 2);
    }

    #[test]
    fn completes_paths_one_directory_at_a_time() {
        let dir = tempfile::tempdir().expect("temp dir");
        let mut h = harness(&dir);
        h.keys(":o sr<tab>");
        // The lone directory is entered and listed.
        assert_eq!(line(&h), "o src/|");
        assert_eq!(
            open(&h).completion.candidates,
            [
                Candidate::File("lib.rs".to_owned()),
                Candidate::File("main.rs".to_owned()),
                Candidate::Dir("ui/".to_owned()),
            ]
        );
        h.keys("<S-tab>");
        assert_eq!(line(&h), "o src/ui/|");
        // Enter on a chosen directory lists it instead of submitting.
        h.keys("<ret>");
        assert_eq!(
            open(&h).completion.candidates,
            [Candidate::File("view.rs".to_owned())]
        );
        h.keys("<tab><ret>");
        assert!(matches!(h.state.overlay, Overlay::None));
        assert!(
            h.focused_path()
                .is_some_and(|path| path.ends_with("src/ui/view.rs"))
        );
    }

    #[test]
    fn paths_outside_the_root_do_not_complete() {
        let dir = tempfile::tempdir().expect("temp dir");
        let mut h = harness(&dir);
        h.keys(":o ../");
        assert!(open(&h).completion.candidates.is_empty());
        h.keys("<C-u>w /tm");
        assert!(open(&h).completion.candidates.is_empty());
        h.keys("<C-u>goto 1");
        assert!(open(&h).completion.candidates.is_empty());
    }
}
