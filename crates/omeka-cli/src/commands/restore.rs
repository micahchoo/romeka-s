//! `romeka restore`: write an archive back into a database and a store.
//!
//! Nothing touches the target until the archive has been read, its manifest
//! parsed, and every checksum re-computed against the bytes on disk. The whole
//! archive lands in a staging directory beside the target first, so a truncated
//! or forged archive leaves the target untouched.
//!
//! An occupied target is refused unless `--force`. A forced restore overwrites
//! the database and mirrors the store, which removes a store file the archive
//! does not hold. Stale `-wal` and `-shm` sidecars beside the database are
//! removed either way, because SQLite would replay a stale log over the restored
//! file.

use std::collections::{BTreeMap, BTreeSet};
use std::fs::File;
use std::io::{BufReader, Write};
use std::path::{Component, Path, PathBuf};

use omeka_db::open_file;

use crate::archive::{
    DATABASE_ENTRY, MANIFEST_ENTRY, Manifest, Staging, StoreFileRef, collect_store_files,
    count_rows, store_checksum, store_relative, verify_store_references,
};
use crate::error::CliError;
use crate::tar::{TarReader, file_digest};

/// The staging name for the extracted database.
const STAGED_DATABASE: &str = "database.sqlite";
/// The staging directory for the extracted store.
const STAGED_STORE: &str = "store";
/// The suffix a sidecar journal carries beside a database file.
const WAL_SUFFIX: &str = "-wal";
/// The suffix the shared-memory sidecar carries beside a database file.
const SHM_SUFFIX: &str = "-shm";

/// Input for `romeka restore`.
#[derive(Debug, Clone)]
pub struct RestoreOptions {
    /// The SQLite database file to write.
    pub database: PathBuf,
    /// The store root to write.
    pub store: PathBuf,
    /// The archive to read.
    pub input: PathBuf,
    /// Overwrite an occupied database or store.
    pub force: bool,
}

/// What a restore wrote.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RestoreReport {
    /// Row counts, measured from the restored database.
    pub counts: BTreeMap<String, i64>,
    /// How many store files were written.
    pub store_files: usize,
    /// `Module::VERSION` of the port that wrote the archive.
    pub version: String,
    /// The commit the archive's binary was built from.
    pub commit: String,
}

/// Restore an archive into a database and a store.
///
/// # Errors
/// A [`CliError`] when the archive is missing, malformed, fails a checksum, or
/// holds a path that would escape the target; when the target is occupied and
/// `force` is false; or when a write fails.
pub async fn run(options: &RestoreOptions) -> Result<RestoreReport, CliError> {
    if !options.input.is_file() {
        return Err(CliError::message(format!(
            "no archive at {}",
            options.input.display()
        )));
    }
    if !options.force
        && let Some(reason) = occupied(&options.database, &options.store)
    {
        return Err(CliError::message(format!(
            "{reason}; pass --force to overwrite"
        )));
    }
    let staging = staging_beside(&options.store)?;
    let manifest = extract(&options.input, staging.directory())?;
    verify(staging.directory(), &manifest)?;
    install(staging.directory(), options)?;
    let counts = verify_restored(options).await?;
    staging.retire();
    Ok(RestoreReport {
        counts,
        store_files: manifest.store.files.len(),
        version: manifest.version,
        commit: manifest.commit,
    })
}

/// A staging directory beside the store root. The restore target need not exist,
/// so its parent is created first: an operator restoring into a new directory
/// names the directory, not its parents.
fn staging_beside(store: &Path) -> Result<Staging, CliError> {
    let parent = store
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    std::fs::create_dir_all(parent)?;
    Staging::new_beside(store, "romeka-restore")
}

/// A reason the target cannot be written, or `None` when it is empty.
fn occupied(database: &Path, store: &Path) -> Option<String> {
    if database.exists() {
        return Some(format!(
            "the database path {} already exists",
            database.display()
        ));
    }
    if store.exists() && collect_store_files(store).is_ok_and(|files| !files.is_empty()) {
        return Some(format!("the store {} is not empty", store.display()));
    }
    None
}

