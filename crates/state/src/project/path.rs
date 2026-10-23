use std::borrow::Borrow;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::path::{Component, Path, PathBuf};

/// `/`-separated UTF-8 under the root with only normal components; never the root itself.
#[derive(
    Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
#[serde(try_from = "String", into = "String")]
pub struct RelPath(String); // private: checked by `new` and `parse`

/// A folder: the root, or one under it. Hashes and borrows as its path, empty for the root.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Folder {
    Root,
    Sub(RelPath),
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum RelPathError {
    #[error("{0:?} is outside the root")]
    Outside(PathBuf),
    #[error("{0:?} is not valid UTF-8")]
    NotUtf8(PathBuf),
    #[error("the root itself is not a file or folder under it")]
    Root,
}

impl RelPath {
    pub fn new(root: &Path, path: &Path) -> Result<RelPath, RelPathError> {
        let relative = path
            .strip_prefix(root)
            .map_err(|_| RelPathError::Outside(path.to_owned()))?;
        let mut parts = Vec::new();
        for component in relative.components() {
            match component {
                Component::Normal(name) => parts.push(
                    name.to_str()
                        .ok_or_else(|| RelPathError::NotUtf8(path.to_owned()))?,
                ),
                Component::CurDir => {}
                Component::Prefix(_) | Component::RootDir | Component::ParentDir => {
                    return Err(RelPathError::Outside(path.to_owned()));
                }
            }
        }
        match parts.is_empty() {
            true => Err(RelPathError::Root),
            false => Ok(RelPath(parts.join("/"))),
        }
    }

    /// Parses an already root-relative `/`-separated path, as git reports them.
    pub fn parse(path: &str) -> Result<RelPath, RelPathError> {
        match (
            path.is_empty(),
            path.split('/').all(|part| !matches!(part, "" | "." | "..")),
        ) {
            (true, _) => Err(RelPathError::Root),
            (false, true) => Ok(RelPath(path.to_owned())),
            (false, false) => Err(RelPathError::Outside(PathBuf::from(path))),
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// `name` must be a single normal component.
    pub fn join(&self, name: &str) -> RelPath {
        RelPath(format!("{}/{name}", self.0))
    }

    pub fn kind(&self) -> FileKind {
        FileKind::of(Path::new(&self.0))
    }

    pub fn name(&self) -> &str {
        match self.0.rsplit_once('/') {
            Some((_, name)) => name,
            None => &self.0,
        }
    }

    pub fn folder(&self) -> Folder {
        Folder::of(self.folder_path())
    }

    /// The path of the folder this sits in, empty for the root.
    pub fn folder_path(&self) -> &str {
        match self.0.rsplit_once('/') {
            Some((folder, _)) => folder,
            None => "",
        }
    }

    /// Paths of the folders above, from the nearest up to the root's empty path.
    pub fn ancestors(&self) -> impl Iterator<Item = &str> {
        let mut rest = Some(self.0.as_str());
        std::iter::from_fn(move || {
            let current = rest?;
            let (folder, next) = match current.rsplit_once('/') {
                Some((folder, _)) => (folder, Some(folder)),
                None => ("", None),
            };
            rest = next;
            Some(folder)
        })
    }

    /// True when `self` is `dir` or lies beneath it.
    pub fn is_within(&self, dir: &RelPath) -> bool {
        self.0 == dir.0
            || (self.0.starts_with(&dir.0) && self.0.as_bytes().get(dir.0.len()) == Some(&b'/'))
    }

    pub fn to_path(&self, root: &Path) -> PathBuf {
        root.join(&self.0)
    }

    /// `self` with its `from` prefix replaced by `to`, when it is within `from`.
    pub fn rebased(&self, from: &RelPath, to: &RelPath) -> Option<RelPath> {
        match self.0.strip_prefix(&from.0)? {
            "" => Some(to.clone()),
            rest => rest.strip_prefix('/').map(|rest| to.join(rest)),
        }
    }
}

impl Folder {
    /// A folder path as `ancestors` yields it: empty for the root.
    pub fn of(path: &str) -> Folder {
        match path {
            "" => Folder::Root,
            path => Folder::Sub(RelPath(path.to_owned())),
        }
    }

    /// `/`-separated, empty for the root.
    pub fn as_str(&self) -> &str {
        match self {
            Folder::Root => "",
            Folder::Sub(path) => path.as_str(),
        }
    }

    /// `name` must be a single normal component.
    pub fn join(&self, name: &str) -> RelPath {
        match self {
            Folder::Root => RelPath(name.to_owned()),
            Folder::Sub(path) => path.join(name),
        }
    }

    pub fn to_path(&self, root: &Path) -> PathBuf {
        match self {
            Folder::Root => root.to_owned(),
            Folder::Sub(path) => path.to_path(root),
        }
    }

    /// Where this folder is once `from` has moved to `to`.
    pub fn moved(self, from: &RelPath, to: &RelPath) -> Folder {
        match self {
            Folder::Sub(dir) => Folder::Sub(dir.rebased(from, to).unwrap_or(dir)),
            Folder::Root => Folder::Root,
        }
    }

    /// True for everything under the root, or `path` within this folder.
    pub fn contains(&self, path: &RelPath) -> bool {
        match self {
            Folder::Root => true,
            Folder::Sub(folder) => path.is_within(folder),
        }
    }
}

impl Hash for Folder {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.as_str().hash(state)
    }
}

impl TryFrom<String> for RelPath {
    type Error = RelPathError;

    fn try_from(path: String) -> Result<RelPath, RelPathError> {
        RelPath::parse(&path)
    }
}

impl From<RelPath> for String {
    fn from(path: RelPath) -> String {
        path.0
    }
}

impl Borrow<str> for RelPath {
    fn borrow(&self) -> &str {
        &self.0
    }
}

impl Borrow<str> for Folder {
    fn borrow(&self) -> &str {
        self.as_str()
    }
}

impl fmt::Display for RelPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl fmt::Display for Folder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Folder::Root => f.write_str("."),
            Folder::Sub(path) => f.write_str(path.as_str()),
        }
    }
}

/// What a file holds, by its extension: the one place a file's kind is decided.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileKind {
    Text(TextKind),
    Paged,
    Board,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextKind {
    Code,
    Markdown,
    Typst,
}

impl FileKind {
    pub fn of(path: &Path) -> FileKind {
        match path.extension().and_then(|extension| extension.to_str()) {
            Some("md" | "markdown") => FileKind::Text(TextKind::Markdown),
            Some("typ") => FileKind::Text(TextKind::Typst),
            Some("pdt") => FileKind::Paged,
            Some("bdt") => FileKind::Board,
            Some(_) | None => FileKind::Text(TextKind::Code),
        }
    }
}
