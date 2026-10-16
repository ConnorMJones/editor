//! Typesets LaTeX math, mermaid diagrams and typst documents into SVG, and typst into PDF.
//!
//! [`Typesetter`] is `Send` and its methods take `&mut self`. Share one instance behind a
//! `Mutex` across background jobs: fonts are scanned once and typst's comemo cache stays
//! warm, so jobs run one at a time but each is fast. A second instance rescans fonts.

mod world;

use std::collections::HashMap;
use std::fmt::Write as _;
use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError};

use model::color::{Base16, Palette, Rgb};
pub use model::typeset::{
    Compilation, Diagnostic, Diagnostics, Display, DocumentError, Jump, LineColumn, Origin,
    PackageFile, PagePoint, Pages, SourceFile, Svg,
};
pub use model::units::{Pt, Px};
use typst::diag::{FileError, SourceDiagnostic, Warned};
use typst::foundations::Bytes;
use typst::introspection::{PagedPosition, Tag};
use typst::layout::{Abs, Frame, FrameItem, Point, Size};
use typst::math::EquationElem;
use typst::syntax::{FileId, RealizeError, RootedPath, Source, VirtualPath, VirtualRoot};
use typst::text::{FontVariant, FontVariations};
use typst::utils::LazyHash;
use typst::{Library, LibraryExt, World, WorldExt};
use typst_kit::fonts::{self, FontStore};
use typst_layout::{Page, PagedDocument};

use crate::world::{Disk, EMBEDDED, EMBEDDED_NAMESPACE, Slot, TypesetWorld, embedded_id};

/// comemo results unused for this many compiles are evicted.
const CACHE_AGE: usize = 30;

pub enum FontSource {
    /// Only typst's bundled fonts (Libertinus, New Computer Modern, DejaVu Sans Mono).
    Embedded,
    /// Fonts found through fontconfig, plus typst's bundled fonts.
    SystemAndEmbedded,
}

#[derive(Debug, thiserror::Error)]
pub enum TypesetError {
    #[error("the math font New Computer Modern Math is missing")]
    NoMathFont,
    #[error("the prose font {0} is missing or has no x glyph")]
    NoXHeight(String),
    #[error("unsupported LaTeX: {0}")]
    Latex(String),
    #[error("typst rejected the converted LaTeX: {}", .0.join("; "))]
    Math(Vec<String>),
    #[error("typst laid out the inline equation without its start tag")]
    NoBaseline,
    #[error("the source is not a mermaid diagram")]
    NotMermaid,
    #[error("mermaid: {0}")]
    Mermaid(#[from] merman::svg::HeadlessError),
    #[error("the mermaid SVG has no usable viewBox")]
    MermaidSize,
    #[error("the mermaid output has no root element")]
    MermaidSvg,
}

/// A place typst named that cannot be turned into a file and position.
#[derive(Debug, thiserror::Error)]
pub enum LookupError {
    #[error("no compiled document for {0:?}")]
    NotCompiled(PathBuf),
    #[error("typst cannot read {file:?}: {error}")]
    Unreadable { file: FileId, error: FileError },
    #[error("byte {byte} is outside {file:?}")]
    OutOfRange { file: FileId, byte: usize },
    #[error("{file:?} does not map under the root: {error:?}")]
    OutsideRoot { file: FileId, error: RealizeError },
}

/// A merman renderer themed from one palette.
pub struct Mermaid {
    pub palette: Palette,
    pub renderer: merman::svg::HeadlessRenderer,
    pub pipeline: merman::svg::SvgPipeline,
}

pub struct Typesetter {
    pub world: TypesetWorld,
    /// Rebuilt when a call brings a different palette.
    pub mermaid: Option<Mermaid>,
    /// Reused buffer for the generated math document.
    pub math_text: String,
    /// SVG of each page from the last `typst_pages`, keyed by the page's hash, so an edit
    /// re-exports only the pages it changed.
    pub exported_pages: Vec<(u128, Svg)>,
    /// Last good document of each main file, for `locate` and `click`.
    pub documents: HashMap<PathBuf, Kept>,
    /// Math size per prose size that gives both the same x-height.
    pub math_scale: f32,
}

impl Typesetter {
    /// `prose` names the font family math is set beside; math matches its x-height.
    pub fn new(fonts: FontSource, prose: &str) -> Result<Typesetter, TypesetError> {
        let mut store = FontStore::new();
        store.extend(fonts::embedded());
        match fonts {
            FontSource::Embedded => {}
            FontSource::SystemAndEmbedded => store.extend(fonts::system()),
        }
        let book = store.book();
        tracing::debug!(families = book.families().count(), "typeset fonts loaded");
        let prose_x =
            x_height(&store, prose).ok_or_else(|| TypesetError::NoXHeight(prose.to_owned()))?;
        // Equations default to this family.
        let math_x =
            x_height(&store, "New Computer Modern Math").ok_or(TypesetError::NoMathFont)?;

        let math = embedded_id("/math.typ");
        Ok(Typesetter {
            world: TypesetWorld {
                library: LazyHash::new(<Library as LibraryExt>::default()),
                fonts: store,
                main: math,
                math: Source::new(math, String::new()),
                embedded: EMBEDDED
                    .map(|(path, text)| Source::new(embedded_id(path), text.to_owned())),
                root: PathBuf::new(),
                disk: Mutex::new(Disk::default()),
            },
            mermaid: None,
            math_text: String::new(),
            exported_pages: Vec::new(),
            documents: HashMap::new(),
            math_scale: (prose_x / math_x) as f32,
        })
    }

