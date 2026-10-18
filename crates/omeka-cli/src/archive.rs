//! The Omeka backup archive: its layout, its manifest, and the rules that make
//! an archive restorable.
//!
//! The archive is a tar (see [`crate::tar`]) with three kinds of entry:
//!
//! ```text
//! omeka.db          a consistent SQLite copy, from `VACUUM INTO`
//! store/<path>      every file under the store root, at its store-relative path
//! manifest.json     the commit, the counts and the checksums
//! ```
//!
//! The store files keep their store-relative paths, so restore writes them back
//! where `FileStore::local_path` reads them.
//!
//! Consistency. The database copy is taken first, then the store is read. A
//! media row can only reference a file that existed when the row was written, so
//! a store read after the database snapshot holds every file the snapshot names.
//! [`verify_store_references`] proves it for the rows in hand and refuses the
//! backup when a named file is absent, so an archive never holds a row without
//! its file.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use chrono::{Timelike, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::SqlitePool;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};

use omeka_db::DbError;
use omeka_db::repos::{MediaFilter, MediaRepo, Paging};
use omeka_media::file_render::ThumbnailType;
use omeka_media::store::{FileStore, StorePrefix};
use omeka_media::thumbnailer::thumbnail_path;

use crate::error::CliError;
use crate::tar::hex_encode;

/// The manifest's `format` value.
pub const FORMAT: &str = "omeka-backup";
/// The manifest layout this build writes and reads.
pub const FORMAT_VERSION: u32 = 1;
/// The archive entry holding the database copy.
pub const DATABASE_ENTRY: &str = "omeka.db";
/// The archive directory holding the store.
pub const STORE_DIR: &str = "store";
/// The archive entry holding the manifest.
pub const MANIFEST_ENTRY: &str = "manifest.json";

/// The commit this binary was built from, or `unknown` outside a checkout.
/// `build.rs` runs `git rev-parse HEAD`.
pub const COMMIT: &str = env!("OMEKA_COMMIT");

/// The tables whose row counts the manifest records, in the order it records
/// them. Upstream's resource graph, with the vocabulary tables and the sites.
pub const COUNTED_TABLES: &[&str] = &[
    "resource",
    "item",
    "item_set",
    "media",
    "value",
    "user",
    "vocabulary",
    "resource_class",
    "property",
    "resource_template",
    "asset",
    "site",
    "site_page",
    "site_page_block",
];

/// The archive manifest, written as `manifest.json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Manifest {
    /// Always [`FORMAT`].
    pub format: String,
    /// The layout version, [`FORMAT_VERSION`].
    pub format_version: u32,
    /// When the archive was written, in the upstream datetime format.
    pub created: String,
    /// `Module::VERSION` of the port that wrote the archive.
    pub version: String,
    /// The git commit the binary was built from.
    pub commit: String,
    /// Row counts of [`COUNTED_TABLES`], taken from the archived copy.
    pub counts: BTreeMap<String, i64>,
    /// The database entry.
    pub database: DatabaseEntry,
    /// The store entries.
    pub store: StoreSection,
}

/// The database copy inside an archive.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DatabaseEntry {
    /// The archive entry name, [`DATABASE_ENTRY`].
    pub entry: String,
    /// Size in bytes.
    pub size: u64,
    /// Lowercase hex SHA-256.
    pub sha256: String,
}

/// The store section of an archive.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoreSection {
    /// Every store file, sorted by its store-relative path.
    pub files: Vec<StoreFile>,
    /// The sum of the files' sizes.
    pub size: u64,
    /// A SHA-256 over the `(path, sha256)` pairs, see [`store_checksum`].
    pub checksum: String,
}

/// One file under the store root.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoreFile {
    /// The path relative to the store root, with `/` separators.
    pub path: String,
    /// Size in bytes.
    pub size: u64,
    /// Lowercase hex SHA-256.
    pub sha256: String,
}

impl Manifest {
    /// The manifest as the bytes of its `manifest.json` entry, with a trailing
    /// newline.
    ///
    /// # Errors
    /// A [`serde_json::Error`] if it cannot be serialized, which is a bug.
    pub fn to_json(&self) -> Result<Vec<u8>, serde_json::Error> {
        let mut bytes = serde_json::to_vec_pretty(self)?;
        bytes.push(b'\n');
        Ok(bytes)
    }

