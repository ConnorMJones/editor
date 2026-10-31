pub mod annotation;
pub mod text;

use std::borrow::Cow;
use std::fs::File;
use std::hash::Hasher;
use std::io::{self, Write};
use std::ops::Range;
use std::os::unix::fs::FileExt;
use std::path::{Component, Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use reader::pdf::Document;
use rusqlite::{OptionalExtension, Row, Transaction, params};
use siphasher::sip128::{Hasher128, SipHasher13};
use uuid::Uuid;

pub use annotation::{
    Annotation, AnnotationId, Bookmark, Change, Comment, Highlight, Lost, Notebook, Quote,
};
pub use text::{Part, PartText, Place, TextSpan, Texts};

pub const FOLDER: &str = ".edt";
pub const FILE: &str = "library.db";
pub const VERSION: i32 = 1;
pub const MOST_HITS: usize = 200;

const CHUNK: usize = 64 * 1024;
const STRIDE: usize = 1024 * 1024;

const SCHEMA: &str = "
CREATE TABLE documents (
    id INTEGER PRIMARY KEY,
    path TEXT NOT NULL UNIQUE,
    hash BLOB NOT NULL,
    size INTEGER NOT NULL,
    parts INTEGER NOT NULL,
    position_part INTEGER NOT NULL DEFAULT 0,
    position_char INTEGER NOT NULL DEFAULT 0,
    markup INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX documents_hash ON documents(hash);
CREATE TABLE annotations (
    id BLOB PRIMARY KEY,
    document INTEGER NOT NULL REFERENCES documents(id) ON DELETE CASCADE,
    kind INTEGER NOT NULL,
    part INTEGER NOT NULL,
    start INTEGER,
    end INTEGER,
    exact TEXT,
    prefix TEXT,
    suffix TEXT,
    comment TEXT
);
CREATE INDEX annotations_document ON annotations(document);
CREATE VIRTUAL TABLE annotations_fts USING fts5(
    exact, comment, content='annotations', content_rowid='rowid'
);
CREATE TRIGGER annotations_added AFTER INSERT ON annotations BEGIN
    INSERT INTO annotations_fts(rowid, exact, comment) VALUES (new.rowid, new.exact, new.comment);
END;
CREATE TRIGGER annotations_removed AFTER DELETE ON annotations BEGIN
    INSERT INTO annotations_fts(annotations_fts, rowid, exact, comment)
        VALUES ('delete', old.rowid, old.exact, old.comment);
END;
CREATE TRIGGER annotations_changed AFTER UPDATE ON annotations BEGIN
    INSERT INTO annotations_fts(annotations_fts, rowid, exact, comment)
        VALUES ('delete', old.rowid, old.exact, old.comment);
    INSERT INTO annotations_fts(rowid, exact, comment) VALUES (new.rowid, new.exact, new.comment);
END;
";

const COLUMNS: &str = "id, kind, part, start, end, exact, prefix, suffix, comment";

const HIGHLIGHT: i64 = 0;
const BOOKMARK: i64 = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FastHash(pub [u8; 16]);

impl FastHash {
    pub fn of(bytes: &[u8]) -> FastHash {
        let Ok(hash) = FastHash::sampled(bytes.len(), |range| {
            Ok::<_, std::convert::Infallible>(Cow::Borrowed(&bytes[range]))
        });
        hash
    }

    pub fn read(file: &File) -> io::Result<FastHash> {
        let length = file.metadata()?.len() as usize;
        FastHash::sampled(length, |range| {
            let mut chunk = vec![0; range.len()];
            file.read_exact_at(&mut chunk, range.start as u64)?;
            Ok(Cow::Owned(chunk))
        })
    }

    fn sampled<'a, E>(
        length: usize,
        mut chunk: impl FnMut(Range<usize>) -> Result<Cow<'a, [u8]>, E>,
    ) -> Result<FastHash, E> {
        let mut hasher = SipHasher13::new();
        hasher.write(&(length as u64).to_le_bytes());
        hasher.write(&chunk(0..CHUNK.min(length))?);
        for start in (STRIDE..length).step_by(STRIDE) {
            hasher.write(&chunk(start..(start + CHUNK).min(length))?);
        }
        hasher.write(&chunk(length.saturating_sub(CHUNK)..length)?);
        Ok(FastHash(hasher.finish128().as_bytes()))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct DocumentId(i64); // private: writes may only name a document the library listed

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct LibPath(String); // private: built by `place` alone, so `path_of` never leaves the folder

impl LibPath {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone)]
pub struct Library {
    pub folder: PathBuf,
    // private: opened by `with` on first use, so a project that never records anything gets no `.edt`
    connection: Arc<Mutex<Option<rusqlite::Connection>>>,
}

impl std::fmt::Debug for Library {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Library({})", self.folder.display())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Markup {
    None,
    Unflattened,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Identity {
    Known,
    Moved { from: LibPath },
    Changed,
    New,
}

#[derive(Clone, Debug)]
pub struct Listed {
    pub id: DocumentId,
    pub identity: Identity,
    pub notebook: Notebook,
    pub position: Place,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SearchQuery(String); // private: FTS5 rejects empty or unquoted queries; `new` quotes every term

#[derive(Clone, Debug)]
pub struct SearchHit {
    pub path: LibPath,
    pub annotation: Annotation,
}

#[derive(Debug, thiserror::Error)]
pub enum LibraryError {
    #[error("library folder: {0}")]
    Folder(#[from] io::Error),
    #[error("library database: {0}")]
    Sql(#[from] rusqlite::Error),
    #[error("the library database is version {0}; this build reads version {VERSION}")]
    Version(i32),
    #[error("the library database has a malformed {0:?}")]
    Malformed(Malformed),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Malformed {
    Id,
    Kind,
    Part,
    Span,
    Quote,
}

impl SearchQuery {
    pub fn new(text: &str) -> Option<SearchQuery> {
        let terms: Vec<String> = (text.split_whitespace())
            .map(|term| format!("\"{}\"*", term.replace('"', "\"\"")))
            .collect();
        (!terms.is_empty()).then(|| SearchQuery(terms.join(" ")))
    }
}

impl Library {
    pub fn at(folder: PathBuf) -> Library {
        Library {
            folder,
            connection: Arc::new(Mutex::new(None)),
        }
    }

    pub fn existing(start: &Path) -> Option<&Path> {
        (start.ancestors()).find(|folder| folder.join(FOLDER).join(FILE).is_file())
    }

    pub fn place(&self, path: &Path) -> Option<LibPath> {
        let parts = (path.strip_prefix(&self.folder).ok()?.components())
            .map(|part| match part {
                Component::Normal(part) => part.to_str(),
                Component::Prefix(_)
                | Component::RootDir
                | Component::CurDir
                | Component::ParentDir => None,
            })
            .collect::<Option<Vec<_>>>()?;
        (!parts.is_empty()).then(|| LibPath(parts.join("/")))
    }

    pub fn path_of(&self, path: &LibPath) -> PathBuf {
        self.folder.join(&path.0)
    }

    fn with<T>(
        &self,
        run: impl FnOnce(&mut rusqlite::Connection) -> Result<T, LibraryError>,
    ) -> Result<T, LibraryError> {
        let mut guard = self
            .connection
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        let connection = match guard.take() {
            Some(connection) => connection,
            None => {
                let dir = self.folder.join(FOLDER);
                std::fs::create_dir_all(&dir)?;
                match (std::fs::File::options().write(true).create_new(true))
                    .open(dir.join(".gitignore"))
                {
                    Ok(mut ignore) => ignore.write_all(b"*\n")?,
                    Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
                    Err(error) => return Err(error.into()),
                }
                let connection = rusqlite::Connection::open(dir.join(FILE))?;
                connection.busy_timeout(Duration::from_secs(5))?;
                connection.pragma_update_and_check(None, "journal_mode", "WAL", |_| Ok(()))?;
                connection.pragma_update(None, "synchronous", "NORMAL")?;
                connection.pragma_update(None, "foreign_keys", true)?;
                let version: i32 =
                    connection.pragma_query_value(None, "user_version", |row| row.get(0))?;
                match version {
                    0 => {
                        connection.execute_batch(SCHEMA)?;
                        connection.pragma_update(None, "user_version", VERSION)?;
                    }
                    VERSION => {}
                    other => return Err(LibraryError::Version(other)),
                }
                connection
            }
        };
        run(guard.insert(connection))
    }

    pub fn identify(
        &self,
        path: &LibPath,
        hash: FastHash,
        size: u64,
        texts: &dyn Texts,
    ) -> Result<Listed, LibraryError> {
        let size = size as i64;
        let parts = texts.parts() as i64;
        let last = Part(texts.parts().saturating_sub(1));
        self.with(|connection| {
            let tx = connection.transaction()?;
            let at_path = (tx.query_row(
                "SELECT id, hash FROM documents WHERE path = ?1",
                [&path.0],
                |row| Ok((row.get::<_, i64>(0)?, row.get::<_, Vec<u8>>(1)?)),
            ))
            .optional()?;
            let (id, identity) = match at_path {
                Some((id, stored)) if stored == hash.0 => (id, Identity::Known),
                Some((id, _)) => (id, Identity::Changed),
                None => {
                    let mut same = tx.prepare("SELECT id, path FROM documents WHERE hash = ?1")?;
                    let moved = (same.query_map([&hash.0[..]], |row| {
                        Ok((row.get::<_, i64>(0)?, LibPath(row.get(1)?)))
                    })?)
                    .collect::<Result<Vec<_>, _>>()?
                    .into_iter()
                    .find(|(_, from)| !self.path_of(from).exists());
                    drop(same);
                    match moved {
                        Some((id, from)) => {
                            tx.execute(
                                "UPDATE documents SET path = ?1 WHERE id = ?2",
                                params![path.0, id],
                            )?;
                            (id, Identity::Moved { from })
                        }
                        None => {
                            tx.execute(
                                "INSERT INTO documents (path, hash, size, parts) VALUES (?1, ?2, ?3, ?4)",
                                params![path.0, &hash.0[..], size, parts],
                            )?;
                            (tx.last_insert_rowid(), Identity::New)
                        }
                    }
                }
            };
            let id = DocumentId(id);
            let stored = {
                let mut rows =
                    tx.prepare(&format!("SELECT {COLUMNS} FROM annotations WHERE document = ?1"))?;
                (rows.query_map([id.0], |row| Ok(annotation(row, 0)))?)
                    .map(|row| row?)
                    .collect::<Result<Vec<_>, _>>()?
            };
            let (part, char): (i64, i64) = tx.query_row(
                "SELECT position_part, position_char FROM documents WHERE id = ?1",
                [id.0],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )?;
            let position = match (usize::try_from(part), usize::try_from(char)) {
                (Ok(part), Ok(char)) => Place {
                    part: Part(part).min(last),
                    char,
                },
                (Err(_), _) | (_, Err(_)) => return Err(LibraryError::Malformed(Malformed::Part)),
            };
            let notebook = match identity {
                Identity::Changed => {
                    let reanchored: Vec<Annotation> = (stored.into_iter())
                        .map(|annotation| match annotation {
                            Annotation::Highlight(highlight) => highlight.reanchor(texts),
                            Annotation::Lost(lost) => lost.reanchor(texts),
                            Annotation::Bookmark(bookmark) => Annotation::Bookmark(Bookmark {
                                at: Place {
                                    part: bookmark.at.part.min(last),
                                    ..bookmark.at
                                },
                                ..bookmark
                            }),
                        })
                        .collect();
                    for annotation in &reanchored {
                        put(&tx, id, annotation)?;
                    }
                    tx.execute(
                        "UPDATE documents SET hash = ?1, size = ?2, parts = ?3 WHERE id = ?4",
                        params![&hash.0[..], size, parts, id.0],
                    )?;
                    reanchored.into_iter().collect()
                }
                Identity::Known | Identity::Moved { .. } | Identity::New => {
                    stored.into_iter().collect()
                }
            };
            tx.commit()?;
            Ok(Listed {
                id,
                identity,
                notebook,
                position,
            })
        })
    }

    pub fn write(&self, document: DocumentId, change: &Change) -> Result<(), LibraryError> {
        self.with(|connection| {
            let tx = connection.transaction()?;
            match change {
                Change::Put(annotation) => put(&tx, document, annotation)?,
                Change::Remove(id) => {
                    tx.execute(
                        "DELETE FROM annotations WHERE id = ?1",
                        [&id.0.as_bytes()[..]],
                    )?;
                }
            }
            Ok(tx.commit()?)
        })
    }

    pub fn rewritten(
        &self,
        document: DocumentId,
        written: &Document,
        markup: Markup,
    ) -> Result<(), LibraryError> {
        let hash = FastHash::of(&written.bytes);
        let markup = match markup {
            Markup::None => 0,
            Markup::Unflattened => 1,
        };
        self.with(|connection| {
            connection.execute(
                "UPDATE documents SET hash = ?1, size = ?2, parts = ?3, markup = ?4 WHERE id = ?5",
                params![
                    &hash.0[..],
                    written.bytes.len() as i64,
                    written.pages.sizes().len() as i64,
                    markup,
                    document.0
                ],
            )?;
            Ok(())
        })
    }

    pub fn marked_up(&self) -> Result<Vec<LibPath>, LibraryError> {
        match std::fs::metadata(self.folder.join(FOLDER).join(FILE)) {
            Ok(_) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(error) => return Err(error.into()),
        }
        self.with(|connection| {
            let mut rows = connection.prepare("SELECT path FROM documents WHERE markup = 1")?;
            let paths = (rows.query_map([], |row| row.get::<_, String>(0))?)
                .map(|path| path.map(LibPath))
                .collect::<Result<Vec<_>, _>>()?;
            Ok(paths)
        })
    }

    pub fn keep_place(&self, document: DocumentId, at: Place) -> Result<(), LibraryError> {
        self.with(|connection| {
            connection.execute(
                "UPDATE documents SET position_part = ?1, position_char = ?2 WHERE id = ?3",
                params![at.part.0 as i64, at.char as i64, document.0],
            )?;
            Ok(())
        })
    }

    pub fn search(&self, query: &SearchQuery) -> Result<Vec<SearchHit>, LibraryError> {
        self.with(|connection| {
            let mut found = connection.prepare(&format!(
                "SELECT d.path, {} FROM annotations_fts f
                 JOIN annotations a ON a.rowid = f.rowid
                 JOIN documents d ON d.id = a.document
                 WHERE annotations_fts MATCH ?1 ORDER BY rank LIMIT ?2",
                COLUMNS
                    .split(", ")
                    .map(|column| format!("a.{column}"))
                    .collect::<Vec<_>>()
                    .join(", ")
            ))?;
            let rows = found.query_map(params![query.0, MOST_HITS as i64], |row| {
                Ok((LibPath(row.get(0)?), annotation(row, 1)))
            })?;
            rows.map(|row| {
                let (path, annotation) = row?;
                Ok(SearchHit {
                    path,
                    annotation: annotation?,
                })
            })
            .collect()
        })
    }
}

fn annotation(row: &Row<'_>, from: usize) -> Result<Annotation, LibraryError> {
    let id = Uuid::from_slice(&row.get::<_, Vec<u8>>(from)?)
        .map(AnnotationId)
        .map_err(|_| LibraryError::Malformed(Malformed::Id))?;
    let part = Part(
        usize::try_from(row.get::<_, i64>(from + 2)?)
            .map_err(|_| LibraryError::Malformed(Malformed::Part))?,
    );
    let index = |at: usize| -> Result<Option<usize>, LibraryError> {
        (row.get::<_, Option<i64>>(from + at)?)
            .map(|value| {
                usize::try_from(value).map_err(|_| LibraryError::Malformed(Malformed::Span))
            })
            .transpose()
    };
    match row.get::<_, i64>(from + 1)? {
        HIGHLIGHT => {
            let quote = Quote::new(
                row.get::<_, Option<String>>(from + 5)?.unwrap_or_default(),
                row.get::<_, Option<String>>(from + 6)?.unwrap_or_default(),
                row.get::<_, Option<String>>(from + 7)?.unwrap_or_default(),
            )
            .ok_or(LibraryError::Malformed(Malformed::Quote))?;
            let comment = row
                .get::<_, Option<String>>(from + 8)?
                .and_then(Comment::new);
            match (index(3)?, index(4)?) {
                (Some(start), Some(end)) if start < end => Ok(Annotation::Highlight(Highlight {
                    id,
                    span: TextSpan {
                        part,
                        chars: start..end,
                    },
                    quote,
                    comment,
                })),
                (None, None) => Ok(Annotation::Lost(Lost {
                    id,
                    part,
                    quote,
                    comment,
                })),
                (Some(_), Some(_)) | (Some(_), None) | (None, Some(_)) => {
                    Err(LibraryError::Malformed(Malformed::Span))
                }
            }
        }
        BOOKMARK => Ok(Annotation::Bookmark(Bookmark {
            id,
            at: Place {
                part,
                char: index(3)?.unwrap_or(0),
            },
        })),
        _ => Err(LibraryError::Malformed(Malformed::Kind)),
    }
}

fn put(
    tx: &Transaction<'_>,
    document: DocumentId,
    annotation: &Annotation,
) -> Result<(), LibraryError> {
    let (kind, part, (start, end), quote, comment) = match annotation {
        Annotation::Highlight(Highlight {
            span,
            quote,
            comment,
            ..
        }) => (
            HIGHLIGHT,
            span.part,
            (Some(span.chars.start), Some(span.chars.end)),
            Some(quote),
            comment.as_ref(),
        ),
        Annotation::Lost(Lost {
            part,
            quote,
            comment,
            ..
        }) => (
            HIGHLIGHT,
            *part,
            (None, None),
            Some(quote),
            comment.as_ref(),
        ),
        Annotation::Bookmark(Bookmark { at, .. }) => {
            (BOOKMARK, at.part, (Some(at.char), None), None, None)
        }
    };
    tx.execute(
        &format!(
            "INSERT INTO annotations (document, {COLUMNS}) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
             ON CONFLICT(id) DO UPDATE SET kind = excluded.kind, part = excluded.part,
                start = excluded.start, end = excluded.end, exact = excluded.exact,
                prefix = excluded.prefix, suffix = excluded.suffix, comment = excluded.comment"
        ),
        params![
            document.0,
            &annotation.id().0.as_bytes()[..],
            kind,
            part.0 as i64,
            start.map(|start| start as i64),
            end.map(|end| end as i64),
            quote.map(Quote::exact),
            quote.map(|quote| quote.prefix.as_str()),
            quote.map(|quote| quote.suffix.as_str()),
            comment.map(Comment::as_str),
        ],
    )?;
    Ok(())
}
