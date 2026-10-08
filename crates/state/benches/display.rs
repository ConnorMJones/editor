use std::hint::black_box;
use std::path::Path;

use criterion::{Criterion, criterion_group, criterion_main};
use helix_core::{Rope, Selection};
use model::mode::Mode;
use state::buffer::{BufferId, TextDoc};
use state::display::{self, Focus, Frame, Indent, text_format};
use state::layout::{Scroll, TextView, Viewport};
use state::scroll;

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
        doc.syntax.is_some(),
        "rust grammar missing from HELIX_RUNTIME"
    );
    let viewport = Viewport {
        rows: 60,
        cols: 120,
    };
    let middle = doc.text.line_to_char(doc.text.len_lines() / 2);
    let view = TextView {
        buffer: BufferId::default(),
        selection: Selection::point(middle + 4),
        scroll: Scroll {
            anchor: middle,
            viewport,
        },
    };
    let mut frame = Frame::default();
    c.bench_function("layout+highlight 60x120 of 20k-line rust", |b| {
        b.iter(|| {
            display::layout(
                &mut frame,
                black_box(&doc),
                black_box(&view),
                &loader,
                Focus::Focused(Mode::Normal),
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
                middle,
                viewport.rows,
                &format,
            )
        })
    });
}

criterion_group!(benches, display);
criterion_main!(benches);
