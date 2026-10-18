//! A small ustar tar reader and writer, and the SHA-256 digests the archive
//! manifest carries.
//!
//! `omeka backup` writes a plain (uncompressed) tar. The system `tar` can list
//! and extract the archive, which matters more for a backup an operator has to
//! inspect than the bytes a compressor would save. Neither a compression crate
//! nor an archive crate is pulled in for that reason.
//!
//! Only regular files are written. Directory entries carry no data, so restore
//! creates parent directories as it goes. Paths over 100 bytes use the ustar
//! `prefix` field, which reaches 255 bytes with a `/` split; a longer path is an
//! error rather than a silent failure.

use std::io::{self, Read, Write};
use std::path::Path;

use sha2::{Digest as _, Sha256};
use thiserror::Error;

/// The size of one tar block.
pub const BLOCK: usize = 512;

/// A tar operation failure.
#[derive(Debug, Error)]
pub enum TarError {
    /// The underlying reader or writer failed.
    #[error("archive I/O error: {0}")]
    Io(#[from] io::Error),
    /// The archive ended in the middle of an entry.
    #[error("the archive ends before its last entry is complete")]
    Truncated,
    /// The bytes do not form a tar entry this reader understands.
    #[error("malformed archive entry: {0}")]
    Malformed(String),
    /// A path does not fit a ustar header.
    #[error("entry path \"{0}\" is too long for a tar header")]
    PathTooLong(String),
}

/// A file's size and SHA-256, computed while archiving or verifying it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileDigest {
    /// Size in bytes.
    pub size: u64,
    /// Lowercase hex SHA-256.
    pub sha256: String,
}

/// Writes a ustar archive.
pub struct TarWriter<W: Write> {
    inner: W,
}

impl<W: Write> TarWriter<W> {
    /// Wrap a writer.
    pub fn new(inner: W) -> Self {
        Self { inner }
    }

    /// Write one entry from memory and return its digest.
    ///
    /// # Errors
    /// A [`TarError`] when the path cannot be encoded or the write fails.
    pub fn add_bytes(&mut self, name: &str, bytes: &[u8]) -> Result<FileDigest, TarError> {
        self.write_header(name, bytes.len() as u64)?;
        self.inner.write_all(bytes)?;
        self.pad(bytes.len() as u64)?;
        Ok(FileDigest {
            size: bytes.len() as u64,
            sha256: sha256_hex(bytes),
        })
    }

    /// Write one entry by streaming a file from disk, hashing it as it is
    /// written, and return its digest.
    ///
    /// # Errors
    /// A [`TarError`] when the file cannot be read, its size changes while it is
    /// read, the path cannot be encoded, or the write fails.
    pub fn add_file(&mut self, name: &str, path: &Path) -> Result<FileDigest, TarError> {
        let mut file = std::fs::File::open(path)?;
        let size = file.metadata()?.len();
        self.write_header(name, size)?;
        let mut hasher = Sha256::new();
        let mut buffer = [0u8; 64 * 1024];
        let mut written = 0u64;
        loop {
            let read = file.read(&mut buffer)?;
            if read == 0 {
                break;
            }
            hasher.update(&buffer[..read]);
            self.inner.write_all(&buffer[..read])?;
            written += read as u64;
        }
        if written != size {
            return Err(TarError::Malformed(format!(
                "{name} changed size while it was read"
            )));
        }
        self.pad(written)?;
        Ok(FileDigest {
            size,
            sha256: hex_encode(&hasher.finalize()),
        })
    }

    /// Write the two zero blocks that end an archive, flush, and return the
    /// wrapped writer.
    ///
    /// # Errors
    /// A [`TarError`] when the flush fails.
    pub fn finish(mut self) -> Result<W, TarError> {
        self.inner.write_all(&[0u8; BLOCK * 2])?;
        self.inner.flush()?;
        Ok(self.inner)
    }

    fn pad(&mut self, len: u64) -> Result<(), TarError> {
        let remainder = (len % BLOCK as u64) as usize;
        if remainder != 0 {
            self.inner.write_all(&[0u8; BLOCK][..BLOCK - remainder])?;
        }
        Ok(())
    }

    fn write_header(&mut self, name: &str, size: u64) -> Result<(), TarError> {
        let header = header(name, size)?;
        self.inner.write_all(&header)?;
        Ok(())
    }
}

