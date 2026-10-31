use std::collections::HashMap;
use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom};
use std::sync::{Mutex, PoisonError};

use miniz_oxide::inflate::TINFLStatus;
use miniz_oxide::inflate::core::{DecompressorOxide, decompress, inflate_flags};

pub const END_SIGNATURE: u32 = 0x0605_4b50;
pub const CENTRAL_SIGNATURE: u32 = 0x0201_4b50;
pub const LOCAL_SIGNATURE: u32 = 0x0403_4b50;
pub const END_LENGTH: u64 = 22;
pub const LONGEST_COMMENT: u64 = 0xffff;
pub const LOCAL_LENGTH: usize = 30;

pub struct Archive {
    file: Mutex<File>,                 // private: reads seek it, so one at a time
    entries: HashMap<Box<str>, Entry>, // private: read once from the central directory
}

#[derive(Clone, Copy, Debug)]
pub struct Entry {
    pub method: Method,
    pub local: u64,
    pub packed: usize,
    pub size: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Method {
    Stored,
    Deflated,
}

#[derive(Debug, thiserror::Error)]
pub enum ZipError {
    #[error(transparent)]
    Io(#[from] io::Error),
    #[error("not a zip archive")]
    NotZip,
    #[error("the zip archive is cut short")]
    Truncated,
    #[error("{name} is packed with method {method}, which isn't read")]
    Method { name: String, method: u16 },
    #[error("there is no {0} in the archive")]
    Missing(String),
    #[error("{0} could not be unpacked")]
    Inflate(String),
}

fn u16_at(bytes: &[u8], at: usize) -> Result<u16, ZipError> {
    (bytes.get(at..at + 2))
        .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
        .ok_or(ZipError::Truncated)
}

fn u32_at(bytes: &[u8], at: usize) -> Result<u32, ZipError> {
    (bytes.get(at..at + 4))
        .map(|four| u32::from_le_bytes([four[0], four[1], four[2], four[3]]))
        .ok_or(ZipError::Truncated)
}

fn read_range(file: &mut File, from: u64, length: usize) -> Result<Vec<u8>, ZipError> {
    file.seek(SeekFrom::Start(from))?;
    let mut bytes = vec![0; length];
    file.read_exact(&mut bytes)
        .map_err(|error| match error.kind() {
            io::ErrorKind::UnexpectedEof => ZipError::Truncated,
            _ => ZipError::Io(error),
        })?;
    Ok(bytes)
}

impl Archive {
    pub fn open(mut file: File) -> Result<Archive, ZipError> {
        let length = file.metadata()?.len();
        let tail_from = length.saturating_sub(END_LENGTH + LONGEST_COMMENT);
        let tail = read_range(&mut file, tail_from, (length - tail_from) as usize)?;
        let end = (0..tail.len().saturating_sub(END_LENGTH as usize - 1))
            .rev()
            .find(|at| u32_at(&tail, *at).is_ok_and(|signature| signature == END_SIGNATURE))
            .ok_or(ZipError::NotZip)?;
        let count = usize::from(u16_at(&tail, end + 10)?);
        let directory_length = u32_at(&tail, end + 12)? as usize;
        let directory_from = u64::from(u32_at(&tail, end + 16)?);
        let directory = read_range(&mut file, directory_from, directory_length)?;
        let mut at = 0;
        let mut entries = HashMap::with_capacity(count);
        for _ in 0..count {
            match u32_at(&directory, at)? {
                CENTRAL_SIGNATURE => {}
                _ => return Err(ZipError::NotZip),
            }
            let method = u16_at(&directory, at + 10)?;
            let packed = u32_at(&directory, at + 20)? as usize;
            let size = u32_at(&directory, at + 24)? as usize;
            let name_length = usize::from(u16_at(&directory, at + 28)?);
            let extra = usize::from(u16_at(&directory, at + 30)?);
            let comment = usize::from(u16_at(&directory, at + 32)?);
            let local = u64::from(u32_at(&directory, at + 42)?);
            let name =
                (directory.get(at + 46..at + 46 + name_length)).ok_or(ZipError::Truncated)?;
            let name = String::from_utf8_lossy(name).into_owned();
            let method = match method {
                0 => Method::Stored,
                8 => Method::Deflated,
                method => return Err(ZipError::Method { name, method }),
            };
            entries.insert(
                name.into_boxed_str(),
                Entry {
                    method,
                    local,
                    packed,
                    size,
                },
            );
            at += 46 + name_length + extra + comment;
        }
        Ok(Archive {
            file: Mutex::new(file),
            entries,
        })
    }

    pub fn entry(&self, name: &str) -> Option<&Entry> {
        self.entries.get(name)
    }

    fn packed(&self, name: &str, most: usize) -> Result<(Entry, Vec<u8>), ZipError> {
        let entry = *(self.entries.get(name)).ok_or_else(|| ZipError::Missing(name.to_owned()))?;
        let mut file = self.file.lock().unwrap_or_else(PoisonError::into_inner);
        let header = read_range(&mut file, entry.local, LOCAL_LENGTH)?;
        match u32_at(&header, 0)? {
            LOCAL_SIGNATURE => {}
            _ => return Err(ZipError::NotZip),
        }
        let skip = usize::from(u16_at(&header, 26)?) + usize::from(u16_at(&header, 28)?);
        let packed = read_range(
            &mut file,
            entry.local + (LOCAL_LENGTH + skip) as u64,
            entry.packed.min(most),
        )?;
        Ok((entry, packed))
    }

    pub fn read(&self, name: &str) -> Result<Vec<u8>, ZipError> {
        let (entry, packed) = self.packed(name, usize::MAX)?;
        match entry.method {
            Method::Stored => Ok(packed),
            Method::Deflated => {
                miniz_oxide::inflate::decompress_to_vec_with_limit(&packed, entry.size)
                    .map_err(|_| ZipError::Inflate(name.to_owned()))
            }
        }
    }

    pub fn head(&self, name: &str, most: usize) -> Result<Vec<u8>, ZipError> {
        let (entry, packed) = self.packed(name, most)?;
        match entry.method {
            Method::Stored => Ok(packed),
            Method::Deflated => {
                let mut out = vec![0; most.min(entry.size)];
                let flags = inflate_flags::TINFL_FLAG_USING_NON_WRAPPING_OUTPUT_BUF
                    | inflate_flags::TINFL_FLAG_HAS_MORE_INPUT;
                let (status, _, written) =
                    decompress(&mut DecompressorOxide::new(), &packed, &mut out, 0, flags);
                match status {
                    TINFLStatus::Done
                    | TINFLStatus::NeedsMoreInput
                    | TINFLStatus::HasMoreOutput => {
                        out.truncate(written);
                        Ok(out)
                    }
                    TINFLStatus::FailedCannotMakeProgress
                    | TINFLStatus::BadParam
                    | TINFLStatus::Adler32Mismatch
                    | TINFLStatus::Failed => Err(ZipError::Inflate(name.to_owned())),
                }
            }
        }
    }
}
