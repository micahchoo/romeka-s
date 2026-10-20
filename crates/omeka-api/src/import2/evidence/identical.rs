//! Identical files: files of one source with equal size and equal SHA-256
//! (`docs/import-v2.md`, "What the admin screens need at scale").
//!
//! Only files whose size another file shares are read. Each of those is
//! first hashed over its first [`PREFIX`] bytes, and only files whose prefix
//! hash collides are hashed in full, so a folder of same-size scans that
//! differ early costs one block per file, not every byte. The answer is
//! exact either way: two files are listed only when their sizes and their
//! whole-file SHA-256 agree. File names play no part.

use std::collections::{BTreeMap, HashMap};
use std::fs::File;
use std::io::{self, Read};
use std::path::Path;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::FileEntry;

/// How much of a file the first hash reads.
pub const PREFIX: u64 = 64 * 1024;

/// One set of files with the same bytes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Identical {
    pub size: u64,
    /// The SHA-256 of the whole file, lower-case hex.
    pub sha256: String,
    /// Sorted.
    pub paths: Vec<String>,
}

/// The sets of identical files among `entries`, read under `dir`, sorted by
/// their first path. A file that cannot be read is in no set.
#[must_use]
pub fn identical(dir: &Path, entries: &[FileEntry]) -> Vec<Identical> {
    let mut by_size: BTreeMap<u64, Vec<&FileEntry>> = BTreeMap::new();
    for entry in entries {
        by_size.entry(entry.size).or_default().push(entry);
    }
    let mut found = Vec::new();
    for (size, same_size) in by_size {
        if same_size.len() < 2 {
            continue;
        }
        let hash = |entry: &FileEntry, limit| hash_file(&dir.join(entry.disk_path()), limit).ok();
        let prefixed = same_size
            .iter()
            .filter_map(|entry| Some((*entry, hash(entry, Some(PREFIX))?)));
        for same_prefix in groups(prefixed) {
            let sets = if size <= PREFIX {
                vec![same_prefix]
            } else {
                groups(
                    same_prefix
                        .into_iter()
                        .filter_map(|(entry, _)| Some((entry, hash(entry, None)?))),
                )
            };
            for same in sets {
                let mut paths: Vec<String> = same
                    .iter()
                    .map(|(entry, _)| entry.path.to_string())
                    .collect();
                paths.sort();
                found.push(Identical {
                    size,
                    sha256: hex(&same[0].1),
                    paths,
                });
            }
        }
    }
    found.sort_by(|a, b| a.paths.cmp(&b.paths));
    found
}

/// The groups of two or more entries whose digest agrees.
fn groups<'e>(
    hashed: impl Iterator<Item = (&'e FileEntry, [u8; 32])>,
) -> Vec<Vec<(&'e FileEntry, [u8; 32])>> {
    let mut by_digest: HashMap<[u8; 32], Vec<(&'e FileEntry, [u8; 32])>> = HashMap::new();
    for (entry, digest) in hashed {
        by_digest.entry(digest).or_default().push((entry, digest));
    }
    by_digest
        .into_values()
        .filter(|group| group.len() > 1)
        .collect()
}

/// The SHA-256 of the file's first `limit` bytes, or of all of it, read in
/// bounded chunks.
fn hash_file(path: &Path, limit: Option<u64>) -> io::Result<[u8; 32]> {
    let file = File::open(path)?;
    let mut reader: Box<dyn Read> = match limit {
        Some(limit) => Box::new(file.take(limit)),
        None => Box::new(file),
    };
    let mut hasher = Sha256::new();
    let mut buffer = vec![0_u8; 64 * 1024];
    loop {
        let read = reader.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(hasher.finalize().into())
}

fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write;
    bytes.iter().fold(String::new(), |mut out, byte| {
        let _ = write!(out, "{byte:02x}");
        out
    })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::unwrap_used)]

    use super::*;
    use crate::import2::archive::SourceDir;
    use crate::import2::evidence::walk;

    fn write(dir: &Path, path: &str, bytes: &[u8]) {
        let full = dir.join(path);
        std::fs::create_dir_all(full.parent().unwrap()).unwrap();
        std::fs::write(full, bytes).unwrap();
    }

    #[test]
    fn files_are_identical_only_when_size_and_hash_agree() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "a/scan.tif", b"same bytes");
        write(dir.path(), "b/copy of scan.tif", b"same bytes");
        write(dir.path(), "scan.tif", b"other byte");
        write(dir.path(), "c/scan.tif", b"same bytes, longer");
        let walked = walk(&SourceDir::standalone(dir.path()).unwrap()).unwrap();
        assert_eq!(
            walked.anomalies.identical,
            [Identical {
                size: 10,
                sha256: omeka_media::store::sha256_hex(b"same bytes"),
                paths: vec!["a/scan.tif".into(), "b/copy of scan.tif".into()],
            }],
            "equal size with other bytes, and equal names with other sizes, are not identical"
        );
    }

    #[test]
    fn files_past_the_prefix_compare_every_byte() {
        let dir = tempfile::tempdir().unwrap();
        let size = usize::try_from(PREFIX).unwrap() + 10;
        let mut one = vec![7_u8; size];
        write(dir.path(), "one.tif", &one);
        write(dir.path(), "two.tif", &one);
        one[size - 1] = 8;
        write(dir.path(), "three.tif", &one);
        let walked = walk(&SourceDir::standalone(dir.path()).unwrap()).unwrap();
        let sets = &walked.anomalies.identical;
        assert_eq!(sets.len(), 1, "{sets:?}");
        assert_eq!(sets[0].paths, ["one.tif", "two.tif"]);
        assert_eq!(sets[0].size, PREFIX + 10);
    }
}
