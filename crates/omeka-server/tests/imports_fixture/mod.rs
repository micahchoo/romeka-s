//! The import route tests' shared ground: a router with an archive root in a
//! tempdir and a job runner that records what it was asked to start, the
//! admin and editor callers, and the generator of the BHC_006 demo folder.

#![allow(clippy::unwrap_used, clippy::expect_used, dead_code)]

use std::path::Path;
use std::sync::{Arc, Mutex};

use axum::Router;
use omeka_api::import::archive::ArchiveRoots;
use omeka_api::jsonld::BaseUrl;
use omeka_db::DbPool;
use omeka_db::ids::{ImportId, JobId};
use omeka_db::repos::{ApiKeyRepo, ImportRepo, JobRepo, UserRepo};
use omeka_db::rows::{ImportRow, JobRow, NewApiKey, NewUser};
use omeka_db::with_db;
use omeka_media::store::FileStore;
use omeka_server::{Config, DispatchError, JobDispatch, Services};
use serde_json::{Value, json};
use tempfile::TempDir;

use crate::common::{
    Answer, KEY_CREDENTIAL, KEY_IDENTITY, json_request, request, seeded_engine_pool, send,
};

/// Records each job it is asked to start; refuses when told to.
#[derive(Default)]
pub struct Recorder {
    pub started: Mutex<Vec<JobId>>,
    pub refuse: Mutex<bool>,
}

impl JobDispatch for Recorder {
    fn start(&self, job: JobId) -> Result<(), DispatchError> {
        if *self.refuse.lock().unwrap() {
            return Err(DispatchError("no runner in this test".into()));
        }
        self.started.lock().unwrap().push(job);
        Ok(())
    }
}

/// A second admin and an editor, each with a key.
pub const SECOND_ADMIN: (&str, &str) = ("secondadminkey00000000000000000a", "second-credential");
pub const EDITOR: (&str, &str) = ("editorkey000000000000000000000ab", "editor-credential");

pub struct Fixture {
    pub app: Router,
    /// SQLite, or MySQL when `ROMEKA_MYSQL_URL` is set (`common/engine.rs`).
    pub pool: DbPool,
    pub jobs: Arc<Recorder>,
    /// The archive root, named `archive`.
    pub archive: TempDir,
    /// The file store root.
    pub files: TempDir,
}

pub async fn fixture() -> Fixture {
    let (pool, _) = seeded_engine_pool().await;
    for (email, role, (identity, credential)) in [
        ("second@example.com", "global_admin", SECOND_ADMIN),
        ("editor@example.com", "editor", EDITOR),
    ] {
        let user = with_db!(&pool, |db| UserRepo::new(db)
            .create(&NewUser {
                email: email.to_string(),
                name: email.to_string(),
                password_hash: None,
                role: role.to_string(),
                is_active: true,
                created: None,
            })
            .await)
        .unwrap();
        with_db!(&pool, |db| ApiKeyRepo::new(db)
            .create(&NewApiKey {
                id: identity.to_string(),
                owner_id: user.id,
                label: role.to_string(),
                credential_hash: bcrypt::hash(credential, 4).unwrap(),
                last_ip: None,
                last_accessed: None,
                created: None,
            })
            .await)
        .unwrap();
    }
    let archive = tempfile::tempdir().unwrap();
    let files = tempfile::tempdir().unwrap();
    let config = Config {
        base_url: BaseUrl::parse("http://localhost").unwrap(),
        archive_roots: ArchiveRoots::new([("archive".to_string(), archive.path().to_path_buf())])
            .unwrap(),
        ..Config::default()
    };
    let jobs = Arc::new(Recorder::default());
    let store = Arc::new(FileStore::new(files.path(), "/files"));
    let app = omeka_server::build_router_with_services(
        pool.clone(),
        &config,
        Services::new(store).with_jobs(jobs.clone()),
    );
    Fixture {
        app,
        pool,
        jobs,
        archive,
        files,
    }
}

fn keyed(uri: &str, (identity, credential): (&str, &str)) -> String {
    let separator = if uri.contains('?') { '&' } else { '?' };
    format!("{uri}{separator}key_identity={identity}&key_credential={credential}")
}

