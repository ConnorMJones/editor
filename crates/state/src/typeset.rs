//! Typesetting the host runs for the state: math and mermaid figures of rendered markdown,
//! typst preview pages and PDF export. The state decides what to typeset and keeps the
//! results; the host owns the typesetter and answers each `Request` with a `Done`.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use helix_core::Rope;
use model::color::{Palette, Rgb};
use model::notice::Notice;
use model::typeset::{Diagnostic, DocumentError, Origin, Pages, Svg};
use slotmap::SlotMap;

use crate::buffer::{Buffer, BufferId, Content};
use crate::job::Job;
use crate::markdown::{Block, BlockKind, Figure, Format, SpanKind};
#[cfg(feature = "typeset")]
use crate::markdown::{Display, FigureKind};
use crate::rendered::{Parsed, Preview, Previews};
use crate::{Effect, State};

/// Unreferenced figures kept for undo and reopened views; older ones are evicted.
pub const SPARE: usize = 64;

/// How figures are drawn; a change drops every cached figure.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Look {
    pub text: Rgb,
    pub inline_pt: f32,
    pub block_pt: f32,
    pub palette: Palette,
}

pub enum Request {
    Figure {
        figure: Figure,
        look: Look,
        source: Arc<str>,
    },
    Pages {
        buffer: BufferId,
        version: u64,
        root: PathBuf,
        main: PathBuf,
        text: Rope,
    },
    Pdf {
        root: PathBuf,
        main: PathBuf,
        text: Rope,
        out: PathBuf,
    },
}

pub enum Done {
    Figure {
        figure: Figure,
        look: Look,
        result: Result<Svg, String>,
    },
    Pages {
        buffer: BufferId,
        version: u64,
        result: Result<Pages, DocumentError>,
    },
    Pdf {
        out: PathBuf,
        result: Result<Vec<u8>, DocumentError>,
    },
}

/// Typeset figures by content, in one look.
pub struct Figures {
    pub look: Look,
    pub entries: HashMap<Figure, Entry>,
    /// Bumped whenever an entry is filled or evicted.
    pub generation: u64,
    /// Bumped by every reference walk; entries seen by the latest walk are live.
    pub clock: u64,
}

pub struct Entry {
    pub render: Render,
    /// `Figures::clock` of the last walk that found the figure in a parsed document.
    pub used: u64,
}

#[derive(Clone, Debug)]
pub enum Render {
    Pending,
    Ready(Svg),
    Failed(Arc<str>),
}

impl Figures {
    pub fn new(look: Look) -> Figures {
        Figures {
            look,
            entries: HashMap::new(),
            generation: 0,
            clock: 0,
        }
    }

    pub fn get(&self, figure: Figure) -> Option<&Render> {
        self.entries.get(&figure).map(|entry| &entry.render)
    }
}