    /// Parse a `manifest.json` entry.
    ///
    /// # Errors
    /// A [`serde_json::Error`] when the bytes are not a manifest.
    pub fn from_json(bytes: &[u8]) -> Result<Self, serde_json::Error> {
        serde_json::from_slice(bytes)
    }

    /// Check the format fields a restore must trust.
    ///
    /// # Errors
    /// A message naming the field that does not match this build.
    pub fn validate(&self) -> Result<(), String> {
        if self.format != FORMAT {
            return Err(format!(
                "the archive is a \"{}\", not a \"{FORMAT}\"",
                self.format
            ));
        }
        if self.format_version != FORMAT_VERSION {
            return Err(format!(
                "the archive layout is version {}, this build reads version {FORMAT_VERSION}",
                self.format_version
            ));
        }
        if self.database.entry != DATABASE_ENTRY {
            return Err(format!(
                "the archive names its database \"{}\", expected \"{DATABASE_ENTRY}\"",
                self.database.entry
            ));
        }
        Ok(())
    }
}

/// The store-relative path of an archive entry, or `None` when the entry is not
/// under `store/`.
#[must_use]
pub fn store_relative(entry: &str) -> Option<&str> {
    entry
        .strip_prefix(STORE_DIR)
        .and_then(|rest| rest.strip_prefix('/'))
        .filter(|rest| !rest.is_empty())
}

/// The archive entry name for a store-relative path.
#[must_use]
pub fn store_entry(relative: &str) -> String {
    format!("{STORE_DIR}/{relative}")
}

/// A SHA-256 over the `(path, sha256)` pairs in order, one pair per line as
/// `path\0sha256\n`. The list is sorted, so the value is stable.
#[must_use]
pub fn store_checksum(files: &[StoreFile]) -> String {
    let mut hasher = Sha256::new();
    for file in files {
        hasher.update(file.path.as_bytes());
        hasher.update([0u8]);
        hasher.update(file.sha256.as_bytes());
        hasher.update(b"\n");
    }
    hex_encode(&hasher.finalize())
}

/// A file found under a store root.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoreFileRef {
    /// The path relative to the root, with `/` separators.
    pub relative: String,
    /// The absolute path on disk.
    pub path: PathBuf,
}

/// Every regular file under `root`, sorted by store-relative path. A missing
/// root yields an empty list; a path that is not UTF-8 is an error, because the
/// archive names files by their UTF-8 path. A symlink to a file is followed and
/// archived; a symlink to a directory is not walked, so a link loop cannot hang
/// the walk.
///
/// # Errors
/// A [`CliError`] when a directory cannot be read or a path is not UTF-8.
pub fn collect_store_files(root: &Path) -> Result<Vec<StoreFileRef>, CliError> {
    let mut files = Vec::new();
    if root.is_dir() {
        walk(root, root, &mut files)?;
    }
    files.sort_by(|left, right| left.relative.cmp(&right.relative));
    Ok(files)
}

fn walk(root: &Path, dir: &Path, out: &mut Vec<StoreFileRef>) -> Result<(), CliError> {
    let mut entries = std::fs::read_dir(dir)?.collect::<Result<Vec<_>, std::io::Error>>()?;
    entries.sort_by_key(std::fs::DirEntry::file_name);
    for entry in entries {
        let path = entry.path();
        let kind = entry.file_type()?;
        if kind.is_dir() {
            walk(root, &path, out)?;
        } else if path.is_file() {
            let relative = path.strip_prefix(root).map_err(|error| {
                CliError::message(format!(
                    "cannot make {} relative to {}: {error}",
                    path.display(),
                    root.display()
                ))
            })?;
            let relative = relative.to_str().ok_or_else(|| {
                CliError::message(format!(
                    "the store holds a path that is not UTF-8: {}",
                    relative.display()
                ))
            })?;
            out.push(StoreFileRef {
                relative: relative.replace(std::path::MAIN_SEPARATOR, "/"),
                path,
            });
        }
    }
    Ok(())
}

