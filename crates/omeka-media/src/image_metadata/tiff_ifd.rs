//! A TIFF's directories read as raw entries, by seeking.
//!
//! The `tiff` decoder refuses an ASCII field that is not UTF-8, and an old
//! scanner's 1252 `Artist` is exactly such a field, so the text tags, the
//! IPTC block and the XMP packet are read here as the bytes the file holds.
//! Classic TIFF and BigTIFF both read; every value is bounded.

use std::io::{Read, Seek, SeekFrom};

/// The largest value read: an XMP packet or an IPTC block fits.
const MAX_VALUE: u64 = 1024 * 1024;
/// More entries than this is not a directory a writer made.
const MAX_ENTRIES: u64 = 4096;

pub(super) struct Tiff<R> {
    reader: R,
    big_endian: bool,
    big: bool,
    first: u64,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct Entry {
    pub tag: u16,
    kind: u16,
    count: u64,
    /// The value field as written: the value itself when it fits, else
    /// the offset of the value.
    field: [u8; 8],
}

impl<R: Read + Seek> Tiff<R> {
    pub(super) fn open(mut reader: R) -> Option<Self> {
        reader.seek(SeekFrom::Start(0)).ok()?;
        let mut head = [0_u8; 16];
        reader.read_exact(&mut head[..8]).ok()?;
        let big_endian = match &head[..2] {
            b"II" => false,
            b"MM" => true,
            _ => return None,
        };
        let mut tiff = Self {
            reader,
            big_endian,
            big: false,
            first: 0,
        };
        match tiff.u16_at(&head[2..4]) {
            42 => tiff.first = u64::from(tiff.u32_at(&head[4..8])),
            43 => {
                tiff.big = true;
                tiff.reader.read_exact(&mut head[8..16]).ok()?;
                tiff.first = tiff.u64_at(&head[8..16]);
            }
            _ => return None,
        }
        Some(tiff)
    }

    /// The first directory, the image's own.
    pub(super) fn first(&mut self) -> Option<Vec<Entry>> {
        self.directory(self.first)
    }

    /// The directory a pointer entry (EXIF, GPS) names.
    pub(super) fn directory_of(&mut self, pointer: Option<Entry>) -> Option<Vec<Entry>> {
        let at = *self.unsigned(pointer?)?.first()?;
        self.directory(at)
    }

    fn directory(&mut self, at: u64) -> Option<Vec<Entry>> {
        self.reader.seek(SeekFrom::Start(at)).ok()?;
        let count = if self.big {
            let mut eight = [0_u8; 8];
            self.reader.read_exact(&mut eight).ok()?;
            self.u64_at(&eight)
        } else {
            let mut two = [0_u8; 2];
            self.reader.read_exact(&mut two).ok()?;
            u64::from(self.u16_at(&two))
        };
        if count > MAX_ENTRIES {
            return None;
        }
        let size = if self.big { 20 } else { 12 };
        let mut raw = vec![0_u8; usize::try_from(count * size).ok()?];
        self.reader.read_exact(&mut raw).ok()?;
        Some(
            raw.chunks_exact(usize::try_from(size).ok()?)
                .map(|chunk| {
                    let mut field = [0_u8; 8];
                    let (count, value) = if self.big {
                        (self.u64_at(&chunk[4..12]), &chunk[12..20])
                    } else {
                        (u64::from(self.u32_at(&chunk[4..8])), &chunk[8..12])
                    };
                    field[..value.len()].copy_from_slice(value);
                    Entry {
                        tag: self.u16_at(&chunk[0..2]),
                        kind: self.u16_at(&chunk[2..4]),
                        count,
                        field,
                    }
                })
                .collect(),
        )
    }

    /// The value's bytes as the file holds them; `None` past [`MAX_VALUE`].
    pub(super) fn bytes(&mut self, entry: Entry) -> Option<Vec<u8>> {
        let width = match entry.kind {
            1 | 2 | 6 | 7 => 1,
            3 | 8 => 2,
            4 | 9 | 11 | 13 => 4,
            5 | 10 | 12 | 16..=18 => 8,
            _ => return None,
        };
        let length = entry.count.checked_mul(width)?;
        if length > MAX_VALUE {
            return None;
        }
        let length = usize::try_from(length).ok()?;
        let inline = if self.big { 8 } else { 4 };
        if length <= inline {
            return Some(entry.field[..length].to_vec());
        }
        let at = if self.big {
            self.u64_at(&entry.field)
        } else {
            u64::from(self.u32_at(&entry.field[..4]))
        };
        self.reader.seek(SeekFrom::Start(at)).ok()?;
        let mut value = vec![0_u8; length];
        self.reader.read_exact(&mut value).ok()?;
        Some(value)
    }

    /// A SHORT, LONG, LONG8 or IFD value as numbers.
    pub(super) fn unsigned(&mut self, entry: Entry) -> Option<Vec<u64>> {
        let bytes = self.bytes(entry)?;
        Some(match entry.kind {
            3 => bytes
                .as_chunks::<2>()
                .0
                .iter()
                .map(|two| u64::from(self.u16_at(two)))
                .collect(),
            4 | 13 => bytes
                .as_chunks::<4>()
                .0
                .iter()
                .map(|four| u64::from(self.u32_at(four)))
                .collect(),
            16 | 18 => bytes
                .as_chunks::<8>()
                .0
                .iter()
                .map(|eight| self.u64_at(eight))
                .collect(),
            _ => return None,
        })
    }

    /// A RATIONAL value as numbers.
    pub(super) fn rationals(&mut self, entry: Entry) -> Option<Vec<f64>> {
        if entry.kind != 5 {
            return None;
        }
        let bytes = self.bytes(entry)?;
        Some(
            bytes
                .as_chunks::<8>()
                .0
                .iter()
                .map(|pair| f64::from(self.u32_at(&pair[..4])) / f64::from(self.u32_at(&pair[4..])))
                .collect(),
        )
    }

    fn u16_at(&self, bytes: &[u8]) -> u16 {
        let two = [bytes[0], bytes[1]];
        if self.big_endian {
            u16::from_be_bytes(two)
        } else {
            u16::from_le_bytes(two)
        }
    }

    fn u32_at(&self, bytes: &[u8]) -> u32 {
        let four = [bytes[0], bytes[1], bytes[2], bytes[3]];
        if self.big_endian {
            u32::from_be_bytes(four)
        } else {
            u32::from_le_bytes(four)
        }
    }

    fn u64_at(&self, bytes: &[u8]) -> u64 {
        let mut eight = [0_u8; 8];
        eight.copy_from_slice(&bytes[..8]);
        if self.big_endian {
            u64::from_be_bytes(eight)
        } else {
            u64::from_le_bytes(eight)
        }
    }
}

/// The entry for `tag` in `entries`.
pub(super) fn find(entries: &[Entry], tag: u16) -> Option<Entry> {
    entries.iter().copied().find(|entry| entry.tag == tag)
}
