//! The run against a real SQLite database, a real file store and a real
//! folder of small JPEG and TIFF files. [`fixture`] is shared by the undo,
//! value, existing-row and left-out tests.

#![allow(clippy::expect_used, clippy::unwrap_used)]

use std::collections::BTreeMap;
use std::io::Cursor;
use std::path::{Path, PathBuf};

use omeka_db::ids::{ImportId, ItemSetId, PropertyId};
use omeka_db::repos::{
    ImportRepo, ImportResourceRepo, ItemRepo, ItemSetRepo, MediaFilter, MediaRepo, Paging,
    PropertyRepo, TemplateRepo, ValueRepo, VocabularyRepo,
};
use omeka_db::rows::{ImportKind, NewImport, NewItemSet, NewProperty, NewResource, NewVocabulary};
use omeka_db::{migrate, open_file};
use omeka_media::store::FileStore;
use serde_json::json;
use sqlx::SqlitePool;

use super::archive::ArchiveRoot;
use super::catalog::Catalog;
use super::plan::ImportPlan;
use super::run::{ImportSummary, RunInput, find_property, run};
use super::undo::{UndoReport, undo};
use crate::jsonld::BaseUrl;

pub(super) struct Fixture {
    _dir: tempfile::TempDir,
    pub(super) pool: SqlitePool,
    pub(super) store: FileStore,
    store_root: PathBuf,
    pub(super) archive: PathBuf,
    pub(super) photos: ItemSetId,
    pub(super) catalog: Catalog,
}

fn image_bytes(format: image::ImageFormat) -> Vec<u8> {
    let pixels = image::RgbImage::from_fn(16, 12, |x, y| {
        image::Rgb([(x * 15) as u8, (y * 20) as u8, 120])
    });
    let mut out = Cursor::new(Vec::new());
    image::DynamicImage::ImageRgb8(pixels)
        .write_to(&mut out, format)
        .expect("encode image");
    out.into_inner()
}

fn write(root: &Path, relative: &str, bytes: &[u8]) {
    let path = root.join(relative);
    std::fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
    std::fs::write(path, bytes).expect("write");
}

async fn install_vocabularies(pool: &SqlitePool) {
    for vocabulary in &crate::vocab_import::defaults::DEFAULT_VOCABULARIES {
        let members = vocabulary.members().expect("bundled vocabulary parses");
        let row = VocabularyRepo::new(pool)
            .create(&NewVocabulary {
                owner_id: None,
                namespace_uri: vocabulary.namespace_uri.to_string(),
                prefix: vocabulary.prefix.to_string(),
                label: vocabulary.label.to_string(),
                comment: None,
            })
            .await
            .expect("vocabulary");
        for property in members.properties {
            PropertyRepo::new(pool)
                .create(&NewProperty {
                    owner_id: None,
                    vocabulary_id: row.id,
                    local_name: property.local_name,
                    label: property.label,
                    comment: None,
                })
                .await
                .expect("property");
        }
    }
}

/// `Box/` with two letters (the second a corrupt "JPEG"), one photograph, and
/// a register; the database has the default vocabularies and a "Photos" set.
pub(super) async fn fixture() -> Fixture {
    let dir = tempfile::tempdir().expect("tempdir");
    let archive = dir.path().join("archive");
    let folder = archive.join("Box");
    write(
        &folder,
        "Letters/L_001_front.jpg",
        &image_bytes(image::ImageFormat::Jpeg),
    );
    write(
        &folder,
        "Letters/L_001_back.tif",
        &image_bytes(image::ImageFormat::Tiff),
    );
    write(
        &folder,
        "Letters/L_002_front.jpg",
        // A JPEG signature over a broken body: it reads as `image/jpeg`, so
        // file validation passes it, and it does not decode.
        b"\xFF\xD8\xFF\xE0 not really a jpeg",
    );
    write(
        &folder,
        "Photos/P_001.jpg",
        &image_bytes(image::ImageFormat::Jpeg),
    );
    write(
        &folder,
        "register.csv",
        b"id,Title,Shelf mark\nL_001,First letter,A-1\nL_002,Second letter,A-2\nP_001,Harbour,B-1\n",
    );
    let pool = open_file(dir.path().join("omeka.db")).await.expect("db");
    migrate(&pool).await.expect("migrate");
    install_vocabularies(&pool).await;
    let photos = ItemSetRepo::new(&pool)
        .create(&NewItemSet {
            resource: NewResource {
                title: Some("Photos".to_string()),
                ..NewResource::default()
            },
            is_open: false,
        })
        .await
        .expect("photos set")
        .id();
    let store_root = dir.path().join("files");
    let catalog = super::catalog::load(&pool).await.expect("catalog");
    Fixture {
        catalog,
        store: FileStore::new(&store_root, "/files"),
        store_root,
        archive,
        pool,
        photos,
        _dir: dir,
    }
}

