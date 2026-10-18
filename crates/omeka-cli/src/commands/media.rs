//! `omeka import media`: copy files into the store and link them to items.
//!
//! Upstream ingests a media file through `Media/Ingester/Upload.php`, which sets
//! `media.source` from the uploaded file name (`Ingester/Upload.php:61`) and hands
//! the file to `File/TempFile.php::mediaIngestFile`. That method stores the
//! original under `files/original/<storage_id>.<extension>` and writes the file
//! facts onto the row (`File/TempFile.php:420`), and `Entity/Media.php:223` reads
//! the stored file name back as `<storage_id>.<extension>`. This command is the
//! same path without a browser: it reads each file from disk, stores it through
//! `omeka-media`'s [`ingest_file`], and inserts the row through `omeka-db`'s
//! `MediaRepo`.
//!
//! Two inputs, one output. `--source` walks a directory and links each media file
//! to the item whose `dcterms:identifier` value matches the file's stem.
//! `--manifest` reads the acceptance unit's `media.json` shape
//! (`verification/acceptance/media.py`), so the 100 file sample imports
//! unchanged:
//!
//! ```json
//! {"sample": [{"identifier": "BHC_006_AF_001", "source": "BHC006_GMT_AF.xlsx:12",
//!              "file": "Artifacts/foo.jpg"}]}
//! ```
//!
//! `identifier` is the join key: the item whose `dcterms:identifier` literal it
//! equals. `source` is provenance for the reader and does not enter the row; the
//! row's `media.source` is the file name, as upstream stores it. `file` resolves
//! against `--source` when given, else against the manifest's directory. An entry
//! may carry `item`, and `--item ID` forces one item for every file; both skip the
//! identifier lookup.
//!
//! A second run is a no-op. A file already imported onto its item (same
//! `media.item_id`, same `media.source`) is reported as skipped, and neither row
//! nor bytes are written again. The dedupe key is the row's own identity, so two
//! files that share a base name and land on the same item collapse to one.
//!
//! Thumbnails for the decodable image types come from `omeka-media`'s
//! [`ImageThumbnailer`], which writes the three sizes through the store, so
//! `has_thumbnails` is true for a file whose bytes decoded as an image.
//! `media_type` comes from the file extension (upstream sniffs the content with
//! `finfo`); the table lives here because the CLI is the boundary that knows the
//! source name.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use chrono::NaiveDateTime;
use serde::Deserialize;
use sqlx::SqlitePool;

use omeka_core::ids::{ItemId as CoreItemId, MediaId as CoreMediaId};
use omeka_core::resource::{Media as CoreMedia, Timestamp};
use omeka_db::ids::{ItemId, ItemSetId, PropertyId};
use omeka_db::open_file;
use omeka_db::repos::{ItemFilter, ItemRepo, MediaFilter, MediaRepo, Paging, ValueRepo};
use omeka_db::rows::{NewMedia, NewResource};
use omeka_media::data::MediaData;
use omeka_media::http::{FetchError, HttpFetch, HttpResponse};
use omeka_media::ingest::{IngestEnv, IngestFile, IngestFlags, MediaDraft, Purifier, ingest_file};
use omeka_media::oembed::{Oembed, OembedAllowList};
use omeka_media::store::FileStore;
use omeka_media::thumbnailer::ImageThumbnailer;

use crate::error::CliError;

/// The join term: the item's identifier literal names the file.
const IDENTIFIER_TERM: &str = "dcterms:identifier";

/// The ingester this command drives (`application/config/module.config.php:860`).
const INGESTER: &str = "upload";

/// Media file extensions, matching `verification/acceptance/media.py`.
const MEDIA_EXTENSIONS: &[&str] = &[
    "jpg", "jpeg", "png", "tif", "tiff", "nef", "mp4", "mov", "mp3", "wav", "pdf",
];