/// One entry's header.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TarEntry {
    /// The path within the archive.
    pub name: String,
    /// The size of the entry's data in bytes.
    pub size: u64,
}

/// Reads a ustar archive of regular files, as [`TarWriter`] writes one.
///
/// Call [`TarReader::copy_entry`] for each entry to consume its data before the
/// next [`TarReader::next_entry`], or let `next_entry` skip it.
pub struct TarReader<R: Read> {
    inner: R,
    data_remaining: u64,
    padding_remaining: usize,
    done: bool,
}

impl<R: Read> TarReader<R> {
    /// Wrap a reader.
    pub fn new(inner: R) -> Self {
        Self {
            inner,
            data_remaining: 0,
            padding_remaining: 0,
            done: false,
        }
    }

    /// The next entry header, or `None` at the end-of-archive marker. The
    /// previous entry's data and padding are skipped when the caller did not
    /// consume them.
    ///
    /// # Errors
    /// A [`TarError`] for a truncated, malformed, or unreadable archive, or for
    /// an entry type other than a regular file.
    pub fn next_entry(&mut self) -> Result<Option<TarEntry>, TarError> {
        self.skip_previous()?;
        if self.done {
            return Ok(None);
        }
        let mut block = [0u8; BLOCK];
        match self.inner.read_exact(&mut block) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::UnexpectedEof => {
                return Err(TarError::Truncated);
            }
            Err(error) => return Err(TarError::Io(error)),
        }
        if block.iter().all(|&byte| byte == 0) {
            self.done = true;
            return Ok(None);
        }
        verify_checksum(&block)?;
        if &block[257..262] != b"ustar" {
            return Err(TarError::Malformed(
                "the header is not a ustar header".to_string(),
            ));
        }
        if !matches!(block[156], 0 | b'0') {
            return Err(TarError::Malformed(format!(
                "entry type {:?} is not a regular file",
                char::from(block[156])
            )));
        }
        let name = entry_name(&block)?;
        let size = parse_octal(&block[124..136])?;
        self.data_remaining = size;
        self.padding_remaining = padding(size);
        Ok(Some(TarEntry { name, size }))
    }

    /// Copy the current entry's data to `out` and skip its padding. Returns the
    /// number of bytes copied.
    ///
    /// # Errors
    /// A [`TarError`] when the archive is truncated or the copies fail.
    pub fn copy_entry(&mut self, out: &mut impl Write) -> Result<u64, TarError> {
        let data = self.data_remaining;
        let copied = io::copy(&mut self.inner.by_ref().take(data), out)?;
        if copied != data {
            return Err(TarError::Truncated);
        }
        self.data_remaining = 0;
        self.skip_previous()?;
        Ok(copied)
    }

    fn skip_previous(&mut self) -> Result<(), TarError> {
        let data = self.data_remaining;
        if data > 0 {
            let skipped = io::copy(&mut self.inner.by_ref().take(data), &mut io::sink())?;
            if skipped != data {
                return Err(TarError::Truncated);
            }
            self.data_remaining = 0;
        }
        let padding = self.padding_remaining;
        if padding > 0 {
            let skipped = io::copy(
                &mut self.inner.by_ref().take(padding as u64),
                &mut io::sink(),
            )?;
            if skipped != padding as u64 {
                return Err(TarError::Truncated);
            }
            self.padding_remaining = 0;
        }
        Ok(())
    }
}

/// The SHA-256 of a byte slice, as lowercase hex.
#[must_use]
pub fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex_encode(&hasher.finalize())
}

/// Bytes as lowercase hex.
#[must_use]
pub fn hex_encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

/// The size and SHA-256 of a file, read from disk.
///
/// # Errors
/// A [`TarError`] when the file cannot be read.
pub fn file_digest(path: &Path) -> Result<FileDigest, TarError> {
    let mut file = std::fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 64 * 1024];
    let mut size = 0u64;
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
        size += read as u64;
    }
    Ok(FileDigest {
        size,
        sha256: hex_encode(&hasher.finalize()),
    })
}

fn padding(size: u64) -> usize {
    let remainder = (size % BLOCK as u64) as usize;
    if remainder == 0 { 0 } else { BLOCK - remainder }
}

