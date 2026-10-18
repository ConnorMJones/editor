//! Grammars and queries come from the `grammars` crate, compiled into the binary.

use anyhow::Result;
use tree_house::tree_sitter::Grammar;

pub fn get_language(name: &str) -> Result<Option<Grammar>> {
    Ok(grammars::language(name)
        .map(Grammar::try_from)
        .transpose()?)
}

pub fn load_runtime_file(language: &str, filename: &str) -> Result<String, std::io::Error> {
    grammars::query(language, filename)
        .map(str::to_owned)
        .ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("no {filename} query for {language}"),
            )
        })
}