/// A `.typ` buffer's preview: the pages of its last good compile and how the latest went.
pub struct Typst {
    pub compile: Compile,
    /// `TextDoc::version` of the compile in flight or last sent.
    pub requested: Option<u64>,
    pub pages: Option<Compiled>,
    pub outcome: Outcome,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Compile {
    Idle,
    /// A newer version waits for this one to finish, so at most one compile runs.
    Running,
}

pub struct Compiled {
    pub version: u64,
    pub pages: Vec<Svg>,
    /// Width and height bits of each page, parallel to `pages`.
    pub sizes: Vec<u64>,
    pub warnings: Vec<Diagnostic>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outcome {
    Pending,
    Compiled,
    /// Never empty; the last good pages stay shown.
    Failed(Vec<Diagnostic>),
}

impl Typst {
    pub fn new() -> Typst {
        Typst {
            compile: Compile::Idle,
            requested: None,
            pages: None,
            outcome: Outcome::Pending,
        }
    }
}

/// The directory a typst file compiles under: the project root, or the file's folder outside it.
pub fn root_of<'a>(root: &'a Path, main: &'a Path) -> &'a Path {
    match main.starts_with(root) {
        true => root,
        false => main.parent().unwrap_or(root),
    }
}

/// Calls `f` with every figure of `blocks` and its source text.
pub fn each_figure(blocks: &[Block], mut f: impl FnMut(Figure, &str)) {
    for block in blocks {
        match &block.kind {
            BlockKind::Math(math) => f(math.figure, &math.tex),
            BlockKind::Mermaid(mermaid) => f(mermaid.figure, &mermaid.src),
            _ => match block.inline() {
                Some(inline) => {
                    for span in inline.spans.iter() {
                        match span.kind {
                            SpanKind::Math(figure) => f(figure, &inline.text[span.range.clone()]),
                            SpanKind::Text
                            | SpanKind::Code
                            | SpanKind::Html
                            | SpanKind::Footnote
                            | SpanKind::Image => {}
                        }
                    }
                }
                None => {}
            },
        }
    }
}

/// Marks the figures of every parse live, requests the new ones and evicts the oldest
/// unreferenced ones beyond `SPARE`.
pub fn mark(figures: &mut Figures, previews: &Previews, out: &mut Vec<Effect>) {
    figures.clock += 1;
    let Figures {
        look,
        entries,
        clock,
        generation,
    } = figures;
    for preview in previews.values() {
        let Preview::Markdown(Parsed { markdown, .. }) = preview else {
            continue;
        };
        each_figure(&markdown.blocks, |figure, source| {
            match entries.get_mut(&figure) {
                Some(entry) => entry.used = *clock,
                None => {
                    entries.insert(
                        figure,
                        Entry {
                            render: Render::Pending,
                            used: *clock,
                        },
                    );
                    out.push(Effect::Typeset(Request::Figure {
                        figure,
                        look: *look,
                        source: Arc::from(source),
                    }));
                }
            }
        });
    }
    let unreferenced = entries
        .values()
        .filter(|entry| entry.used != *clock)
        .count();
    match unreferenced > SPARE {
        false => {}
        true => {
            let mut ages: Vec<u64> = (entries.values())
                .filter(|entry| entry.used != *clock)
                .map(|entry| entry.used)
                .collect();
            let cut = unreferenced - SPARE;
            ages.select_nth_unstable(cut - 1);
            let oldest = ages[cut - 1];
            let mut evict = cut;
            entries.retain(|_, entry| match (entry.used <= oldest, evict) {
                (true, 1..) => {
                    evict -= 1;
                    false
                }
                (true, 0) | (false, _) => true,
            });
            *generation += 1;
        }
    }
}

/// Starts a compile of each shown typst document whose version has none yet, unless one runs.
pub fn request_pages(
    previews: &mut Previews,
    buffers: &SlotMap<BufferId, Buffer>,
    root: &Path,
    shown: &[BufferId],
    out: &mut Vec<Effect>,
) {
    for &id in shown {
        let (
            Some(Buffer {
                path,
                content: Content::Text { doc, .. },
            }),
            Some(Preview::Typst(typst)),
        ) = (buffers.get(id), previews.get_mut(id))
        else {
            continue;
        };
        let version = doc.version;
        match (typst.compile, typst.requested == Some(version)) {
            (Compile::Running, _) | (Compile::Idle, true) => {}
            (Compile::Idle, false) => {
                typst.compile = Compile::Running;
                typst.requested = Some(version);
                out.push(Effect::Typeset(Request::Pages {
                    buffer: id,
                    version,
                    root: root_of(root, path).to_owned(),
                    main: path.clone(),
                    text: doc.text.clone(),
                }));
            }
        }
    }
}

/// `:export-pdf`: compiles the focused typst buffer to `<file>.pdf` beside it.
pub fn export_pdf(state: &mut State, out: &mut Vec<Effect>) {
    let focus = state.layout.focus;
    let Some(view) = state.layout.panes.get(focus).and_then(|pane| pane.active()) else {
        state.notice = Some(Notice::NoBuffer);
        return;
    };
    let Some(Buffer { path, content }) = state.buffers.get(view.buffer()) else {
        state.notice = Some(Notice::NoBuffer);
        return;
    };
    let notice = match (Format::of(path), content) {
        (Some(Format::Typst), Content::Text { doc, .. }) => {
            out.push(Effect::Typeset(Request::Pdf {
                root: root_of(state.root.path(), path).to_owned(),
                main: path.clone(),
                text: doc.text.clone(),
                out: path.with_extension("pdf"),
            }));
            return;
        }
        (Some(Format::Typst), Content::Loading { .. }) => {
            Notice::StillLoading { path: path.clone() }
        }
        (Some(Format::Markdown), _) => Notice::MarkdownExport,
        (None, _) => Notice::NotTypst { path: path.clone() },
    };
    state.notice = Some(notice);
}

/// Takes a finished typeset; results for another look, version or a closed view are dropped.
pub fn finish(state: &mut State, done: Done, out: &mut Vec<Effect>) {
    match done {
        Done::Figure {
            figure,
            look,
            result,
        } => {
            let figures = &mut state.figures;
            match (look == figures.look, figures.entries.get_mut(&figure)) {
                (
                    true,
                    Some(
                        entry @ Entry {
                            render: Render::Pending,
                            ..
                        },
                    ),
                ) => {
                    entry.render = match result {
                        Ok(svg) => Render::Ready(svg),
                        Err(error) => {
                            tracing::debug!(?figure, %error, "figure failed");
                            Render::Failed(Arc::from(error))
                        }
                    };
                    figures.generation += 1;
                }
                (true, Some(_) | None) | (false, _) => {}
            }
        }
        Done::Pages {
            buffer,
            version,
            result,
        } => {
            let State {
                buffers,
                previews,
                notice,
                ..
            } = state;
            let (Some(doc), Some(Preview::Typst(typst))) = (
                crate::buffer::doc(buffers, buffer),
                previews.get_mut(buffer),
            ) else {
                return;
            };
            let current = doc.version;
            typst.compile = Compile::Idle;
            match (version == current, result) {
                (false, _) => {}
                (true, Ok(pages)) => {
                    typst.pages = Some(Compiled {
                        version,
                        sizes: (pages.pages.iter())
                            .map(|page| {
                                u64::from(page.width_pt.to_bits()) << 32
                                    | u64::from(page.height_pt.to_bits())
                            })
                            .collect(),
                        pages: pages.pages,
                        warnings: pages.warnings,
                    });
                    typst.outcome = Outcome::Compiled;
                }
                (true, Err(error)) => {
                    let errors = errors_of(error);
                    match &typst.outcome {
                        Outcome::Failed(_) => {}
                        Outcome::Pending | Outcome::Compiled => {
                            *notice = Some(Notice::TypstErrors {
                                first: errors[0].clone(),
                                count: errors.len(),
                            })
                        }
                    }
                    typst.outcome = Outcome::Failed(errors);
                }
            }
        }
        Done::Pdf { out: path, result } => match result {
            Ok(bytes) => out.push(Effect::Job(Job::Export { path, bytes })),
            Err(error) => {
                let errors = errors_of(error);
                state.notice = Some(Notice::TypstErrors {
                    first: errors[0].clone(),
                    count: errors.len(),
                });
            }
        },
    }
}

/// The errors of a failed compile, never empty.
fn errors_of(error: DocumentError) -> Vec<Diagnostic> {
    match error {
        DocumentError::Compile(diagnostics) if !diagnostics.errors.is_empty() => diagnostics.errors,
        error @ (DocumentError::Compile(_) | DocumentError::OutsideRoot { .. }) => {
            vec![Diagnostic {
                origin: Origin::Detached,
                message: error.to_string(),
                hints: Vec::new(),
            }]
        }
    }
}

/// Answers `request` on the host's typesetter; `text` is a reused buffer for document sources.
#[cfg(feature = "typeset")]
pub fn run(typesetter: &mut ::typeset::Typesetter, request: Request, text: &mut String) -> Done {
    let mut source = |rope: &Rope| {
        text.clear();
        for chunk in rope.chunks() {
            text.push_str(chunk);
        }
    };
    match request {
        Request::Figure {
            figure,
            look,
            source,
        } => {
            let result = match figure.kind {
                FigureKind::Math(display) => typesetter.math(
                    &source,
                    display,
                    look.text,
                    match display {
                        Display::Inline => look.inline_pt,
                        Display::Block => look.block_pt,
                    },
                ),
                FigureKind::Mermaid => typesetter.mermaid(&source, &look.palette),
            };
            Done::Figure {
                figure,
                look,
                result: result.map_err(|error| error.to_string()),
            }
        }
        Request::Pages {
            buffer,
            version,
            root,
            main,
            text: rope,
        } => {
            source(&rope);
            Done::Pages {
                buffer,
                version,
                result: typesetter.typst_pages(&root, &main, text),
            }
        }
        Request::Pdf {
            root,
            main,
            text: rope,
            out,
        } => {
            source(&rope);
            Done::Pdf {
                out,
                result: typesetter.typst_pdf(&root, &main, text),
            }
        }
    }
}
