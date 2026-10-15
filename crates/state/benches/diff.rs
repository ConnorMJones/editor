use std::hint::black_box;

use criterion::{Criterion, criterion_group, criterion_main};
use git2::Oid;
use helix_core::Rope;
use state::git::diff::{Base, Lines, Outcome, Stamp};

// Estimate: re-interning 10k buffer lines at ~150 ns each (rope line iteration, hashing and
// comparing slices) dominates at ~1.5 ms; the diff itself strips the common prefix and suffix
// and touches only the edited line, plus a linear postprocess pass, so ~2 ms in total.
fn diff(c: &mut Criterion) {
    let source = include_str!("../../../vendor/gpui/src/window.rs");
    let mut text = String::new();
    for line in source.lines().cycle().take(10_000) {
        text.push_str(line);
        text.push('\n');
    }
    let head = Base::Blob {
        oid: Oid::from_bytes(&[1; 20]).expect("20 bytes"),
        text: Rope::from(text.as_str()),
    };
    let index = Base::Blob {
        oid: Oid::from_bytes(&[2; 20]).expect("20 bytes"),
        text: Rope::from(text.as_str()),
    };
    let edited = {
        let mut rope = Rope::from(text.as_str());
        let at = rope.line_to_char(5_000);
        rope.insert(at, "    let edited = 1;\n");
        rope
    };
    let bases = [head.oid(), index.oid()];
    let mut lines = Lines::new();
    let mut version = 0;
    lines.compute(Stamp { version, bases }, &head, &index, edited.clone());
    c.bench_function("diff 10k lines after a one-line edit", |b| {
        b.iter(|| {
            version += 1;
            let outcome = lines.compute(Stamp { version, bases }, &head, &index, edited.clone());
            match outcome {
                Outcome::Diffed { staged } => black_box(staged.is_none()),
                Outcome::TooLarge => unreachable!("10k lines is within limits"),
            };
            black_box(lines.hunks.len())
        })
    });
}

criterion_group!(benches, diff);
criterion_main!(benches);
