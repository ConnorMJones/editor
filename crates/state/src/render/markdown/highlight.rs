//! Syntax highlighting of fenced code blocks.

use helix_core::syntax::{HighlightEvent, Loader};
use helix_core::{Rope, Syntax};
use model::theme::{self};

use super::{BlockKind, Markdown, Run};

/// Highlights code blocks missing from the cache, by their fence language.
pub fn highlight(markdown: &mut Markdown, loader: &Loader) {
    let Markdown {
        blocks, highlights, ..
    } = markdown;
    for block in blocks.iter() {
        let BlockKind::Code(code) = &block.kind else {
            continue;
        };
        match highlights.contains_key(&code.hash) {
            true => continue,
            false => {}
        }
        let runs = code.lang.as_deref().map_or_else(Box::default, |lang| {
            highlight_code(&code.text, lang, loader)
        });
        highlights.insert(code.hash, runs);
    }
}

/// Style runs of `code` in the language named `lang`; empty when it is unknown.
pub fn highlight_code(code: &str, lang: &str, loader: &Loader) -> Box<[Run]> {
    let name = Rope::from(lang);
    let Some(language) = loader.language_for_match(name.slice(..)) else {
        return Box::default();
    };
    let rope = Rope::from(code);
    let text = rope.slice(..);
    let Ok(syntax) = Syntax::new(text, language, loader) else {
        return Box::default();
    };
    let mut highlighter = syntax.highlighter(text, loader, 0..code.len() as u32);
    let mut runs: Vec<Run> = Vec::new();
    let mut style = theme::TEXT;
    let mut at = 0usize;
    loop {
        let next = match highlighter.next_event_offset() {
            u32::MAX => code.len(),
            next => (next as usize).min(code.len()),
        };
        match (next > at, runs.last_mut()) {
            (true, Some(last)) if last.style == style => last.end = next,
            (true, Some(_) | None) => runs.push(Run { end: next, style }),
            (false, _) => {}
        }
        at = next;
        match at >= code.len() {
            true => break,
            false => {}
        }
        let (event, active) = highlighter.advance();
        let base = match event {
            HighlightEvent::Refresh => theme::TEXT,
            HighlightEvent::Push => style,
        };
        style = active.fold(base, |acc, highlight| {
            acc.patch(theme::SCOPES[highlight.idx()].1)
        });
    }
    runs.into_boxed_slice()
}