pub(super) fn plan() -> ImportPlan {
    let mut value = ImportPlan::new_draft().to_json();
    value["pattern"] =
        json!(r"^(?<set>[^/]+)/(?<identifier>[A-Z]_\d{3})(?:_(?<part>[a-z]+))?\.(?:jpg|tif)$");
    value["typeFor"] = json!({ "Letters": { "starter": "letter", "name": "Letter" } });
    value["csv"] = json!({
        "file": "register.csv", "delimiter": ",", "headerRow": 1, "windows1252": false,
        "trim": false, "joinColumn": "id",
        "columns": { "Title": "dcterms:title", "Shelf mark": { "newField": "Shelf mark" } }
    });
    value["meta"] =
        json!([{ "tag": "File name", "term": "dcterms:source", "on": "media", "from": "first" }]);
    ImportPlan::from_json(&value).expect("plan")
}

impl Fixture {
    pub(super) async fn import(&self, plan: &ImportPlan) -> ImportId {
        ImportRepo::new(&self.pool)
            .create_draft(&NewImport {
                owner_id: None,
                folder: "Box".to_string(),
                plan: plan.to_json(),
            })
            .await
            .expect("draft")
            .id
    }

    pub(super) fn input(&self, import_id: ImportId, plan: ImportPlan) -> RunInput {
        let root = ArchiveRoot::new(&self.archive).expect("root");
        RunInput {
            import_id,
            owner: None,
            folder: root.folder("Box").expect("folder"),
            plan,
            catalog: self.catalog.clone(),
            base_url: BaseUrl::parse("https://archive.example").expect("base"),
        }
    }

    pub(super) async fn run(&self) -> (ImportId, ImportSummary, Vec<String>) {
        self.run_plan(plan()).await
    }

    pub(super) async fn run_plan(
        &self,
        plan: ImportPlan,
    ) -> (ImportId, ImportSummary, Vec<String>) {
        let import = self.import(&plan).await;
        let mut lines = Vec::new();
        let summary = run(&self.pool, &self.store, self.input(import, plan), |step| {
            lines.push(step.line.to_string());
        })
        .await
        .expect("run");
        (import, summary, lines)
    }

    pub(super) async fn undo(&self, import: ImportId) -> UndoReport {
        undo(&self.pool, &self.store, import, |_| {})
            .await
            .expect("undo")
    }

    pub(super) async fn count(&self, table: &str) -> i64 {
        sqlx::query_scalar(&format!("SELECT COUNT(*) FROM {table}"))
            .fetch_one(&self.pool)
            .await
            .expect("count")
    }

    pub(super) async fn recorded(&self, import: ImportId, kind: ImportKind) -> usize {
        ImportResourceRepo::new(&self.pool)
            .list(import, Some(kind))
            .await
            .expect("records")
            .len()
    }

    /// Every stored file, as `<folder>/<name>`.
    pub(super) fn stored(&self) -> Vec<String> {
        let mut found = Vec::new();
        let Ok(folders) = std::fs::read_dir(&self.store_root) else {
            return found;
        };
        for folder in folders.flatten() {
            for file in std::fs::read_dir(folder.path()).expect("folder").flatten() {
                found.push(format!(
                    "{}/{}",
                    folder.file_name().to_string_lossy(),
                    file.file_name().to_string_lossy()
                ));
            }
        }
        found.sort();
        found
    }

    /// The files the media rows name: originals and three thumbnails each.
    async fn expected_files(&self) -> Vec<String> {
        let rows = MediaRepo::new(&self.pool)
            .list(&MediaFilter::default(), &Paging::default())
            .await
            .expect("media");
        let mut files = Vec::new();
        for row in rows {
            let id = row.storage_id.expect("storage id");
            if row.has_original {
                files.push(format!(
                    "original/{id}.{}",
                    row.extension.expect("extension")
                ));
            }
            if row.has_thumbnails {
                for kind in ["large", "medium", "square"] {
                    files.push(format!("{kind}/{id}.jpg"));
                }
            }
        }
        files.sort();
        files
    }

    pub(super) async fn property(&self, term: &str) -> PropertyId {
        find_property(&self.pool, term)
            .await
            .expect("lookup")
            .expect("property")
    }

    pub(super) async fn values(&self, resource: u64) -> BTreeMap<String, String> {
        let mut out = BTreeMap::new();
        for value in ValueRepo::new(&self.pool)
            .list_for_resource(omeka_db::ids::ResourceId(resource))
            .await
            .expect("values")
        {
            let property = PropertyRepo::new(&self.pool)
                .get(value.property_id)
                .await
                .expect("get")
                .expect("property");
            out.insert(property.local_name, value.value.unwrap_or_default());
        }
        out
    }
}

