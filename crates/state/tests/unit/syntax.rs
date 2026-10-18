use std::path::Path;

use helix_core::syntax::HighlighterError;
use helix_core::{Rope, Selection, Transaction};
use model::notice::Notice;
use state::text::buffer::{Highlight, ParseJob, TextDoc};

/// Top-level node kinds and the root's byte length.
fn shape(doc: &TextDoc) -> (Vec<String>, u32) {
    let root = doc.highlight.syntax().expect("parsed").tree().root_node();
    let kinds = root.children().map(|node| node.kind().to_owned()).collect();
    (kinds, root.end_byte())
}

#[test]
fn a_failed_parse_waits_for_a_background_parse_on_the_next_edit() {
    let loader = helix_core::config::default_lang_loader();
    let mut doc = TextDoc::new(Rope::from("fn a() {}\n"), Path::new("a.rs"), &loader);
    let language = doc.highlight.language().expect("rust");
    doc.highlight = Highlight::Failed {
        language,
        error: HighlighterError::Timeout,
    };
    assert!(matches!(doc.highlight.tree(), Err(Notice::SyntaxFailed)));

    let tx = Transaction::change(&doc.text, [(0, 0, Some("struct B;\n".into()))].into_iter());
    doc.apply(&tx, &Selection::point(0), &loader);
    assert!(matches!(
        doc.highlight,
        Highlight::Parsing {
            job: ParseJob::Wanted,
            ..
        }
    ));
}

#[test]
fn edits_update_the_tree() {
    let loader = helix_core::config::default_lang_loader();
    let mut doc = TextDoc::new(Rope::from("fn a() {}\n"), Path::new("a.rs"), &loader);
    let tx = Transaction::change(
        &doc.text,
        [(10, 10, Some("struct B;\n".into()))].into_iter(),
    );
    doc.apply(&tx, &Selection::point(0), &loader);
    assert_eq!(
        shape(&doc),
        (
            vec!["function_item".to_owned(), "struct_item".to_owned()],
            20
        )
    );
}

#[test]
fn files_without_a_grammar_are_plain() {
    let loader = helix_core::config::default_lang_loader();
    let doc = TextDoc::new(Rope::from("text\n"), Path::new("notes.unknownext"), &loader);
    assert!(matches!(doc.highlight, Highlight::Plain));
    assert!(matches!(doc.highlight.tree(), Err(Notice::NoSyntax)));
}

#[test]
fn an_unparsed_doc_waits_for_its_tree() {
    let loader = helix_core::config::default_lang_loader();
    let doc = TextDoc::unparsed(Rope::from("fn a() {}\n"), Path::new("a.rs"), &loader);
    assert!(matches!(
        doc.highlight,
        Highlight::Parsing {
            job: ParseJob::Wanted,
            ..
        }
    ));
    assert!(matches!(doc.highlight.tree(), Err(Notice::SyntaxPending)));
    assert!(doc.highlight.language().is_some());
}
