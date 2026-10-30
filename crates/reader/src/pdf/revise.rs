use std::io::Write as _;

use hayro::hayro_syntax::object::{Dict, MaybeRef, ObjRef, Object};
use hayro::hayro_syntax::reader::{Reader, ReaderExt as _};
use pdf_writer::{Chunk, Ref};

use crate::pdf::Document;

pub const STARTXREF_SEARCH: usize = 4096;
pub const PARENT_DEPTH: usize = 64;
pub const OURS: &[u8] = b"EdtMark";
pub const BLOB: &[u8] = b"EdtInk";
pub const OUR_BASE: &[u8] = b"EdtBase";
pub const OUR_SIZE: &[u8] = b"EdtSize";

pub struct Revision<'a> {
    pub document: &'a Document,
    trailer: Trailer, // private: read from the document's last section, written after ours
    next: i32,        // private: past every object number the document and this revision use
    objects: Vec<Written>, // private: each object once, as its bytes start at its header
}

pub struct Update {
    pub keep: usize,
    pub bytes: Vec<u8>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ending {
    Replaceable,
    Permanent,
}

impl Update {
    pub fn onto(self, original: &[u8]) -> Vec<u8> {
        let mut bytes = original[..self.keep].to_vec();
        bytes.extend(self.bytes);
        bytes
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Owner {
    Ours,
    Theirs,
}

struct Trailer {
    keep: usize,
    size: i32,
    root: ObjRef,
    info: Option<ObjRef>,
    id: Option<Vec<u8>>,
    start: usize,
    section: Section,
}

enum Section {
    Table,
    Stream,
}

struct Written {
    number: i32,
    generation: i32,
    bytes: Vec<u8>,
}

#[derive(Debug, thiserror::Error)]
pub enum ReviseError {
    #[error("the PDF's last cross-reference section can't be read")]
    NoTrailer,
    #[error("the PDF is encrypted")]
    Encrypted,
    #[error("there is no page {0}")]
    NoPage(usize),
    #[error("a written object has no header: {0}")]
    Unwritten(i32),
}

impl Document {
    pub fn revise(&self) -> Result<Revision<'_>, ReviseError> {
        let bytes: &[u8] = &self.bytes;
        let tail = &bytes[bytes.len().saturating_sub(STARTXREF_SEARCH)..];
        let at = (tail.windows(9).rposition(|window| window == b"startxref"))
            .ok_or(ReviseError::NoTrailer)?;
        let mut reader = Reader::new(&tail[at + 9..]);
        reader.skip_white_spaces_and_comments();
        let start = reader
            .read_without_context::<i64>()
            .and_then(|start| usize::try_from(start).ok())
            .filter(|start| *start < bytes.len())
            .ok_or(ReviseError::NoTrailer)?;
        let section_bytes = &bytes[start..];
        let section = match section_bytes {
            [b'x', b'r', b'e', b'f', ..] => Section::Table,
            _ => Section::Stream,
        };
        let dict_at = match section {
            Section::Table => {
                (section_bytes.windows(7).position(|w| w == b"trailer")).and_then(|at| {
                    (section_bytes[at..].windows(2).position(|w| w == b"<<")).map(|from| at + from)
                })
            }
            Section::Stream => section_bytes.windows(2).position(|w| w == b"<<"),
        }
        .ok_or(ReviseError::NoTrailer)?;
        let dict = Reader::new(&section_bytes[dict_at..])
            .read_without_context::<Dict<'_>>()
            .ok_or(ReviseError::NoTrailer)?;
        match dict.get_raw::<Object<'_>>(b"Encrypt") {
            Some(_) => return Err(ReviseError::Encrypted),
            None => {}
        }
        let ours = (dict.get::<i64>(OUR_BASE))
            .zip(dict.get::<i32>(OUR_SIZE))
            .zip(dict.get::<i64>(b"Prev"))
            .and_then(|((base, size), previous)| {
                let base = usize::try_from(base).ok().filter(|base| *base <= start)?;
                let previous = usize::try_from(previous)
                    .ok()
                    .filter(|previous| *previous < base)?;
                Some((base, size, previous))
            });
        let (keep, size, start) = match ours {
            Some(replaced) => replaced,
            None => (
                bytes.len(),
                dict.get::<i32>(b"Size").ok_or(ReviseError::NoTrailer)?,
                start,
            ),
        };
        let trailer = Trailer {
            keep,
            size,
            root: dict.get_ref(b"Root").ok_or(ReviseError::NoTrailer)?,
            info: dict.get_ref(b"Info"),
            id: (dict.get_raw::<Object<'_>>(b"ID")).map(|id| raw(&id)),
            start,
            section,
        };
        Ok(Revision {
            document: self,
            trailer,
            next: size,
            objects: Vec::new(),
        })
    }
}

impl<'a> Revision<'a> {
    pub fn allocate(&mut self) -> Ref {
        let id = Ref::new(self.next);
        self.next += 1;
        id
    }