#[tokio::test]
async fn a_run_creates_items_sets_a_type_a_field_media_and_files_and_records_them() {
    let fixture = fixture().await;
    let (import, summary, lines) = fixture.run().await;

    assert_eq!(summary.items, 3);
    assert_eq!(summary.failed, 0);
    assert_eq!(summary.files, 4);
    assert_eq!(
        summary.thumbnails, 3,
        "the corrupt JPEG keeps its original only"
    );
    assert_eq!(summary.new_sets, ["Letters"]);
    assert_eq!(summary.new_types, ["Letter"]);
    assert_eq!(summary.new_fields, ["Shelf mark"]);
    assert_eq!((summary.progress.done, summary.progress.total), (3, 3));
    assert!(
        lines.iter().any(|line| line.starts_with("created L_001")),
        "{lines:?}"
    );
    let stored: serde_json::Value = ImportRepo::new(&fixture.pool)
        .get(import)
        .await
        .expect("get")
        .expect("import")
        .summary
        .expect("summary written");
    assert_eq!(stored["items"], 3);
    assert_eq!(stored["newFields"], json!(["Shelf mark"]));

    let template = TemplateRepo::new(&fixture.pool)
        .find_by_label("Letter")
        .await
        .expect("find")
        .expect("the new type");
    let fields = TemplateRepo::new(&fixture.pool)
        .list_properties(template.id)
        .await
        .expect("fields");
    assert_eq!(fields.len(), 7, "the letter starter's fields");
    assert!(fields[0].is_required);

    let items = ItemRepo::new(&fixture.pool)
        .list(&Default::default(), &Paging::default())
        .await
        .expect("items");
    assert_eq!(items.len(), 3);
    let letter = items
        .iter()
        .find(|item| item.resource.title.as_deref() == Some("First letter"))
        .expect("L_001");
    assert_eq!(letter.resource.resource_template_id, Some(template.id));
    let values = fixture.values(letter.resource.id.0).await;
    assert_eq!(values["identifier"], "L_001");
    assert_eq!(values["shelfMark"], "A-1");
    let photo = items
        .iter()
        .find(|item| item.resource.title.as_deref() == Some("Harbour"))
        .expect("P_001");
    let sets = ItemRepo::new(&fixture.pool)
        .list_item_sets(photo.id())
        .await
        .expect("sets");
    assert_eq!(
        sets,
        [fixture.photos],
        "an existing set is joined, not made again"
    );
    assert!(fixture.property("local:shelfMark").await.0 > 0);

    let media = MediaRepo::new(&fixture.pool)
        .list(
            &MediaFilter {
                item_id: Some(letter.id()),
                ..MediaFilter::default()
            },
            &Paging::default(),
        )
        .await
        .expect("media");
    assert_eq!(media.len(), 2);
    let tiff = media
        .iter()
        .find(|m| m.source.as_deref() == Some("L_001_back.tif"))
        .expect("tiff media");
    assert_eq!(tiff.media_type.as_deref(), Some("image/tiff"));
    assert!(tiff.has_thumbnails, "a TIFF gets thumbnails");
    assert_eq!(
        fixture.values(tiff.resource.id.0).await["source"],
        "L_001_back.tif"
    );

    assert_eq!(fixture.stored(), fixture.expected_files().await);
    assert_eq!(fixture.stored().len(), 4 + 3 * 3);
    assert_eq!(fixture.recorded(import, ImportKind::Item).await, 3);
    assert_eq!(fixture.recorded(import, ImportKind::Media).await, 4);
    assert_eq!(fixture.recorded(import, ImportKind::ItemSet).await, 1);
    assert_eq!(
        fixture.recorded(import, ImportKind::ResourceTemplate).await,
        1
    );
    assert_eq!(fixture.recorded(import, ImportKind::Property).await, 1);
}

#[tokio::test]
async fn an_item_whose_transaction_fails_fails_alone_and_leaves_no_file() {
    let fixture = fixture().await;
    sqlx::query(
        "CREATE TRIGGER refuse BEFORE INSERT ON value WHEN NEW.value = 'Second letter'
         BEGIN SELECT RAISE(ABORT, 'value refused'); END",
    )
    .execute(&fixture.pool)
    .await
    .expect("trigger");
    let (import, summary, lines) = fixture.run().await;

    assert_eq!((summary.items, summary.failed), (2, 1));
    assert_eq!(summary.left, 1);
    assert!(
        lines.iter().any(|line| line.starts_with("failed L_002")),
        "{lines:?}"
    );
    assert_eq!(fixture.count("item").await, 2);
    assert_eq!(fixture.count("media").await, 3);
    assert_eq!(
        fixture.stored(),
        fixture.expected_files().await,
        "no stray file"
    );
    assert_eq!(fixture.recorded(import, ImportKind::Item).await, 2);
}

