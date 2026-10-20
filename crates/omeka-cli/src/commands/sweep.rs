//! `romeka media sweep`: find, and optionally delete, store files no row names.
//!
//! An import stores an item's files and then commits its row. A crash between
//! the two leaves files that no `media` row names, and the undo job, which
//! works from rows, cannot see them. This command works from the store: it
//! lists the files under `original/`, `asset/` and the thumbnail directories
//! and reports those that neither a `media` row (the original, and a thumbnail
//! of every type, by `storage_id`) nor an `asset` row names.
//!
//! Deletion is irreversible, so the default is a dry run and `--delete` is
//! explicit. Three rules keep `--delete` safe. It only handles paths that
//! `Store::list` returns for the store's own folders, which are regular files
//! directly inside them and never a symlink. It skips a file modified in the
//! last [`MIN_AGE`], because an import may be between storing and committing.
//! And it reads the rows before it lists the files, so a file stored and
//! committed after the read is young and skipped, never judged against a stale
//! row set.
//!
//! Idempotent: the decision for each file is derived from the rows and the
//! store as they are now, so a second `--delete` finds nothing left to remove.

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::time::{Duration, SystemTime};

use omeka_db::open_file;
use omeka_db::repos::{AssetRepo, MediaRepo};
use omeka_media::file_render::ThumbnailType;
use omeka_media::store::{StorePrefix, storage_path};

use crate::error::CliError;

/// A file younger than this is never reported or removed.
pub const MIN_AGE: Duration = Duration::from_secs(10 * 60);

/// Input for `romeka media sweep`.
#[derive(Debug, Clone)]
pub struct SweepOptions {
    /// SQLite database file.
    pub database: PathBuf,
    /// Store root. Defaults to `files` beside the database.
    pub store: Option<PathBuf>,
    /// Remove the orphans. Without it the run only reports them.
    pub delete: bool,
    /// Files modified more recently than this are left alone.
    pub min_age: Duration,
}

/// The outcome of a sweep.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SweepReport {
    /// Old orphans, as storage paths. Under `--delete` these were removed
    /// unless they are also in `failed`.
    pub orphans: Vec<String>,
    /// Orphans left alone for being younger than the minimum age.
    pub young: usize,
    /// Orphans `--delete` could not remove, with the reason.
    pub failed: Vec<(String, String)>,
}

/// List, and with `delete` remove, the store files no row names.
///
/// # Errors
/// A [`CliError`] when the database cannot be opened or read, or the store
/// cannot be listed. A file that cannot be removed is in the report.
pub async fn run(options: &SweepOptions) -> Result<SweepReport, CliError> {
    let pool = open_file(&options.database).await?;
    let store =
        crate::commands::file_store::open_store(&options.database, options.store.as_deref())?;

    let mut named = BTreeSet::new();
    for (storage_id, extension) in MediaRepo::new(&pool).stored_names().await? {
        named.insert(storage_path(
            StorePrefix::Original,
            &storage_id,
            extension.as_deref(),
        ));
        for kind in ThumbnailType::ALL {
            named.insert(omeka_media::thumbnailer::thumbnail_path(&storage_id, kind));
        }
    }
    for (storage_id, extension) in AssetRepo::new(&pool).stored_names().await? {
        named.insert(storage_path(
            StorePrefix::Asset,
            &storage_id,
            extension.as_deref(),
        ));
    }

    let now = SystemTime::now();
    let mut report = SweepReport::default();
    let mut listed = Vec::new();
    for prefix in StorePrefix::ALL {
        listed.extend(store.list(prefix.as_str())?);
    }
    listed.sort_by(|a, b| a.storage_path.cmp(&b.storage_path));
    for file in listed {
        if named.contains(&file.storage_path) {
            continue;
        }
        // A modification time in the future counts as young.
        let age = now.duration_since(file.modified).unwrap_or_default();
        if age < options.min_age {
            report.young += 1;
            continue;
        }
        if options.delete
            && let Err(error) = store.delete(&file.storage_path)
        {
            report
                .failed
                .push((file.storage_path.clone(), error.to_string()));
        }
        report.orphans.push(file.storage_path);
    }
    Ok(report)
}
