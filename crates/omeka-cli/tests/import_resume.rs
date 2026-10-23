//! A run a restart killed goes on by itself when the server boots: the
//! server resumes each
//! failed run whose worker went without the person asking, and never one the
//! person stopped.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod import_site;

use std::sync::Arc;
use std::time::Duration;

use import_site::{Site, jpeg, site};
use omeka_api::import::archive::ArchiveRoots;
use omeka_db::ids::JobId;
use omeka_db::repos::JobRepo;
use omeka_db::rows::{ImportStatus, ImportTarget};
use omeka_server::{JobDispatch, ResumeEnv, resume_imports};
use serde_json::{Value, json};

const PATTERN: &str = r"^(?:[^/]+/)*(?<identifier>BHC_006_AF_\d{3})(?:_(?<part>[a-z0-9]+))?\.jpg$";

fn artifacts(site: &Site) {
    let jpeg = jpeg();
    for n in 1..=6 {
        site.write_object(&format!("Artifacts/BHC_006_AF_00{n}_front.jpg"), &jpeg);
        site.write_object(&format!("Artifacts/BHC_006_AF_00{n}_back.jpg"), &jpeg);
    }
}

fn env(site: &Site) -> ResumeEnv {
    ResumeEnv {
        jobs: Arc::clone(&site.jobs) as Arc<dyn JobDispatch>,
        store: site.store.clone(),
        roots: ArchiveRoots::new([("archive".to_string(), site.archive.clone())]).unwrap(),
    }
}

fn summary(row: &omeka_db::rows::ImportRow) -> Value {
    row.summary.clone().unwrap_or(Value::Null)
}

/// Hold the run at its file after `after`, and answer once it is held.
async fn held(site: &Site, after: usize) -> import_site::GateHandle {
    let gate = site.store.hold_after(after);
    tokio::task::spawn_blocking(move || {
        gate.reached
            .recv_timeout(Duration::from_secs(30))
            .expect("the run reached the held file");
        gate
    })
    .await
    .unwrap()
}

#[tokio::test(flavor = "multi_thread")]
async fn a_run_whose_worker_died_is_resumed_at_boot_and_a_stopped_one_is_not() {
    let site = site().await;
    artifacts(&site);
    let plan = |plan: &mut Value| plan["grouping"]["pattern"] = json!(PATTERN);

    let stopped = site.draft(plan).await;
    site.start(stopped).await;
    let gate = held(&site, 2).await;
    let asked = site
        .send("POST", &format!("/api/romeka/imports/{stopped}/stop"), None)
        .await;
    assert_eq!(asked.status, 202, "{}", asked.body);
    drop(gate.release);
    assert_eq!(site.ended(stopped).await.status, ImportStatus::RunFailed);

    let killed = site.draft(plan).await;
    let job = site.start(killed).await;
    let gate = held(&site, 3).await;
    for task in site.jobs.tasks.lock().unwrap().drain(..) {
        task.abort();
    }
    drop(gate.release);
    JobRepo::new(&site.pool)
        .finish(JobId::from(job), "error")
        .await
        .unwrap();
    let before = summary(&site.row(killed).await)["progress"].clone();

    resume_imports(
        &site.pool.clone().into(),
        &env(&site),
        Duration::from_millis(10),
    )
    .await;

    let resumed = site.row(killed).await;
    assert_ne!(resumed.job_id, Some(JobId::from(job)), "a new job runs it");
    let ended = site.ended(killed).await;
    assert_eq!(ended.status, ImportStatus::Done, "{}", summary(&ended));
    assert_eq!(summary(&ended)["resumed"], 1);
    assert_eq!(
        summary(&ended)["progress"]["total"],
        before["total"],
        "the resume keeps the total"
    );
    let items = site
        .ledger(killed)
        .await
        .into_iter()
        .filter(|row| matches!(row.target, ImportTarget::Item(_)))
        .count();
    assert_eq!(items, 6, "each item once");
    let shown = site
        .send("GET", &format!("/api/romeka/imports/{killed}"), None)
        .await;
    assert_eq!(shown.json()["resumed"], 1, "{}", shown.body);
    let left = site.row(stopped).await;
    assert_eq!(left.status, ImportStatus::RunFailed, "a stopped run stays");
    assert_eq!(summary(&left)["stopped"], true);

    resume_imports(
        &site.pool.clone().into(),
        &env(&site),
        Duration::from_millis(10),
    )
    .await;

    assert_eq!(site.row(killed).await.status, ImportStatus::Done);
    assert_eq!(site.row(stopped).await.status, ImportStatus::RunFailed);
}