/// Input for `omeka import media`.
#[derive(Debug, Clone)]
pub struct MediaOptions {
    /// SQLite database file.
    pub database: PathBuf,
    /// Directory holding the source files. Optional when `manifest` is set.
    pub source: Option<PathBuf>,
    /// A `media.json` manifest.
    pub manifest: Option<PathBuf>,
    /// Store root (`file_store.local.base_path`). Defaults to `files` beside the database.
    pub store: Option<PathBuf>,
    /// Restrict the identifier lookup to this item set.
    pub item_set: Option<u64>,
    /// Link every file to this item, ignoring the identifier.
    pub item: Option<u64>,
}

/// What happened to one file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaStatus {
    /// A row and a stored file were written.
    Imported,
    /// A row for this item and source already exists; nothing was written.
    Skipped,
    /// The file did not import; `message` says why.
    Failed,
}

/// The outcome of one file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MediaResult {
    /// The file, as it appeared in the manifest or relative to the source root.
    pub file: String,
    /// The item the file was linked to, when it was resolved.
    pub item: Option<u64>,
    /// The new media id, when a row was written.
    pub media: Option<u64>,
    /// What happened.
    pub status: MediaStatus,
    /// Whether the three thumbnails were written for this file.
    pub thumbnails: bool,
    /// The failure reason, when the status is [`MediaStatus::Failed`].
    pub message: Option<String>,
}

/// The outcome of a media import.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MediaReport {
    /// One result per input file, in input order.
    pub results: Vec<MediaResult>,
    /// Files that were imported.
    pub imported: usize,
    /// Files already imported.
    pub skipped: usize,
    /// Files that failed.
    pub failed: usize,
    /// Imported files whose three thumbnails were written.
    pub thumbnails: usize,
}

/// One file to import, with the join information the input supplied.
struct Planned {
    /// The path to read.
    file: PathBuf,
    /// The path as the report shows it.
    display: String,
    /// The `dcterms:identifier` to join on; `None` uses the file stem.
    identifier: Option<String>,
    /// An explicit item id, from the manifest entry or `--item`.
    item: Option<u64>,
}

/// The `media.json` document `media.py` writes; unknown keys are ignored.
#[derive(Debug, Deserialize)]
struct MediaManifest {
    #[serde(default)]
    sample: Vec<ManifestEntry>,
}

/// One `sample` entry. The `source` key `media.py` writes is provenance; serde
/// drops it, and the row's `media.source` is the file name instead.
#[derive(Debug, Clone, Deserialize)]
struct ManifestEntry {
    #[serde(default)]
    identifier: Option<String>,
    file: String,
    #[serde(default)]
    item: Option<u64>,
}

/// Import media files and link each to its item.
///
/// # Errors
/// A [`CliError`] when neither input is given, a file cannot be read, the database
/// cannot be opened, or the `dcterms:identifier` property is missing. A file that
/// cannot be stored or has no item is reported in the [`MediaReport`], not here.
pub async fn run(options: &MediaOptions) -> Result<MediaReport, CliError> {
    let planned = plan(options)?;
    let pool = open_file(&options.database).await?;
    let property = crate::terms::resolve_property(&pool, IDENTIFIER_TERM)
        .await
        .map_err(CliError::message)?;
    let items = ItemIndex::build(&pool, options.item_set.map(ItemSetId::from), property).await?;
    let mut existing = imported_media(&pool).await?;

    let store = FileStore::new(
        store_root(&options.database, options.store.as_deref()),
        "/files",
    );
    let renderer = omeka_media::IngesterRegistry::with_defaults()
        .get(INGESTER)
        .renderer();
    let network = NoNetwork;
    let purifier = Passthrough;
    let thumbnailer = ImageThumbnailer;
    let oembed = Oembed::new(OembedAllowList::shipped());
    let env = IngestEnv {
        store: &store,
        http: &network,
        purifier: &purifier,
        thumbnailer: &thumbnailer,
        oembed: &oembed,
    };

    let mut report = MediaReport::default();
    for planned in &planned {
        let result = import_one(
            &pool,
            &env,
            renderer,
            &items,
            &mut existing,
            planned,
            options.item,
        )
        .await;
        match result.status {
            MediaStatus::Imported => report.imported += 1,
            MediaStatus::Skipped => report.skipped += 1,
            MediaStatus::Failed => report.failed += 1,
        }
        if result.thumbnails {
            report.thumbnails += 1;
        }
        report.results.push(result);
    }
    Ok(report)
}

