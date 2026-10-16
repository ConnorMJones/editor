//! Background parses of opened buffers; at most one runs per buffer, and the tree kept matches
//! the current text.

use std::sync::Arc;

use helix_core::Syntax;
use helix_core::syntax::{HighlighterError, Loader};

use crate::job::Job;
use crate::text::buffer::{BufferId, Content, Highlight, TextDoc};
use crate::{Effect, State};

/// The parse of `doc`'s current text, when it waits for one.
pub fn job(buffer: BufferId, doc: &mut TextDoc, languages: &Arc<Loader>) -> Option<Job> {
    match &mut doc.highlight {
        Highlight::Parsing { language, version } => {
            *version = doc.version;
            Some(Job::Parse {
                buffer,
                version: doc.version,
                text: doc.text.clone(),
                language: *language,
                languages: languages.clone(),
            })
        }
        Highlight::Plain | Highlight::Parsed(_) | Highlight::Failed { .. } => None,
    }
}

/// Takes a finished parse of `buffer` at `version`; text edited since is parsed again.
pub fn parsed(
    state: &mut State,
    buffer: BufferId,
    version: u64,
    result: Result<Syntax, HighlighterError>,
    out: &mut Vec<Effect>,
) {
    // A buffer closed since the job started drops its result.
    let Some(Content::Text { doc, .. }) = state.buffers.get_mut(buffer).map(|b| &mut b.content)
    else {
        return;
    };
    let language = match doc.highlight {
        Highlight::Parsing { language, .. } => language,
        Highlight::Plain | Highlight::Parsed(_) | Highlight::Failed { .. } => return,
    };
    match version == doc.version {
        true => doc.highlight = Highlight::of(language, result),
        false => out.extend(job(buffer, doc, &state.languages).map(Effect::Job)),
    }
}