    pub fn root(&self) -> ObjRef {
        self.trailer.root
    }

    pub fn catalog(&self) -> Option<Dict<'a>> {
        let root = self.trailer.root;
        self.document.pdf.xref().get::<Dict<'_>>(root.into())
    }

    pub fn page(&self, page: usize) -> Result<(ObjRef, Dict<'a>), ReviseError> {
        let read = (self.document.pdf.pages().get(page)).ok_or(ReviseError::NoPage(page))?;
        let dict = read.raw().clone();
        let id = dict.obj_id().ok_or(ReviseError::NoPage(page))?;
        Ok((ObjRef::new(id.obj_number, id.gen_number), dict))
    }

    pub fn annotations(&self, page: &Dict<'a>) -> Vec<(Vec<u8>, Owner)> {
        let xref = self.document.pdf.xref();
        let list = match page.get_raw::<Object<'_>>(b"Annots") {
            Some(MaybeRef::Ref(id)) => {
                xref.get::<hayro::hayro_syntax::object::Array<'_>>(id.into())
            }
            Some(MaybeRef::NotRef(Object::Array(list))) => Some(list),
            Some(MaybeRef::NotRef(_)) | None => None,
        };
        (list.iter())
            .flat_map(|list| list.raw_iter())
            .map(|item| {
                let dict = match &item {
                    MaybeRef::Ref(id) => xref.get::<Dict<'_>>((*id).into()),
                    MaybeRef::NotRef(Object::Dict(dict)) => Some(dict.clone()),
                    MaybeRef::NotRef(_) => None,
                };
                let owner = match dict.and_then(|dict| dict.get_raw::<Object<'_>>(OURS)) {
                    Some(_) => Owner::Ours,
                    None => Owner::Theirs,
                };
                (raw(&item), owner)
            })
            .collect()
    }

    pub fn contents(&self, page: &Dict<'a>) -> Vec<Vec<u8>> {
        match page.get_raw::<Object<'_>>(b"Contents") {
            Some(MaybeRef::NotRef(Object::Array(list))) => {
                list.raw_iter().map(|item| raw(&item)).collect()
            }
            Some(other) => vec![raw(&other)],
            None => Vec::new(),
        }
    }

    pub fn resources(&self, page: &Dict<'a>) -> Option<Dict<'a>> {
        let mut node = page.clone();
        for _ in 0..PARENT_DEPTH {
            match node.get::<Dict<'_>>(b"Resources") {
                Some(found) => return Some(found),
                None => node = node.get::<Dict<'_>>(b"Parent")?,
            }
        }
        None
    }

    pub fn chunk(&mut self, chunk: &Chunk) -> Result<(), ReviseError> {
        let bytes = chunk.as_bytes();
        let ids: Vec<Ref> = chunk.refs().collect();
        let mut starts = Vec::with_capacity(ids.len());
        let mut from = 0;
        for id in &ids {
            let header = format!("{} 0 obj", id.get());
            let at = (bytes[from..].windows(header.len()))
                .position(|window| window == header.as_bytes())
                .ok_or(ReviseError::Unwritten(id.get()))?;
            starts.push(from + at);
            from += at + header.len();
        }
        for (index, id) in ids.iter().enumerate() {
            let end = starts.get(index + 1).copied().unwrap_or(bytes.len());
            self.objects.push(Written {
                number: id.get(),
                generation: 0,
                bytes: bytes[starts[index]..end].to_vec(),
            });
        }
        Ok(())
    }

    pub fn replace_dict(
        &mut self,
        id: ObjRef,
        dict: &Dict<'_>,
        dropped: &[&[u8]],
        added: &[(&[u8], Vec<u8>)],
    ) {
        let mut bytes = Vec::new();
        let _ = write!(bytes, "{} {} obj\n", id.obj_number, id.gen_number);
        bytes.extend(inline_dict(dict, dropped, added));
        bytes.extend(b"\nendobj\n");
        self.objects.push(Written {
            number: id.obj_number,
            generation: id.gen_number,
            bytes,
        });
    }

    pub fn finish(self, ending: Ending) -> Update {
        let Revision {
            trailer,
            next,
            objects,
            ..
        } = self;
        let mut out = vec![b'\n'];
        let base = trailer.keep;
        let mut offsets: Vec<(i32, i32, usize)> = Vec::with_capacity(objects.len() + 1);
        for object in &objects {
            offsets.push((object.number, object.generation, base + out.len()));
            out.extend(&object.bytes);
            match object.bytes.last() {
                Some(b'\n') => {}
                Some(_) | None => out.push(b'\n'),
            }
        }
        let size = next.max(trailer.size);
        let mut tail = Vec::new();
        let _ = write!(
            tail,
            "/Root {} {} R",
            trailer.root.obj_number, trailer.root.gen_number
        );
        match trailer.info {
            Some(info) => {
                let _ = write!(tail, " /Info {} {} R", info.obj_number, info.gen_number);
            }
            None => {}
        }
        match &trailer.id {
            Some(id) => {
                tail.extend(b" /ID ");
                tail.extend(id);
            }
            None => {}
        }
        let _ = write!(tail, " /Prev {}", trailer.start);
        match ending {
            Ending::Replaceable => {
                let _ = write!(tail, " /EdtBase {base} /EdtSize {}", trailer.size);
            }
            Ending::Permanent => {}
        }
        let start = base + out.len();
        match trailer.section {
            Section::Table => {
                offsets.sort_by_key(|(number, ..)| *number);
                out.extend(b"xref\n");
                for run in offsets.chunk_by(|a, b| b.0 == a.0 + 1) {
                    let _ = writeln!(out, "{} {}", run[0].0, run.len());
                    for (_, generation, offset) in run {
                        let _ = write!(out, "{offset:010} {generation:05} n\r\n");
                    }
                }
                let _ = write!(out, "trailer\n<< /Size {size} ");
                out.extend(&tail);
                let _ = write!(out, " >>\nstartxref\n{start}\n%%EOF\n");
            }
            Section::Stream => {
                let own = size;
                offsets.push((own, 0, start));
                offsets.sort_by_key(|(number, ..)| *number);
                let mut index = Vec::new();
                let mut rows = Vec::new();
                for run in offsets.chunk_by(|a, b| b.0 == a.0 + 1) {
                    let _ = write!(index, "{} {} ", run[0].0, run.len());
                    for (_, generation, offset) in run {
                        rows.push(1);
                        rows.extend((*offset as u32).to_be_bytes());
                        rows.extend((*generation as u16).to_be_bytes());
                    }
                }
                let _ = write!(
                    out,
                    "{own} 0 obj\n<< /Type /XRef /Size {} /W [1 4 2] /Index [",
                    own + 1
                );
                out.extend(index.trim_ascii_end());
                out.extend(b"] ");
                out.extend(&tail);
                let _ = write!(out, " /Length {} >>\nstream\n", rows.len());
                out.extend(&rows);
                let _ = write!(out, "\nendstream\nendobj\nstartxref\n{start}\n%%EOF\n");
            }
        }
        Update {
            keep: base,
            bytes: out,
        }
    }
}

