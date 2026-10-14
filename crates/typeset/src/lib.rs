//! Typesets LaTeX math, mermaid diagrams and typst documents into SVG, and typst into PDF.
//!
//! [`Typesetter`] is `Send` and its methods take `&mut self`. Share one instance behind a
//! `Mutex` across background jobs: fonts are scanned once and typst's comemo cache stays
//! warm, so jobs run one at a time but each is fast. A second instance rescans fonts.

mod world;

use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use model::color::{Base16, Palette, Rgb};
pub use model::typeset::{
    Diagnostic, Diagnostics, Display, DocumentError, LineColumn, Origin, PX_PER_PT, Pages,
    SourceFile, Svg,
};
use typst::diag::{SourceDiagnostic, Warned};
use typst::foundations::Bytes;
use typst::introspection::Tag;
use typst::layout::{Abs, Frame, FrameItem, Size};
use typst::math::EquationElem;
use typst::syntax::{RootedPath, Source, VirtualPath, VirtualRoot};
use typst::text::FontFlags;
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
    #[error("no font with a math table was found")]
    NoMathFont,
    #[error("unsupported LaTeX: {0}")]
    Latex(String),
    #[error("typst rejected the converted LaTeX: {}", .0.join("; "))]
    Math(Vec<String>),
    #[error("the source is not a mermaid diagram")]
    NotMermaid,
    #[error("mermaid: {0}")]
    Mermaid(#[from] merman::svg::HeadlessError),
    #[error("the mermaid SVG has no usable viewBox")]
    MermaidSize,
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
}

impl Typesetter {
    pub fn new(fonts: FontSource) -> Result<Typesetter, TypesetError> {
        let mut store = FontStore::new();
        store.extend(fonts::embedded());
        match fonts {
            FontSource::Embedded => {}
            FontSource::SystemAndEmbedded => store.extend(fonts::system()),
        }
        let book = store.book();
        match (0..)
            .map_while(|index| book.info(index))
            .any(|info| info.flags.contains(FontFlags::MATH))
        {
            true => {}
            false => return Err(TypesetError::NoMathFont),
        }
        tracing::debug!(families = book.families().count(), "typeset fonts loaded");

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
        })
    }

    pub fn math(
        &mut self,
        tex: &str,
        display: Display,
        color: Rgb,
        size_pt: f32,
    ) -> Result<Svg, TypesetError> {
        let converted = mitex::convert_math(tex, None).map_err(TypesetError::Latex)?;
        let block = match display {
            Display::Inline => "false",
            Display::Block => "true",
        };
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
        Ok(page_svg(
            page,
            match display {
                Display::Inline => {
                    equation_baseline(&page.frame, Abs::zero()).map(|y| y.to_pt() as f32)
                }
                Display::Block => None,
            },
        ))
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
        let root_end = svg.find('>').unwrap_or(0);
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
                width_pt: width / PX_PER_PT,
                height_pt: height / PX_PER_PT,
                baseline_pt: None,
            }),
            _ => Err(TypesetError::MermaidSize),
        }
    }

    pub fn typst_pages(
        &mut self,
        root: &Path,
        main: &Path,
        source: &str,
    ) -> Result<Pages, DocumentError> {
        let document = self.compile_document(root, main, source)?;
        let exported: Vec<(u128, Svg)> = document
            .output
            .pages()
            .iter()
            .enumerate()
            .map(|(index, page)| {
                let hash = typst::utils::hash128(page);
                match self.exported_pages.get(index) {
                    Some((cached, svg)) if *cached == hash => (hash, svg.clone()),
                    _ => (hash, page_svg(page, None)),
                }
            })
            .collect();
        let pages = exported.iter().map(|(_, svg)| svg.clone()).collect();
        self.exported_pages = exported;
        Ok(Pages {
            pages,
            warnings: self.diagnostics(&document.warnings),
        })
    }

    pub fn typst_pdf(
        &mut self,
        root: &Path,
        main: &Path,
        source: &str,
    ) -> Result<Vec<u8>, DocumentError> {
        let document = self.compile_document(root, main, source)?;
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
    ) -> Result<Warned<PagedDocument>, DocumentError> {
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
            Ok(document) => Ok(Warned {
                output: document,
                warnings,
            }),
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
                origin: diagnostic
                    .span
                    .id()
                    .zip(self.world.range(diagnostic.span))
                    .and_then(|(id, range)| {
                        let source = self.world.source(id).ok()?;
                        let lines = source.lines();
                        let (start_line, start_column) = lines.byte_to_line_column(range.start)?;
                        let (end_line, end_column) = lines.byte_to_line_column(range.end)?;
                        let file = match id.root() {
                            VirtualRoot::Project => {
                                SourceFile::Project(id.vpath().realize(&self.world.root).ok()?)
                            }
                            VirtualRoot::Package(spec) if spec.namespace == EMBEDDED_NAMESPACE => {
                                SourceFile::Embedded(id.get().vpath().get_with_slash())
                            }
                            VirtualRoot::Package(_) => return None,
                        };
                        Some(Origin::Span {
                            file,
                            start: LineColumn {
                                line: start_line,
                                column: start_column,
                            },
                            end: LineColumn {
                                line: end_line,
                                column: end_column,
                            },
                        })
                    })
                    .unwrap_or(Origin::Detached),
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

fn page_svg(page: &Page, baseline_pt: Option<f32>) -> Svg {
    // typst-svg clamps pages to at least 1pt, so the reported size does too.
    let size = page.frame.size().max(Size::splat(Abs::pt(1.0)));
    Svg {
        bytes: Arc::from(typst_svg::svg(page, &typst_svg::SvgOptions::default()).into_bytes()),
        width_pt: size.x.to_pt() as f32,
        height_pt: size.y.to_pt() as f32,
        baseline_pt,
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
