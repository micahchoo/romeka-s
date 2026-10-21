//! `romeka backup`: one archive holding a consistent database copy and the store.
//!
//! The database copy comes from `DatabaseRepo::snapshot_into`, SQLite's
//! `VACUUM INTO`, which writes a complete SQLite file from a read transaction. It sees one point in time even while the server
//! writes, which a file copy of a write-ahead-log database does not: the copy
//! would miss the frames still in the `-wal`. `VACUUM INTO` also compacts, so the
//! archive holds no free pages.
//!
//! The order is database first, store second. A media row can only name a file
//! that existed when the row was written, so a store read after the database
//! snapshot holds every file the snapshot names. The reverse order would archive
//! a store the database has not caught up with. [`crate::archive::verify_store_references`]
//! checks the rows in hand before the archive is written and refuses the backup
//! when a named file is absent, so an archive never holds a row without its file.

use std::collections::BTreeMap;
use std::fs::File;
use std::io::BufWriter;
use std::path::{Path, PathBuf};

use omeka_db::repos::DatabaseRepo;
use omeka_db::{Location, open_file, with_db};

use crate::archive::{
    COMMIT, DATABASE_ENTRY, DatabaseEntry, FORMAT, FORMAT_VERSION, MANIFEST_ENTRY, Manifest,
    Staging, StoreFile, StoreSection, collect_store_files, count_rows, now_stamp, open_read_only,
    store_checksum, store_entry, verify_store_references,
};
use crate::commands::install::VERSION;
use crate::error::CliError;
use crate::tar::TarWriter;

/// The database copy's name inside the staging directory.
const EXPORT_NAME: &str = "romeka.db";
/// The half-written archive's name inside the staging directory.
const ARCHIVE_NAME: &str = "archive.tar";

/// Input for `romeka backup`.
#[derive(Debug, Clone)]
pub struct BackupOptions {
    /// The database. A server is refused: the server's own tools back it up.
    pub database: Location,
    /// Store root (`file_store.local.base_path`), the directory holding
    /// `original/` and the thumbnail types.
    pub store: PathBuf,
    /// The archive to write.
    pub out: PathBuf,
}

/// What a backup wrote.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackupReport {
    /// The archive path.
    pub archive: PathBuf,
    /// The archive's size in bytes.
    pub size: u64,
    /// The database copy's size in bytes.
    pub database_size: u64,
    /// How many store files the archive holds.
    pub store_files: usize,
    /// The sum of the store files' sizes.
    pub store_size: u64,
    /// The row counts the manifest records.
    pub counts: BTreeMap<String, i64>,
}

/// Write the archive for a database and a store.
///
/// # Errors
/// A [`CliError`] when the database or the store is missing, a media row names a
/// file the store lacks, the database cannot be exported, or the archive cannot
/// be written. A failed run leaves no archive at `out`.
pub async fn run(options: &BackupOptions) -> Result<BackupReport, CliError> {
    let database = match &options.database {
        Location::File(path) => path,
        Location::Url(_) => {
            let pool = options.database.open().await?;
            with_db!(&pool, |db| DatabaseRepo::new(db).snapshot_supported())?;
            // A server writes a snapshot on its own host, not beside this
            // archive, so even an engine that had one could not fill it.
            return Err(CliError::message(format!(
                "{} cannot be archived",
                options.database
            )));
        }
    };
    require_file(database, "database")?;
    require_dir(&options.store, "store")?;
    let staging = Staging::new_beside(&options.out, "romeka-backup")?;
    let report = build(database, options, &staging).await?;
    std::fs::rename(staging.path(ARCHIVE_NAME), &options.out)?;
    staging.retire();
    Ok(report)
}

async fn build(
    database: &Path,
    options: &BackupOptions,
    staging: &Staging,
) -> Result<BackupReport, CliError> {
    let exported = staging.path(EXPORT_NAME);
    export_database(database, &options.store, &exported).await?;
    let counts = count_export(&exported).await?;
    let files = collect_store_files(&options.store)?;

    let file = File::create(staging.path(ARCHIVE_NAME))?;
    let mut writer = TarWriter::new(BufWriter::new(file));
    let database = writer.add_file(DATABASE_ENTRY, &exported)?;
    let mut store_size = 0u64;
    let mut store_files = Vec::with_capacity(files.len());
    for found in &files {
        let digest = writer.add_file(&store_entry(&found.relative), &found.path)?;
        store_size += digest.size;
        store_files.push(StoreFile {
            path: found.relative.clone(),
            size: digest.size,
            sha256: digest.sha256,
        });
    }
    let manifest = Manifest {
        format: FORMAT.to_string(),
        format_version: FORMAT_VERSION,
        created: now_stamp(),
        version: VERSION.to_string(),
        commit: COMMIT.to_string(),
        counts: counts.clone(),
        database: DatabaseEntry {
            entry: DATABASE_ENTRY.to_string(),
            size: database.size,
            sha256: database.sha256,
        },
        store: StoreSection {
            checksum: store_checksum(&store_files),
            size: store_size,
            files: store_files,
        },
    };
    writer.add_bytes(MANIFEST_ENTRY, &manifest.to_json()?)?;
    let buffered = writer.finish()?;
    let file = buffered
        .into_inner()
        .map_err(|error| CliError::Io(error.into_error()))?;
    file.sync_all()?;

    Ok(BackupReport {
        archive: options.out.clone(),
        size: std::fs::metadata(staging.path(ARCHIVE_NAME))?.len(),
        database_size: database.size,
        store_files: manifest.store.files.len(),
        store_size,
        counts,
    })
}

