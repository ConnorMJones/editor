use std::hint::black_box;
use std::sync::Arc;

use criterion::{Criterion, criterion_group, criterion_main};
use state::backlinks::{self, OutLink};
use state::path::RelPath;
use state::vault::{self, Index};

const NOTES: usize = 2000;
const FOLDERS: usize = 20;

fn note_path(n: usize) -> String {
    format!("folder{}/note {n}.md", n % FOLDERS)
}

/// About 1.5 KB of prose, a list, a code block and eight links to other notes.
fn note(n: usize) -> String {
    let link = |k: usize| (n * 7 + k * 131) % NOTES;
    format!(
        "# Note {n}\n\nSome prose about [[note {a}]] and [[note {b}|an alias]] that goes on for \
         a while so the paragraph has a realistic length, with *emphasis* and `code`.\n\n\
         ## Details\n\n- item one links [[folder{f}/note {c}]]\n- item two links \
         [text](../folder{g}/note%20{d}.md#Details)\n- [ ] a task with [[note {e}#Details]]\n\n\
         ```rust\nfn main() {{ println!(\"[[not a link]]\"); }}\n```\n\n> [!note] Callout\n> with \
         [[note {h}]] inside\n\nClosing paragraph with ![[pic {n}.png]] and a [[missing {n}]] \
         link, plus more words to pad the note out to a typical size for a personal vault \
         entry, which is often a few hundred words long.\n",
        a = link(1),
        b = link(2),
        c = link(3),
        d = link(4),
        e = link(5),
        f = link(3) % FOLDERS,
        g = link(4) % FOLDERS,
        h = link(6),
    )
}

fn backlinks(c: &mut Criterion) {
    let dir = tempfile::tempdir().unwrap();
    let mut paths = Vec::with_capacity(NOTES);
    let mut texts = Vec::with_capacity(NOTES);
    for n in 0..NOTES {
        let path = note_path(n);
        let full = dir.path().join(&path);
        std::fs::create_dir_all(full.parent().unwrap()).unwrap();
        let text = note(n);
        std::fs::write(&full, &text).unwrap();
        paths.push(RelPath::parse(&path).unwrap());
        texts.push(text);
    }
    paths.sort_unstable();
    let bytes: usize = texts.iter().map(String::len).sum();
    let reads = backlinks::read(dir.path(), paths.clone());
    let links: usize = reads
        .iter()
        .map(|(_, read)| match read {
            backlinks::Read::Links(links) => links.len(),
            backlinks::Read::Gone => 0,
        })
        .sum();
    println!("{NOTES} notes, {} KB, {links} links", bytes / 1024);

    c.bench_function("backlinks: read and extract 2000 notes", |b| {
        b.iter(|| backlinks::read(black_box(dir.path()), paths.clone()).len())
    });
    let mut out: Vec<OutLink> = Vec::new();
    c.bench_function("backlinks: extract 2000 notes in memory", |b| {
        b.iter(|| {
            out.clear();
            for text in &texts {
                backlinks::extract(black_box(text), &mut out);
            }
            out.len()
        })
    });
    let files = Arc::new(paths.clone());
    c.bench_function("vault: index 2000 notes", |b| {
        b.iter(|| Index::new(1, black_box(files.clone())).names.len())
    });
    let index = Index::new(1, files);
    let all: Vec<(RelPath, Vec<OutLink>)> = reads
        .into_iter()
        .map(|(path, read)| match read {
            backlinks::Read::Links(links) => (path, links),
            backlinks::Read::Gone => (path, Vec::new()),
        })
        .collect();
    c.bench_function("vault: resolve every link of 2000 notes", |b| {
        b.iter(|| {
            let mut found = 0usize;
            for (path, links) in &all {
                let dir = path.parent().unwrap_or("");
                for link in links {
                    match vault::resolve(&index, dir, link.dest.as_str()) {
                        vault::Resolution::File(_) => found += 1,
                        vault::Resolution::Missing(_) | vault::Resolution::Invalid => {}
                    }
                }
            }
            found
        })
    });
    let wanted = vault::link_key("note 7.md");
    c.bench_function("backlinks: filter 2000 notes for one target", |b| {
        b.iter(|| {
            all.iter()
                .flat_map(|(_, links)| links)
                .filter(|link| link.key == black_box(wanted))
                .count()
        })
    });
}

criterion_group!(benches, backlinks);
criterion_main!(benches);