    pub fn math(
        &mut self,
        tex: &str,
        display: Display,
        color: Rgb,
        prose_size: Pt,
    ) -> Result<Svg, TypesetError> {
        let converted = mitex::convert_math(tex, None).map_err(TypesetError::Latex)?;
        let block = match display {
            Display::Inline => "false",
            Display::Block => "true",
        };
        let size_pt = prose_size.0 * self.math_scale;
        self.math_text.clear();
        write!(
            self.math_text,
            "#import \"mitex/mod.typ\": mitex-scope\n\
             #set page(width: auto, height: auto, margin: 0pt, fill: none)\n\
             #set text(fill: rgb({}, {}, {}), size: {size_pt}pt, top-edge: \"bounds\", bottom-edge: \"bounds\")\n\
             #set par(leading: 0pt)\n\
             #math.equation(block: {block}, eval(scope: mitex-scope, \"$",
            color.r, color.g, color.b,
        )
        .expect("writing to a String cannot fail");
        for char in converted.chars() {
            match char {
                '\\' => self.math_text.push_str("\\\\"),
                '"' => self.math_text.push_str("\\\""),
                char => self.math_text.push(char),
            }
        }
        self.math_text.push_str("$\"))\n");
        self.world.math.replace(&self.math_text);
        self.world.main = self.world.math.id();

        let Warned { output, warnings } = typst::compile::<PagedDocument>(&self.world);
        typst::comemo::evict(CACHE_AGE);
        for warning in &warnings {
            tracing::debug!(message = %warning.message, "math warning");
        }
        let document = output.map_err(|errors| {
            TypesetError::Math(
                errors
                    .iter()
                    .map(|error| error.message.to_string())
                    .collect(),
            )
        })?;
        let [page] = document.pages() else {
            return Err(TypesetError::Math(vec![format!(
                "expected one page, got {}",
                document.pages().len()
            )]));
        };
        let svg = page_svg(page);
        Ok(match display {
            Display::Inline => Svg {
                baseline: Pt(equation_baseline(&page.frame, Abs::zero())
                    .ok_or(TypesetError::NoBaseline)?
                    .to_pt() as f32),
                ..svg
            },
            Display::Block => svg,
        })
    }

