//! Import v2 on an S3 store (`docs/import-v2.md`, "Storage and file
//! access"): scenario S1 through the routes and the job runner puts every
//! original and thumbnail in the bucket and nothing on the local disk, the
//! API names them with signed URLs that fetch the bytes, and an undo removes
//! them; uploads are staged in the bucket part by part and unpacked from it.
//!
//! Gated on `ROMEKA_S3_TEST_ENDPOINT`, as `omeka-media`'s S3 suite is
//! (`crates/omeka-media/tests/s3_fixture/mod.rs` shows the MinIO command);
//! `ROMEKA_S3_TEST_ACCESS_KEY` and `ROMEKA_S3_TEST_SECRET_KEY` default to
//! `minioadmin`.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod import_site;

use std::sync::Arc;
use std::time::{Duration, Instant};

use import_site::{Site, jpeg, rule, site_with, tiff};
use omeka_api::import::staging::{self, Limits};
use omeka_db::ids::{ImportId, JobId};
use omeka_db::repos::JobRepo;
use omeka_db::rows::{ImportStatus, JobRow, NewJob};
use omeka_media::store::{S3Config, S3Store, Store};
use omeka_server::JobDispatch;
use serde_json::{Value, json};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

const BHC_PATTERN: &str = r"^(?<set>[^/]+)/(?:[^/]+/)*(?<identifier>BHC_006_[A-Z]{2}_\d{3})(?:_(?<part>[a-z0-9]+))?\.(?:jpg|tif)$";

const LIMITS: Limits = Limits {
    max_file: 64 * 1024 * 1024,
    max_total: 64 * 1024 * 1024,
};

fn s3_store(test: &str) -> Option<Arc<S3Store>> {
    let Ok(endpoint) = std::env::var("ROMEKA_S3_TEST_ENDPOINT") else {
        eprintln!("SKIP: ROMEKA_S3_TEST_ENDPOINT is not set, so {test} did not run");
        return None;
    };
    let key = |name: &str| std::env::var(name).unwrap_or_else(|_| "minioadmin".to_string());
    let bucket = format!(
        "romeka-import-{}",
        &omeka_media::store::new_storage_id().unwrap()[..16]
    );
    let store = S3Store::new(S3Config {
        endpoint: endpoint.parse().unwrap(),
        bucket,
        region: "us-east-1".to_string(),
        access_key: key("ROMEKA_S3_TEST_ACCESS_KEY"),
        secret_key: key("ROMEKA_S3_TEST_SECRET_KEY"),
        public_base: None,
        url_ttl: Duration::from_secs(900),
    })
    .unwrap();
    store.create_bucket().unwrap();
    Some(Arc::new(store))
}

async fn s3_site(store: &Arc<S3Store>) -> Site {
    let store: Arc<dyn Store> = store.clone();
    site_with(|_| store).await
}

/// Every object under the media prefixes and `staging/`.
fn listed(store: &dyn Store) -> Vec<String> {
    let mut files: Vec<String> = ["original", "large", "medium", "square", "staging"]
        .into_iter()
        .flat_map(|prefix| store.list(prefix).unwrap())
        .map(|file| file.storage_path)
        .collect();
    files.sort();
    files
}

/// Each media row's original and, when it has them, its three thumbnails.
async fn expected_files(site: &Site) -> Vec<String> {
    let mut files = Vec::new();
    for media in site.media().await {
        let id = media.storage_id.as_deref().unwrap();
        if media.has_original {
            files.push(format!(
                "original/{id}.{}",
                media.extension.as_deref().unwrap()
            ));
        }
        if media.has_thumbnails {
            for kind in ["large", "medium", "square"] {
                files.push(format!("{kind}/{id}.jpg"));
            }
        }
    }
    files.sort();
    files
}

/// The body of a plain HTTP GET of `url`, which names a host and port.
async fn fetch(url: &str) -> (u16, Vec<u8>) {
    let rest = url.strip_prefix("http://").expect("an http URL");
    let (host, path) = rest.split_at(rest.find('/').unwrap());
    let mut stream = tokio::net::TcpStream::connect(host).await.unwrap();
    let request = format!("GET {path} HTTP/1.1\r\nHost: {host}\r\nConnection: close\r\n\r\n");
    stream.write_all(request.as_bytes()).await.unwrap();
    let mut raw = Vec::new();
    stream.read_to_end(&mut raw).await.unwrap();
    let split = raw.windows(4).position(|w| w == b"\r\n\r\n").unwrap();
    let status = std::str::from_utf8(&raw[9..12]).unwrap().parse().unwrap();
    (status, raw[split + 4..].to_vec())
}

