//! The state decides what to typeset and keeps the results; the host owns the typesetter.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use helix_core::Rope;
use model::color::{Palette, Rgb};
use model::notice::Notice;
use model::typeset::{
    Compilation, Diagnostic, DocumentError, Jump, Origin, PagePoint, SourceFile, Svg,
};
use model::units::Pt;
use slotmap::SlotMap;

use crate::job::Job;
use crate::render::document::Documents;
#[cfg(feature = "typeset")]
use crate::render::markdown::FigureKind;
use crate::render::markdown::{Block, BlockKind, Figure, Format, SpanKind};
use crate::render::pages::{self, Compiles, PagesScroll};
use crate::text::buffer::{Buffer, BufferId};
use crate::view::layout::PaneId;
use crate::view::tab::{BesideMut, ShowingMut, Tab};
use crate::{Effect, State};
use model::job::Busy;

/// Unreferenced figures kept for undo and reopened views; older ones are evicted.
pub const SPARE: usize = 64;

/// How figures are drawn; a change drops every cached figure.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Look {
    pub text: Rgb,
    /// Size of the prose figures sit in; the typesetter matches math to its x-height.
    pub prose_size: Pt,
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
    /// Where byte `cursor` of `main` landed on its pages.
    Locate {
        buffer: BufferId,
        main: PathBuf,
        cursor: usize,
    },
    /// What a click at `at` on a page of `main` leads to, for the preview in `pane`.
    Click {
        pane: PaneId,
        main: PathBuf,
        at: PagePoint,
    },
    Pdf {
        root: PathBuf,
        main: PathBuf,
        text: Rope,
        out: PathBuf,
    },
}

/// typst named a place that could not be resolved; the typesetter logged why.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ClickFailed;

pub enum Done {
    Figure {
        figure: Figure,
        look: Look,
        result: Result<Svg, String>,
    },
    Pages {
        buffer: BufferId,
        version: u64,
        compiled: Compilation,
    },
    Pdf {
        out: PathBuf,
        result: Result<Vec<u8>, DocumentError>,
    },
    Located {
        buffer: BufferId,
        at: Option<PagePoint>,
    },
    Clicked {
        pane: PaneId,
        jump: Result<Option<Jump>, ClickFailed>,
    },
}

/// Typeset figures by content, in one look.
pub struct Figures {
    pub look: Look,
    pub entries: HashMap<Figure, Entry>,
    /// Bumped when an entry fills; eviction leaves it, as no parse references an evicted entry.
    pub generation: u64,
    /// Bumped by every reference walk; entries seen by the latest walk are live.
    pub clock: u64,
}

