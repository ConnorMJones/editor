//! Typesetting output shared by the typesetter, the state that requests it and the host.

use std::path::PathBuf;
use std::sync::Arc;

use crate::units::Pt;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Display {
    Inline,
    Block,
}

#[derive(Clone)]
pub struct Svg {
    pub bytes: Arc<[u8]>,
    pub width: Pt,
    pub height: Pt,
    /// Distance from the top to the line the drawing sits on: the text baseline of inline math,
    /// the bottom edge of anything else.
    pub baseline: Pt,
}

impl Svg {
    pub fn picture(&self) -> crate::picture::Picture {
        crate::picture::Picture {
            source: crate::picture::Source::Bytes(self.bytes.clone()),
            encoding: crate::picture::Encoding::Svg,
        }
    }
}

impl std::fmt::Debug for Svg {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Svg")
            .field("bytes", &self.bytes.len())
            .field("width", &self.width)
            .field("height", &self.height)
            .field("baseline", &self.baseline)
            .finish()
    }
}

#[derive(Clone, Debug)]
pub struct Pages {
    pub pages: Vec<Svg>,
    pub warnings: Vec<Diagnostic>,
}

/// A compile of a typst document and the project files it read, whether or not it succeeded.
#[derive(Clone, Debug)]
pub struct Compilation {
    pub result: Result<Pages, DocumentError>,
    pub deps: Vec<PathBuf>,
}

/// A point on a typst page, from its top left.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PagePoint {
    /// Zero-based.
    pub page: usize,
    pub x: Pt,
    pub y: Pt,
}

/// Where a click on a typst page leads.
#[derive(Clone, Debug, PartialEq)]
pub enum Jump {
    Source { file: SourceFile, at: LineColumn },
    Url(String),
    Page(PagePoint),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Diagnostic {
    pub origin: Origin,
    pub message: String,
    pub hints: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Origin {
    Detached,
    Span {
        file: SourceFile,
        start: LineColumn,
        end: LineColumn,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SourceFile {
    /// A file under the document root, joined onto the root.
    Project(PathBuf),
    /// A file compiled into the typesetter, such as the mitex definitions.
    Embedded(&'static str),
    Package(Box<PackageFile>),
}

/// A file of a typst package, such as `@preview/cetz:0.3.0` and `/src/draw.typ`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PackageFile {
    pub package: String,
    pub path: String,
}

/// Zero-based line, and column counted in characters.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LineColumn {
    pub line: usize,
    pub column: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Diagnostics {
    pub errors: Vec<Diagnostic>,
    pub warnings: Vec<Diagnostic>,
}

#[derive(Clone, Debug, thiserror::Error)]
pub enum DocumentError {
    #[error("{main} is not inside the root {root}")]
    OutsideRoot { root: PathBuf, main: PathBuf },
    #[error("typst reported {} errors", .0.errors.len())]
    Compile(Diagnostics),
}

impl std::fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.origin {
            Origin::Detached => {}
            Origin::Span { file, start, .. } => {
                write!(f, "{file}:{}:{}: ", start.line + 1, start.column + 1)?
            }
        }
        f.write_str(&self.message)
    }
}

impl std::fmt::Display for SourceFile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SourceFile::Project(path) => write!(f, "{}", path.display()),
            SourceFile::Embedded(path) => f.write_str(path),
            SourceFile::Package(file) => write!(f, "{}{}", file.package, file.path),
        }
    }
}
