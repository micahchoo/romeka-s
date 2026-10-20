//! The walk: a source's folder read into entries (`docs/import-v2.md`,
//! "Evidence").
//!
//! [`walk`] lists every file under a [`SourceDir`] with its size, modified
//! time and sniffed media type. Nothing is dropped silently: what is not an
//! entry is counted or named in [`Anomalies`]. Sheets are listed apart from
//! file entries, because a sheet is read as rows, never imported as a file.
//! The walk holds no database; the evidence index stores what it returns.

mod identical;
mod names;
#[cfg(test)]
mod tests;

use std::ffi::OsStr;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

pub use identical::{Identical, PREFIX as IDENTICAL_PREFIX};
pub use names::NameEncoding;

use super::archive::SourceDir;
use super::plan::RelPath;

/// How much of a file the media type is sniffed from.
pub const SNIFF_BYTES: usize = 8 * 1024;

/// One file of a source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileEntry {
    /// The identity: relative to the source, NFC, decoded when the disk
    /// name is not UTF-8.
    pub path: RelPath,
    pub size: u64,
    pub modified: Option<SystemTime>,
    /// The media type `omeka-media` reads from the first [`SNIFF_BYTES`].
    pub sniffed: String,
    /// Set when a name on the path was not UTF-8; the entry then flags
    /// `name-encoding`.
    pub decoded: Option<DecodedName>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecodedName {
    pub encoding: NameEncoding,
    /// The path relative to the source as the disk spells it.
    pub raw: Vec<u8>,
}

impl FileEntry {
    /// The path relative to the source as the disk spells it, for
    /// [`SourceDir::locate`].
    #[must_use]
    pub fn disk_path(&self) -> PathBuf {
        match &self.decoded {
            Some(decoded) => path_from_bytes(&decoded.raw),
            None => PathBuf::from(self.path.as_str()),
        }
    }

    /// The last segment of the path.
    #[must_use]
    pub fn name(&self) -> &str {
        let path = self.path.as_str();
        path.rsplit_once('/').map_or(path, |(_, name)| name)
    }

    /// The folders above the file, `""` at the top of the source.
    #[must_use]
    pub fn folder(&self) -> &str {
        self.path
            .as_str()
            .rsplit_once('/')
            .map_or("", |(folder, _)| folder)
    }
}

/// What a walk found that is not an entry, or that an entry hides.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Anomalies {
    /// Symlinks not followed: to a folder, out of the boundary, or broken.
    pub symlinks: u64,
    /// Dot files and dot folders; a dot folder counts once, unread.
    pub dot: u64,
    /// Sockets, pipes and devices.
    pub special: u64,
    /// Folders and files the walk could not read.
    pub unreadable: Vec<String>,
    /// Names a [`RelPath`] refuses, such as one holding a `\`.
    pub unusable_names: Vec<String>,
    /// Files whose path equals an earlier entry's after decoding and NFC.
    pub collisions: Vec<String>,
    /// Files with the same size and the same SHA-256.
    pub identical: Vec<Identical>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Walk {
    /// Sorted by path.
    pub entries: Vec<FileEntry>,
    /// CSV and TSV files, and text files that read as delimited, sorted by
    /// path. ODS joins in P8; until then an `.ods` file is a file entry.
    pub sheets: Vec<FileEntry>,
    pub anomalies: Anomalies,
}

impl Walk {
    /// Each decoded name, as the disk spells it (lossy) and as decoded.
    pub fn decoded_names(&self) -> impl Iterator<Item = (String, &RelPath)> {
        self.entries.iter().chain(&self.sheets).filter_map(|entry| {
            let decoded = entry.decoded.as_ref()?;
            Some((
                String::from_utf8_lossy(&decoded.raw).into_owned(),
                &entry.path,
            ))
        })
    }
}

struct Found {
    path: RelPath,
    disk: PathBuf,
    decoded: Option<DecodedName>,
}