/// Split a path into the ustar `prefix` and `name` fields.
fn split_name(path: &str) -> Result<(Option<&str>, &str), TarError> {
    if path.is_empty() || path.starts_with('/') || path.contains('\0') {
        return Err(TarError::PathTooLong(path.to_string()));
    }
    if path.len() <= 100 {
        return Ok((None, path));
    }
    let bytes = path.as_bytes();
    for index in (1..bytes.len()).rev() {
        if bytes[index] != b'/' {
            continue;
        }
        let prefix = &path[..index];
        let base = &path[index + 1..];
        if prefix.len() <= 155 && !base.is_empty() && base.len() <= 100 {
            return Ok((Some(prefix), base));
        }
    }
    Err(TarError::PathTooLong(path.to_string()))
}

fn header(name: &str, size: u64) -> Result<[u8; BLOCK], TarError> {
    let (prefix, base) = split_name(name)?;
    let mut header = [0u8; BLOCK];
    header[..base.len()].copy_from_slice(base.as_bytes());
    octal(&mut header[100..108], 0o644)?;
    octal(&mut header[108..116], 0)?;
    octal(&mut header[116..124], 0)?;
    octal(&mut header[124..136], size)?;
    octal(&mut header[136..148], 0)?;
    header[156] = b'0';
    header[257..263].copy_from_slice(b"ustar\0");
    header[263..265].copy_from_slice(b"00");
    if let Some(prefix) = prefix {
        header[345..345 + prefix.len()].copy_from_slice(prefix.as_bytes());
    }
    for byte in &mut header[148..156] {
        *byte = b' ';
    }
    let checksum: u32 = header.iter().map(|&byte| u32::from(byte)).sum();
    let text = format!("{checksum:06o}\0 ");
    header[148..156].copy_from_slice(text.as_bytes());
    Ok(header)
}

fn octal(field: &mut [u8], value: u64) -> Result<(), TarError> {
    let digits = field.len() - 1;
    let text = format!("{value:0digits$o}");
    let bytes = text.as_bytes();
    if bytes.len() > digits {
        return Err(TarError::Malformed(format!(
            "the value {value} does not fit its {digits} digit header field"
        )));
    }
    field[..bytes.len()].copy_from_slice(bytes);
    field[bytes.len()] = 0;
    Ok(())
}

fn parse_octal(field: &[u8]) -> Result<u64, TarError> {
    let text: Vec<u8> = field
        .iter()
        .copied()
        .take_while(|&byte| byte != 0 && byte != b' ')
        .collect();
    if text.is_empty() {
        return Ok(0);
    }
    let text = std::str::from_utf8(&text)
        .map_err(|_| TarError::Malformed("an octal header field is not text".to_string()))?;
    u64::from_str_radix(text, 8)
        .map_err(|_| TarError::Malformed(format!("the octal field \"{text}\" has no digits")))
}

fn verify_checksum(block: &[u8; BLOCK]) -> Result<(), TarError> {
    let stored = parse_octal(&block[148..156])?;
    let mut copy = *block;
    for byte in &mut copy[148..156] {
        *byte = b' ';
    }
    let computed: u32 = copy.iter().map(|&byte| u32::from(byte)).sum();
    if u64::from(computed) != stored {
        return Err(TarError::Malformed(format!(
            "the header checksum is {stored}, computed {computed}"
        )));
    }
    Ok(())
}

fn entry_name(block: &[u8; BLOCK]) -> Result<String, TarError> {
    let base = field(&block[0..100]);
    let prefix = field(&block[345..500]);
    if base.is_empty() {
        return Err(TarError::Malformed("an entry has no name".to_string()));
    }
    if prefix.is_empty() {
        Ok(base)
    } else {
        Ok(format!("{prefix}/{base}"))
    }
}