    pub fn mermaid(&mut self, src: &str, palette: &Palette) -> Result<Svg, TypesetError> {
        let mermaid = match self.mermaid.take() {
            Some(mermaid) if mermaid.palette == *palette => self.mermaid.insert(mermaid),
            _ => {
                let hex = |slot: Base16| {
                    let Rgb { r, g, b } = palette[slot];
                    serde_json::Value::String(format!("#{r:02x}{g:02x}{b:02x}"))
                };
                let variables: serde_json::Map<String, serde_json::Value> = MERMAID_THEME
                    .iter()
                    .map(|&(name, slot)| (name.to_owned(), hex(slot)))
                    .chain(
                        MERMAID_ACCENTS
                            .iter()
                            .enumerate()
                            .flat_map(|(index, &slot)| {
                                [
                                    (format!("cScale{index}"), hex(slot)),
                                    (format!("cScaleLabel{index}"), hex(Base16::Base00)),
                                    (format!("pie{}", index + 1), hex(slot)),
                                ]
                            }),
                    )
                    .collect();
                let Rgb { r, g, b } = palette[Base16::Base00];
                let luma = 0.2126 * f32::from(r) + 0.7152 * f32::from(g) + 0.0722 * f32::from(b);
                let config = merman::MermaidConfig::from_value(serde_json::json!({
                    "theme": "base",
                    "darkMode": luma < 128.0,
                    "fontFamily": "DejaVu Sans, sans-serif",
                    // The resvg-safe pipeline turns HTML labels into wrapped SVG text.
                    "htmlLabels": true,
                    "flowchart": { "htmlLabels": true, "padding": 16 },
                    "themeVariables": variables,
                    // Mermaid draws edge label boxes at half opacity, which washes out on dark themes.
                    "themeCSS": ".edgeLabel rect { opacity: 1; }",
                }));
                self.mermaid.insert(Mermaid {
                    palette: *palette,
                    renderer: merman::svg::HeadlessRenderer::new()
                        .with_site_config(config)
                        .with_vendored_text_measurer(),
                    pipeline: merman::svg::SvgPipeline::resvg_safe().with_postprocessor(
                        merman::svg::CssOverridePostprocessor::strip_existing_important(),
                    ),
                })
            }
        };
        let mut svg = mermaid
            .renderer
            .render_svg_with_pipeline_sync(src, &mermaid.pipeline)?
            .ok_or(TypesetError::NotMermaid)?;
        // Mermaid paints the root white; the diagram sits on the reader's background instead.
        let root_end = svg.find('>').ok_or(TypesetError::MermaidSvg)?;
        match svg[..root_end].find("background-color:white") {
            Some(at) => svg.replace_range(
                at..at + "background-color:white".len(),
                "background-color:none",
            ),
            None => {}
        }
        let root = svg
            .find("<svg")
            .and_then(|start| svg[start..].find('>').map(|end| &svg[start..start + end]))
            .ok_or(TypesetError::MermaidSize)?;
        let view_box = root
            .split_once("viewBox=\"")
            .and_then(|(_, rest)| rest.split_once('"'))
            .map(|(view_box, _)| view_box)
            .ok_or(TypesetError::MermaidSize)?;
        let [_, _, width, height] = {
            let mut numbers = view_box
                .split([' ', ','])
                .filter(|part| !part.is_empty())
                .map(str::parse::<f32>);
            [(); 4].map(|()| numbers.next().and_then(Result::ok))
        };
        match (width, height) {
            (Some(width), Some(height)) if width > 0.0 && height > 0.0 => Ok(Svg {
                bytes: Arc::from(svg.into_bytes()),
                width: Pt::from(Px(width)),
                height: Pt::from(Px(height)),
                baseline: Pt::from(Px(height)),
            }),
            _ => Err(TypesetError::MermaidSize),
        }
    }

