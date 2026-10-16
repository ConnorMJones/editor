use std::fs;
use std::path::Path;

use helix_core::Selection;
use model::command::{Command, PickerCommand, PickerKind};
use model::notice::Notice;
use state::apply::apply;
use state::git::navigate::Arrive;
use state::job::{self, Job};
use state::text::buffer::{Content, Disk, Jump, Position, Target};
use state::{Effect, State};

use crate::harness::{Harness, disk_of};

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
                | Effect::OpenUrl(_)
                | Effect::Typeset(_) => {}
            }
        }
    });
}

#[test]
fn open_reads_and_parses_in_a_job() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("a.rs"), "fn a() {}\n").unwrap();
    let mut h = Harness::new(dir.path());

    let held = apply_held(&mut h.state, Command::Open("a.rs".into()));
    assert_eq!(open_jobs(&held), 1);
    let id = h.state.buffers.keys().next().unwrap();
    assert!(matches!(
        h.state.buffers[id].content,
        Content::Loading { jump: None }
    ));
    assert_eq!(h.focused_path(), Some(canonical(dir.path(), "a.rs")));

    // Opening it again reuses the loading buffer.
    let again = apply_held(&mut h.state, Command::Open("a.rs".into()));
    assert_eq!(jobs(&again), 0);
    assert_eq!(h.state.buffers.len(), 1);

    finish(&mut h, held);
    match &h.state.buffers[id].content {
        Content::Text { doc, disk, .. } => {
            assert_eq!(doc.text, "fn a() {}\n");
            assert!(doc.highlight.syntax().is_some(), "parsed off the UI thread");
            assert!(matches!(disk, Disk::Synced { .. }));
        }
        Content::Loading { .. } => panic!("still loading"),
    }
}

#[test]
fn loading_buffers_ignore_edits_and_cannot_be_written() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("a.txt"), "disk\n").unwrap();
    let mut h = Harness::new(dir.path());
    let held = apply_held(&mut h.state, Command::Open("a.txt".into()));

    for command in [
        Command::InsertMode,
        Command::InsertChar('x'),
        Command::NormalMode,
        Command::Move(model::command::Motion::LineDown),
        Command::Undo,
        Command::Reload,
    ] {
        let effects = apply_held(&mut h.state, command);
        assert_eq!(jobs(&effects), 0);
    }
    let write = apply_held(&mut h.state, Command::Write);
    assert_eq!(jobs(&write), 0);
    assert!(matches!(h.state.notice, Some(Notice::StillLoading { .. })));
    let write_as = apply_held(
        &mut h.state,
        Command::WriteAs("b.txt".into(), model::command::Dirs::Existing),
    );
    assert_eq!(jobs(&write_as), 0);
    assert_eq!(
        h.state.buffers.values().next().unwrap().path,
        canonical(dir.path(), "a.txt")
    );
    // Nothing is modified, so quitting is allowed while loading.
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
    assert_eq!(disk_of(h.state.buffers.values().next().unwrap()), Disk::New);
}

#[test]
fn failed_open_drops_the_buffer_and_restores_the_pane() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("a.txt"), "a\n").unwrap();
    fs::write(dir.path().join("bad.bin"), [0xff, 0xfe, 0x00]).unwrap();
    let mut h = Harness::new(dir.path());
    h.keys(":o a.txt<ret>");

    let held = apply_held(&mut h.state, Command::Open("bad.bin".into()));
    assert_eq!(h.state.buffers.len(), 2);
    assert_eq!(h.focused_path(), Some(canonical(dir.path(), "bad.bin")));
    finish(&mut h, held);

    assert!(matches!(h.state.notice, Some(Notice::NotUtf8 { .. })));
    assert_eq!(h.state.buffers.len(), 1);
    assert_eq!(h.state.buffers.len(), 1);
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
}

#[test]
fn grep_jump_waits_for_the_text() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("a.txt"), "one\ntwo needle\nthree\n").unwrap();
    let mut h = Harness::new(dir.path());
    h.keys("<space>/needle");

    let held = apply_held(&mut h.state, Command::Picker(PickerCommand::Submit));
    let focus = h.state.layout.focus();
    let id = h.state.buffers.keys().next().unwrap();
    assert_eq!(
        match &h.state.buffers[id].content {
            Content::Loading { jump } => jump.clone(),
            Content::Text { .. } => panic!("opened synchronously"),
        },
        Some(Jump {
            pane: focus,
            arrive: Arrive::Text,
            to: Target::Position(Position { line: 1, column: 4 }),
        })
    );
    finish(&mut h, held);
    assert_eq!(h.state.layout.focus(), focus);
    assert_eq!(h.view().selection, Selection::point(8));
}

#[test]
fn picker_kinds_still_open_while_loading() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("a.txt"), "a\n").unwrap();
    let mut h = Harness::new(dir.path());
    let held = apply_held(&mut h.state, Command::Open("a.txt".into()));
    let picker = apply_held(
        &mut h.state,
        Command::Picker(PickerCommand::Open(PickerKind::Buffers)),
    );
    assert_eq!(jobs(&picker), 0, "an open buffer previews from memory");
    finish(&mut h, held);
}

#[test]
fn write_quit_waits_for_nothing_while_loading() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("a.txt"), "a\n").unwrap();
    let mut h = Harness::new(dir.path());
    let held = apply_held(&mut h.state, Command::Open("a.txt".into()));
    let effects = apply_held(&mut h.state, Command::WriteQuit);
    assert_eq!(jobs(&effects), 0);
    assert_eq!(h.state.exit, state::Exit::Running);
    assert!(matches!(h.state.notice, Some(Notice::StillLoading { .. })));
    finish(&mut h, held);
}
