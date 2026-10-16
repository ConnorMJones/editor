use std::hint::black_box;
use std::path::Path;

use criterion::{Criterion, criterion_group, criterion_main};
use helix_core::text_annotations::TextAnnotations;
use helix_core::{Rope, Selection};
use model::mode::Mode;
use state::git::conflict::{self, Conflicts};
use state::text::buffer::TextDoc;
use state::view::display::{self, Focus, Frame, Indent, text_format};
use state::view::layout::{Scroll, Viewport};
use state::view::scroll;

fn display(c: &mut Criterion) {
    let loader = helix_core::config::default_lang_loader();
    loader.set_scopes(
        model::theme::SCOPES
            .iter()
            .map(|(scope, _)| (*scope).to_owned())
            .collect(),
    );
    // About 20k lines of real Rust from the vendored GPUI window module.
    let source = include_str!("../../../vendor/gpui/src/window.rs").repeat(2);
    let text = Rope::from(source);
    let doc = TextDoc::new(text, Path::new("window.rs"), &loader);
    assert!(
        doc.highlight.syntax().is_some(),
        "rust grammar missing from HELIX_RUNTIME"
    );
    let viewport = Viewport {
        rows: 60,
        cols: 120,
    };
    let middle = doc.text.line_to_char(doc.text.len_lines() / 2);
    let selection = Selection::point(middle + 4);
    let scroll = Scroll {
        anchor: middle,
        rows: 0,
        viewport,
    };
    let mut frame = Frame::default();
    c.bench_function("layout+highlight 60x120 of 20k-line rust", |b| {
        b.iter(|| {
            display::layout(
                &mut frame,
                black_box(&doc),
                black_box(&selection),
                scroll,
                &loader,
                Focus::Focused(Mode::Normal),
                &TextAnnotations::default(),
                &[],
            );
            frame.rows.len()
        })
    });

    let format = text_format(viewport, Indent::of(&doc, &loader));
    let far = doc.text.line_to_char(doc.text.len_lines() / 2 + 500);
    c.bench_function("follow cursor 500 lines away", |b| {
        b.iter(|| {
            scroll::follow(
                doc.text.slice(..),
                black_box(far),
                scroll,
                0,
                &format,
                &TextAnnotations::default(),
            )
        })
    });

    // Runs once per edit batch; the scan only reads lines starting with a marker character.
    let mut conflicts = Conflicts::EMPTY;
    c.bench_function("conflict scan of 20k-line rust", |b| {
        b.iter(|| {
            conflict::parse(black_box(doc.text.slice(..)), 1, &mut conflicts);
            conflicts.list.len()
        })
    });
}

criterion_group!(benches, display);
criterion_main!(benches);