fn field(bytes: &[u8]) -> String {
    let end = bytes
        .iter()
        .position(|&byte| byte == 0)
        .unwrap_or(bytes.len());
    String::from_utf8_lossy(&bytes[..end]).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(entries: &[(&str, &[u8])]) -> Vec<u8> {
        let mut writer = TarWriter::new(Vec::new());
        for (name, bytes) in entries {
            writer.add_bytes(name, bytes).expect("add entry");
        }
        writer.finish().expect("finish")
    }

    fn read_all(archive: &[u8]) -> Vec<(String, Vec<u8>)> {
        let mut reader = TarReader::new(archive);
        let mut out = Vec::new();
        while let Some(entry) = reader.next_entry().expect("next entry") {
            let mut bytes = Vec::new();
            let copied = reader.copy_entry(&mut bytes).expect("copy entry");
            assert_eq!(copied, entry.size);
            out.push((entry.name, bytes));
        }
        out
    }

    #[test]
    fn a_written_archive_reads_back_with_its_names_and_bytes() {
        let archive = write(&[("manifest.json", b"{}"), ("store/original/a.jpg", b"jpeg")]);
        let entries = read_all(&archive);
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].0, "manifest.json");
        assert_eq!(entries[0].1, b"{}");
        assert_eq!(entries[1].0, "store/original/a.jpg");
        assert_eq!(entries[1].1, b"jpeg");
    }

    #[test]
    fn the_archive_ends_with_two_zero_blocks() {
        let archive = write(&[("a", b"x")]);
        // header 512 + data block 512 + two end blocks 1024.
        assert_eq!(archive.len(), BLOCK * 4);
        assert_eq!(archive[BLOCK..BLOCK + 3], [b'x', 0, 0]);
        assert!(archive[BLOCK * 2..].iter().all(|&byte| byte == 0));
    }

    #[test]
    fn the_digest_matches_the_bytes_written() {
        let mut writer = TarWriter::new(Vec::<u8>::new());
        let digest = writer.add_bytes("a", b"abc").expect("add");
        assert_eq!(digest.size, 3);
        assert_eq!(
            digest.sha256,
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn a_long_path_uses_the_prefix_field() {
        let name = format!("store/original/{}.jpg", "a".repeat(90));
        assert!(name.len() > 100);
        let archive = write(&[(name.as_str(), b"bytes")]);
        let entries = read_all(&archive);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].0, name);
        assert_eq!(entries[0].1, b"bytes");
    }

    #[test]
    fn a_path_that_does_not_fit_is_refused() {
        let name = "store/original/".to_string() + &"a".repeat(300);
        let mut writer = TarWriter::new(Vec::<u8>::new());
        assert!(matches!(
            writer.add_bytes(&name, b"x"),
            Err(TarError::PathTooLong(_))
        ));
    }

    #[test]
    fn an_entry_without_consumed_data_is_skipped_by_the_next_header() {
        let archive = write(&[("a", b"first bytes"), ("b", b"second")]);
        let mut reader = TarReader::new(archive.as_slice());
        let first = reader.next_entry().expect("first").expect("entry");
        assert_eq!(first.name, "a");
        let second = reader.next_entry().expect("second").expect("entry");
        assert_eq!(second.name, "b");
        let mut bytes = Vec::new();
        reader.copy_entry(&mut bytes).expect("copy");
        assert_eq!(bytes, b"second");
    }

    /// Every truncation point either yields complete entries or an error. A cut
    /// inside an entry must not read a short body as if it were whole.
    #[test]
    fn a_truncated_archive_is_an_error_not_a_panic() {
        let archive = write(&[("a", b"0123456789"), ("b", &[7u8; 600])]);
        for cut in 1..archive.len() {
            let mut reader = TarReader::new(&archive[..cut]);
            while let Ok(Some(entry)) = reader.next_entry() {
                let mut sink = Vec::new();
                if reader.copy_entry(&mut sink).is_err() {
                    break;
                }
                assert_eq!(sink.len() as u64, entry.size, "cut {cut} read a short body");
            }
        }
    }

    #[test]
    fn a_corrupt_header_checksum_is_refused() {
        let mut archive = write(&[("a", b"x")]);
        // Flip a byte of the name, which the checksum covers.
        archive[0] = b'z';
        let mut reader = TarReader::new(archive.as_slice());
        assert!(matches!(reader.next_entry(), Err(TarError::Malformed(_))));
    }

    #[test]
    fn file_digest_matches_sha256_hex_of_the_bytes() {
        let directory = std::env::temp_dir().join(format!("omeka-tar-{}", std::process::id()));
        std::fs::create_dir_all(&directory).expect("create dir");
        let path = directory.join("bytes.bin");
        std::fs::write(&path, b"hello").expect("write");
        let digest = file_digest(&path).expect("digest");
        assert_eq!(digest.size, 5);
        assert_eq!(digest.sha256, sha256_hex(b"hello"));
        let _ = std::fs::remove_dir_all(&directory);
    }
}
