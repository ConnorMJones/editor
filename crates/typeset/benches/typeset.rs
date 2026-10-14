use std::hint::black_box;

use criterion::{BatchSize, Criterion, criterion_group, criterion_main};
use model::color::Rgb;
use typeset::{Display, FontSource, Pt, Typesetter};

const WHITE: Rgb = Rgb {
    r: 0xff,
    g: 0xff,
    b: 0xff,
};

// Estimate: fontdb scans the fontconfig dirs and reads each face's name table (~20 us),
// then typst builds a FontInfo with a cmap coverage pass (~50-100 us); a few hundred
// system faces (Iosevka Nerd Font, DejaVu, ...) plus ~15 parsed embedded faces is ~30-40 ms.
// Embedded fonts alone are ~15 faces, ~2-5 ms.
fn new(c: &mut Criterion) {
    let mut group = c.benchmark_group("new");
    group.sample_size(10);
    group.bench_function("system_and_embedded", |b| {
        b.iter(|| {
            black_box(
                Typesetter::new(FontSource::SystemAndEmbedded, "Libertinus Serif").expect("fonts"),
            )
        })
    });
    group.bench_function("embedded", |b| {
        b.iter(|| {
            black_box(Typesetter::new(FontSource::Embedded, "Libertinus Serif").expect("fonts"))
        })
    });
    group.finish();
}

// Estimate: the first call evaluates the ~1300 lines of mitex definitions (~5-10 ms),
// parses New Computer Modern Math (~1-2 ms) and lays out one formula (<1 ms): ~10-15 ms.
// comemo's cache is process-global, so each sample clears it to stay cold.
fn math_cold(c: &mut Criterion) {
    let mut group = c.benchmark_group("math");
    group.sample_size(10);
    group.bench_function("cold", |b| {
        b.iter_batched(
            || {
                typst::comemo::evict(0);
                Typesetter::new(FontSource::SystemAndEmbedded, "Libertinus Serif").expect("fonts")
            },
            |mut typesetter| {
                black_box(
                    typesetter
                        .math(r"\frac{a+b}{c^2}", Display::Inline, WHITE, Pt(16.0))
                        .expect("math"),
                )
            },
            BatchSize::PerIteration,
        )
    });
    group.finish();
}

// Estimate: the mitex scope import is a comemo hit; per formula mitex converts (~10 us),
// typst reparses one line and evaluates the string (~50 us), lays out a small equation
// (~100-200 us) and writes SVG (~50 us): ~0.3-0.5 ms. Each formula is new, so no layout
// cache hit.
fn math_warm(c: &mut Criterion) {
    let mut typesetter =
        Typesetter::new(FontSource::SystemAndEmbedded, "Libertinus Serif").expect("fonts");
    typesetter
        .math("x", Display::Inline, WHITE, Pt(16.0))
        .expect("warm up");
    let mut counter = 0u64;
    c.bench_function("math/warm", |b| {
        b.iter(|| {
            counter += 1;
            let tex = format!(r"\sum_{{i=1}}^{{{counter}}} \frac{{x_i}}{{{counter}}}");
            black_box(
                typesetter
                    .math(&tex, Display::Inline, WHITE, Pt(16.0))
                    .expect("math"),
            )
        })
    });
}

// Estimate: a one-character edit reparses one paragraph; comemo reuses evaluation of the
// other sections and the layout of untouched paragraphs, so relayout is one paragraph plus
// page assembly (~1-3 ms), and typst-svg writes 5 text pages (~0.5 ms each): ~5 ms.
fn document_incremental(c: &mut Criterion) {
    let mut typesetter =
        Typesetter::new(FontSource::SystemAndEmbedded, "Libertinus Serif").expect("fonts");
    let dir = tempfile::tempdir().expect("tempdir");
    let main = dir.path().join("main.typ");
    let sentence = "The quick brown fox jumps over the lazy dog while typst lays out \
        this paragraph with justification and hyphenation enabled. ";
    let mut source = String::from("#set page(paper: \"a5\")\n#set par(justify: true)\n");
    for section in 0..5 {
        source.push_str(&format!("= Section {section}\n\n"));
        for _ in 0..2 {
            source.push_str(&sentence.repeat(5));
            source.push_str("\n\n");
        }
        source.push_str("$ sum_(i=1)^n i = (n(n+1))/2 $\n\n");
        match section {
            2 => source.push_str("Edited paragraph 000000 sits here.\n\n"),
            _ => {}
        }
        source.push_str("#pagebreak(weak: true)\n");
    }
    let marker = source.find("000000").expect("marker");
    let pages = typesetter
        .typst_pages(dir.path(), &main, &source)
        .result
        .expect("compiles");
    assert_eq!(pages.pages.len(), 5);
    let mut counter = 0u32;
    c.bench_function("typst/incremental_5_pages", |b| {
        b.iter(|| {
            counter += 1;
            source.replace_range(marker..marker + 6, &format!("{:06}", counter % 1_000_000));
            black_box(
                typesetter
                    .typst_pages(dir.path(), &main, &source)
                    .result
                    .expect("compiles"),
            )
        })
    });
    // Reference point for the incremental number: the same edit with comemo emptied.
    c.bench_function("typst/uncached_5_pages", |b| {
        b.iter(|| {
            counter += 1;
            source.replace_range(marker..marker + 6, &format!("{:06}", counter % 1_000_000));
            typst::comemo::evict(0);
            black_box(
                typesetter
                    .typst_pages(dir.path(), &main, &source)
                    .result
                    .expect("compiles"),
            )
        })
    });
}

criterion_group!(benches, new, math_cold, math_warm, document_incremental);
criterion_main!(benches);