/// Read every entry into the staging directory and return the manifest.
fn extract(archive: &Path, staging: &Path) -> Result<Manifest, CliError> {
    let file = File::open(archive)?;
    let mut reader = TarReader::new(BufReader::new(file));
    let mut manifest = None;
    let mut seen = BTreeSet::new();
    while let Some(entry) = reader.next_entry()? {
        if !seen.insert(entry.name.clone()) {
            return Err(CliError::message(format!(
                "the archive holds \"{}\" twice",
                entry.name
            )));
        }
        if entry.name == MANIFEST_ENTRY {
            let mut bytes = Vec::new();
            reader.copy_entry(&mut bytes)?;
            manifest = Some(Manifest::from_json(&bytes)?);
        } else if entry.name == DATABASE_ENTRY {
            write_entry(&mut reader, &staging.join(STAGED_DATABASE))?;
        } else if let Some(relative) = store_relative(&entry.name) {
            let relative = safe_relative(relative, &entry.name)?;
            write_entry(&mut reader, &staging.join(STAGED_STORE).join(relative))?;
        } else {
            return Err(CliError::message(format!(
                "the archive holds the unexpected entry \"{}\"",
                entry.name
            )));
        }
    }
    let manifest =
        manifest.ok_or_else(|| CliError::message("the archive holds no manifest.json"))?;
    manifest.validate().map_err(CliError::message)?;
    Ok(manifest)
}

fn write_entry<R: std::io::Read>(
    reader: &mut TarReader<R>,
    destination: &Path,
) -> Result<(), CliError> {
    if let Some(parent) = destination.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut file = File::create(destination)?;
    reader.copy_entry(&mut file)?;
    file.flush()?;
    Ok(())
}

/// Reject an absolute path, a `..` component, and anything but plain names, so
/// an entry can never land outside the staging directory.
fn safe_relative<'a>(relative: &'a str, entry: &str) -> Result<&'a Path, CliError> {
    let path = Path::new(relative);
    if path
        .components()
        .all(|component| matches!(component, Component::Normal(_)))
    {
        Ok(path)
    } else {
        Err(CliError::message(format!(
            "the archive entry \"{entry}\" is not a plain relative path"
        )))
    }
}

/// Re-compute every checksum against the extracted bytes before anything is
/// installed.
fn verify(staging: &Path, manifest: &Manifest) -> Result<(), CliError> {
    let database = staging.join(STAGED_DATABASE);
    check_digest(&database, manifest.database.size, &manifest.database.sha256)?;
    for file in &manifest.store.files {
        let relative = safe_relative(&file.path, &file.path)?;
        let path = staging.join(STAGED_STORE).join(relative);
        check_digest(&path, file.size, &file.sha256)?;
    }
    let staged = collect_store_files(&staging.join(STAGED_STORE))?;
    let named: BTreeSet<&str> = manifest
        .store
        .files
        .iter()
        .map(|file| file.path.as_str())
        .collect();
    for found in &staged {
        if !named.contains(found.relative.as_str()) {
            return Err(CliError::message(format!(
                "the archive holds store/{} that the manifest does not name",
                found.relative
            )));
        }
    }
    if staged.len() != manifest.store.files.len() {
        return Err(CliError::message(format!(
            "the manifest names {} store files, the archive holds {}",
            manifest.store.files.len(),
            staged.len()
        )));
    }
    if store_checksum(&manifest.store.files) != manifest.store.checksum {
        return Err(CliError::message(
            "the manifest's store checksum does not match its own file list",
        ));
    }
    Ok(())
}

fn check_digest(path: &Path, size: u64, sha256: &str) -> Result<(), CliError> {
    let digest = file_digest(path).map_err(|error| match error {
        crate::tar::TarError::Io(source) => CliError::Io(source),
        other => CliError::message(format!("cannot read {}: {other}", path.display())),
    })?;
    if digest.size != size || digest.sha256 != sha256 {
        return Err(CliError::message(format!(
            "the archive's {} does not match its manifest ({} bytes and {}, expected {size} and {sha256})",
            path.display(),
            digest.size,
            digest.sha256
        )));
    }
    Ok(())
}

/// Write the staged database and store into the target.
fn install(staging: &Path, options: &RestoreOptions) -> Result<(), CliError> {
    create_parent(&options.database)?;
    let staged_database = staging.join(STAGED_DATABASE);
    let temporary = unique_sibling(&options.database, "restore");
    std::fs::copy(&staged_database, &temporary)?;
    for suffix in [WAL_SUFFIX, SHM_SUFFIX] {
        let sidecar = sidecar(&options.database, suffix);
        if sidecar.exists() {
            std::fs::remove_file(&sidecar)?;
        }
    }
    std::fs::rename(&temporary, &options.database)?;

    create_parent(&options.store)?;
    std::fs::create_dir_all(&options.store)?;
    if options.force {
        remove_extra_files(&options.store, &staging.join(STAGED_STORE))?;
    }
    copy_tree(&staging.join(STAGED_STORE), &options.store)?;
    Ok(())
}

