use std::fs;
use std::path::Path;

use helix_core::Selection;
use model::command::{Command, PickerCommand, PickerKind};
use model::notice::Notice;
use state::apply::apply;
use state::job::{self, Job};
use state::text::buffer::{Disk, Position, Target};
use state::{Effect, State};

use crate::harness::Harness;

fn canonical(dir: &Path, rel: &str) -> std::path::PathBuf {
    dir.canonicalize().unwrap().join(rel)
}

/// Applies `command` without running the jobs it queues.
fn apply_held(state: &mut State, command: Command) -> Vec<Effect> {
    let mut out = Vec::new();
    apply(state, command, &mut out);
    out
}

fn jobs(effects: &[Effect]) -> usize {
    effects
        .iter()
        .filter(|effect| matches!(effect, Effect::Job(_)))
        .count()
}

fn open_jobs(effects: &[Effect]) -> usize {
    effects
        .iter()
        .filter(|effect| matches!(effect, Effect::Job(Job::Open { .. })))
        .count()
}

/// Runs held jobs to completion, as the host does when they finish.
fn finish(h: &mut Harness, effects: Vec<Effect>) {
    h.run(|state, out| {
        for effect in effects {
            match effect {
                Effect::Job(work) => job::complete(state, job::run(work), out),
                Effect::Quit
                | Effect::Watch(_)
                | Effect::WriteClipboard { .. }
                | Effect::ReadClipboard { .. }
                | Effect::ReadClipboardImage { .. }
                | Effect::OpenUrl(_)
                | Effect::Typeset(_) => {}
            }
        }
    });
}

#[test]
fn open_reads_in_a_job_and_shows_the_file_once_read() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("a.rs"), "fn a() {}\n").unwrap();
    let mut h = Harness::new(dir.path());

    let held = apply_held(&mut h.state, Command::Open("a.rs".into()));
    assert_eq!(open_jobs(&held), 1);
    assert!(
        h.state.buffers.is_empty(),
        "a buffer exists once its file is read"
    );
    assert_eq!(h.focused_path(), None);

    // Opening it again waits on the same read.
    let again = apply_held(&mut h.state, Command::Open("a.rs".into()));
    assert_eq!(jobs(&again), 0);
    assert_eq!(h.state.reads.len(), 1);

    finish(&mut h, held);
    assert!(h.state.reads.is_empty());
    assert_eq!(h.focused_path(), Some(canonical(dir.path(), "a.rs")));
    assert_eq!(h.focused_tabs(), ["a.rs"]);
    let buffer = h.state.buffers.values().next().unwrap();
    assert_eq!(buffer.doc.text, "fn a() {}\n");
    assert!(
        buffer.doc.highlight.syntax().is_some(),
        "parsed off the UI thread"
    );
    assert!(matches!(buffer.disk, Disk::Synced { .. }));
}

#[test]
fn commands_before_the_read_lands_act_on_what_the_pane_shows() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("a.txt"), "disk\n").unwrap();
    let mut h = Harness::new(dir.path());
    let held = apply_held(&mut h.state, Command::Open("a.txt".into()));

    for command in [Command::Write, Command::Reload] {
        let effects = apply_held(&mut h.state, command);
        assert_eq!(jobs(&effects), 0);
        assert!(matches!(h.state.notice, Some(Notice::NoBuffer)));
    }
    let write_as = apply_held(
        &mut h.state,
        Command::WriteAs("b.txt".into(), model::command::Dirs::Existing),
    );
    assert_eq!(jobs(&write_as), 0);
    // Nothing is modified, so quitting is allowed while a read is in flight.
    let quit = apply_held(&mut h.state, Command::Quit);
    assert!(quit.iter().any(|effect| matches!(effect, Effect::Quit)));

    finish(&mut h, held);
    assert_eq!(h.doc().text, "disk\n");
    assert!(!h.doc().is_modified());
    assert!(!dir.path().join("b.txt").exists());
}

#[test]
fn missing_files_open_as_new() {
    let dir = tempfile::tempdir().unwrap();
    let mut h = Harness::new(dir.path());
    h.keys(":o new.txt<ret>");
    assert_eq!(h.doc().text, "");
    assert_eq!(h.state.buffers.values().next().unwrap().disk, Disk::New);
}

#[test]
fn a_failed_read_leaves_the_pane_as_it_was() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("a.txt"), "a\n").unwrap();
    fs::write(dir.path().join("bad.bin"), [0xff, 0xfe, 0x00]).unwrap();
    let mut h = Harness::new(dir.path());
    h.keys(":o a.txt<ret>");

    let held = apply_held(&mut h.state, Command::Open("bad.bin".into()));
    assert_eq!(h.state.buffers.len(), 1);
    assert_eq!(h.focused_path(), Some(canonical(dir.path(), "a.txt")));
    finish(&mut h, held);

    assert!(matches!(h.state.notice, Some(Notice::NotUtf8 { .. })));
    assert_eq!(h.state.buffers.len(), 1);
    assert!(h.state.reads.is_empty());
    assert_eq!(h.focused_path(), Some(canonical(dir.path(), "a.txt")));
}

#[test]
fn directories_are_refused_before_any_job() {
    let dir = tempfile::tempdir().unwrap();
    fs::create_dir(dir.path().join("sub")).unwrap();
    let mut h = Harness::new(dir.path());
    let held = apply_held(&mut h.state, Command::Open("sub".into()));
    assert_eq!(jobs(&held), 0);
    assert!(matches!(h.state.notice, Some(Notice::NotAFile { .. })));
    assert!(h.state.buffers.is_empty());
    assert!(h.state.reads.is_empty());
}

#[test]
fn grep_jump_waits_for_the_text() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("a.txt"), "one\ntwo needle\nthree\n").unwrap();
    let mut h = Harness::new(dir.path());
    h.keys("<space>/needle");

    let held = apply_held(&mut h.state, Command::Picker(PickerCommand::Submit));
    let focus = h.state.layout.focus();
    let read = h.state.reads.values().next().expect("a read in flight");
    assert_eq!(read.waiting.len(), 1);
    assert_eq!(read.waiting[0].pane, focus);
    assert_eq!(
        read.waiting[0].to,
        Some(Target::Position(Position { line: 1, column: 4 }))
    );
    finish(&mut h, held);
    assert_eq!(h.state.layout.focus(), focus);
    assert_eq!(h.view().selection, Selection::point(8));
}

#[test]
fn a_read_whose_pane_closed_shows_in_the_focus() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("a.txt"), "a\n").unwrap();
    fs::write(dir.path().join("b.txt"), "b\n").unwrap();
    let mut h = Harness::new(dir.path());
    h.keys(":o a.txt<ret><C-w>v");
    let held = apply_held(&mut h.state, Command::Open("b.txt".into()));
    h.keys("<C-w>q");
    let focus = h.state.layout.focus();
    finish(&mut h, held);
    assert_eq!(h.state.layout.focus(), focus);
    assert_eq!(h.focused_path(), Some(canonical(dir.path(), "b.txt")));
}

#[test]
fn picker_kinds_still_open_while_reading() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("a.txt"), "a\n").unwrap();
    let mut h = Harness::new(dir.path());
    let held = apply_held(&mut h.state, Command::Open("a.txt".into()));
    let picker = apply_held(
        &mut h.state,
        Command::Picker(PickerCommand::Open(PickerKind::Buffers)),
    );
    assert_eq!(jobs(&picker), 0);
    finish(&mut h, held);
}
