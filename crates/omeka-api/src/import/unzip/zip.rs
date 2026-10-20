//! The ZIP container, read through ranges (PKWARE APPNOTE 6.3.10): the
//! central directory, ZIP64 included, and each entry's bytes as a stream.
//!
//! Only what an extraction needs: names with their UTF-8 flag, sizes, the
//! method (stored or deflated), the CRC-32, encryption, and whether an entry
//! is a folder or a Unix symbolic link. Multi-disk archives are refused.

use std::io::{self, Read};
use std::ops::Range;

/// One entry of the central directory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ZipEntry {
    /// The name as the archive spells it.
    pub name: Vec<u8>,
    /// General purpose bit 11: the name is UTF-8.
    pub utf8: bool,
    /// The Info-ZIP Unicode Path extra field (0x7075), when present.
    pub unicode_name: Option<String>,
    pub method: u16,
    pub crc32: u32,
    pub compressed: u64,
    pub size: u64,
    pub local_offset: u64,
    pub encrypted: bool,
    pub is_link: bool,
}

impl ZipEntry {
    /// A folder entry: its name ends with a separator.
    #[must_use]
    pub fn is_folder(&self) -> bool {
        matches!(self.name.last(), Some(b'/' | b'\\'))
    }
}

/// Why the archive cannot be read.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ZipError {
    #[error("no end of central directory record")]
    NoDirectory,
    #[error("archives split over several disks are not read")]
    MultiDisk,
    #[error("the central directory is damaged: {0}")]
    Damaged(&'static str),
    #[error("{0}")]
    Io(String),
}

const EOCD: u32 = 0x0605_4b50;
const ZIP64_LOCATOR: u32 = 0x0706_4b50;
const ZIP64_EOCD: u32 = 0x0606_4b50;
const CENTRAL: u32 = 0x0201_4b50;
const LOCAL: u32 = 0x0403_4b50;
/// The end record is 22 bytes and its comment at most 65,535.
const TAIL: u64 = 22 + 65_535;

fn u16_at(bytes: &[u8], at: usize) -> Result<u16, ZipError> {
    bytes
        .get(at..at + 2)
        .map(|b| u16::from_le_bytes([b[0], b[1]]))
        .ok_or(ZipError::Damaged("a record is cut short"))
}

fn u32_at(bytes: &[u8], at: usize) -> Result<u32, ZipError> {
    bytes
        .get(at..at + 4)
        .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
        .ok_or(ZipError::Damaged("a record is cut short"))
}

fn u64_at(bytes: &[u8], at: usize) -> Result<u64, ZipError> {
    bytes
        .get(at..at + 8)
        .map(|b| u64::from_le_bytes([b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7]]))
        .ok_or(ZipError::Damaged("a record is cut short"))
}

/// Where the central directory is and how many entries it holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Directory {
    pub entries: u64,
    offset: u64,
    size: u64,
}

/// Find the central directory of an archive of `len` bytes.
///
/// # Errors
/// [`ZipError`] when there is no end record or it does not hold together.
pub fn directory(
    read: &dyn Fn(Range<u64>) -> io::Result<Vec<u8>>,
    len: u64,
) -> Result<Directory, ZipError> {
    let start = len.saturating_sub(TAIL);
    let tail = read(start..len).map_err(|error| ZipError::Io(error.to_string()))?;
    let at = (0..tail.len().saturating_sub(21))
        .rev()
        .find(|at| u32_at(&tail, *at).is_ok_and(|sig| sig == EOCD))
        .ok_or(ZipError::NoDirectory)?;
    let disk = u16_at(&tail, at + 4)?;
    let directory_disk = u16_at(&tail, at + 6)?;
    let mut entries = u64::from(u16_at(&tail, at + 10)?);
    let mut size = u64::from(u32_at(&tail, at + 12)?);
    let mut offset = u64::from(u32_at(&tail, at + 16)?);
    if disk != 0 || directory_disk != 0 {
        return Err(ZipError::MultiDisk);
    }
    if entries == 0xFFFF || size == 0xFFFF_FFFF || offset == 0xFFFF_FFFF {
        let locator_at = (start + at as u64)
            .checked_sub(20)
            .ok_or(ZipError::Damaged("no ZIP64 locator"))?;
        let locator =
            read(locator_at..locator_at + 20).map_err(|error| ZipError::Io(error.to_string()))?;
        if u32_at(&locator, 0)? != ZIP64_LOCATOR {
            return Err(ZipError::Damaged("no ZIP64 locator"));
        }
        let record_at = u64_at(&locator, 8)?;
        let record =
            read(record_at..record_at + 56).map_err(|error| ZipError::Io(error.to_string()))?;
        if u32_at(&record, 0)? != ZIP64_EOCD {
            return Err(ZipError::Damaged("no ZIP64 end record"));
        }
        entries = u64_at(&record, 32)?;
        size = u64_at(&record, 40)?;
        offset = u64_at(&record, 48)?;
    }
    if offset.saturating_add(size) > len {
        return Err(ZipError::Damaged(
            "the directory passes the end of the file",
        ));
    }
    Ok(Directory {
        entries,
        offset,
        size,
    })
}

