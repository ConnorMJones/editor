use std::sync::Arc;

use state::input::picker::{self, Items, Narrowing, Picker};
use state::project::path::RelPath;

fn files(paths: &[&str]) -> Picker {
    Picker::new(Items::Files(Arc::new(
        paths.iter().map(|p| RelPath::parse(p).unwrap()).collect(),
    )))
}

fn labels(picker: &Picker) -> Vec<&str> {
    picker
        .matches
        .iter()
        .map(|&i| picker.items.label(i))
        .collect()
}

#[test]
fn empty_query_lists_everything_in_order() {
    let picker = files(&["b", "a"]);
    assert_eq!(labels(&picker), ["b", "a"]);
}

#[test]
fn fuzzy_filter_ranks_and_narrows() {
    let mut matcher = nucleo::Matcher::default();
    let mut picker = files(&[
        "crates/state/src/apply.rs",
        "crates/editor/src/main.rs",
        "vendor/gpui/src/app.rs",
        "README.md",
    ]);
    picker.query = "app".to_owned();
    picker::filter(&mut picker, &mut matcher, Narrowing::All);
    assert_eq!(
        labels(&picker),
        ["vendor/gpui/src/app.rs", "crates/state/src/apply.rs"]
    );

    picker.query = "apply".to_owned();
    picker::filter(
        &mut picker,
        &mut matcher,
        Narrowing::between("app", "apply"),
    );
    assert_eq!(labels(&picker), ["crates/state/src/apply.rs"]);

    picker.query.clear();
    picker::filter(&mut picker, &mut matcher, Narrowing::between("apply", ""));
    assert_eq!(picker.matches.len(), 4);
}

#[test]
fn narrowing_only_when_query_grows_without_negation() {
    assert_eq!(Narrowing::between("ab", "abc"), Narrowing::FromMatches);
    assert_eq!(Narrowing::between("abc", "ab"), Narrowing::All);
    assert_eq!(Narrowing::between("", "a"), Narrowing::All);
    assert_eq!(Narrowing::between("a", "a !b"), Narrowing::All);
}

#[test]
fn grep_regex_is_smart_case_and_tolerates_bad_patterns() {
    assert!(picker::grep_regex("needle").is_match(b"A NEEDLE"));
    assert!(!picker::grep_regex("Needle").is_match(b"needle"));
    assert!(picker::grep_regex("fn(").is_match(b"fn(x)"));
}