/// Open the restored database and prove the pair is usable: every media row's
/// file is in the restored store, and the row counts are what the manifest says.
async fn verify_restored(options: &RestoreOptions) -> Result<BTreeMap<String, i64>, CliError> {
    let pool = open_file(&options.database).await?;
    verify_store_references(&pool, &options.store).await?;
    let counts = count_rows(&pool).await?;
    pool.close().await;
    Ok(counts)
}

fn create_parent(path: &Path) -> Result<(), CliError> {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent)?;
    }
    Ok(())
}

/// Remove every file under `target` that `source` does not hold, then every
/// directory left empty, so a forced restore mirrors the archive.
fn remove_extra_files(target: &Path, source: &Path) -> Result<(), CliError> {
    let wanted: BTreeSet<String> = collect_store_files(source)?
        .into_iter()
        .map(|file| file.relative)
        .collect();
    for found in collect_store_files(target)? {
        if !wanted.contains(&found.relative) {
            std::fs::remove_file(&found.path)?;
        }
    }
    prune_empty_directories(target)?;
    Ok(())
}

fn copy_tree(source: &Path, target: &Path) -> Result<(), CliError> {
    for StoreFileRef { relative, path } in collect_store_files(source)? {
        let destination = target.join(&relative);
        if let Some(parent) = destination.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::copy(&path, &destination)?;
    }
    Ok(())
}

/// Remove empty directories under `root`, deepest first. Returns whether `root`
/// itself is empty.
fn prune_empty_directories(root: &Path) -> Result<bool, CliError> {
    let mut empty = true;
    for entry in std::fs::read_dir(root)? {
        let entry = entry?;
        let path = entry.path();
        if entry.file_type()?.is_dir() {
            if prune_empty_directories(&path)? {
                std::fs::remove_dir(&path)?;
            } else {
                empty = false;
            }
        } else {
            empty = false;
        }
    }
    Ok(empty)
}

fn sidecar(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path.as_os_str().to_os_string();
    name.push(suffix);
    PathBuf::from(name)
}