/// Every entry of the directory, in archive order.
///
/// # Errors
/// [`ZipError`] when a header does not hold together.
pub fn entries(
    read: &dyn Fn(Range<u64>) -> io::Result<Vec<u8>>,
    directory: Directory,
) -> Result<Vec<ZipEntry>, ZipError> {
    let bytes = read(directory.offset..directory.offset + directory.size)
        .map_err(|error| ZipError::Io(error.to_string()))?;
    let mut found = Vec::new();
    let mut at = 0;
    while (found.len() as u64) < directory.entries {
        if u32_at(&bytes, at)? != CENTRAL {
            return Err(ZipError::Damaged("a central header has no signature"));
        }
        let made_by = u16_at(&bytes, at + 4)?;
        let flags = u16_at(&bytes, at + 8)?;
        let method = u16_at(&bytes, at + 10)?;
        let crc32 = u32_at(&bytes, at + 16)?;
        let mut compressed = u64::from(u32_at(&bytes, at + 20)?);
        let mut size = u64::from(u32_at(&bytes, at + 24)?);
        let name_len = usize::from(u16_at(&bytes, at + 28)?);
        let extra_len = usize::from(u16_at(&bytes, at + 30)?);
        let comment_len = usize::from(u16_at(&bytes, at + 32)?);
        let external = u32_at(&bytes, at + 38)?;
        let mut local_offset = u64::from(u32_at(&bytes, at + 42)?);
        let name_at = at + 46;
        let name = bytes
            .get(name_at..name_at + name_len)
            .ok_or(ZipError::Damaged("a name is cut short"))?
            .to_vec();
        let extra = bytes
            .get(name_at + name_len..name_at + name_len + extra_len)
            .ok_or(ZipError::Damaged("an extra field is cut short"))?;
        let mut unicode_name = None;
        let mut field = 0;
        while field + 4 <= extra.len() {
            let id = u16_at(extra, field)?;
            let len = usize::from(u16_at(extra, field + 2)?);
            let data = extra
                .get(field + 4..field + 4 + len)
                .ok_or(ZipError::Damaged("an extra field is cut short"))?;
            match id {
                0x0001 => {
                    let mut next = 0;
                    for value in [&mut size, &mut compressed, &mut local_offset] {
                        if *value == 0xFFFF_FFFF {
                            *value = u64_at(data, next)?;
                            next += 8;
                        }
                    }
                }
                0x7075 if data.first() == Some(&1) && data.len() > 5 => {
                    unicode_name = String::from_utf8(data[5..].to_vec()).ok();
                }
                _ => {}
            }
            field += 4 + len;
        }
        let unix = made_by >> 8 == 3;
        found.push(ZipEntry {
            name,
            utf8: flags & 0x0800 != 0,
            unicode_name,
            method,
            crc32,
            compressed,
            size,
            local_offset,
            encrypted: flags & 0x0001 != 0,
            is_link: unix && (external >> 16) & 0o170_000 == 0o120_000,
        });
        at = name_at + name_len + extra_len + comment_len;
    }
    Ok(found)
}

/// Where an entry's packed bytes start: past its local header.
///
/// # Errors
/// [`ZipError`] when the local header is missing.
pub fn data_start(
    read: &dyn Fn(Range<u64>) -> io::Result<Vec<u8>>,
    entry: &ZipEntry,
) -> Result<u64, ZipError> {
    let header = read(entry.local_offset..entry.local_offset + 30)
        .map_err(|error| ZipError::Io(error.to_string()))?;
    if u32_at(&header, 0)? != LOCAL {
        return Err(ZipError::Damaged("a local header has no signature"));
    }
    let name_len = u64::from(u16_at(&header, 26)?);
    let extra_len = u64::from(u16_at(&header, 28)?);
    Ok(entry.local_offset + 30 + name_len + extra_len)
}

/// An entry's unpacked bytes, from a reader of its packed bytes. `None` for
/// a method the reader lacks.
pub fn unpacked<'r>(entry: &ZipEntry, packed: Box<dyn Read + 'r>) -> Option<Box<dyn Read + 'r>> {
    match entry.method {
        0 => Some(packed),
        8 => Some(Box::new(flate2::read::DeflateDecoder::new(packed))),
        _ => None,
    }
}