pub fn inline_dict(dict: &Dict<'_>, dropped: &[&[u8]], added: &[(&[u8], Vec<u8>)]) -> Vec<u8> {
    let mut bytes = b"<<".to_vec();
    let kept = dict.entries().filter(|(key, _)| {
        !dropped.contains(&key.as_ref()) && !added.iter().any(|(name, _)| *name == key.as_ref())
    });
    for (key, value) in kept {
        bytes.extend(name(key.as_ref()));
        bytes.push(b' ');
        bytes.extend(raw(&value));
    }
    for (key, value) in added {
        bytes.extend(name(key));
        bytes.push(b' ');
        bytes.extend(value);
    }
    bytes.extend(b">>");
    bytes
}

fn name(key: &[u8]) -> Vec<u8> {
    let mut out = vec![b'/'];
    for &byte in key {
        match byte {
            b'!'..=b'~' if !b"#()<>[]{}/%".contains(&byte) => out.push(byte),
            _ => {
                let _ = write!(out, "#{byte:02X}");
            }
        }
    }
    out
}

fn raw(value: &MaybeRef<Object<'_>>) -> Vec<u8> {
    let mut out = Vec::new();
    match value {
        MaybeRef::Ref(id) => {
            let _ = write!(out, "{} {} R", id.obj_number, id.gen_number);
        }
        MaybeRef::NotRef(object) => match object {
            Object::Null(_) | Object::Stream(_) => out.extend(b"null"),
            Object::Boolean(value) => {
                let _ = write!(out, "{value}");
            }
            Object::Number(number) => match number.as_f64() {
                whole if whole.fract() == 0.0 => {
                    let _ = write!(out, "{}", number.as_i64());
                }
                fraction => {
                    let _ = write!(out, "{fraction}");
                }
            },
            Object::String(string) => {
                out.push(b'<');
                for byte in string.as_bytes() {
                    let _ = write!(out, "{byte:02X}");
                }
                out.push(b'>');
            }
            Object::Name(key) => out.extend(name(key.as_ref())),
            Object::Dict(dict) => out.extend(dict.data()),
            Object::Array(array) => {
                out.push(b'[');
                out.extend(array.data());
                out.push(b']');
            }
        },
    }
    out
}