fn unique_sibling(path: &Path, tag: &str) -> PathBuf {
    let name = path.file_name().map_or_else(
        || "romeka.db".to_string(),
        |name| name.to_string_lossy().into_owned(),
    );
    path.with_file_name(format!(".{name}.{tag}.{}.tmp", std::process::id()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::archive::{FORMAT, FORMAT_VERSION, StoreFile, StoreSection};
    use crate::tar::TarWriter;

    /// A manifest for an archive whose bytes the test controls.
    fn manifest(database_sha: &str, files: Vec<StoreFile>, checksum: &str) -> Manifest {
        let size = files.iter().map(|file| file.size).sum();
        Manifest {
            format: FORMAT.to_string(),
            format_version: FORMAT_VERSION,
            created: "2026-10-17 10:00:00".to_string(),
            version: "4.3.0-alpha".to_string(),
            commit: "abc".to_string(),
            counts: BTreeMap::new(),
            database: crate::archive::DatabaseEntry {
                entry: DATABASE_ENTRY.to_string(),
                size: 7,
                sha256: database_sha.to_string(),
            },
            store: StoreSection {
                files,
                size,
                checksum: checksum.to_string(),
            },
        }
    }

    fn write_archive(path: &Path, entries: &[(&str, &[u8])]) {
        let mut writer = TarWriter::new(Vec::new());
        for (name, bytes) in entries {
            writer.add_bytes(name, bytes).expect("add entry");
        }
        let archive = writer.finish().expect("finish");
        std::fs::write(path, archive).expect("write archive");
    }

    fn empty_target() -> (tempfile::TempDir, RestoreOptions) {
        let directory = tempfile::tempdir().expect("target dir");
        let options = RestoreOptions {
            database: directory.path().join("romeka.db"),
            store: directory.path().join("files"),
            input: directory.path().join("archive.tar"),
            force: false,
        };
        (directory, options)
    }

    #[test]
    fn staging_creates_the_target_directory_that_does_not_exist_yet() {
        let root = tempfile::tempdir().expect("dir");
        let store = root.path().join("new/place/files");
        let staging = staging_beside(&store).expect("staging");
        assert!(staging.directory().is_dir());
        assert!(store.parent().expect("parent").is_dir());
        staging.retire();
    }

    #[test]
    fn a_parent_component_in_an_entry_is_refused() {
        let error = safe_relative("../escape", "store/../escape").expect_err("refused");
        assert!(
            error.to_string().contains("plain relative path"),
            "got {error}"
        );
        assert!(safe_relative("original/a.jpg", "x").is_ok());
        assert!(safe_relative("/etc/passwd", "x").is_err());
    }

    #[tokio::test]
    async fn a_forged_checksum_refuses_the_restore() {
        let (_directory, options) = empty_target();
        let bytes = b"payload";
        let manifest = manifest(
            &crate::tar::sha256_hex(b"database"),
            vec![StoreFile {
                path: "original/a.jpg".to_string(),
                size: bytes.len() as u64,
                sha256: "00".repeat(32),
            }],
            "cc",
        );
        write_archive(
            &options.input,
            &[
                (DATABASE_ENTRY, b"database"),
                ("store/original/a.jpg", bytes),
                (MANIFEST_ENTRY, &manifest.to_json().expect("json")),
            ],
        );

        let error = run(&options).await.expect_err("refused");
        assert!(
            error.to_string().contains("does not match its manifest"),
            "got {error}"
        );
        assert!(!options.database.exists(), "nothing is installed");
        assert!(!options.store.exists());
    }

    #[tokio::test]
    async fn an_entry_that_escapes_the_store_is_refused() {
        let (_directory, options) = empty_target();
        let manifest = manifest(
            &crate::tar::sha256_hex(b"database"),
            Vec::new(),
            &store_checksum(&[]),
        );
        write_archive(
            &options.input,
            &[
                (DATABASE_ENTRY, b"database"),
                ("store/../escape.txt", b"escape"),
                (MANIFEST_ENTRY, &manifest.to_json().expect("json")),
            ],
        );

        let error = run(&options).await.expect_err("refused");
        assert!(
            error.to_string().contains("plain relative path"),
            "got {error}"
        );
        assert!(!options.database.exists());
    }

    #[tokio::test]
    async fn an_unknown_entry_is_refused() {
        let (_directory, options) = empty_target();
        let manifest = manifest(
            &crate::tar::sha256_hex(b"database"),
            Vec::new(),
            &store_checksum(&[]),
        );
        write_archive(
            &options.input,
            &[
                (DATABASE_ENTRY, b"database"),
                ("extra.bin", b"extra"),
                (MANIFEST_ENTRY, &manifest.to_json().expect("json")),
            ],
        );

        let error = run(&options).await.expect_err("refused");
        assert!(
            error.to_string().contains("unexpected entry"),
            "got {error}"
        );
    }

    #[tokio::test]
    async fn an_archive_without_a_manifest_is_refused() {
        let (_directory, options) = empty_target();
        write_archive(&options.input, &[(DATABASE_ENTRY, b"database")]);
        let error = run(&options).await.expect_err("refused");
        assert!(error.to_string().contains("no manifest"), "got {error}");
    }

    #[test]
    fn occupied_names_the_database_then_the_store() {
        let directory = tempfile::tempdir().expect("dir");
        let database = directory.path().join("romeka.db");
        let store = directory.path().join("files");
        std::fs::write(&database, b"db").expect("write db");
        let reason = occupied(&database, &store).expect("occupied");
        assert!(reason.contains("already exists"), "got {reason}");

        std::fs::remove_file(&database).expect("remove db");
        assert!(occupied(&database, &store).is_none());
        std::fs::create_dir_all(&store).expect("create store");
        assert!(
            occupied(&database, &store).is_none(),
            "an empty store is free"
        );
        std::fs::write(store.join("orphan"), b"x").expect("write");
        let reason = occupied(&database, &store).expect("occupied");
        assert!(reason.contains("not empty"), "got {reason}");
    }

    #[test]
    fn sidecars_sit_beside_the_database_file() {
        let path = Path::new("/srv/romeka/romeka.db");
        assert_eq!(
            sidecar(path, WAL_SUFFIX),
            PathBuf::from("/srv/romeka/romeka.db-wal")
        );
        assert_eq!(
            sidecar(path, SHM_SUFFIX),
            PathBuf::from("/srv/romeka/romeka.db-shm")
        );
    }

    #[test]
    fn prune_removes_only_empty_directories() {
        let directory = tempfile::tempdir().expect("dir");
        std::fs::create_dir_all(directory.path().join("empty/inner")).expect("create");
        std::fs::create_dir_all(directory.path().join("full")).expect("create");
        std::fs::write(directory.path().join("full/keep"), b"keep").expect("write");
        assert!(!prune_empty_directories(directory.path()).expect("prune"));
        assert!(!directory.path().join("empty").exists());
        assert!(directory.path().join("full/keep").exists());
    }
}