/// Check the store against the media rows, then export the database.
async fn export_database(database: &Path, store: &Path, target: &Path) -> Result<(), CliError> {
    let pool = open_file(database).await?;
    verify_store_references(&pool, store).await?;
    DatabaseRepo::new(&pool).snapshot_into(target).await?;
    pool.close().await;
    Ok(())
}

/// Count the rows of the exported copy, not the live database, so the manifest
/// describes the bytes it carries.
async fn count_export(exported: &Path) -> Result<BTreeMap<String, i64>, CliError> {
    let pool = open_read_only(exported).await?;
    let counts = count_rows(&pool).await?;
    pool.close().await;
    Ok(counts)
}

fn require_file(path: &Path, what: &str) -> Result<(), CliError> {
    if path.is_file() {
        Ok(())
    } else {
        Err(CliError::message(format!(
            "no {what} file at {}",
            path.display()
        )))
    }
}

fn require_dir(path: &Path, what: &str) -> Result<(), CliError> {
    if path.is_dir() {
        Ok(())
    } else {
        Err(CliError::message(format!(
            "no {what} directory at {}",
            path.display()
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use omeka_db::ids::{ItemId, ItemSetId};
    use omeka_db::repos::{
        ItemFilter, ItemRepo, ItemSetRepo, MediaFilter, MediaRepo, Paging, ValueRepo,
    };
    use omeka_db::rows::{NewItem, NewItemSet, NewMedia, NewResource, NewValue};
    use omeka_media::file_render::ThumbnailType;
    use omeka_media::store::{FileStore, Store, StorePrefix, sha256_hex, storage_path};
    use omeka_media::thumbnailer::thumbnail_path;

    use crate::commands::restore::{self, RestoreOptions};
    use crate::testing::{TempDb, png_rgb, seeded_db};

    /// One item set with one item carrying a title and an identifier.
    async fn seed_item(db: &TempDb) -> (ItemSetId, ItemId, String) {
        let set = ItemSetRepo::new(&db.pool)
            .create(&NewItemSet::default())
            .await
            .expect("item set")
            .id();
        let items = ItemRepo::new(&db.pool);
        let item = items.create(&NewItem::default()).await.expect("item").id();
        items.add_to_item_set(item, set).await.expect("attach");
        let title = "Round Trip Title".to_string();
        let property = crate::terms::resolve_property(&db.db_pool, "dcterms:title")
            .await
            .expect("title property");
        ValueRepo::new(&db.pool)
            .create(&NewValue {
                resource_id: item.as_resource(),
                property_id: property,
                value_resource_id: None,
                value_annotation_id: None,
                r#type: "literal".to_string(),
                lang: None,
                value: Some(title.clone()),
                uri: None,
                is_public: true,
            })
            .await
            .expect("title value");
        (set, item, title)
    }

    /// One media row with its bytes in the store, as `import media` writes them.
    async fn seed_media(
        db: &TempDb,
        item: ItemId,
        store: &Path,
        name: &str,
        bytes: &[u8],
    ) -> String {
        let file_store = FileStore::new(store, "/files");
        let storage_id = omeka_media::store::new_storage_id().expect("storage id");
        let extension = name.rsplit_once('.').map(|(_, extension)| extension);
        let path = storage_path(StorePrefix::Original, &storage_id, extension);
        file_store.put(bytes, &path, None).expect("store bytes");
        let thumbnails = bytes.starts_with(&[0x89, b'P', b'N', b'G']);
        if thumbnails {
            for kind in ThumbnailType::ALL {
                file_store
                    .put(b"thumb", &thumbnail_path(&storage_id, kind), None)
                    .expect("store thumbnail");
            }
        }
        MediaRepo::new(&db.pool)
            .create(&NewMedia {
                resource: NewResource::default(),
                item_id: item,
                ingester: "upload".to_string(),
                renderer: "file".to_string(),
                data: None,
                source: Some(name.to_string()),
                media_type: Some(if thumbnails {
                    "image/png".to_string()
                } else {
                    "application/pdf".to_string()
                }),
                storage_id: Some(storage_id.clone()),
                extension: extension.map(str::to_string),
                sha256: Some(sha256_hex(bytes)),
                size: Some(bytes.len() as i64),
                has_original: true,
                has_thumbnails: thumbnails,
                position: None,
                lang: None,
                alt_text: None,
            })
            .await
            .expect("media row");
        storage_id
    }

    fn options(db: &TempDb, store: &Path, out: &Path) -> BackupOptions {
        BackupOptions {
            database: db.path.clone().into(),
            store: store.to_path_buf(),
            out: out.to_path_buf(),
        }
    }

    fn count_files(root: &Path) -> usize {
        collect_store_files(root).expect("collect").len()
    }

    /// A seeded database and a store with an original and three thumbnails round
    /// trip: the row counts match, the title survives, the store files come back
    /// byte for byte, and a second restore without `--force` is refused.
    #[tokio::test]
    async fn backup_and_restore_round_trip_the_database_and_the_store() {
        let db = seeded_db().await;
        let (_set, item, title) = seed_item(&db).await;
        let store = tempfile::tempdir().expect("store dir");
        let storage_id = seed_media(&db, item, store.path(), "photo.png", &png_rgb(64, 48)).await;
        seed_media(&db, item, store.path(), "doc.pdf", b"%PDF-1.4 body").await;

        let archive_dir = tempfile::tempdir().expect("archive dir");
        let archive = archive_dir.path().join("backup.tar");
        let report = run(&options(&db, store.path(), &archive))
            .await
            .expect("backup");

        assert!(archive.is_file());
        assert!(report.size > 0);
        assert_eq!(
            report.store_files, 5,
            "one original and three thumbnails, plus a pdf"
        );
        assert_eq!(report.counts["item"], 1);
        assert_eq!(report.counts["media"], 2);

        let target = tempfile::tempdir().expect("target dir");
        let target_db = target.path().join("romeka.db");
        let target_store = target.path().join("files");
        let restore_options = RestoreOptions {
            database: target_db.clone(),
            store: target_store.clone(),
            input: archive.clone(),
            force: false,
        };
        let restored = restore::run(&restore_options).await.expect("restore");

        assert_eq!(restored.counts, report.counts);
        assert_eq!(restored.store_files, report.store_files);
        let expected = count_files(store.path());
        assert_eq!(count_files(&target_store), expected);

        let pool = open_file(&target_db).await.expect("open restored");
        let db = omeka_db::DbPool::from(pool.clone());
        let items = ItemRepo::new(&pool)
            .list(&ItemFilter::default(), &Paging::default())
            .await
            .expect("list items");
        assert_eq!(items.len(), 1);
        let property = crate::terms::resolve_property(&db, "dcterms:title")
            .await
            .expect("title property");
        let values = ValueRepo::new(&pool)
            .list_for_property(items[0].resource.id, property)
            .await
            .expect("list values");
        assert_eq!(values[0].value.as_deref(), Some(title.as_str()));
        assert_eq!(
            MediaRepo::new(&pool)
                .list(&MediaFilter::default(), &Paging::default())
                .await
                .expect("list media")
                .len(),
            2
        );
        pool.close().await;

        let original = storage_path(StorePrefix::Original, &storage_id, Some("png"));
        let stored = std::fs::read(store.path().join(&original)).expect("source bytes");
        assert_eq!(
            std::fs::read(target_store.join(&original)).expect("restored bytes"),
            stored
        );

        let refused = restore::run(&restore_options)
            .await
            .expect_err("a second restore is refused");
        assert!(refused.to_string().contains("--force"), "got {refused}");

        let forced = RestoreOptions {
            force: true,
            ..restore_options.clone()
        };
        restore::run(&forced).await.expect("forced restore");
    }

    /// `--force` mirrors the store: a file the archive does not hold is removed.
    #[tokio::test]
    async fn a_forced_restore_removes_store_files_the_archive_lacks() {
        let db = seeded_db().await;
        let (_set, item, _title) = seed_item(&db).await;
        let store = tempfile::tempdir().expect("store dir");
        seed_media(&db, item, store.path(), "doc.pdf", b"%PDF-1.4 body").await;

        let archive_dir = tempfile::tempdir().expect("archive dir");
        let archive = archive_dir.path().join("backup.tar");
        run(&options(&db, store.path(), &archive))
            .await
            .expect("backup");

        let target = tempfile::tempdir().expect("target dir");
        let target_store = target.path().join("files");
        std::fs::create_dir_all(target_store.join("original")).expect("create store");
        std::fs::write(target_store.join("original/orphan.jpg"), b"orphan").expect("orphan");
        std::fs::write(target_store.join("stale.txt"), b"stale").expect("stale");

        restore::run(&RestoreOptions {
            database: target.path().join("romeka.db"),
            store: target_store.clone(),
            input: archive,
            force: true,
        })
        .await
        .expect("restore");

        assert!(!target_store.join("original/orphan.jpg").exists());
        assert!(!target_store.join("stale.txt").exists());
        let files = collect_store_files(&target_store).expect("collect");
        assert_eq!(files.len(), 1);
    }

    /// A media row whose file is absent stops the backup, and no archive is left
    /// behind.
    #[tokio::test]
    async fn a_missing_store_file_refuses_the_backup() {
        let db = seeded_db().await;
        let (_set, item, _title) = seed_item(&db).await;
        let store = tempfile::tempdir().expect("store dir");
        let storage_id = seed_media(&db, item, store.path(), "doc.pdf", b"body").await;
        let original = storage_path(StorePrefix::Original, &storage_id, Some("pdf"));
        std::fs::remove_file(store.path().join(&original)).expect("remove original");

        let archive_dir = tempfile::tempdir().expect("archive dir");
        let archive = archive_dir.path().join("backup.tar");
        let error = run(&options(&db, store.path(), &archive))
            .await
            .expect_err("the backup refuses");

        assert!(error.to_string().contains(&original), "got {error}");
        assert!(!archive.exists(), "no archive on a failed run");
        assert_eq!(
            std::fs::read_dir(archive_dir.path())
                .expect("read dir")
                .count(),
            0,
            "the staging directory is removed"
        );
    }

    /// The manifest names the commit, and the counts are the exported copy's.
    #[tokio::test]
    async fn the_manifest_names_the_commit_the_counts_and_the_checksums() {
        let db = seeded_db().await;
        let store = tempfile::tempdir().expect("store dir");
        let archive_dir = tempfile::tempdir().expect("archive dir");
        let archive = archive_dir.path().join("backup.tar");
        run(&options(&db, store.path(), &archive))
            .await
            .expect("backup");

        let manifest = read_manifest(&archive);
        assert_eq!(manifest.format, FORMAT);
        assert_eq!(manifest.format_version, FORMAT_VERSION);
        assert_eq!(manifest.version, VERSION);
        assert!(!manifest.commit.is_empty(), "the manifest names the commit");
        assert_eq!(manifest.counts["vocabulary"], 4);
        assert_eq!(manifest.counts["item"], 0);
        assert_eq!(manifest.database.sha256.len(), 64);
        assert_eq!(manifest.store.files.len(), 0);
        assert_eq!(manifest.store.checksum, store_checksum(&[]));
        assert!(manifest.validate().is_ok());
    }

    /// Read the manifest back out of an archive with the archive reader.
    fn read_manifest(archive: &Path) -> Manifest {
        let file = File::open(archive).expect("open archive");
        let mut reader = crate::tar::TarReader::new(std::io::BufReader::new(file));
        while let Some(entry) = reader.next_entry().expect("next entry") {
            let mut bytes = Vec::new();
            reader.copy_entry(&mut bytes).expect("copy entry");
            if entry.name == MANIFEST_ENTRY {
                return Manifest::from_json(&bytes).expect("parse manifest");
            }
        }
        panic!("the archive holds no manifest");
    }

    /// An extension-less media row still names a file the check finds.
    #[tokio::test]
    async fn a_store_file_without_an_extension_is_referenced() {
        let db = seeded_db().await;
        let (_set, item, _title) = seed_item(&db).await;
        let store = tempfile::tempdir().expect("store dir");
        let row = MediaRepo::new(&db.pool)
            .list(&MediaFilter::default(), &Paging::default())
            .await
            .expect("list");
        assert!(row.is_empty());
        let storage_id = "a".repeat(40);
        FileStore::new(store.path(), "/files")
            .put(
                b"raw",
                &storage_path(StorePrefix::Original, &storage_id, None),
                None,
            )
            .expect("store bytes");
        MediaRepo::new(&db.pool)
            .create(&NewMedia {
                resource: NewResource::default(),
                item_id: item,
                ingester: "upload".to_string(),
                renderer: "file".to_string(),
                data: None,
                source: Some("raw".to_string()),
                media_type: None,
                storage_id: Some(storage_id),
                extension: None,
                sha256: None,
                size: None,
                has_original: true,
                has_thumbnails: false,
                position: None,
                lang: None,
                alt_text: None,
            })
            .await
            .expect("media row");

        let archive_dir = tempfile::tempdir().expect("archive dir");
        let archive = archive_dir.path().join("backup.tar");
        let report = run(&options(&db, store.path(), &archive))
            .await
            .expect("backup");
        assert_eq!(report.store_files, 1);
    }
}