#[tokio::test]
async fn a_file_that_cannot_be_read_fails_its_item_and_removes_its_other_files() {
    let fixture = fixture().await;
    use std::os::unix::fs::PermissionsExt;
    let front = fixture.archive.join("Box/Letters/L_001_front.jpg");
    std::fs::set_permissions(&front, std::fs::Permissions::from_mode(0o000)).expect("chmod");
    if std::fs::read(&front).is_ok() {
        return; // root reads any file; this case needs an ordinary user
    }
    let import = fixture.import(&plan()).await;
    let input = fixture.input(import, plan());
    let summary = run(&fixture.pool, &fixture.store, input, |_| {})
        .await
        .expect("run");

    assert_eq!((summary.items, summary.failed), (2, 1));
    assert_eq!(
        fixture.count("media").await,
        2,
        "L_001's stored TIFF went with it"
    );
    assert_eq!(
        fixture.stored(),
        fixture.expected_files().await,
        "no stray file"
    );
}

#[tokio::test]
async fn a_plan_that_names_an_unknown_term_writes_nothing() {
    let fixture = fixture().await;
    let import = fixture.import(&plan()).await;
    let mut input = fixture.input(import, plan());
    let mut value = input.plan.to_json();
    value["csv"]["columns"]["Title"] = json!("dcterms:nonsense");
    input.plan = ImportPlan::from_json(&value).expect("plan");
    let error = run(&fixture.pool, &fixture.store, input, |_| {})
        .await
        .expect_err("unknown term");
    assert!(
        matches!(
            &error,
            super::run::RunError::Work(super::work::WorkError::Issue(
                super::derive::Issue::UnknownTerm(term)
            )) if term == "dcterms:nonsense"
        ),
        "{error:?}"
    );
    assert_eq!(fixture.count("item_set").await, 1);
    assert_eq!(fixture.count("romeka_import_resource").await, 0);
}

/// The Types tab lists `catalog::STARTERS`; a run must create exactly those
/// fields, labels and class, or the tab promises what the run does not make.
#[tokio::test]
async fn a_new_type_is_made_exactly_as_its_starter_lists() {
    use omeka_db::repos::ClassRepo;
    use omeka_db::rows::NewClass;

    let fixture = fixture().await;
    for vocabulary in &crate::vocab_import::defaults::DEFAULT_VOCABULARIES {
        let prefix = VocabularyRepo::new(&fixture.pool)
            .get_by_prefix(vocabulary.prefix)
            .await
            .expect("vocabulary")
            .expect("installed");
        for class in vocabulary.members().expect("members").classes {
            ClassRepo::new(&fixture.pool)
                .create(&NewClass {
                    owner_id: None,
                    vocabulary_id: prefix.id,
                    local_name: class.local_name,
                    label: class.label,
                    comment: None,
                })
                .await
                .expect("class");
        }
    }
    let mut plan = plan();
    let mut value = plan.to_json();
    value["typeFor"] = json!({
        "Letters": { "starter": "letter", "name": "Letter" },
        "Photos": { "starter": "photograph", "name": "Photograph" },
    });
    plan = ImportPlan::from_json(&value).expect("plan");
    fixture.run_plan(plan).await;

    for starter in &super::catalog::STARTERS {
        if !matches!(starter.name, "Letter" | "Photograph") {
            continue;
        }
        let template = TemplateRepo::new(&fixture.pool)
            .find_by_label(starter.name)
            .await
            .expect("find")
            .expect("the new type");
        let (prefix, local) = starter.class_term.split_once(':').expect("term");
        let vocabulary = VocabularyRepo::new(&fixture.pool)
            .get_by_prefix(prefix)
            .await
            .expect("vocabulary")
            .expect("installed");
        let class = ClassRepo::new(&fixture.pool)
            .find(vocabulary.id, local)
            .await
            .expect("class")
            .expect("installed");
        assert_eq!(
            template.resource_class_id,
            Some(class.id),
            "{}",
            starter.name
        );
        let rows = TemplateRepo::new(&fixture.pool)
            .list_properties(template.id)
            .await
            .expect("fields");
        assert_eq!(rows.len(), starter.fields.len(), "{}", starter.name);
        for (row, field) in rows.iter().zip(starter.fields) {
            let id = find_property(&fixture.pool, field.term)
                .await
                .expect("find")
                .expect("property");
            assert_eq!(row.property_id, id, "{}", field.term);
            assert_eq!(row.is_required, field.required, "{}", field.term);
            assert_eq!(
                row.alternate_label.as_deref(),
                Some(field.label),
                "{}",
                field.term
            );
            let types: Vec<String> = field.data_types.iter().map(|t| (*t).to_string()).collect();
            assert_eq!(row.data_type, Some(types), "{}", field.term);
        }
    }
}
