use std::hint::black_box;
use std::path::Path;
use std::time::Instant;

use ::typeset::{FontSource, Typesetter};
use criterion::{Criterion, criterion_group, criterion_main};
use helix_core::Rope;
use model::color::{Palette, Rgb};
use state::markdown::{self, Base};
use state::typeset::{self, Done, Look, Request};
use state::vault::Index;

const LOOK: Look = Look {
    text: Rgb {
        r: 0xd0,
        g: 0xd0,
        b: 0xd0,
    },
    prose_size: model::typeset::Pt(12.0),
    palette: Palette(
        [Rgb {
            r: 0x80,
            g: 0x80,
            b: 0x80,
        }; 16],
    ),
};

/// A note with 40 inline and 10 display formulas, distinct per `seed`.
fn note(seed: usize) -> String {
    let mut note = String::new();
    for i in 0..40 {
        note.push_str(&format!(
            "Line {i}: let $x_{{{seed}}}^{{{i}}} + \\frac{{a_{i}}}{{b}}$ hold.\n\n"
        ));
    }
    for i in 0..10 {
        note.push_str(&format!(
            "$$\n\\sum_{{k=0}}^{{{seed}}} \\binom{{n}}{{k}} x^{{{i}}} = \\int_0^1 f(t)\\,dt\n$$\n\n"
        ));
    }
    note
}

/// Parses the note and typesets each figure, as the state and the typeset thread do.
fn turnaround(typesetter: &mut Typesetter, text: &mut String, seed: usize) -> usize {
    let vault = Index::new(1, Default::default());
    let base = Base {
        dir: Path::new("/nonexistent"),
        root: Path::new("/nonexistent"),
        folder: "",
        vault: &vault,
    };
    let source = Rope::from(note(seed));
    let parsed = markdown::parse(source.slice(..), base, None);
    let mut requests = Vec::new();
    typeset::each_figure(&parsed.blocks, |figure, source| {
        requests.push(Request::Figure {
            figure,
            look: LOOK,
            source: source.into(),
        })
    });
    requests
        .into_iter()
        .map(|request| match typeset::run(typesetter, request, text) {
            Done::Figure { result, .. } => result.expect("formula typesets").bytes.len(),
            Done::Pages { .. } | Done::Pdf { .. } | Done::Located { .. } | Done::Clicked { .. } => {
                unreachable!("only figures were asked")
            }
        })
        .sum()
}

// Estimate: the typesetter starts on its own thread at launch (~30-40 ms of font scanning), so
// a note pays only for its formulas. Cold, the first formula evaluates the mitex definitions
// (~10 ms); each further one converts, lays out and writes SVG in ~0.3-0.5 ms, so 50 formulas
// take ~25-35 ms cold and ~15-25 ms warm. Parsing the 100-line note is ~0.1 ms.
fn figures(c: &mut Criterion) {
    let started = Instant::now();
    let mut typesetter =
        Typesetter::new(FontSource::SystemAndEmbedded, "Libertinus Serif").expect("fonts");
    let fonts = started.elapsed();
    let mut text = String::new();
    let started = Instant::now();
    black_box(turnaround(&mut typesetter, &mut text, 0));
    println!(
        "typesetter start {:.1} ms; 50 figures cold {:.1} ms",
        fonts.as_secs_f64() * 1e3,
        started.elapsed().as_secs_f64() * 1e3
    );
    let mut seed = 1;
    let mut group = c.benchmark_group("figures");
    group.sample_size(20);
    group.bench_function("50 new formulas, warm typesetter", |b| {
        b.iter(|| {
            seed += 1;
            turnaround(&mut typesetter, &mut text, seed)
        })
    });
    group.finish();
}

criterion_group!(benches, figures);
criterion_main!(benches);