/// The row counts of [`COUNTED_TABLES`], read from a migrated database.
///
/// # Errors
/// A [`DbError`] when a count query fails.
pub async fn count_rows(pool: &SqlitePool) -> Result<BTreeMap<String, i64>, DbError> {
    let mut counts = BTreeMap::new();
    for table in COUNTED_TABLES {
        let sql = format!("SELECT COUNT(*) FROM \"{table}\"");
        let count: i64 = sqlx::query_scalar(&sql).fetch_one(pool).await?;
        counts.insert((*table).to_string(), count);
    }
    Ok(counts)
}

/// A one-connection read-only pool, so counting rows cannot write a journal
/// beside a finished database copy.
///
/// # Errors
/// A [`DbError`] when the file cannot be opened.
pub async fn open_read_only(path: &Path) -> Result<SqlitePool, DbError> {
    let options = SqliteConnectOptions::new().filename(path).read_only(true);
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await?;
    Ok(pool)
}

/// Check that every file the media rows name exists under `root`, and return the
/// number of files checked. A row with `has_original` or `has_thumbnails` but no
/// storage id is a failure too.
///
/// # Errors
/// A [`CliError`] naming the first media row whose file is absent, or a
/// [`DbError`] when the rows cannot be listed.
pub async fn verify_store_references(pool: &SqlitePool, root: &Path) -> Result<usize, CliError> {
    let rows = MediaRepo::new(pool)
        .list(&MediaFilter::default(), &Paging::default())
        .await?;
    let mut checked = 0usize;
    for row in &rows {
        if !row.has_original && !row.has_thumbnails {
            continue;
        }
        let Some(storage_id) = row.storage_id.as_deref() else {
            return Err(CliError::message(format!(
                "media {} claims a stored file but has no storage_id",
                row.id().0
            )));
        };
        if row.has_original {
            let path = FileStore::storage_path(
                StorePrefix::Original,
                storage_id,
                row.extension.as_deref(),
            );
            require_file(root, &path, row.id().0)?;
            checked += 1;
        }
        if row.has_thumbnails {
            for kind in ThumbnailType::ALL {
                require_file(root, &thumbnail_path(storage_id, kind), row.id().0)?;
                checked += 1;
            }
        }
    }
    Ok(checked)
}

fn require_file(root: &Path, relative: &str, media: u64) -> Result<(), CliError> {
    if root.join(relative).is_file() {
        Ok(())
    } else {
        Err(CliError::message(format!(
            "media {media} references {relative}, which the store does not hold"
        )))
    }
}

/// The current time in the upstream datetime format, used for the manifest's
/// `created` field.
#[must_use]
pub fn now_stamp() -> String {
    let now = Utc::now().naive_utc();
    let now = now.with_nanosecond(0).unwrap_or(now);
    now.format("%Y-%m-%d %H:%M:%S").to_string()
}

/// A private directory beside a target path.
///
/// Staging keeps a half-written archive or a half-extracted restore out of the
/// target, and placing it beside the target keeps a later `rename` on one
/// filesystem. The directory is removed when the value is dropped, or at once by
/// [`Staging::retire`].
pub struct Staging {
    directory: PathBuf,
    retired: bool,
}

impl Staging {
    /// Create a staging directory beside `reference`. The reference's parent
    /// must exist, so a typo in the target path fails before any work.
    ///
    /// # Errors
    /// A [`CliError`] when the parent is missing or the directory cannot be
    /// created.
    pub fn new_beside(reference: &Path, tag: &str) -> Result<Self, CliError> {
        let parent = reference
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        if !parent.is_dir() {
            return Err(CliError::message(format!(
                "the directory {} does not exist",
                parent.display()
            )));
        }
        let directory = parent.join(format!(
            ".{tag}-{}-{}-{}",
            std::process::id(),
            serial(),
            nano_stamp()
        ));
        std::fs::create_dir(&directory)?;
        Ok(Self {
            directory,
            retired: false,
        })
    }

    /// The staging directory.
    #[must_use]
    pub fn directory(&self) -> &Path {
        &self.directory
    }

    /// A path inside the staging directory.
    #[must_use]
    pub fn path(&self, name: &str) -> PathBuf {
        self.directory.join(name)
    }

    /// Remove the staging directory now.
    pub fn retire(mut self) {
        self.retired = true;
        let _ = std::fs::remove_dir_all(&self.directory);
    }
}

impl Drop for Staging {
    fn drop(&mut self) {
        if !self.retired {
            let _ = std::fs::remove_dir_all(&self.directory);
        }
    }
}

