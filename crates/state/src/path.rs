use std::borrow::Borrow;
use std::fmt;
use std::path::{Component, Path, PathBuf};

/// Root-relative, `/`-separated UTF-8 path with only normal components; empty for the root.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct RelPath(pub(crate) String);

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum RelPathError {
    #[error("{0:?} is outside the root")]
    Outside(PathBuf),
    #[error("{0:?} is not valid UTF-8")]
    NotUtf8(PathBuf),
}

impl RelPath {
    pub const ROOT: RelPath = RelPath(String::new());

    pub fn new(root: &Path, path: &Path) -> Result<RelPath, RelPathError> {
        let relative = path
            .strip_prefix(root)
            .map_err(|_| RelPathError::Outside(path.to_owned()))?;
        let mut out = String::with_capacity(relative.as_os_str().len());
        for component in relative.components() {
            match component {
                Component::Normal(name) => {
                    let name = name
                        .to_str()
                        .ok_or_else(|| RelPathError::NotUtf8(path.to_owned()))?;
                    match out.is_empty() {
                        true => {}
                        false => out.push('/'),
                    }
                    out.push_str(name);
                }
                Component::CurDir => {}
                Component::Prefix(_) | Component::RootDir | Component::ParentDir => {
                    return Err(RelPathError::Outside(path.to_owned()));
                }
            }
        }
        Ok(RelPath(out))
    }

    /// Parses an already root-relative `/`-separated path, as git reports them.
    pub fn parse(path: &str) -> Result<RelPath, RelPathError> {
        match (
            path.is_empty(),
            path.split('/').all(|part| !matches!(part, "" | "." | "..")),
        ) {
            (true, _) => Ok(RelPath::ROOT),
            (false, true) => Ok(RelPath(path.to_owned())),
            (false, false) => Err(RelPathError::Outside(PathBuf::from(path))),
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn is_root(&self) -> bool {
        self.0.is_empty()
    }

    /// `name` must be a single normal component.
    pub fn join(&self, name: &str) -> RelPath {
        match self.0.is_empty() {
            true => RelPath(name.to_owned()),
            false => RelPath(format!("{}/{name}", self.0)),
        }
    }

    /// Last component; empty for the root.
    pub fn name(&self) -> &str {
        match self.0.rsplit_once('/') {
            Some((_, name)) => name,
            None => &self.0,
        }
    }

    /// `None` for the root.
    pub fn parent(&self) -> Option<&str> {
        match (self.0.is_empty(), self.0.rsplit_once('/')) {
            (true, _) => None,
            (false, Some((parent, _))) => Some(parent),
            (false, None) => Some(""),
        }
    }

    /// Proper ancestors from the parent up to and including the root.
    pub fn ancestors(&self) -> impl Iterator<Item = &str> {
        let mut rest = Some(self.0.as_str()).filter(|s| !s.is_empty());
        std::iter::from_fn(move || {
            let current = rest?;
            let parent = match current.rsplit_once('/') {
                Some((parent, _)) => parent,
                None => "",
            };
            rest = Some(parent).filter(|p| !p.is_empty());
            Some(parent)
        })
    }

    /// True when `self` equals `dir` or lies beneath it.
    pub fn is_within(&self, dir: &str) -> bool {
        match dir.is_empty() {
            true => true,
            false => {
                self.0 == dir
                    || (self.0.starts_with(dir) && self.0.as_bytes().get(dir.len()) == Some(&b'/'))
            }
        }
    }

    pub fn to_path(&self, root: &Path) -> PathBuf {
        match self.0.is_empty() {
            true => root.to_owned(),
            false => root.join(&self.0),
        }
    }

    /// Replaces the `from` prefix with `to`; `self` must be within `from`.
    pub fn rebase(&self, from: &RelPath, to: &RelPath) -> RelPath {
        let rest = match from.0.is_empty() {
            true => self.0.as_str(),
            false => self.0[from.0.len()..].trim_start_matches('/'),
        };
        match (rest.is_empty(), to.0.is_empty()) {
            (true, _) => to.clone(),
            (false, true) => RelPath(rest.to_owned()),
            (false, false) => RelPath(format!("{}/{rest}", to.0)),
        }
    }
}

impl Borrow<str> for RelPath {
    fn borrow(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for RelPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0.is_empty() {
            true => f.write_str("."),
            false => f.write_str(&self.0),
        }
    }
}