/// Turn the inputs into the list of files to import.
fn plan(options: &MediaOptions) -> Result<Vec<Planned>, CliError> {
    match (&options.manifest, &options.source) {
        (Some(manifest), source) => plan_from_manifest(manifest, source.as_deref()),
        (None, Some(source)) => plan_from_directory(source, options.item),
        (None, None) => Err(CliError::message(
            "give --source DIR or --manifest FILE.json",
        )),
    }
}

fn plan_from_manifest(manifest: &Path, source: Option<&Path>) -> Result<Vec<Planned>, CliError> {
    let text = std::fs::read_to_string(manifest)?;
    let base = source.map_or_else(
        || manifest.parent().unwrap_or(Path::new(".")).to_path_buf(),
        Path::to_path_buf,
    );
    let entries = parse_manifest(&text)?;
    Ok(entries
        .into_iter()
        .map(|entry| Planned {
            file: base.join(&entry.file),
            display: entry.file,
            identifier: entry.identifier,
            item: entry.item,
        })
        .collect())
}

fn plan_from_directory(root: &Path, item: Option<u64>) -> Result<Vec<Planned>, CliError> {
    let mut files = Vec::new();
    collect_media_files(root, &mut files)?;
    files.sort();
    Ok(files
        .into_iter()
        .map(|file| {
            let display = file
                .strip_prefix(root)
                .unwrap_or(&file)
                .to_string_lossy()
                .into_owned();
            Planned {
                file,
                display,
                identifier: None,
                item,
            }
        })
        .collect())
}

/// Parse the `media.json` document. Empty or missing `sample` is an error here
/// rather than a silent no-op.
fn parse_manifest(text: &str) -> Result<Vec<ManifestEntry>, CliError> {
    let manifest: MediaManifest = serde_json::from_str(text)?;
    if manifest.sample.is_empty() {
        return Err(CliError::message("the manifest holds no sample entries"));
    }
    Ok(manifest.sample)
}

/// Walk a directory for media files, skipping dot directories.
fn collect_media_files(dir: &Path, out: &mut Vec<PathBuf>) -> Result<(), CliError> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        if path.is_dir() {
            if !name.starts_with('.') {
                collect_media_files(&path, out)?;
            }
        } else if is_media_name(&name) {
            out.push(path);
        }
    }
    Ok(())
}

fn is_media_name(name: &str) -> bool {
    extension_of(name).is_some_and(|extension| MEDIA_EXTENSIONS.contains(&extension.as_str()))
}

/// The media type for a source name, mirroring `TempFile::getMediaType`.
///
/// Upstream sniffs the content with `finfo` and then applies its alias table. The
/// CLI knows only the source name, so it maps the extension. A name with no
/// usable extension falls back to `application/octet-stream`.
fn detect_media_type(source_name: &str) -> &'static str {
    let Some(extension) = extension_of(source_name) else {
        return "application/octet-stream";
    };
    match extension.as_str() {
        "jpg" | "jpeg" | "jpe" => "image/jpeg",
        "png" => "image/png",
        "gif" => "image/gif",
        "tif" | "tiff" | "nef" => "image/tiff",
        "webp" => "image/webp",
        "pdf" => "application/pdf",
        "mp3" => "audio/mpeg",
        "wav" => "audio/x-wav",
        "mp4" => "video/mp4",
        "mov" => "video/quicktime",
        "avi" => "video/x-msvideo",
        "ogg" => "audio/ogg",
        "txt" => "text/plain",
        _ => "application/octet-stream",
    }
}

fn extension_of(name: &str) -> Option<String> {
    name.rsplit_once('.')
        .map(|(_, extension)| extension.to_ascii_lowercase())
        .filter(|extension| !extension.is_empty())
}