    /// Compiles `main` to pages, keeping the document for `locate` and `click`.
    pub fn typst_pages(&mut self, root: &Path, main: &Path, source: &str) -> Compilation {
        let compiled = self.compile_document(root, main, source);
        let deps = match &compiled {
            Err(DocumentError::OutsideRoot { .. }) => Vec::new(),
            Ok(_) | Err(DocumentError::Compile(_)) => {
                let disk = self
                    .world
                    .disk
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner);
                (disk.slots.iter())
                    .filter(|(_, slot)| slot.generation == disk.generation)
                    .filter_map(|(id, _)| match id.root() {
                        VirtualRoot::Project => id.vpath().realize(&self.world.root).ok(),
                        VirtualRoot::Package(_) => None,
                    })
                    .collect()
            }
        };
        let result = compiled.map(|(id, Warned { output, warnings })| {
            let exported: Vec<(u128, Svg)> = (output.pages().iter().enumerate())
                .map(|(index, page)| {
                    let hash = typst::utils::hash128(page);
                    match self.exported_pages.get(index) {
                        Some((cached, svg)) if *cached == hash => (hash, svg.clone()),
                        _ => (hash, page_svg(page)),
                    }
                })
                .collect();
            let pages = exported.iter().map(|(_, svg)| svg.clone()).collect();
            self.exported_pages = exported;
            let warnings = self.diagnostics(&warnings);
            // The main source as compiled; the world's slots move on with the next root.
            let source = self
                .world
                .source(id)
                .expect("the main source was just compiled");
            self.documents.insert(
                main.to_owned(),
                Kept {
                    source,
                    document: output,
                },
            );
            Pages { pages, warnings }
        });
        Compilation { result, deps }
    }

    /// Where byte `cursor` of `main` landed in its last compiled document; off text, the first
    /// text after it on the same line.
    pub fn locate(&self, main: &Path, cursor: usize) -> Option<PagePoint> {
        let kept = self.documents.get(main)?;
        let source = &kept.source;
        let lines = source.lines();
        let end = lines.line_to_range(lines.byte_to_line(cursor)?)?.end;
        let position = (cursor..=end)
            .filter(|&byte| source.text().is_char_boundary(byte))
            .find_map(|byte| {
                typst_ide::jump_from_cursor(&kept.document, source, byte)
                    .into_iter()
                    .next()
            })?;
        Some(page_point(&position))
    }

    /// What the point `at` on a page of `main`'s last compiled document leads to.
    /// `None` when nothing at the point leads anywhere.
    pub fn click(&self, main: &Path, at: PagePoint) -> Result<Option<Jump>, LookupError> {
        let kept =
            (self.documents.get(main)).ok_or_else(|| LookupError::NotCompiled(main.to_owned()))?;
        let position = PagedPosition {
            page: NonZeroUsize::MIN.saturating_add(at.page),
            point: Point::new(Abs::pt(at.x.0.into()), Abs::pt(at.y.0.into())),
        };
        Ok(
            match typst_ide::jump_from_click(&self.world, &kept.document, &position) {
                None => None,
                Some(typst_ide::Jump::File(id, byte)) => {
                    let source = self.source(id)?;
                    Some(Jump::Source {
                        file: self.source_file(id)?,
                        at: line_column(&source, id, byte)?,
                    })
                }
                Some(typst_ide::Jump::Url(url)) => Some(Jump::Url(url.as_str().to_owned())),
                Some(typst_ide::Jump::Position(position)) => {
                    Some(Jump::Page(page_point(&position)))
                }
            },
        )
    }

    fn source(&self, id: FileId) -> Result<Source, LookupError> {
        (self.world.source(id)).map_err(|error| LookupError::Unreadable { file: id, error })
    }

    fn source_file(&self, id: FileId) -> Result<SourceFile, LookupError> {
        match id.root() {
            VirtualRoot::Project => (id.vpath().realize(&self.world.root))
                .map(SourceFile::Project)
                .map_err(|error| LookupError::OutsideRoot { file: id, error }),
            VirtualRoot::Package(spec) if spec.namespace == EMBEDDED_NAMESPACE => {
                Ok(SourceFile::Embedded(id.get().vpath().get_with_slash()))
            }
            VirtualRoot::Package(spec) => Ok(SourceFile::Package(Box::new(PackageFile {
                package: spec.to_string(),
                path: id.vpath().get_with_slash().to_owned(),
            }))),
        }
    }

