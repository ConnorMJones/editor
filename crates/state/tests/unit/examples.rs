use std::path::{Path, PathBuf};
use std::sync::Arc;

use helix_core::Rope;
use state::project::links::{self, Dest, Index, Resolution};
use state::project::path::RelPath;
use state::render::markdown::{
    self, Base, BlockKind, EmbedKind, ImageFile, Lines, LinkTarget, Markdown, NoteBlock,
};

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

fn notes() -> (Index, Vec<(RelPath, Markdown)>) {
    let root = examples();
    let mut found = Vec::new();
    files(&root, &root, &mut found);
    found.sort_unstable();
    let index = Index::new(1, Arc::new(found.clone()));
    let notes = found
        .into_iter()
        .filter(|file| file.as_str().ends_with(".md"))
        .filter(|file| file.as_str().starts_with("vault/") || file.as_str() == "README.md")
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
    let kinds: Vec<&BlockKind<Lines>> = notes
        .iter()
        .flat_map(|(_, parsed)| parsed.blocks.iter().map(|block| &block.kind))
        .collect();
    let has = |wanted: fn(&BlockKind<Lines>) -> bool| kinds.iter().any(|kind| wanted(kind));
    assert!(
        has(|kind| matches!(kind, BlockKind::Own(NoteBlock::Callout(_)))),
        "callout"
    );
    assert!(
        has(|kind| matches!(kind, BlockKind::Item(markdown::Task::Open { .. }))),
        "open task"
    );
    assert!(
        has(|kind| matches!(kind, BlockKind::Item(markdown::Task::Done { .. }))),
        "done task"
    );
    assert!(has(|kind| matches!(kind, BlockKind::Table(_))), "table");
    assert!(
        has(|kind| matches!(kind, BlockKind::Own(NoteBlock::Math(_)))),
        "math"
    );
    assert!(
        has(|kind| matches!(kind, BlockKind::Own(NoteBlock::Mermaid(_)))),
        "mermaid"
    );
    assert!(has(|kind| matches!(kind, BlockKind::Code(_))), "code");
    assert!(
        has(
            |kind| matches!(kind, BlockKind::Image(image) if matches!(image.file, ImageFile::Local(_)))
        ),
        "local image"
    );
    assert!(
        has(
            |kind| matches!(kind, BlockKind::Own(NoteBlock::Embed(embed)) if matches!(&embed.kind, EmbedKind::Note(Resolution::File(file)) if file.as_str() == "vault/math.md"))
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

#[test]
fn example_canvases_read_and_every_file_card_finds_its_file() {
    let root = examples();
    let vault = root.join("vault");
    let json_canvases: Vec<_> = std::fs::read_dir(&vault)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "canvas")
        })
        .collect();
    assert!(!json_canvases.is_empty());
    for path in json_canvases {
        let text = std::fs::read_to_string(&path).unwrap();
        let read = state::json_canvas::parse(&text, &root, &vault)
            .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
        for (node, file) in &read.files {
            assert!(
                !matches!(file, state::json_canvas::Embedded::Missing(_)),
                "{}: {} names a missing file",
                path.display(),
                node.as_str()
            );
        }
    }
}