/// The store root: the explicit `--store`, else `files` beside the database.
pub(crate) fn store_root(database: &Path, store: Option<&Path>) -> PathBuf {
    store.map_or_else(
        || {
            let parent = database
                .parent()
                .filter(|parent| !parent.as_os_str().is_empty())
                .unwrap_or(Path::new("."));
            parent.join("files")
        },
        Path::to_path_buf,
    )
}

/// The `(item_id, media.source)` pairs a previous run wrote.
async fn imported_media(pool: &SqlitePool) -> Result<BTreeSet<(u64, String)>, CliError> {
    let rows = MediaRepo::new(pool)
        .list(&MediaFilter::default(), &Paging::default())
        .await?;
    Ok(rows
        .into_iter()
        .filter_map(|row| row.source.map(|source| (row.item_id.0, source)))
        .collect())
}

/// Import one planned file. Failures come back in the result, not as errors, so
/// one bad file does not stop the run.
async fn import_one(
    pool: &SqlitePool,
    env: &IngestEnv<'_>,
    renderer: &str,
    items: &ItemIndex,
    existing: &mut BTreeSet<(u64, String)>,
    planned: &Planned,
    forced_item: Option<u64>,
) -> MediaResult {
    let failure = |message: String| MediaResult {
        file: planned.display.clone(),
        item: None,
        media: None,
        status: MediaStatus::Failed,
        thumbnails: false,
        message: Some(message),
    };

    let item = match forced_item.or(planned.item) {
        Some(item) => item,
        None => {
            let key = planned
                .identifier
                .clone()
                .unwrap_or_else(|| file_stem(&planned.file));
            match items.resolve(&key) {
                Ok(item) => item,
                Err(message) => return failure(message),
            }
        }
    };

    let bytes = match std::fs::read(&planned.file) {
        Ok(bytes) => bytes,
        Err(error) => return failure(format!("cannot read {}: {error}", planned.display)),
    };
    let source = source_name(&planned.file);
    if source.is_empty() {
        return failure(format!("{} has no file name", planned.display));
    }

    if existing.contains(&(item, source.clone())) {
        return MediaResult {
            file: planned.display.clone(),
            item: Some(item),
            media: None,
            status: MediaStatus::Skipped,
            thumbnails: false,
            message: None,
        };
    }

    let draft = CoreMedia::new(
        CoreMediaId::new(0),
        CoreItemId::new(item),
        INGESTER.to_string(),
        renderer.to_string(),
        Timestamp::from_naive(NaiveDateTime::default()),
    );
    let mut draft = MediaDraft::new(draft);
    // `Ingester/Upload.php:61`: with no `o:source`, the source is the file name.
    draft.row.source = Some(source.clone());
    let file = IngestFile::new(bytes)
        .with_source_name(source.clone())
        .with_media_type(detect_media_type(&source).to_string());
    if let Err(error) = ingest_file(env, &mut draft, file, IngestFlags::default()) {
        return failure(format!("cannot store {}: {error}", planned.display));
    }

    let thumbnails = draft.row.has_thumbnails;
    let MediaDraft { row, data } = draft;
    let new = NewMedia {
        resource: NewResource::default(),
        item_id: ItemId::from(item),
        ingester: INGESTER.to_string(),
        renderer: renderer.to_string(),
        data: match data {
            MediaData::None => None,
            other => Some(other.to_value()),
        },
        source: row.source,
        media_type: row.media_type,
        storage_id: row.storage_id,
        extension: row.extension,
        sha256: row.sha256,
        size: row.size,
        has_original: row.has_original,
        has_thumbnails: row.has_thumbnails,
        position: None,
        lang: None,
        alt_text: None,
    };
    match MediaRepo::new(pool).create(&new).await {
        Ok(created) => {
            existing.insert((item, source));
            MediaResult {
                file: planned.display.clone(),
                item: Some(item),
                media: Some(created.id().0),
                status: MediaStatus::Imported,
                thumbnails,
                message: None,
            }
        }
        Err(error) => failure(format!("cannot insert the media row: {error}")),
    }
}