pub const ADMIN: (&str, &str) = (KEY_IDENTITY, KEY_CREDENTIAL);

impl Fixture {
    pub async fn call(&self, method: &str, uri: &str, body: Option<&Value>) -> Answer {
        self.call_as(ADMIN, method, uri, body).await
    }

    pub async fn call_as(
        &self,
        caller: (&str, &str),
        method: &str,
        uri: &str,
        body: Option<&Value>,
    ) -> Answer {
        let uri = keyed(uri, caller);
        let built = match body {
            Some(body) => json_request(method, &uri, &body.to_string()),
            None => request(method, &uri),
        };
        send(self.app.clone(), built).await
    }

    pub async fn anonymous(&self, method: &str, uri: &str) -> Answer {
        send(self.app.clone(), json_request(method, uri, "{}")).await
    }

    /// Write `files` (path, bytes) under the archive root.
    pub fn write(&self, files: &[(&str, &[u8])]) {
        write_tree(self.archive.path(), files);
    }

    /// Write `files` as [`write_objects`] does.
    pub fn write_objects(&self, files: &[(&str, &[u8])]) {
        write_objects(self.archive.path(), files);
    }

    /// A draft on the whole archive root, as its JSON view.
    pub async fn draft(&self) -> Value {
        let made = self
            .call(
                "POST",
                "/api/romeka/imports",
                Some(&json!({ "sources": [{ "kind": "folder", "id": "s1", "root": "archive", "path": "" }] })),
            )
            .await;
        assert_eq!(made.status, 201, "{}", made.body);
        made.json()
    }

    /// Save `edit` applied to the draft's plan, at its revision.
    pub async fn save(&self, draft: &Value, edit: impl FnOnce(&mut Value)) -> Answer {
        let mut plan = draft["plan"].clone();
        edit(&mut plan);
        self.call(
            "PUT",
            &format!("/api/romeka/imports/{}", draft["id"]),
            Some(&json!({ "plan": plan, "revision": draft["revision"] })),
        )
        .await
    }

    pub async fn preview(&self, id: &Value, body: &Value) -> Answer {
        self.call(
            "POST",
            &format!("/api/romeka/imports/{id}/preview"),
            Some(body),
        )
        .await
    }

    /// `POST /api/romeka/imports/{id}/sheets` with one file, as the admin.
    pub async fn upload(&self, id: &Value, name: &str, bytes: &[u8]) -> Answer {
        self.upload_to(id, "sheets", name, bytes).await
    }

    /// `POST /api/romeka/imports/{id}/{route}` with one file, as the admin.
    pub async fn upload_to(&self, id: &Value, route: &str, name: &str, bytes: &[u8]) -> Answer {
        const BOUNDARY: &str = "romeka-list-boundary";
        let mut body = format!(
            "--{BOUNDARY}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"{name}\"\r\nContent-Type: text/csv\r\n\r\n"
        )
        .into_bytes();
        body.extend_from_slice(bytes);
        body.extend_from_slice(format!("\r\n--{BOUNDARY}--\r\n").as_bytes());
        let request = axum::http::Request::builder()
            .method("POST")
            .uri(keyed(&format!("/api/romeka/imports/{id}/{route}"), ADMIN))
            .header(
                "content-type",
                format!("multipart/form-data; boundary={BOUNDARY}"),
            )
            .body(axum::body::Body::from(body))
            .unwrap();
        send(self.app.clone(), request).await
    }

    pub async fn row(&self, id: &Value) -> ImportRow {
        with_db!(&self.pool, |db| ImportRepo::new(db)
            .get(ImportId(id.as_u64().unwrap()))
            .await)
        .unwrap()
        .unwrap()
    }

    pub async fn job(&self, id: u64) -> JobRow {
        with_db!(&self.pool, |db| JobRepo::new(db).get(JobId::from(id)).await)
            .unwrap()
            .unwrap()
    }
}

/// The plan a draft on `sources` started with before the admin wrote v3, as
/// v2 JSON: what a test that edits v2 members saves, read through the lift.
pub fn v2_plan(sources: &Value) -> Value {
    let mut plan: Value = serde_json::from_str(include_str!(
        "../../../omeka-api/src/import/fixtures/default.json"
    ))
    .unwrap();
    plan["sources"] = sources.clone();
    plan
}

