//! Files that arrive by upload, end to end (`docs/import-v2.md`, "Arrival"):
//! a ZIP unpacked by the unzip job (scenario S11's decoded name and escaping
//! entry), scanned from the store, and run into media; and a ZIP bomb
//! refused by the job with its reason as the last log line.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod import_site;

use std::sync::Arc;
use std::time::{Duration, Instant};

use import_site::{Site, jpeg, site};
use omeka_api::import2::staging::{self, Limits};
use omeka_db::ids::{ImportId, JobId};
use omeka_db::repos::{JobRepo, UploadRepo};
use omeka_db::rows::{ImportStatus, JobRow, NewJob};
use omeka_media::store::Store;
use omeka_server::JobDispatch;
use serde_json::json;

const LIMITS: Limits = Limits {
    max_file: 64 * 1024 * 1024,
    max_total: 64 * 1024 * 1024,
};

/// A ZIP of stored entries, names as given (no UTF-8 flag).
fn zip_of(items: &[(&[u8], &[u8])]) -> Vec<u8> {
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
            head.extend_from_slice(&[0; 8]);
            head.extend_from_slice(&crc.sum().to_le_bytes());
            head.extend_from_slice(&(data.len() as u32).to_le_bytes());
            head.extend_from_slice(&(data.len() as u32).to_le_bytes());
            head.extend_from_slice(&(name.len() as u16).to_le_bytes());
            head.extend_from_slice(&0_u16.to_le_bytes());
            if central {
                head.extend_from_slice(&[0; 10]);
                head.extend_from_slice(&offset.to_le_bytes());
            }
            head.extend_from_slice(name);
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

async fn staging_draft(site: &Site, pattern: &str) -> u64 {
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
    plan["grouping"]["pattern"] = json!(pattern);
    let saved = site
        .send(
            "PUT",
            &format!("/api/romeka/imports/{id}"),
            Some(json!({ "plan": plan, "revision": draft["revision"] })),
        )
        .await;
    assert_eq!(saved.status, 200, "{}", saved.body);
    id
}

/// Upload `bytes` at `path` and run the unzip job on it, as the PATCH that
/// brings the last byte of a ZIP does. Answers the ended job.
async fn upload_zip(site: &Site, import: u64, path: &str, bytes: &[u8]) -> JobRow {
    let store: Arc<dyn Store> = site.store.clone();
    let row = staging::create(
        &site.pool,
        ImportId(import),
        path,
        bytes.len() as u64,
        LIMITS,
    )
    .await
    .unwrap();
    staging::append(
        &site.pool,
        &store,
        ImportId(import),
        &row.id,
        0,
        Box::new(std::io::Cursor::new(bytes.to_vec())),
    )
    .await
    .unwrap();
    let jobs = JobRepo::new(&site.pool);
    let job = jobs
        .create(&NewJob {
            owner_id: None,
            pid: None,
            status: Some("starting".to_string()),
            class: omeka_api::import2::unzip::UNZIP_JOB.to_string(),
            args: Some(json!({ "importId": import, "upload": row.id })),
            log: None,
            started: None,
        })
        .await
        .unwrap();
    site.jobs.start(job.id).unwrap();
    ended_job(site, job.id).await
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
        assert!(
            Instant::now() < deadline,
            "job {} stayed {:?}",
            job.0,
            row.status
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn an_uploaded_zip_is_unpacked_scanned_and_run_into_media() {
    let site = site().await;
    let id = staging_draft(&site, r"^A[^/]*o/(?<identifier>BHC_\d+)\.jpg$").await;
    let photo = jpeg();
    let archive = zip_of(&[
        (b"A\xA4o/BHC_001.jpg", &photo),
        (b"../../escape.jpg", &photo),
    ]);

    let job = upload_zip(&site, id, "batch.zip", &archive).await;

    assert_eq!(job.status.as_deref(), Some("completed"), "{:?}", job.log);
    let log = job.log.unwrap_or_default();
    assert!(
        log.contains("\"../../escape.jpg\" in the ZIP would land outside its folder"),
        "{log}"
    );
    assert!(
        log.contains("The ZIP was unpacked: 1 files were added."),
        "{log}"
    );
    let rows = UploadRepo::new(&site.pool)
        .list(ImportId(id))
        .await
        .unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].path, "Año/BHC_001.jpg");

    let scanned = site
        .send("POST", &format!("/api/romeka/imports/{id}/scan"), None)
        .await;
    assert_eq!(scanned.status, 200, "{}", scanned.body);
    let check = site.check(id).await;
    assert!(check["canRun"].as_bool().unwrap(), "{check}");

    let row = site.run(id).await;

    assert_eq!(row.status, ImportStatus::Done, "{:?}", row.summary);
    let media = site.media().await;
    assert_eq!(media.len(), 1, "{:?}", row.summary);
    assert_eq!(media[0].source.as_deref(), Some("BHC_001.jpg"));
    assert_eq!(media[0].size, Some(photo.len() as i64));
    assert!(
        media[0].has_thumbnails,
        "a small staged file gets thumbnails"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_zip_bomb_is_refused_by_the_job_and_nothing_stays() {
    let site = site().await;
    let id = staging_draft(&site, r"^(?<identifier>.+)$").await;
    let mut encoder = flate2::write::DeflateEncoder::new(Vec::new(), flate2::Compression::best());
    std::io::Write::write_all(&mut encoder, &vec![0; 12 * 1024 * 1024]).unwrap();
    let packed = encoder.finish().unwrap();
    let mut archive = zip_of(&[(b"bomb.bin", &packed)]);
    // Mark the entry deflated and give its true unpacked size and CRC.
    let mut crc = flate2::Crc::new();
    crc.update(&vec![0; 12 * 1024 * 1024]);
    let central = archive.len() - 22 - (46 + 8);
    for at in [0, central] {
        let shift = if at == 0 { 0 } else { 2 };
        archive[at + 8 + shift..at + 10 + shift].copy_from_slice(&8_u16.to_le_bytes());
        archive[at + 14 + shift..at + 18 + shift].copy_from_slice(&crc.sum().to_le_bytes());
        archive[at + 22 + shift..at + 26 + shift]
            .copy_from_slice(&(12_u32 * 1024 * 1024).to_le_bytes());
    }

    let job = upload_zip(&site, id, "bomb.zip", &archive).await;

    assert_eq!(job.status.as_deref(), Some("error"));
    let log = job.log.unwrap_or_default();
    assert!(
        log.trim_end().ends_with(
            "The ZIP was refused: \"bomb.bin\" unpacks to more than 100 times its packed size. Nothing from it was added."
        ),
        "{log}"
    );
    assert!(
        UploadRepo::new(&site.pool)
            .list(ImportId(id))
            .await
            .unwrap()
            .is_empty()
    );
}