fn source_name(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default()
}

fn file_stem(path: &Path) -> String {
    path.file_stem()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// Items indexed by their `dcterms:identifier` literal.
struct ItemIndex {
    by_identifier: BTreeMap<String, Vec<u64>>,
}

impl ItemIndex {
    async fn build(
        pool: &SqlitePool,
        item_set: Option<ItemSetId>,
        property: PropertyId,
    ) -> Result<Self, CliError> {
        let items = ItemRepo::new(pool)
            .list(
                &ItemFilter {
                    item_set_id: item_set,
                    ..ItemFilter::default()
                },
                &Paging::default(),
            )
            .await?;
        let values = ValueRepo::new(pool);
        let mut by_identifier: BTreeMap<String, Vec<u64>> = BTreeMap::new();
        for item in items {
            let id = item.id().0;
            for value in values.list_for_property(item.resource.id, property).await? {
                let Some(text) = value.value else { continue };
                for key in identifier_keys(&text) {
                    let ids = by_identifier.entry(key).or_default();
                    if !ids.contains(&id) {
                        ids.push(id);
                    }
                }
            }
        }
        Ok(Self { by_identifier })
    }

    /// The item a key names, or the reason it does not name exactly one.
    fn resolve(&self, key: &str) -> Result<u64, String> {
        for candidate in identifier_keys(key) {
            if let Some(ids) = self.by_identifier.get(&candidate) {
                return match ids.as_slice() {
                    [id] => Ok(*id),
                    many => Err(format!(
                        "{} items carry dcterms:identifier \"{key}\"",
                        many.len()
                    )),
                };
            }
        }
        Err(format!("no item carries dcterms:identifier \"{key}\""))
    }
}

/// A key and its underscore-insensitive spelling, the rule `media.py` resolves
/// file names with.
fn identifier_keys(text: &str) -> [String; 2] {
    let normalized = text.trim().to_lowercase();
    let collapsed = normalized.replace('_', "");
    [normalized, collapsed]
}

/// The media ingest seams this command supplies.
///
/// The upload path touches neither the network nor the HTML purifier, so the
/// stubs refuse the first and return the second unchanged. Thumbnails come from
/// [`ImageThumbnailer`], the port of `File/Thumbnailer/Gd.php`.
struct NoNetwork;

impl HttpFetch for NoNetwork {
    fn fetch(&self, _url: &str) -> Result<HttpResponse, FetchError> {
        Err(FetchError::new(
            "the media import path reads local files and does not fetch over the network",
        ))
    }
}

struct Passthrough;

impl Purifier for Passthrough {
    fn purify(&self, html: &str) -> String {
        html.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use omeka_db::repos::ItemSetRepo;
    use omeka_db::rows::{NewItem, NewItemSet, NewValue};
    use omeka_media::file_render::ThumbnailType;
    use omeka_media::store::{StorePrefix, sha256_hex};
    use omeka_media::thumbnailer::thumbnail_path;

    use crate::testing::{TempDb, png_rgb, seeded_db};

    async fn item_set(db: &TempDb) -> ItemSetId {
        ItemSetRepo::new(&db.pool)
            .create(&NewItemSet::default())
            .await
            .expect("item set")
            .id()
    }

    async fn add_item(db: &TempDb, set: ItemSetId, identifier: &str) -> ItemId {
        let items = ItemRepo::new(&db.pool);
        let item = items
            .create(&NewItem::default())
            .await
            .expect("create item");
        items
            .add_to_item_set(item.id(), set)
            .await
            .expect("attach item");
        let property = crate::terms::resolve_property(&db.pool, IDENTIFIER_TERM)
            .await
            .expect("identifier property");
        ValueRepo::new(&db.pool)
            .create(&NewValue {
                resource_id: item.resource.id,
                property_id: property,
                value_resource_id: None,
                value_annotation_id: None,
                r#type: "literal".to_string(),
                lang: None,
                value: Some(identifier.to_string()),
                uri: None,
                is_public: true,
            })
            .await
            .expect("identifier value");
        item.id()
    }

    fn write_file(root: &Path, rel: &str, bytes: &[u8]) -> PathBuf {
        let path = root.join(rel);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("create dir");
        }
        std::fs::write(&path, bytes).expect("write fixture");
        path
    }

    fn write_manifest(root: &Path, entries: &str) -> PathBuf {
        let path = root.join("media.json");
        std::fs::write(&path, format!("{{\"sample\":[{entries}]}}")).expect("write manifest");
        path
    }

    fn options(
        db: &TempDb,
        source: Option<&Path>,
        manifest: Option<&Path>,
        store: &Path,
        set: Option<ItemSetId>,
    ) -> MediaOptions {
        MediaOptions {
            database: db.path.clone(),
            source: source.map(Path::to_path_buf),
            manifest: manifest.map(Path::to_path_buf),
            store: Some(store.to_path_buf()),
            item_set: set.map(|set| set.0),
            item: None,
        }
    }

    async fn media_of(db: &TempDb, item: ItemId) -> Vec<omeka_db::rows::MediaRow> {
        MediaRepo::new(&db.pool)
            .list(
                &MediaFilter {
                    item_id: Some(item),
                    ..MediaFilter::default()
                },
                &Paging::default(),
            )
            .await
            .expect("list media")
    }

    fn count_files(root: &Path) -> usize {
        let mut count = 0;
        let mut stack = vec![root.to_path_buf()];
        while let Some(dir) = stack.pop() {
            for entry in std::fs::read_dir(&dir).expect("read dir") {
                let path = entry.expect("entry").path();
                if path.is_dir() {
                    stack.push(path);
                } else {
                    count += 1;
                }
            }
        }
        count
    }

    /// `Ingester/Upload.php:61` sets the source from the file name, and
    /// `File/TempFile.php:420` records the extension, media type, checksum and
    /// size. `Entity/Media.php:223` reads the file name back as
    /// `<storage_id>.<extension>`.
    #[tokio::test]
    async fn media_import_stores_the_file_and_links_it_to_the_item() {
        let db = seeded_db().await;
        let set = item_set(&db).await;
        let item = add_item(&db, set, "BHC_006_AF_001").await;
        let other = add_item(&db, set, "BHC_006_AF_002").await;

        let source = tempfile::tempdir().expect("source dir");
        let bytes = b"jpeg-bytes-1";
        write_file(source.path(), "Artifacts/one.jpg", bytes);
        write_file(source.path(), "Artifacts/two.pdf", b"%PDF-1.4 body");
        let manifest = write_manifest(
            source.path(),
            r#"{"identifier":"BHC_006_AF_001","source":"AF.xlsx:2","file":"Artifacts/one.jpg"},
               {"identifier":"BHC_006_AF_002","source":"AF.xlsx:3","file":"Artifacts/two.pdf"}"#,
        );
        let store = tempfile::tempdir().expect("store dir");

        let report = run(&options(
            &db,
            Some(source.path()),
            Some(&manifest),
            store.path(),
            Some(set),
        ))
        .await
        .expect("import media");

        assert_eq!(report.imported, 2, "{report:?}");
        assert_eq!(report.failed, 0);
        assert_eq!(report.skipped, 0);

        let rows = media_of(&db, item).await;
        assert_eq!(rows.len(), 1);
        let row = &rows[0];
        assert_eq!(row.source.as_deref(), Some("one.jpg"));
        assert_eq!(row.media_type.as_deref(), Some("image/jpeg"));
        assert_eq!(row.size, Some(bytes.len() as i64));
        assert_eq!(row.sha256.as_deref(), Some(sha256_hex(bytes).as_str()));
        assert_eq!(row.ingester, "upload");
        assert_eq!(row.renderer, "file");
        assert!(row.has_original);
        assert!(!row.has_thumbnails);
        assert!(row.data.is_none());
        assert_eq!(row.item_id, item);

        let storage_id = row.storage_id.clone().expect("storage id");
        assert_eq!(storage_id.len(), 40);
        let filename = format!(
            "{storage_id}.{}",
            row.extension.as_deref().expect("extension")
        );
        let storage_path =
            FileStore::storage_path(StorePrefix::Original, &storage_id, row.extension.as_deref());
        assert_eq!(storage_path, format!("original/{filename}"));
        assert_eq!(
            std::fs::read(store.path().join(&storage_path)).expect("stored bytes"),
            bytes
        );

        let pdf = media_of(&db, other).await;
        assert_eq!(pdf[0].media_type.as_deref(), Some("application/pdf"));
        assert_eq!(pdf[0].source.as_deref(), Some("two.pdf"));
    }

    /// An imported image gets the three thumbnails at the store's paths and the
    /// report counts it. The three sizes are `thumbnails.types`
    /// (`application/config/module.config.php:222`).
    #[tokio::test]
    async fn media_import_generates_the_three_thumbnails_for_an_image() {
        let db = seeded_db().await;
        let set = item_set(&db).await;
        let item = add_item(&db, set, "IMG_001").await;
        let source = tempfile::tempdir().expect("source dir");
        write_file(source.path(), "photo.png", &png_rgb(64, 48));
        let manifest = write_manifest(
            source.path(),
            r#"{"identifier":"IMG_001","file":"photo.png"}"#,
        );
        let store = tempfile::tempdir().expect("store dir");

        let report = run(&options(
            &db,
            Some(source.path()),
            Some(&manifest),
            store.path(),
            Some(set),
        ))
        .await
        .expect("import media");

        assert_eq!(report.imported, 1, "{report:?}");
        assert_eq!(report.thumbnails, 1);

        let rows = media_of(&db, item).await;
        let row = &rows[0];
        assert!(row.has_thumbnails, "the row should carry the flag");
        let storage_id = row.storage_id.clone().expect("storage id");
        for kind in ThumbnailType::ALL {
            let path = thumbnail_path(&storage_id, kind);
            assert!(
                store.path().join(&path).exists(),
                "missing {path} under {:?}",
                store.path()
            );
        }
    }

    /// The second run writes nothing: the `(item, source)` pair already exists.
    #[tokio::test]
    async fn a_second_run_skips_every_file() {
        let db = seeded_db().await;
        let set = item_set(&db).await;
        let item = add_item(&db, set, "BHC_006_AF_001").await;
        let source = tempfile::tempdir().expect("source dir");
        write_file(source.path(), "one.jpg", b"first");
        let manifest = write_manifest(
            source.path(),
            r#"{"identifier":"BHC_006_AF_001","file":"one.jpg"}"#,
        );
        let store = tempfile::tempdir().expect("store dir");
        let opts = options(
            &db,
            Some(source.path()),
            Some(&manifest),
            store.path(),
            Some(set),
        );

        let first = run(&opts).await.expect("first run");
        assert_eq!(first.imported, 1);
        let second = run(&opts).await.expect("second run");

        assert_eq!(second.imported, 0);
        assert_eq!(second.skipped, 1);
        assert_eq!(second.results[0].status, MediaStatus::Skipped);
        assert_eq!(media_of(&db, item).await.len(), 1);
        assert_eq!(count_files(store.path()), 1);
    }

    /// An identifier the item index does not hold is a failure, not a panic and
    /// not a row.
    #[tokio::test]
    async fn an_unresolved_identifier_fails_the_file() {
        let db = seeded_db().await;
        let set = item_set(&db).await;
        let source = tempfile::tempdir().expect("source dir");
        write_file(source.path(), "orphan.jpg", b"bytes");
        let manifest = write_manifest(
            source.path(),
            r#"{"identifier":"NOPE","file":"orphan.jpg"}"#,
        );
        let store = tempfile::tempdir().expect("store dir");

        let report = run(&options(
            &db,
            Some(source.path()),
            Some(&manifest),
            store.path(),
            Some(set),
        ))
        .await
        .expect("import media");

        assert_eq!(report.failed, 1);
        assert_eq!(report.imported, 0);
        let message = report.results[0].message.as_deref().expect("message");
        assert!(message.contains("dcterms:identifier"), "got {message}");
        assert_eq!(
            MediaRepo::new(&db.pool)
                .list(&MediaFilter::default(), &Paging::default())
                .await
                .expect("list")
                .len(),
            0
        );
        assert_eq!(count_files(store.path()), 0);
    }

    /// A manifest entry whose file is missing on disk fails, and the rest of the
    /// run continues.
    #[tokio::test]
    async fn a_missing_file_fails_and_the_run_continues() {
        let db = seeded_db().await;
        let set = item_set(&db).await;
        add_item(&db, set, "PRESENT").await;
        add_item(&db, set, "MISSING").await;
        let source = tempfile::tempdir().expect("source dir");
        write_file(source.path(), "here.jpg", b"ok");
        let manifest = write_manifest(
            source.path(),
            r#"{"identifier":"MISSING","file":"gone.jpg"},
               {"identifier":"PRESENT","file":"here.jpg"}"#,
        );
        let store = tempfile::tempdir().expect("store dir");

        let report = run(&options(
            &db,
            Some(source.path()),
            Some(&manifest),
            store.path(),
            Some(set),
        ))
        .await
        .expect("import media");

        assert_eq!(report.failed, 1);
        assert_eq!(report.imported, 1);
        let message = report.results[0].message.as_deref().expect("message");
        assert!(message.contains("cannot read"), "got {message}");
        assert_eq!(count_files(store.path()), 1);
    }

    /// The directory input links each file by its stem, the rule `media.py`
    /// resolves source identifiers with.
    #[tokio::test]
    async fn the_directory_input_links_by_file_stem() {
        let db = seeded_db().await;
        let set = item_set(&db).await;
        let item = add_item(&db, set, "BHC_006_AF_001").await;
        let source = tempfile::tempdir().expect("source dir");
        write_file(source.path(), "Artifacts/BHC_006_AF_001.jpg", b"jpeg");
        write_file(source.path(), "notes.txt", b"not media in this list");
        let store = tempfile::tempdir().expect("store dir");

        let report = run(&options(
            &db,
            Some(source.path()),
            None,
            store.path(),
            Some(set),
        ))
        .await
        .expect("import media");

        assert_eq!(report.imported, 1, "{report:?}");
        let rows = media_of(&db, item).await;
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].source.as_deref(), Some("BHC_006_AF_001.jpg"));
    }

    #[test]
    fn detect_media_type_maps_the_acceptance_extensions() {
        assert_eq!(detect_media_type("photo.JPG"), "image/jpeg");
        assert_eq!(detect_media_type("scan.tif"), "image/tiff");
        assert_eq!(detect_media_type("raw.nef"), "image/tiff");
        assert_eq!(detect_media_type("clip.mp4"), "video/mp4");
        assert_eq!(detect_media_type("talk.mp3"), "audio/mpeg");
        assert_eq!(detect_media_type("doc.pdf"), "application/pdf");
        assert_eq!(detect_media_type("noext"), "application/octet-stream");
    }

    #[test]
    fn parse_manifest_reads_the_acceptance_shape_and_rejects_an_empty_sample() {
        let entries = parse_manifest(
            r#"{"sample_size":100,"resolved":1,
                "sample":[{"identifier":"A","source":"w.xlsx:2","file":"a.jpg"}]}"#,
        )
        .expect("parse");
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].identifier.as_deref(), Some("A"));
        assert_eq!(entries[0].file, "a.jpg");
        assert!(parse_manifest(r#"{"sample":[]}"#).is_err());
    }

    #[test]
    fn identifier_keys_are_case_and_underscore_insensitive() {
        assert_eq!(
            identifier_keys(" BHC_006_AF_001 "),
            ["bhc_006_af_001".to_string(), "bhc006af001".to_string()]
        );
    }
}