/// The draft with its plan as [`v2_plan`] on its sources, to edit and save.
pub fn as_v2(draft: &Value) -> Value {
    let mut draft = draft.clone();
    draft["plan"] = v2_plan(&draft["plan"]["sources"]);
    draft
}

/// The v2 `plan` as a v3 plan whose tree is `levels`: its pattern, lists
/// and slot rules dropped, every other member kept.
pub fn v3_plan(plan: &Value, levels: Value) -> Value {
    let mut v3 = plan.clone();
    let object = v3.as_object_mut().unwrap();
    for member in ["grouping", "example", "sheets", "slots"] {
        object.remove(member);
    }
    object.insert("version".into(), json!(3));
    object.insert(
        "structure".into(),
        json!({ "levels": levels, "freeSets": [] }),
    );
    v3
}

/// A v3 series over `folder` of source `s1`, grouping `<identifier>.jpg`,
/// with no list and no rule.
pub fn v3_series(id: &str, folder: &str) -> Value {
    json!({
        "kind": "series", "id": id, "set": null,
        "scope": [{ "source": "s1", "folder": folder }],
        "grouping": { "pattern": r"^(?<identifier>[A-Z]+_\d+)\.jpg$", "caseFold": false },
        "example": null, "lists": [],
        "slots": { "item": {}, "media": {}, "itemSet": {} }
    })
}

pub fn write_tree(root: &Path, files: &[(&str, &[u8])]) {
    for (path, bytes) in files {
        let full = root.join(path);
        std::fs::create_dir_all(full.parent().unwrap()).unwrap();
        std::fs::write(full, bytes).unwrap();
    }
}

/// Write each file with its path after its bytes. Files of distinct
/// objects never share their bytes, and the derive keeps only one of
/// identical files (`anomalies.identical: keepOne`).
pub fn write_objects(root: &Path, files: &[(&str, &[u8])]) {
    for (path, bytes) in files {
        write_tree(root, &[(path, &[*bytes, path.as_bytes()].concat())]);
    }
}

pub const JPEG: &[u8] = b"\xFF\xD8\xFF\xE0\x00\x10JFIF\x00\x01\x01\x00\x00\x01\x00\x01\x00\x00";
pub const TIFF: &[u8] = b"II*\x00\x08\x00\x00\x00\x00\x00";

/// The BHC_006 demo (scenario S1): four artifacts with a front and a back,
/// four photographs in a decade folder, one scan in three pages, and a
/// scratch folder of a note and a Windows thumbnail cache.
pub fn bhc_006(root: &Path) {
    let mut objects: Vec<(String, &[u8])> = Vec::new();
    for n in 1..=4 {
        objects.push((format!("Artifacts/BHC_006_AF_00{n}_front.jpg"), JPEG));
        objects.push((format!("Artifacts/BHC_006_AF_00{n}_back.jpg"), JPEG));
        objects.push((format!("Photographs/1950s/BHC_006_PH_10{n}.tif"), TIFF));
    }
    for page in ["p1", "p2", "p10"] {
        objects.push((format!("Scans/BHC_006_SC_001_{page}.tif"), TIFF));
    }
    let listed: Vec<(&str, &[u8])> = objects
        .iter()
        .map(|(path, bytes)| (path.as_str(), *bytes))
        .collect();
    write_objects(root, &listed);
    write_tree(
        root,
        &[
            ("_scratch/notes.txt", b"call the donor about AF_003\n"),
            (
                "_scratch/Thumbs.db",
                b"\xD0\xCF\x11\xE0\xA1\xB1\x1A\xE1\x00\x00",
            ),
        ],
    );
}

/// The pattern S1 groups the demo by: the top folder is the set, the
/// identifier the object code, the optional part the side or page.
pub const BHC_PATTERN: &str = r"^(?<set>[^/]+)/(?:[^/]+/)*(?<identifier>BHC_006_[A-Z]{2}_\d{3})(?:_(?<part>[a-z0-9]+))?\.(?:jpg|tif)$";
