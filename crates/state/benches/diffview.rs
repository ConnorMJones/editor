use std::hint::black_box;
use std::path::Path;

use criterion::{Criterion, criterion_group, criterion_main};
use git2::Oid;
use helix_core::{Rope, Selection};
use model::mode::Mode;
use state::git::diff::{Base, Lines, Side, Stamp};
use state::git::diffview::{self, Align, LEFT, RIGHT};
use state::text::buffer::TextDoc;
use state::view::display::{self, Focus, Frame, Indent, text_format};
use state::view::layout::{Scroll, Viewport};
use state::view::scroll;

// Estimate: a frame lays out both sides, each like one text view (~250 us for 60x120 with
// highlighting), plus filler lookups that are a partition point per frame and a compare per
// line, and the top-row mapping that formats one or two lines (~10 us): ~0.5 ms. Realigning
// formats only hunk lines: ~600 lines at ~2 us is ~1.2 ms, paid once per diff that lands.
fn diffview(c: &mut Criterion) {
    let loader = helix_core::config::default_lang_loader();
    loader.set_scopes(
        model::theme::SCOPES
            .iter()
            .map(|(scope, _)| (*scope).to_owned())
            .collect(),
    );
    let source = include_str!("../../../vendor/gpui/src/window.rs").repeat(2);
    // Every 100th line grows past the width, every 250th gains three lines, every 333rd goes.
    let mut edited = String::with_capacity(source.len() + source.len() / 10);
    for (index, line) in source.lines().enumerate() {
        match (index % 100, index % 250, index % 333) {
            (_, _, 0) => continue,
            (0, ..) => {
                edited.push_str(line);
                edited.push_str(
                    " // a trailing comment long enough to wrap this line onto a second row\n",
                );
            }
            (_, 0, _) => {
                edited.push_str(line);
                edited.push_str("\n    let added = 1;\n    let more = 2;\n    let last = 3;\n");
            }
            _ => {
                edited.push_str(line);
                edited.push('\n');
            }
        }
    }
    let path = Path::new("window.rs");
    let left = TextDoc::new(Rope::from(source.as_str()), path, &loader);
    let right = TextDoc::new(Rope::from(edited.as_str()), path, &loader);
    let head = Base::Blob {
        oid: Oid::from_bytes(&[1; 20]).expect("20 bytes"),
        text: left.text.clone(),
    };
    let mut lines = Lines::new();
    let bases = [head.oid(), head.oid()];
    lines.compute(
        Stamp { version: 0, bases },
        &head,
        &head,
        right.text.clone(),
    );
    let hunks = std::mem::take(&mut lines.hunks);
    let viewport = Viewport {
        rows: 60,
        cols: 120,
    };
    let format = text_format(viewport, Indent::of(&right, &loader));
    let mut align = Align::default();
    c.bench_function("diff realign 20k-line rust, ~320 hunks", |b| {
        b.iter(|| {
            diffview::realign(
                &mut align,
                left.text.slice(..),
                right.text.slice(..),
                black_box(&hunks),
                &format,
            );
            black_box(align.hunks.len())
        })
    });

    let middle = right.text.line_to_char(right.text.len_lines() / 2);
    let selection = Selection::point(middle);
    let top = Scroll {
        anchor: middle,
        rows: 0,
        viewport,
    };
    let mut frames = [Frame::default(), Frame::default()];
    c.bench_function("diff view layout 60x120, both sides", |b| {
        b.iter(|| {
            let spot = diffview::spot(
                right.text.slice(..),
                black_box(top),
                Side::After,
                &hunks,
                &align,
                &format,
            );
            let (anchor, rows) = diffview::place(
                left.text.slice(..),
                spot,
                Side::Before,
                &hunks,
                &align,
                &format,
            );
            let [head_frame, text_frame] = &mut frames;
            let sides = &align.sides;
            display::layout(
                head_frame,
                &left,
                &Selection::point(0),
                Scroll {
                    anchor,
                    rows,
                    viewport,
                },
                &loader,
                Focus::ReadOnly,
                &scroll::annotations(left.text.slice(..), sides[LEFT].padding()),
                &sides[LEFT].shades,
            );
            display::layout(
                text_frame,
                &right,
                &selection,
                top,
                &loader,
                Focus::Focused(Mode::Normal),
                &scroll::annotations(right.text.slice(..), sides[RIGHT].padding()),
                &sides[RIGHT].shades,
            );
            black_box(head_frame.rows.len() + text_frame.rows.len())
        })
    });
}

criterion_group!(benches, diffview);
criterion_main!(benches);