pub struct Entry {
    pub render: Render,
    /// `Figures::clock` of the last walk that found the figure in a parsed document.
    pub used: u64,
    /// `Figures::generation` when the render arrived; 0 while pending.
    pub filled: u64,
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
    pub compile: Busy,
    /// `TextDoc::version` of the compile in flight or last sent.
    pub requested: Option<u64>,
    pub pages: Option<Compiled>,
    pub outcome: Outcome,
    /// Project files other than the main one the last compile read; saving one recompiles.
    pub deps: Vec<PathBuf>,
    pub locate: Busy,
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
            compile: Busy::Idle,
            requested: None,
            pages: None,
            outcome: Outcome::Pending,
            deps: Vec::new(),
            locate: Busy::Idle,
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

/// Requests new figures and evicts the oldest unreferenced ones beyond `SPARE`.
pub fn mark(figures: &mut Figures, documents: &Documents, out: &mut Vec<Effect>) {
    figures.clock += 1;
    let Figures {
        look,
        entries,
        clock,
        ..
    } = figures;
    for markdown in documents.values().map(|parsed| &parsed.markdown) {
        each_figure(&markdown.blocks, |figure, source| {
            match entries.get_mut(&figure) {
                Some(entry) => entry.used = *clock,
                None => {
                    entries.insert(
                        figure,
                        Entry {
                            render: Render::Pending,
                            used: *clock,
                            filled: 0,
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
        }
    }
}

/// Starts a compile of each shown typst document whose version has none yet, unless one runs.
pub fn request_pages(
    compiles: &mut Compiles,
    buffers: &SlotMap<BufferId, Buffer>,
    root: &Path,
    shown: &[BufferId],
    out: &mut Vec<Effect>,
) {
    for &id in shown {
        let (Some(Buffer { path, doc, .. }), Some(typst)) = (buffers.get(id), compiles.get_mut(id))
        else {
            continue;
        };
        let version = doc.version;
        match (typst.compile, typst.requested == Some(version)) {
            (Busy::Running, _) | (Busy::Idle, true) => {}
            (Busy::Idle, false) => {
                typst.compile = Busy::Running;
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
    let focus = state.layout.focus();
    let Some(view) = state
        .layout
        .panes()
        .get(focus)
        .and_then(|pane| pane.active())
    else {
        state.notice = Some(Notice::NoBuffer);
        return;
    };
    let Buffer { path, doc, .. } = &state.buffers[view.buffer()];
    let notice = match Format::of(path) {
        Some(Format::Typst) => {
            out.push(Effect::Typeset(Request::Pdf {
                root: root_of(state.project.root.path(), path).to_owned(),
                main: path.clone(),
                text: doc.text.clone(),
                out: path.with_extension("pdf"),
            }));
            return;
        }
        Some(Format::Markdown) => Notice::MarkdownExport,
        None => Notice::NotTypst { path: path.clone() },
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
                    entry.filled = figures.generation;
                }
                (true, Some(_) | None) | (false, _) => {}
            }
        }
        Done::Pages {
            buffer,
            version,
            compiled: Compilation { result, deps },
        } => {
            let State {
                buffers,
                compiles,
                notice,
                ..
            } = state;
            let (Some(Buffer { path, doc, .. }), Some(typst)) =
                (buffers.get(buffer), compiles.get_mut(buffer))
            else {
                return;
            };
            let current = doc.version;
            typst.compile = Busy::Idle;
            typst.deps = deps.into_iter().filter(|dep| dep != path).collect();
            match (version == current, result) {
                (false, _) => {}
                (true, Ok(pages)) => {
                    typst.pages = Some(Compiled {
                        version,
                        sizes: (pages.pages.iter())
                            .map(|page| {
                                u64::from(page.width.0.to_bits()) << 32
                                    | u64::from(page.height.0.to_bits())
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
        Done::Located { buffer, at } => {
            let State {
                layout, compiles, ..
            } = state;
            let Some(typst) = compiles.get_mut(buffer) else {
                return;
            };
            // The next sync asks again for a cursor that moved meanwhile.
            typst.locate = Busy::Idle;
            // Pages waiting on a source line, or beside the source, go to where it landed.
            for pane in layout.panes_mut().map(|(_, pane)| pane) {
                let Some(ShowingMut::Pages(text, view, beside)) =
                    pane.active_mut().map(Tab::showing_mut)
                else {
                    continue;
                };
                match (text.buffer() == buffer, view.scroll, beside, at) {
                    (true, PagesScroll::Line(_), _, None) => view.scroll = PagesScroll::page(0),
                    (true, PagesScroll::Line(_), _, Some(at))
                    | (true, _, BesideMut::Source(..), Some(at)) => {
                        view.scroll = PagesScroll::Point(at)
                    }
                    (true, _, _, _) | (false, _, _, _) => {}
                }
            }
        }
        Done::Clicked { pane, jump } => match jump {
            Ok(Some(Jump::Source {
                file: SourceFile::Project(path),
                at,
            })) => pages::goto_source(state, pane, path, at, out),
            Ok(Some(Jump::Source {
                file: file @ (SourceFile::Embedded(_) | SourceFile::Package(_)),
                ..
            })) => state.notice = Some(Notice::OutsideProject { file }),
            Ok(Some(Jump::Url(url))) => out.push(Effect::OpenUrl(url)),
            Ok(Some(Jump::Page(at))) => match state.layout.pages_mut(pane) {
                Some((_, view, _)) => view.scroll = PagesScroll::Point(at),
                None => {}
            },
            Ok(None) => {}
            Err(ClickFailed) => state.notice = Some(Notice::JumpFailed),
        },
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
                FigureKind::Math(display) => {
                    typesetter.math(&source, display, look.text, look.prose_size)
                }
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
                compiled: typesetter.typst_pages(&root, &main, text),
            }
        }
        Request::Locate {
            buffer,
            main,
            cursor,
        } => Done::Located {
            buffer,
            at: typesetter.locate(&main, cursor),
        },
        Request::Click { pane, main, at } => Done::Clicked {
            pane,
            jump: typesetter.click(&main, at).map_err(|error| {
                tracing::warn!(%error, "typst click");
                ClickFailed
            }),
        },
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

/// Compiles that read one of `paths` run again.
pub fn invalidate(compiles: &mut Compiles, paths: &[PathBuf]) {
    let read = |typst: &&mut Typst| typst.deps.iter().any(|dep| paths.contains(dep));
    for typst in compiles.values_mut().filter(read) {
        typst.requested = None;
    }
}

pub fn exported(state: &mut State, path: PathBuf, result: std::io::Result<usize>) {
    state.notice = Some(match result {
        Ok(bytes) => Notice::Exported { path, bytes },
        Err(error) => {
            tracing::warn!(?path, %error, "export failed");
            Notice::WriteFailed { path, error }
        }
    });
}