/// The BHC_006 demo with real image bytes, as `tests/import_run.rs` has it.
fn bhc_006(site: &Site) {
    let (jpeg, tiff) = (jpeg(), tiff());
    for n in 1..=4 {
        site.write_object(&format!("Artifacts/BHC_006_AF_00{n}_front.jpg"), &jpeg);
        site.write_object(&format!("Artifacts/BHC_006_AF_00{n}_back.jpg"), &jpeg);
        site.write_object(&format!("Photographs/1950s/BHC_006_PH_10{n}.tif"), &tiff);
    }
    for page in ["p1", "p2", "p10"] {
        site.write_object(&format!("Scans/BHC_006_SC_001_{page}.tif"), &tiff);
    }
    site.write("_scratch/notes.txt", b"call the donor about AF_003\n");
}

#[tokio::test(flavor = "multi_thread")]
async fn s1_on_s3_stores_every_file_in_the_bucket_and_an_undo_removes_them() {
    let Some(bucket) = s3_store("the S1 run on S3") else {
        return;
    };
    let site = s3_site(&bucket).await;
    bhc_006(&site);
    let id = site
        .draft(|plan| {
            plan["grouping"]["pattern"] = json!(BHC_PATTERN);
            plan["slots"]["item"]["sets"] =
                rule(json!([{ "from": "group", "name": "set" }]), "first");
        })
        .await;

    let row = site.run(id).await;

    assert_eq!(row.status, ImportStatus::Done, "{:?}", row.summary);
    assert_eq!(site.items().await.len(), 9);
    let expected = expected_files(&site).await;
    assert_eq!(expected.len(), 60, "15 originals, 45 thumbnails");
    assert_eq!(
        listed(bucket.as_ref()),
        expected,
        "the bucket holds them all"
    );
    assert!(site.stored_files().is_empty(), "nothing on the local disk");

    let media = &site.media().await[0];
    let answer = site
        .send("GET", &format!("/api/media/{}", media.id().0), None)
        .await;
    assert_eq!(answer.status, 200, "{}", answer.body);
    let answer: Value = answer.json();
    let original = answer["o:original_url"].as_str().unwrap();
    assert!(original.contains("X-Amz-Signature="), "{original}");
    let square = answer["o:thumbnail_urls"]["square"].as_str().unwrap();
    assert!(
        square.contains("X-Amz-Signature="),
        "a private media's thumbnail is signed: {square}"
    );
    let source = media.source.as_deref().unwrap();
    let on_disk = walk_find(&site.archive, source);
    let (status, body) = fetch(original).await;
    assert_eq!(status, 200, "{}", String::from_utf8_lossy(&body));
    assert_eq!(body, std::fs::read(on_disk).unwrap(), "byte for byte");

    let undo = site
        .send("POST", &format!("/api/romeka/imports/{id}/undo"), None)
        .await;
    assert_eq!(undo.status, 200, "{}", undo.body);
    let deadline = Instant::now() + Duration::from_secs(60);
    while site.row(id).await.status == ImportStatus::Undoing {
        assert!(Instant::now() < deadline, "import {id} stayed undoing");
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    assert_eq!(site.row(id).await.status, ImportStatus::Undone);
    assert!(site.media().await.is_empty());
    assert!(
        listed(bucket.as_ref()).is_empty(),
        "the undo deletes every file"
    );
}

/// The archive file whose name is `name`.
fn walk_find(root: &std::path::Path, name: &str) -> std::path::PathBuf {
    for entry in std::fs::read_dir(root).unwrap().flatten() {
        let path = entry.path();
        if path.is_dir() {
            let found = walk_find(&path, name);
            if found.exists() {
                return found;
            }
        } else if path.file_name().is_some_and(|file| file == name) {
            return path;
        }
    }
    root.join("missing")
}

/// A ZIP of stored entries with UTF-8 names.
fn zip_of(items: &[(&str, &[u8])]) -> Vec<u8> {
    let (mut out, mut central) = (Vec::new(), Vec::new());
    for (name, data) in items {
        let mut crc = flate2::Crc::new();
        crc.update(data);
        let offset = out.len() as u32;
        let head = |signature: u32, central: bool| {
            let mut head = signature.to_le_bytes().to_vec();
            if central {
                head.extend_from_slice(&20_u16.to_le_bytes());
            }
            head.extend_from_slice(&20_u16.to_le_bytes());
            head.extend_from_slice(&0x0800_u16.to_le_bytes());
            head.extend_from_slice(&[0; 6]);
            head.extend_from_slice(&crc.sum().to_le_bytes());
            head.extend_from_slice(&(data.len() as u32).to_le_bytes());
            head.extend_from_slice(&(data.len() as u32).to_le_bytes());
            head.extend_from_slice(&(name.len() as u16).to_le_bytes());
            head.extend_from_slice(&0_u16.to_le_bytes());
            if central {
                head.extend_from_slice(&[0; 10]);
                head.extend_from_slice(&offset.to_le_bytes());
            }
            head.extend_from_slice(name.as_bytes());
            head
        };
        out.extend(head(0x0403_4b50, false));
        out.extend_from_slice(data);
        central.extend(head(0x0201_4b50, true));
    }
    let at = out.len() as u32;
    out.extend_from_slice(&central);
    out.extend_from_slice(&0x0605_4b50_u32.to_le_bytes());
    out.extend_from_slice(&[0; 4]);
    out.extend_from_slice(&(items.len() as u16).to_le_bytes());
    out.extend_from_slice(&(items.len() as u16).to_le_bytes());
    out.extend_from_slice(&(central.len() as u32).to_le_bytes());
    out.extend_from_slice(&at.to_le_bytes());
    out.extend_from_slice(&[0; 2]);
    out
}

async fn ended_job(site: &Site, job: JobId) -> JobRow {
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        let row = JobRepo::new(&site.pool).get(job).await.unwrap().unwrap();
        if matches!(
            row.status.as_deref(),
            Some("completed" | "error" | "stopped")
        ) {
            return row;
        }
        assert!(Instant::now() < deadline, "job {} stayed", job.0);
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn uploads_are_staged_in_the_bucket_part_by_part_unpacked_and_run() {
    let Some(bucket) = s3_store("the staging run on S3") else {
        return;
    };
    let site = s3_site(&bucket).await;
    let made = site
        .send(
            "POST",
            "/api/romeka/imports",
            Some(json!({ "sources": [{ "kind": "staging", "id": "up" }] })),
        )
        .await;
    assert_eq!(made.status, 201, "{}", made.body);
    let draft = made.json();
    let id = draft["id"].as_u64().unwrap();
    let mut plan = draft["plan"].clone();
    plan["grouping"]["pattern"] = json!(r"^(?:[^/]+/)*(?<identifier>BHC_\d+)\.jpg$");
    let saved = site
        .send(
            "PUT",
            &format!("/api/romeka/imports/{id}"),
            Some(json!({ "plan": plan, "revision": draft["revision"] })),
        )
        .await;
    assert_eq!(saved.status, 200, "{}", saved.body);
    let store: Arc<dyn Store> = site.store.clone();
    let import = ImportId(id);

    let photo = [jpeg(), b"one".to_vec()].concat();
    let upload = staging::create(
        &site.pool,
        import,
        "Box/BHC_001.jpg",
        photo.len() as u64,
        LIMITS,
    )
    .await
    .unwrap();
    let half = photo.len() / 2;
    let chunk = |bytes: &[u8]| Box::new(std::io::Cursor::new(bytes.to_vec()));
    staging::append(
        &site.pool,
        &store,
        import,
        &upload.id,
        0,
        chunk(&photo[..half]),
    )
    .await
    .unwrap();
    let parts = bucket.list(&format!("staging/{id}/parts")).unwrap();
    assert_eq!(parts.len(), 1, "the first PATCH is one part: {parts:?}");
    staging::append(
        &site.pool,
        &store,
        import,
        &upload.id,
        half as u64,
        chunk(&photo[half..]),
    )
    .await
    .unwrap();
    assert!(
        bucket
            .list(&format!("staging/{id}/parts"))
            .unwrap()
            .is_empty(),
        "the last PATCH joins the parts"
    );
    let staged = staging::file_path(import, &upload.id);
    assert_eq!(bucket.read(&staged).unwrap(), photo, "joined byte for byte");

    let second = [jpeg(), b"two".to_vec()].concat();
    let archive = zip_of(&[("Box/BHC_002.jpg", &second)]);
    let zip = staging::create(
        &site.pool,
        import,
        "batch.zip",
        archive.len() as u64,
        LIMITS,
    )
    .await
    .unwrap();
    staging::append(&site.pool, &store, import, &zip.id, 0, chunk(&archive))
        .await
        .unwrap();
    let job = JobRepo::new(&site.pool)
        .create(&NewJob {
            owner_id: None,
            pid: None,
            status: Some("starting".to_string()),
            class: omeka_api::import::unzip::UNZIP_JOB.to_string(),
            args: Some(json!({ "importId": id, "upload": zip.id })),
            log: None,
            started: None,
        })
        .await
        .unwrap();
    site.jobs.start(job.id).unwrap();
    let job = ended_job(&site, job.id).await;
    assert_eq!(job.status.as_deref(), Some("completed"), "{:?}", job.log);

    let scanned = site
        .send("POST", &format!("/api/romeka/imports/{id}/scan"), None)
        .await;
    assert_eq!(scanned.status, 200, "{}", scanned.body);
    let row = site.run(id).await;

    assert_eq!(row.status, ImportStatus::Done, "{:?}", row.summary);
    let mut sizes: Vec<i64> = site.media().await.iter().filter_map(|m| m.size).collect();
    sizes.sort_unstable();
    let mut want = vec![photo.len() as i64, second.len() as i64];
    want.sort_unstable();
    assert_eq!(sizes, want, "{:?}", row.summary);
    let expected = expected_files(&site).await;
    assert_eq!(expected.len(), 8, "2 originals, 6 thumbnails");
    let listed = listed(bucket.as_ref());
    assert!(
        expected.iter().all(|file| listed.contains(file)),
        "{expected:?} in {listed:?}"
    );
    assert!(site.stored_files().is_empty(), "nothing on the local disk");
}
