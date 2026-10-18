use std::path::Path;

use helix_core::syntax::{HighlighterError, LanguageData};
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

#[test]
fn every_compiled_in_grammar_compiles_its_queries() {
    let loader = helix_core::config::default_lang_loader();
    let mut compiled = Vec::new();
    for config in loader.language_configs() {
        let name = &config.language_id;
        match grammars::language(config.grammar.as_deref().unwrap_or(name)) {
            Some(_) => {}
            None => continue,
        }
        let syntax = LanguageData::compile_syntax_config(config, &loader)
            .unwrap_or_else(|error| panic!("{name}: {error:#}"))
            .expect("the grammar is compiled in");
        LanguageData::compile_indent_query(syntax.grammar, config)
            .unwrap_or_else(|error| panic!("{name}: {error:#}"));
        LanguageData::compile_textobject_query(syntax.grammar, config)
            .unwrap_or_else(|error| panic!("{name}: {error:#}"));
        compiled.push(name.clone());
    }
    for expected in [
        "rust",
        "markdown",
        "markdown.inline",
        "typst",
        "tsx",
        "git-commit",
    ] {
        assert!(compiled.iter().any(|name| name == expected), "{expected}");
    }
}