    pub fn typst_pdf(
        &mut self,
        root: &Path,
        main: &Path,
        source: &str,
    ) -> Result<Vec<u8>, DocumentError> {
        let (_, document) = self.compile_document(root, main, source)?;
        typst_pdf::pdf(&document.output, &typst_pdf::PdfOptions::default()).map_err(|errors| {
            DocumentError::Compile(Diagnostics {
                errors: self.diagnostics(&errors),
                warnings: self.diagnostics(&document.warnings),
            })
        })
    }

    /// Compiles `source` as the file `main` under `root`, reading its imports from disk.
    fn compile_document(
        &mut self,
        root: &Path,
        main: &Path,
        source: &str,
    ) -> Result<(FileId, Warned<PagedDocument>), DocumentError> {
        let id = RootedPath::new(
            VirtualRoot::Project,
            VirtualPath::virtualize(root, main).map_err(|_| DocumentError::OutsideRoot {
                root: root.to_owned(),
                main: main.to_owned(),
            })?,
        )
        .intern();
        {
            let world = &mut self.world;
            let disk = world
                .disk
                .get_mut()
                .unwrap_or_else(|error| error.into_inner());
            match world.root == root {
                true => {}
                false => {
                    world.root = root.to_owned();
                    disk.slots.clear();
                }
            }
            disk.generation += 1;
            let main_source = match disk.slots.remove(&id).and_then(|slot| slot.source) {
                Some(Ok(mut old)) => {
                    old.replace(source);
                    old
                }
                _ => Source::new(id, source.to_owned()),
            };
            disk.slots.insert(
                id,
                Slot {
                    generation: disk.generation,
                    bytes: Ok(Bytes::from_string(main_source.clone())),
                    source: Some(Ok(main_source)),
                },
            );
            world.main = id;
        }

        let Warned { output, warnings } = typst::compile::<PagedDocument>(&self.world);
        typst::comemo::evict(CACHE_AGE);
        match output {
            Ok(document) => Ok((
                id,
                Warned {
                    output: document,
                    warnings,
                },
            )),
            Err(errors) => Err(DocumentError::Compile(Diagnostics {
                errors: self.diagnostics(&errors),
                warnings: self.diagnostics(&warnings),
            })),
        }
    }

    fn diagnostics(&self, diagnostics: &[SourceDiagnostic]) -> Vec<Diagnostic> {
        diagnostics
            .iter()
            .map(|diagnostic| Diagnostic {
                origin: match (diagnostic.span.id(), self.world.range(diagnostic.span)) {
                    (Some(id), Some(range)) => {
                        let span = self.source(id).and_then(|source| {
                            Ok(Origin::Span {
                                file: self.source_file(id)?,
                                start: line_column(&source, id, range.start)?,
                                end: line_column(&source, id, range.end)?,
                            })
                        });
                        match span {
                            Ok(span) => span,
                            Err(error) => {
                                tracing::warn!(%error, "typst diagnostic without a usable place");
                                Origin::Detached
                            }
                        }
                    }
                    (None, _) | (_, None) => Origin::Detached,
                },
                message: diagnostic.message.to_string(),
                hints: diagnostic
                    .hints
                    .iter()
                    .map(|hint| hint.v.to_string())
                    .collect(),
            })
            .collect()
    }
}

fn line_column(source: &Source, file: FileId, byte: usize) -> Result<LineColumn, LookupError> {
    let (line, column) =
        (source.lines().byte_to_line_column(byte)).ok_or(LookupError::OutOfRange { file, byte })?;
    Ok(LineColumn { line, column })
}

/// Finds the inline equation's start tag; line layout places tags on the baseline.
fn equation_baseline(frame: &Frame, offset: Abs) -> Option<Abs> {
    frame.items().find_map(|(position, item)| match item {
        FrameItem::Tag(Tag::Start(element, _)) if element.is::<EquationElem>() => {
            Some(offset + position.y)
        }
        FrameItem::Group(group) => equation_baseline(&group.frame, offset + position.y),
        _ => None,
    })
}

