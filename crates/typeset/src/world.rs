use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, PoisonError};
use std::time::{SystemTime, UNIX_EPOCH};

use typst::diag::{FileError, FileResult, PackageError};
use typst::foundations::{Bytes, Datetime, Duration};
use typst::syntax::package::{PackageSpec, PackageVersion};
use typst::syntax::{FileId, RootedPath, Source, VirtualPath, VirtualRoot};
use typst::text::{Font, FontBook};
use typst::utils::LazyHash;
use typst::{Library, World};
use typst_kit::fonts::FontStore;

/// Namespace of the private package root that holds the embedded files.
pub const EMBEDDED_NAMESPACE: &str = "typeset";

/// Typst files compiled into the binary, served from the embedded root.
pub const EMBEDDED: [(&str, &str); 3] = [
    ("/mitex/mod.typ", include_str!("mitex/mod.typ")),
    ("/mitex/prelude.typ", include_str!("mitex/prelude.typ")),
    (
        "/mitex/latex/standard.typ",
        include_str!("mitex/latex/standard.typ"),
    ),
];

pub struct TypesetWorld {
    pub library: LazyHash<Library>,
    pub fonts: FontStore,
    pub main: FileId,
    /// Generated main file for math, in the embedded root so it imports mitex relatively.
    pub math: Source,
    pub embedded: [Source; EMBEDDED.len()],
    /// Project root of the current typst document; the disk slots belong to it.
    pub root: PathBuf,
    pub disk: Mutex<Disk>,
}

/// Files read from `root`, revalidated once per generation (one compile) so unchanged
/// files keep their `Source` and comemo reuses its cached results.
#[derive(Default)]
pub struct Disk {
    pub generation: u64,
    pub slots: HashMap<FileId, Slot>,
}

pub struct Slot {
    pub generation: u64,
    pub bytes: FileResult<Bytes>,
    /// Parsed on first use; reparsed incrementally when the bytes change.
    pub source: Option<FileResult<Source>>,
}

pub fn embedded_id(path: &str) -> FileId {
    let root = VirtualRoot::Package(PackageSpec {
        namespace: EMBEDDED_NAMESPACE.into(),
        name: "embedded".into(),
        version: PackageVersion {
            major: 0,
            minor: 0,
            patch: 0,
        },
    });
    RootedPath::new(
        root,
        VirtualPath::new(path).expect("embedded paths are valid"),
    )
    .intern()
}

fn read(path: &Path) -> FileResult<Bytes> {
    std::fs::read(path)
        .map(Bytes::new)
        .map_err(|error| FileError::from_io(error, path))
}

impl TypesetWorld {
    fn embedded(&self, id: FileId) -> FileResult<&Source> {
        std::iter::once(&self.math)
            .chain(&self.embedded)
            .find(|source| source.id() == id)
            .ok_or_else(|| FileError::NotFound(id.vpath().get_with_slash().into()))
    }

    /// Runs `f` on the slot of a project file, reading it from disk once per generation.
    fn with_slot<T>(&self, id: FileId, f: impl FnOnce(&mut Slot) -> T) -> FileResult<T> {
        let path = id.vpath().realize(&self.root).map_err(FileError::Realize)?;
        // World methods take &self, so the slots sit behind a mutex; it is never contended.
        let mut disk = self.disk.lock().unwrap_or_else(PoisonError::into_inner);
        let generation = disk.generation;
        let slot = match disk.slots.entry(id) {
            Entry::Vacant(entry) => entry.insert(Slot {
                generation,
                bytes: read(&path),
                source: None,
            }),
            Entry::Occupied(entry) if entry.get().generation == generation => entry.into_mut(),
            Entry::Occupied(entry) => {
                let slot = entry.into_mut();
                let fresh = read(&path);
                slot.generation = generation;
                match (&slot.bytes, &fresh) {
                    (Ok(old), Ok(new)) if old.as_slice() == new.as_slice() => {}
                    (_, Ok(new)) => {
                        slot.source = match (slot.source.take(), std::str::from_utf8(new)) {
                            (Some(Ok(mut source)), Ok(text)) => {
                                source.replace(text);
                                Some(Ok(source))
                            }
                            _ => None,
                        };
                        slot.bytes = fresh;
                    }
                    (_, Err(_)) => {
                        slot.source = None;
                        slot.bytes = fresh;
                    }
                }
                slot
            }
        };
        Ok(f(slot))
    }
}

impl World for TypesetWorld {
    fn library(&self) -> &LazyHash<Library> {
        &self.library
    }

    fn book(&self) -> &LazyHash<FontBook> {
        self.fonts.book()
    }

    fn main(&self) -> FileId {
        self.main
    }

    fn source(&self, id: FileId) -> FileResult<Source> {
        match id.root() {
            VirtualRoot::Project => self
                .with_slot(id, |slot| {
                    slot.source
                        .get_or_insert_with(|| {
                            let bytes = slot.bytes.as_ref().map_err(Clone::clone)?;
                            let text =
                                std::str::from_utf8(bytes).map_err(|_| FileError::InvalidUtf8)?;
                            Ok(Source::new(id, text.to_owned()))
                        })
                        .clone()
                })
                .and_then(|source| source),
            VirtualRoot::Package(spec) if spec.namespace == EMBEDDED_NAMESPACE => {
                self.embedded(id).cloned()
            }
            VirtualRoot::Package(spec) => {
                Err(FileError::Package(PackageError::NotFound(spec.clone())))
            }
        }
    }

    fn file(&self, id: FileId) -> FileResult<Bytes> {
        match id.root() {
            VirtualRoot::Project => self
                .with_slot(id, |slot| slot.bytes.clone())
                .and_then(|bytes| bytes),
            VirtualRoot::Package(spec) if spec.namespace == EMBEDDED_NAMESPACE => self
                .embedded(id)
                .map(|source| Bytes::from_string(source.clone())),
            VirtualRoot::Package(spec) => {
                Err(FileError::Package(PackageError::NotFound(spec.clone())))
            }
        }
    }

    fn font(&self, index: usize) -> Option<Font> {
        self.fonts.font(index)
    }

    fn today(&self, offset: Option<Duration>) -> Option<Datetime> {
        // Without a timezone database the local date is approximated by UTC.
        let offset_seconds = offset.map_or(0, |offset| offset.seconds() as i64);
        let seconds = SystemTime::now().duration_since(UNIX_EPOCH).ok()?.as_secs() as i64;
        // Howard Hinnant's days-to-civil conversion.
        let days = (seconds + offset_seconds).div_euclid(86_400) + 719_468;
        let era = days.div_euclid(146_097);
        let day_of_era = days.rem_euclid(146_097);
        let year_of_era =
            (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
        let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
        let month_index = (5 * day_of_year + 2) / 153;
        let day = day_of_year - (153 * month_index + 2) / 5 + 1;
        let month = match month_index < 10 {
            true => month_index + 3,
            false => month_index - 9,
        };
        let year = year_of_era + era * 400 + i64::from(month <= 2);
        Datetime::from_ymd(i32::try_from(year).ok()?, month as u8, day as u8)
    }
}

impl typst_ide::IdeWorld for TypesetWorld {
    fn upcast(&self) -> &dyn World {
        self
    }
}
