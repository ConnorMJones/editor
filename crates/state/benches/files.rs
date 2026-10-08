use std::hint::black_box;
use std::path::Path;
use std::sync::Arc;

use criterion::{Criterion, criterion_group, criterion_main};
use state::files;
use state::path::RelPath;
use state::picker::{self, Items, Narrowing, Picker};
use state::tree;

/// Synthetic repo-shaped paths: 100 top dirs x 10 subdirs x 100 files.
fn paths() -> Arc<Vec<RelPath>> {
    let mut out = Vec::with_capacity(100_000);
    for top in 0..100 {
        for sub in 0..10 {
            for file in 0..100 {
                out.push(
                    RelPath::parse(&format!("crate_{top}/src/module_{sub}/file_{file}_impl.rs"))
                        .unwrap(),
                );
            }
        }
    }
    out.sort_unstable();
    Arc::new(out)
}

fn filtering(c: &mut Criterion) {
    let paths = paths();
    let mut matcher = nucleo::Matcher::default();
    c.bench_function("picker filter 100k paths (full)", |b| {
        let mut picker = Picker::new(Items::Files(paths.clone()));
        picker.query = "mod3file42".to_owned();
        b.iter(|| {
            picker::filter(&mut picker, &mut matcher, Narrowing::All);
            black_box(picker.matches.len())
        })
    });
    c.bench_function("picker filter 100k paths (narrowing keystroke)", |b| {
        let mut picker = Picker::new(Items::Files(paths.clone()));
        picker.query = "crate_7/mod3file4".to_owned();
        picker::filter(&mut picker, &mut matcher, Narrowing::All);
        let narrowed = picker.matches.clone();
        picker.query.push('2');
        b.iter(|| {
            picker.matches.clone_from(&narrowed);
            picker::filter(&mut picker, &mut matcher, Narrowing::FromMatches);
            black_box(picker.matches.len())
        })
    });
}

fn listing(c: &mut Criterion) {
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let vendor = workspace.join("vendor").canonicalize().unwrap();
    c.bench_function("index walk vendor/", |b| {
        b.iter(|| black_box(files::walk(&vendor, &RelPath::ROOT, None).files.len()))
    });
    c.bench_function("tree list vendor/ root", |b| {
        b.iter(|| black_box(tree::list(&vendor, &RelPath::ROOT).unwrap().len()))
    });
    // A large real tree when the gitignored references are checked out.
    let zed = workspace.join("../../../refs/zed");
    match zed.canonicalize() {
        Ok(zed) => {
            let mut group = c.benchmark_group("large");
            group.sample_size(10);
            group.bench_function("index walk refs/zed", |b| {
                b.iter(|| black_box(files::walk(&zed, &RelPath::ROOT, None).files.len()))
            });
            group.finish();
        }
        Err(_) => {}
    }
}

criterion_group!(benches, filtering, listing);
criterion_main!(benches);
