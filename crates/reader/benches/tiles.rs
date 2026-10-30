use std::hint::black_box;
use std::sync::Arc;

use criterion::{Criterion, criterion_group, criterion_main};
use reader::pdf::{Document, Grid, Level, TileBlock};

const TEXTBOOK: &str =
    "../../examples/Analytic Combinatorics - Philippe Flajolet, Robert Sedgewick.pdf";

fn screen_of_tiles(c: &mut Criterion) {
    let bytes = std::fs::read(TEXTBOOK).expect("the textbook is in examples/");
    let document = Arc::new(Document::open(bytes.into()).unwrap());
    let level = Level(3);
    for (name, page) in [("text page", 300), ("figure page", 588)] {
        let grid = Grid::of(document.pages.sizes()[page], level);
        let row = |rows: std::ops::Range<u16>| TileBlock {
            page,
            level,
            columns: 0..grid.columns,
            rows,
        };
        c.bench_function(
            &format!("{name}, every tile at level 3, a row at a time"),
            |b| {
                b.iter(|| {
                    for strip in 0..grid.rows {
                        black_box(document.render_tiles(row(strip..strip + 1)).unwrap());
                    }
                })
            },
        );
        c.bench_function(
            &format!("{name}, every tile at level 3, two rows at a time"),
            |b| {
                b.iter(|| {
                    for strip in (0..grid.rows).step_by(2) {
                        let rows = strip..(strip + 2).min(grid.rows);
                        black_box(document.render_tiles(row(rows)).unwrap());
                    }
                })
            },
        );
    }
}

criterion_group! {
    name = benches;
    config = Criterion::default().sample_size(20);
    targets = screen_of_tiles
}
criterion_main!(benches);
