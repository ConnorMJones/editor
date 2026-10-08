use std::path::Path;

use helix_core::Rope;
use helix_core::syntax::HighlighterError;
use state::buffer::TextDoc;

/// Top-level node kinds and the root's byte length.
fn shape(doc: &TextDoc) -> (Vec<String>, u32) {
    let root = doc.syntax.as_ref().expect("parsed").tree().root_node();
    let kinds = root.children().map(|node| node.kind().to_owned()).collect();
    (kinds, root.end_byte())
}

#[test]
fn failed_update_reparses_from_scratch() {
    let loader = helix_core::config::default_lang_loader();
    let mut doc = TextDoc::new(Rope::from("fn a() {}\n"), Path::new("a.rs"), &loader);
    assert_eq!(shape(&doc), (vec!["function_item".to_owned()], 10));
    // A failed incremental update leaves the tree describing older text.
    doc.text = Rope::from("struct B;\nfn c() {}\n");
    doc.recover_syntax(Some(Err(HighlighterError::Timeout)), &loader);
    assert_eq!(
        shape(&doc),
        (
            vec!["struct_item".to_owned(), "function_item".to_owned()],
            20
        )
    );
}

#[test]
fn successful_updates_are_kept() {
    let loader = helix_core::config::default_lang_loader();
    let mut doc = TextDoc::new(Rope::from("fn a() {}\n"), Path::new("a.rs"), &loader);
    doc.text = Rope::from("struct B;\n");
    doc.recover_syntax(Some(Ok(())), &loader);
    assert_eq!(shape(&doc), (vec!["function_item".to_owned()], 10));
}
