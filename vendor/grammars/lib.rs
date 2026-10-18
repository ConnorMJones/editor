//! Tree-sitter parsers and helix queries compiled into the binary; see `sources.nix`.

use tree_sitter_language::LanguageFn;

include!(concat!(env!("OUT_DIR"), "/grammars.rs"));