fn serial() -> u64 {
    static SERIAL: AtomicU64 = AtomicU64::new(0);
    SERIAL.fetch_add(1, Ordering::Relaxed)
}

fn nano_stamp() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_nanos())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_manifest_survives_a_json_round_trip() {
        let manifest = Manifest {
            format: FORMAT.to_string(),
            format_version: FORMAT_VERSION,
            created: "2026-10-17 10:00:00".to_string(),
            version: "4.3.0-alpha".to_string(),
            commit: "abc".to_string(),
            counts: BTreeMap::from([("item".to_string(), 3)]),
            database: DatabaseEntry {
                entry: DATABASE_ENTRY.to_string(),
                size: 4096,
                sha256: "aa".to_string(),
            },
            store: StoreSection {
                files: vec![StoreFile {
                    path: "original/a.jpg".to_string(),
                    size: 2,
                    sha256: "bb".to_string(),
                }],
                size: 2,
                checksum: "cc".to_string(),
            },
        };
        let bytes = manifest.to_json().expect("serialize");
        let parsed = Manifest::from_json(&bytes).expect("parse");
        assert_eq!(parsed, manifest);
    }

    #[test]
    fn validate_rejects_a_foreign_or_newer_archive() {
        let mut manifest = Manifest {
            format: "zip".to_string(),
            format_version: FORMAT_VERSION,
            created: String::new(),
            version: String::new(),
            commit: String::new(),
            counts: BTreeMap::new(),
            database: DatabaseEntry {
                entry: DATABASE_ENTRY.to_string(),
                size: 0,
                sha256: String::new(),
            },
            store: StoreSection {
                files: Vec::new(),
                size: 0,
                checksum: String::new(),
            },
        };
        assert!(manifest.validate().expect_err("format").contains("zip"));
        manifest.format = FORMAT.to_string();
        manifest.format_version = 99;
        assert!(manifest.validate().expect_err("version").contains("99"));
        manifest.format_version = FORMAT_VERSION;
        manifest.database.entry = "db.sqlite".to_string();
        assert!(
            manifest
                .validate()
                .expect_err("entry")
                .contains("db.sqlite")
        );
        manifest.database.entry = DATABASE_ENTRY.to_string();
        assert!(manifest.validate().is_ok());
    }

    #[test]
    fn the_store_checksum_covers_paths_and_hashes_in_order() {
        let files = vec![
            StoreFile {
                path: "a".to_string(),
                size: 1,
                sha256: "11".to_string(),
            },
            StoreFile {
                path: "b".to_string(),
                size: 1,
                sha256: "22".to_string(),
            },
        ];
        let checksum = store_checksum(&files);
        assert_eq!(checksum.len(), 64);
        let mut swapped = files.clone();
        swapped.swap(0, 1);
        assert_ne!(store_checksum(&swapped), checksum);
        let mut renamed = files.clone();
        renamed[0].path = "c".to_string();
        assert_ne!(store_checksum(&renamed), checksum);
    }

    #[test]
    fn store_entry_names_stay_inside_the_store_directory() {
        assert_eq!(
            store_relative("store/original/a.jpg"),
            Some("original/a.jpg")
        );
        assert_eq!(store_relative("store"), None);
        assert_eq!(store_relative("store/"), None);
        assert_eq!(store_relative("manifest.json"), None);
        assert_eq!(store_entry("original/a.jpg"), "store/original/a.jpg");
    }

    #[test]
    fn collect_store_files_is_sorted_and_path_relative() {
        let directory = std::env::temp_dir().join(format!("omeka-archive-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&directory);
        std::fs::create_dir_all(directory.join("original")).expect("create");
        std::fs::create_dir_all(directory.join("square")).expect("create");
        std::fs::write(directory.join("original/b.png"), b"b").expect("write");
        std::fs::write(directory.join("original/a.png"), b"a").expect("write");
        std::fs::write(directory.join("square/a.jpg"), b"s").expect("write");

        let files = collect_store_files(&directory).expect("collect");
        let names: Vec<&str> = files.iter().map(|file| file.relative.as_str()).collect();
        assert_eq!(names, ["original/a.png", "original/b.png", "square/a.jpg"]);
        let _ = std::fs::remove_dir_all(&directory);
    }
}
