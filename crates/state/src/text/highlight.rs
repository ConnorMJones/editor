//! Background parses of text buffers, at most one per buffer.

use helix_core::Syntax;
use helix_core::syntax::HighlighterError;

use crate::job::Job;
use crate::text::buffer::{Buffer, BufferId, Highlight, ParseJob};
use crate::{Effect, State};

/// Starts a parse for every buffer that wants one.
pub fn sync(state: &mut State, out: &mut Vec<Effect>) {
    for (buffer, entry) in state.buffers.iter_mut() {
        let doc = &mut entry.doc;
        match &mut doc.highlight {
            Highlight::Parsing {
                language,
                job: job @ ParseJob::Wanted,
            } => {
                *job = ParseJob::Running {
                    version: doc.version,
                };
                out.push(Effect::Job(Job::Parse {
                    buffer,
                    version: doc.version,
                    text: doc.text.clone(),
                    language: *language,
                    languages: state.languages.clone(),
                }));
            }
            Highlight::Parsing {
                job: ParseJob::Running { .. },
                ..
            }
            | Highlight::Plain
            | Highlight::Parsed(_)
            | Highlight::Failed { .. } => {}
        }
    }
}

/// Takes a finished parse of `buffer` at `version`; text edited since is parsed again.
pub fn parsed(
    state: &mut State,
    buffer: BufferId,
    version: u64,
    result: Result<Syntax, HighlighterError>,
) {
    // A buffer closed since the job started drops its result.
    let Some(Buffer { doc, .. }) = state.buffers.get_mut(buffer) else {
        return;
    };
    let language = match doc.highlight {
        Highlight::Parsing { language, .. } => language,
        Highlight::Plain | Highlight::Parsed(_) | Highlight::Failed { .. } => return,
    };
    doc.highlight = match version == doc.version {
        true => Highlight::of(language, result),
        false => Highlight::Parsing {
            language,
            job: ParseJob::Wanted,
        },
    };
}
