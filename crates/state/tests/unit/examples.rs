use std::path::{Path, PathBuf};
use std::sync::Arc;

use helix_core::Rope;
use state::project::links::{self, Dest, Index, Resolution};
use state::project::path::RelPath;
use state::render::markdown::{self, Base, BlockKind, EmbedKind, ImageFile, LinkTarget, Markdown};

fn examples() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples")
}

fn files(root: &Path, dir: &Path, found: &mut Vec<RelPath>) {
    for entry in std::fs::read_dir(dir).expect("examples dir") {
        let path = entry.expect("dir entry").path();
        match path.is_dir() {
            true => files(root, &path, found),
            false => found
                .push(RelPath::parse(path.strip_prefix(root).unwrap().to_str().unwrap()).unwrap()),
        }
    }
}

/// Every markdown file under examples, parsed against an index of all example files.
fn notes() -> (Index, Vec<(RelPath, Markdown)>) {
    let root = examples();
    let mut found = Vec::new();
    files(&root, &root, &mut found);
    found.sort_unstable();
    let index = Index::new(1, Arc::new(found.clone()));
    let notes = found
        .into_iter()
        .filter(|file| file.as_str().ends_with(".md"))
        .map(|file| {
            let text = std::fs::read_to_string(root.join(file.as_str())).expect("read note");
            let folder = file.as_str().rsplit_once('/').map_or("", |(dir, _)| dir);
            let base = Base {
                dir: &root.join(folder),
                root: &root,
                folder,
                vault: &index,
            };
            let parsed = markdown::parse(Rope::from(text.as_str()).slice(..), base, None);
            (file, parsed)
        })
        .collect();
    (index, notes)
}

#[test]
fn vault_has_every_showcased_block() {
    let (_, notes) = notes();
    assert_eq!(notes.len(), 5);
    let kinds: Vec<&BlockKind> = notes
        .iter()
        .flat_map(|(_, parsed)| parsed.blocks.iter().map(|block| &block.kind))
        .collect();
    let has = |wanted: fn(&BlockKind) -> bool| kinds.iter().any(|kind| wanted(kind));
    assert!(has(|kind| matches!(kind, BlockKind::Callout(_))), "callout");
    assert!(
        has(|kind| matches!(kind, BlockKind::Item(markdown::Task::Open { .. }))),
        "open task"
    );
    assert!(
        has(|kind| matches!(kind, BlockKind::Item(markdown::Task::Done { .. }))),
        "done task"
    );
    assert!(has(|kind| matches!(kind, BlockKind::Table(_))), "table");
    assert!(has(|kind| matches!(kind, BlockKind::Math(_))), "math");
    assert!(has(|kind| matches!(kind, BlockKind::Mermaid(_))), "mermaid");
    assert!(has(|kind| matches!(kind, BlockKind::Code(_))), "code");
    assert!(
        has(
            |kind| matches!(kind, BlockKind::Image(image) if matches!(image.file, ImageFile::Local(_)))
        ),
        "local image"
    );
    assert!(
        has(
            |kind| matches!(kind, BlockKind::Embed(embed) if matches!(&embed.kind, EmbedKind::Note(Resolution::File(file)) if file.as_str() == "vault/math.md"))
        ),
        "embed of math.md"
    );
}

#[test]
fn only_the_deliberate_wikilink_is_unresolved() {
    let (index, notes) = notes();
    let mut missing = Vec::new();
    for (file, parsed) in &notes {
        let folder = file.as_str().rsplit_once('/').map_or("", |(dir, _)| dir);
        for inline in parsed.blocks.iter().filter_map(|block| block.inline()) {
            for link in &inline.links {
                match &link.target {
                    LinkTarget::Wiki(wiki) if !wiki.target.is_empty() => {
                        match links::resolve(&index, folder, Dest::Wiki(&wiki.target)) {
                            Resolution::File(_) => {}
                            Resolution::Missing(_) | Resolution::Invalid => {
                                missing.push(wiki.target.clone())
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
    }
    assert_eq!(missing, ["not-yet-written"]);
}