/// Every file under the source.
///
/// # Errors
/// The I/O error when the source folder itself cannot be read. A folder or
/// file below it that cannot be read is an anomaly, not an error.
pub fn walk(source: &SourceDir) -> std::io::Result<Walk> {
    let mut anomalies = Anomalies::default();
    let mut found = Vec::new();
    let mut folders = vec![(PathBuf::new(), String::new(), None::<NameEncoding>)];
    while let Some((disk, shown, encoding)) = folders.pop() {
        let listing = match fs::read_dir(source.dir().join(&disk)) {
            Ok(listing) => listing,
            Err(error) if disk.as_os_str().is_empty() => return Err(error),
            Err(_) => {
                anomalies.unreadable.push(shown);
                continue;
            }
        };
        for child in listing {
            let Ok(child) = child else {
                anomalies.unreadable.push(shown.clone());
                continue;
            };
            let raw = child.file_name();
            let bytes = os_bytes(&raw);
            if bytes.first() == Some(&b'.') {
                anomalies.dot += 1;
                continue;
            }
            let (name, decoded_as) = match raw.to_str() {
                Some(name) => (name.to_string(), None),
                None => {
                    let (name, decoded_as) = names::decode(&bytes);
                    (name, Some(decoded_as))
                }
            };
            let disk_child = disk.join(&raw);
            let shown_child = if shown.is_empty() {
                name
            } else {
                format!("{shown}/{name}")
            };
            let encoding = encoding.or(decoded_as);
            let Ok(kind) = child.file_type() else {
                anomalies.unreadable.push(shown_child);
                continue;
            };
            if kind.is_dir() {
                folders.push((disk_child, shown_child, encoding));
                continue;
            }
            let is_file = if kind.is_symlink() {
                let target = fs::canonicalize(child.path());
                let usable = target
                    .is_ok_and(|target| target.starts_with(source.boundary()) && target.is_file());
                if !usable {
                    anomalies.symlinks += 1;
                }
                usable
            } else {
                kind.is_file()
            };
            if !is_file {
                if !kind.is_symlink() {
                    anomalies.special += 1;
                }
                continue;
            }
            let Ok(path) = RelPath::parse(&shown_child) else {
                anomalies.unusable_names.push(shown_child);
                continue;
            };
            let decoded = encoding.map(|encoding| DecodedName {
                encoding,
                raw: os_bytes(disk_child.as_os_str()),
            });
            found.push(Found {
                path,
                disk: disk_child,
                decoded,
            });
        }
    }

    found.sort_by(|a, b| a.path.cmp(&b.path));
    let mut walk = Walk::default();
    let mut last: Option<RelPath> = None;
    for file in found {
        if last.as_ref() == Some(&file.path) {
            anomalies.collisions.push(lossy(&file.disk));
            continue;
        }
        last = Some(file.path.clone());
        let shown = file.path.to_string();
        let Some((entry, is_sheet)) = read_entry(source.dir(), file) else {
            anomalies.unreadable.push(shown);
            continue;
        };
        if is_sheet {
            walk.sheets.push(entry);
        } else {
            walk.entries.push(entry);
        }
    }
    anomalies.identical = identical::identical(source.dir(), &walk.entries);
    walk.anomalies = anomalies;
    Ok(walk)
}

/// The entry, and whether it is a sheet. `None` when the file cannot be
/// read.
fn read_entry(dir: &Path, file: Found) -> Option<(FileEntry, bool)> {
    let full = dir.join(&file.disk);
    let read = fs::metadata(&full).and_then(|metadata| {
        let mut head = Vec::with_capacity(SNIFF_BYTES);
        fs::File::open(&full)?
            .take(SNIFF_BYTES as u64)
            .read_to_end(&mut head)?;
        Ok((metadata, head))
    });
    let (metadata, head) = read.ok()?;
    let is_sheet = is_sheet(&file.path, &head);
    let sniffed = omeka_media::media_type::detected_media_type(whole_chars(&head));
    Some((
        FileEntry {
            path: file.path,
            size: metadata.len(),
            modified: metadata.modified().ok(),
            sniffed,
            decoded: file.decoded,
        },
        is_sheet,
    ))
}

fn is_sheet(path: &RelPath, head: &[u8]) -> bool {
    let extension = path
        .as_str()
        .rsplit_once('.')
        .map(|(_, extension)| extension.to_ascii_lowercase());
    match extension.as_deref() {
        Some("csv" | "tsv") => true,
        Some("txt") => super::sheet::looks_delimited(head),
        _ => false,
    }
}

/// A head cut at [`SNIFF_BYTES`] can end inside a UTF-8 character; without
/// the cut, text would sniff as binary.
fn whole_chars(head: &[u8]) -> &[u8] {
    match std::str::from_utf8(head) {
        Err(error) if error.error_len().is_none() && head.len() == SNIFF_BYTES => {
            &head[..error.valid_up_to()]
        }
        _ => head,
    }
}

fn lossy(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

#[cfg(unix)]
fn os_bytes(text: &OsStr) -> Vec<u8> {
    use std::os::unix::ffi::OsStrExt;
    text.as_bytes().to_vec()
}

#[cfg(not(unix))]
fn os_bytes(text: &OsStr) -> Vec<u8> {
    text.to_string_lossy().into_owned().into_bytes()
}

#[cfg(unix)]
fn path_from_bytes(bytes: &[u8]) -> PathBuf {
    use std::os::unix::ffi::OsStrExt;
    PathBuf::from(OsStr::from_bytes(bytes))
}

#[cfg(not(unix))]
fn path_from_bytes(bytes: &[u8]) -> PathBuf {
    PathBuf::from(String::from_utf8_lossy(bytes).into_owned())
}
