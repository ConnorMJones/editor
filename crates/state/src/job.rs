use std::fs::File;
use std::io::{self, BufWriter, Write as _};
use std::path::PathBuf;

use helix_core::Rope;
use model::notice::Notice;

use crate::buffer::{BufferId, Content, Disk};
use crate::{Effect, Exit, State};

/// Blocking work run off the UI thread.
pub enum Job {
    Write {
        buffer: BufferId,
        path: PathBuf,
        text: Rope,
        revision: usize,
    },
}

pub enum JobResult {
    Written {
        buffer: BufferId,
        path: PathBuf,
        revision: usize,
        result: io::Result<usize>,
    },
}

pub fn run(job: Job) -> JobResult {
    match job {
        Job::Write {
            buffer,
            path,
            text,
            revision,
        } => {
            let result = File::create(&path).and_then(|file| {
                let mut writer = BufWriter::new(file);
                for chunk in text.chunks() {
                    writer.write_all(chunk.as_bytes())?;
                }
                writer.flush()?;
                Ok(text.len_bytes())
            });
            JobResult::Written {
                buffer,
                path,
                revision,
                result,
            }
        }
    }
}

pub fn complete(state: &mut State, result: JobResult, out: &mut Vec<Effect>) {
    match result {
        JobResult::Written {
            buffer,
            path,
            revision,
            result: Ok(bytes),
        } => {
            if let Some(entry) = state.buffers.get_mut(buffer) {
                match &mut entry.content {
                    Content::Text(doc) => doc.saved_revision = revision,
                }
                entry.disk = Disk::Synced;
            }
            state.notice = Some(Notice::Written { path, bytes });
            match state.exit {
                Exit::AfterWrite { buffer: waiting } if waiting == buffer => {
                    state.exit = Exit::Running;
                    crate::apply::apply(state, model::command::Command::Quit, out);
                }
                Exit::AfterWrite { .. } | Exit::Running => {}
            }
        }
        JobResult::Written {
            buffer,
            path,
            result: Err(error),
            ..
        } => {
            tracing::warn!(?path, %error, "write failed");
            state.notice = Some(Notice::WriteFailed { path, error });
            match state.exit {
                Exit::AfterWrite { buffer: waiting } if waiting == buffer => {
                    state.exit = Exit::Running;
                }
                Exit::AfterWrite { .. } | Exit::Running => {}
            }
        }
    }
}
