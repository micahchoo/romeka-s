//! The import route tests' shared ground: a router with an archive root in a
//! tempdir and a job runner that records what it was asked to start, the
//! admin and editor callers, and the generator of the BHC_006 demo folder.

#![allow(clippy::unwrap_used, clippy::expect_used, dead_code)]

use std::path::Path;
use std::sync::{Arc, Mutex};

use axum::Router;
use omeka_api::import2::archive::ArchiveRoots;
use omeka_api::jsonld::BaseUrl;
use omeka_db::ids::{ImportId, JobId};
use omeka_db::repos::{ApiKeyRepo, ImportRepo, JobRepo, UserRepo};
use omeka_db::rows::{ImportRow, JobRow, NewApiKey, NewUser};
use omeka_media::store::FileStore;
use omeka_server::{Config, DispatchError, JobDispatch, Services};
use serde_json::{Value, json};
use sqlx::SqlitePool;
use tempfile::TempDir;

use crate::common::{
    Answer, KEY_CREDENTIAL, KEY_IDENTITY, json_request, request, seeded_pool, send,
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
    pub pool: SqlitePool,
    pub jobs: Arc<Recorder>,
    /// The archive root, named `archive`.
    pub archive: TempDir,
    /// The file store root.
    pub files: TempDir,
}

pub async fn fixture() -> Fixture {
    let (pool, _) = seeded_pool().await;
    for (email, role, (identity, credential)) in [
        ("second@example.com", "global_admin", SECOND_ADMIN),
        ("editor@example.com", "editor", EDITOR),
    ] {
        let user = UserRepo::new(&pool)
            .create(&NewUser {
                email: email.to_string(),
                name: email.to_string(),
                password_hash: None,
                role: role.to_string(),
                is_active: true,
                created: None,
            })
            .await
            .unwrap();
        ApiKeyRepo::new(&pool)
            .create(&NewApiKey {
                id: identity.to_string(),
                owner_id: user.id,
                label: role.to_string(),
                credential_hash: bcrypt::hash(credential, 4).unwrap(),
                last_ip: None,
                last_accessed: None,
                created: None,
            })
            .await
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

    pub async fn row(&self, id: &Value) -> ImportRow {
        ImportRepo::new(&self.pool)
            .get(ImportId(id.as_u64().unwrap()))
            .await
            .unwrap()
            .unwrap()
    }

    pub async fn job(&self, id: u64) -> JobRow {
        JobRepo::new(&self.pool)
            .get(JobId::from(id))
            .await
            .unwrap()
            .unwrap()
    }
}

pub fn write_tree(root: &Path, files: &[(&str, &[u8])]) {
    for (path, bytes) in files {
        let full = root.join(path);
        std::fs::create_dir_all(full.parent().unwrap()).unwrap();
        std::fs::write(full, bytes).unwrap();
    }
}

pub const JPEG: &[u8] = b"\xFF\xD8\xFF\xE0\x00\x10JFIF\x00\x01\x01\x00\x00\x01\x00\x01\x00\x00";
pub const TIFF: &[u8] = b"II*\x00\x08\x00\x00\x00\x00\x00";

/// The BHC_006 demo (scenario S1): four artifacts with a front and a back,
/// four photographs in a decade folder, one scan in three pages, and a
/// scratch folder of a note and a Windows thumbnail cache.
pub fn bhc_006(root: &Path) {
    let mut files: Vec<(String, &[u8])> = Vec::new();
    for n in 1..=4 {
        files.push((format!("Artifacts/BHC_006_AF_00{n}_front.jpg"), JPEG));
        files.push((format!("Artifacts/BHC_006_AF_00{n}_back.jpg"), JPEG));
        files.push((format!("Photographs/1950s/BHC_006_PH_10{n}.tif"), TIFF));
    }
    for page in ["p1", "p2", "p10"] {
        files.push((format!("Scans/BHC_006_SC_001_{page}.tif"), TIFF));
    }
    files.push((
        "_scratch/notes.txt".into(),
        b"call the donor about AF_003\n",
    ));
    files.push((
        "_scratch/Thumbs.db".into(),
        b"\xD0\xCF\x11\xE0\xA1\xB1\x1A\xE1\x00\x00",
    ));
    let listed: Vec<(&str, &[u8])> = files
        .iter()
        .map(|(path, bytes)| (path.as_str(), *bytes))
        .collect();
    write_tree(root, &listed);
}

/// The pattern S1 groups the demo by: the top folder is the set, the
/// identifier the object code, the optional part the side or page.
pub const BHC_PATTERN: &str = r"^(?<set>[^/]+)/(?:[^/]+/)*(?<identifier>BHC_006_[A-Z]{2}_\d{3})(?:_(?<part>[a-z0-9]+))?\.(?:jpg|tif)$";