fn page_svg(page: &Page) -> Svg {
    // typst-svg clamps pages to at least 1pt, so the reported size does too.
    let size = page.frame.size().max(Size::splat(Abs::pt(1.0)));
    Svg {
        bytes: Arc::from(typst_svg::svg(page, &typst_svg::SvgOptions::default()).into_bytes()),
        width: Pt(size.x.to_pt() as f32),
        height: Pt(size.y.to_pt() as f32),
        baseline: Pt(size.y.to_pt() as f32),
    }
}

/// Mermaid theme variables and the base16 slot each takes its colour from.
const MERMAID_THEME: [(&str, Base16); 43] = [
    ("background", Base16::Base00),
    ("primaryColor", Base16::Base01),
    ("primaryTextColor", Base16::Base05),
    ("primaryBorderColor", Base16::Base0D),
    ("secondaryColor", Base16::Base02),
    ("secondaryTextColor", Base16::Base05),
    ("tertiaryColor", Base16::Base01),
    ("tertiaryTextColor", Base16::Base05),
    ("lineColor", Base16::Base04),
    ("textColor", Base16::Base05),
    ("titleColor", Base16::Base06),
    ("mainBkg", Base16::Base01),
    ("nodeBorder", Base16::Base0D),
    ("nodeTextColor", Base16::Base05),
    ("clusterBkg", Base16::Base00),
    ("clusterBorder", Base16::Base03),
    ("edgeLabelBackground", Base16::Base01),
    ("noteBkgColor", Base16::Base02),
    ("noteBorderColor", Base16::Base0A),
    ("noteTextColor", Base16::Base05),
    ("actorBkg", Base16::Base01),
    ("actorBorder", Base16::Base0D),
    ("actorTextColor", Base16::Base05),
    ("actorLineColor", Base16::Base03),
    ("labelTextColor", Base16::Base05),
    ("loopTextColor", Base16::Base05),
    ("signalColor", Base16::Base05),
    ("signalTextColor", Base16::Base05),
    ("activationBkgColor", Base16::Base02),
    ("activationBorderColor", Base16::Base03),
    ("labelBoxBkgColor", Base16::Base01),
    ("labelBoxBorderColor", Base16::Base0D),
    ("classText", Base16::Base05),
    ("labelColor", Base16::Base05),
    ("attributeBackgroundColorOdd", Base16::Base01),
    ("attributeBackgroundColorEven", Base16::Base02),
    ("pieTitleTextColor", Base16::Base05),
    ("pieSectionTextColor", Base16::Base00),
    ("pieLegendTextColor", Base16::Base05),
    ("pieStrokeColor", Base16::Base03),
    ("pieOuterStrokeColor", Base16::Base03),
    ("errorBkgColor", Base16::Base08),
    ("errorTextColor", Base16::Base00),
];

/// Accents for mermaid's colour scales (git branches, pie slices, mindmap levels).
const MERMAID_ACCENTS: [Base16; 8] = [
    Base16::Base08,
    Base16::Base09,
    Base16::Base0A,
    Base16::Base0B,
    Base16::Base0C,
    Base16::Base0D,
    Base16::Base0E,
    Base16::Base0F,
];

/// Ink height of the `x` glyph per em in `family`, measured the same way for every font.
fn x_height(store: &FontStore, family: &str) -> Option<f64> {
    let index = store
        .book()
        .select(&family.to_lowercase(), FontVariant::default())?;
    let font = store.font(index)?.instantiate(
        FontVariant::default(),
        Abs::pt(1.0),
        &FontVariations::default(),
    );
    let ttf = font.ttf();
    let bounds = ttf.glyph_bounding_box(ttf.glyph_index('x')?)?;
    Some(f64::from(bounds.y_max) / font.units_per_em())
}

/// A compiled document and the main source it came from.
pub struct Kept {
    pub source: Source,
    pub document: PagedDocument,
}

fn page_point(position: &PagedPosition) -> PagePoint {
    PagePoint {
        page: position.page.get() - 1,
        x: Pt(position.point.x.to_pt() as f32),
        y: Pt(position.point.y.to_pt() as f32),
    }
}
