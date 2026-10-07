use std::hint::black_box;

use criterion::{Criterion, criterion_group, criterion_main};
use model::command::Command;
use model::key::Key;
use state::keymap::{Context, Keymap, Resolved};
use state::{Root, State, apply::apply, input};

fn keystroke(c: &mut Criterion) {
    let keymap = Keymap::helix().unwrap();
    c.bench_function("resolve C-k C-s", |b| {
        b.iter(|| {
            matches!(
                keymap.resolve(
                    Context::Insert,
                    black_box(&[Key::ctrl('k'), Key::ctrl('s')])
                ),
                Resolved::Command(_)
            )
        })
    });

    let dir = tempfile::tempdir().unwrap();
    let line = "fn main() { println!(\"hello world\"); }\n";
    std::fs::write(
        dir.path().join("big.rs"),
        line.repeat(10 * 1024 * 1024 / line.len()),
    )
    .unwrap();
    let mut state = State::new(
        Root::open(dir.path()).unwrap(),
        helix_core::config::default_lang_loader(),
    )
    .unwrap();
    let mut out = Vec::new();
    apply(&mut state, Command::Open("big.rs".into()), &mut out);
    apply(&mut state, Command::InsertMode, &mut out);
    c.bench_function("insert char into 10MB rust buffer", |b| {
        b.iter(|| input::key(&mut state, black_box(Key::char('x')), &mut out))
    });
}

criterion_group!(benches, keystroke);
criterion_main!(benches);
