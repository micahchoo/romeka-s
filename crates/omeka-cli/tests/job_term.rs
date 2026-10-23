//! A stopping server ends its jobs: each job process gets a TERM, and only it, and an import run then
//! ends after the record in hand as `interrupted`.

#![cfg(unix)]
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod import_site;

use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

use import_site::{Site, jpeg, site};
use omeka_api::import::archive::ArchiveRoots;
use omeka_api::import::catalog_load::Owners;
use omeka_api::import::plan::ImportPlan;
use omeka_api::import::preview::Caches;
use omeka_api::import::run::RUN_JOB;
use omeka_api::import::start::{self, Go};
use omeka_db::ids::{ImportId, JobId};
use omeka_db::repos::{ImportRepo, JobRepo};
use omeka_db::rows::ImportStatus;
use omeka_db::{DbPool, Location};
use omeka_server::JobDispatch;
use serde_json::{Value, json};

use omeka_cli::jobs::{JobCommand, ProcessDispatch, compose_job};

const PATTERN: &str = r"^(?<identifier>BHC_006_AF_\d{3})\.jpg$";

/// Wait up to 60 s for `done`.
async fn until(what: &str, mut done: impl AsyncFnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(60);
    while !done().await {
        assert!(Instant::now() < deadline, "{what}");
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

/// Move import `id` to running under a new job, as the run route does, and
/// answer the job.
async fn started(site: &Site, id: u64) -> JobId {
    let pool: DbPool = site.pool.clone().into();
    let row = ImportRepo::new(&site.pool)
        .get(ImportId(id))
        .await
        .unwrap()
        .unwrap();
    let plan = ImportPlan::from_json(&row.plan.to_string()).unwrap();
    let roots = ArchiveRoots::new([("archive".to_string(), site.archive.clone())]).unwrap();
    let store: Arc<dyn omeka_media::store::Store> = site.store.clone();
    let assessed = start::assess(
        &site.pool,
        &roots,
        &store,
        &Arc::new(Caches::default()),
        omeka_api::settings::file_validation(&pool).await.unwrap(),
        Owners::Anyone,
        row.id,
        plan,
    )
    .await
    .unwrap();
    start::begin(&site.pool, &row, &assessed.check, Go::default())
        .await
        .unwrap();
    let job = compose_job(&pool, RUN_JOB, &json!({ "importId": id }), None)
        .await
        .unwrap();
    ImportRepo::new(&site.pool)
        .set_job(ImportId(id), job)
        .await
        .unwrap();
    job
}

fn command(site: &Site, program: &Path) -> JobCommand {
    JobCommand {
        program: Some(program.to_path_buf()),
        database: Location::File(site.dir.path().join("romeka.db")),
        store: Some(site.files.clone()),
        base_url: None,
        archive_root: Some(site.archive.clone()),
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn a_term_ends_the_run_after_its_unit_as_interrupted() {
    let site = site().await;
    let jpeg = jpeg();
    for n in 0..300 {
        site.write_object(&format!("BHC_006_AF_{n:03}.jpg"), &jpeg);
    }
    let id = site
        .draft(|plan| plan["grouping"]["pattern"] = json!(PATTERN))
        .await;
    let job = started(&site, id).await;
    let dispatch = Arc::new(ProcessDispatch::new(command(
        &site,
        Path::new(env!("CARGO_BIN_EXE_romeka")),
    )));
    dispatch.start(job).unwrap();
    until("the run wrote a record", async || {
        site.ledger(id).await.len() >= 2
    })
    .await;

    let stopping = Arc::clone(&dispatch);
    tokio::task::spawn_blocking(move || stopping.shutdown(Duration::from_secs(30)))
        .await
        .unwrap();

    let row = site.row(id).await;
    let summary = row.summary.clone().unwrap_or(Value::Null);
    assert_eq!(row.status, ImportStatus::RunFailed, "{summary}");
    assert!(summary["interrupted"].is_string(), "{summary}");
    assert_eq!(summary["stopped"], false, "{summary}");
    let done = summary["progress"]["done"].as_u64().unwrap();
    assert!(done > 0 && done < 600, "{summary}");
    let media = site.media().await.len();
    let originals = site
        .stored_files()
        .iter()
        .filter(|path| path.starts_with("original"))
        .count();
    assert_eq!(
        originals, media,
        "the record in hand finished: no orphan bytes"
    );
    assert_eq!(
        site.ledger(id).await.len(),
        site.items().await.len() + media,
        "each item the run made is in its ledger"
    );
    let log = JobRepo::new(&site.pool)
        .get(job)
        .await
        .unwrap()
        .unwrap()
        .log
        .unwrap_or_default();
    assert!(log.contains("interrupted"), "{log}");
    let shown = site
        .send("GET", &format!("/api/romeka/imports/{id}"), None)
        .await
        .json();
    assert!(shown["interrupted"].is_string(), "{shown}");
    assert!(shown["summary"].get("interrupted").is_none(), "{shown}");
}

/// The derivative worker runs ffmpeg and soffice as its children. A TERM to
/// one of them fails its copy and spends one of its attempts, so the job, not
/// the server, decides what its children get. A stand-in job here starts a
/// child, and both write down each TERM they get.
#[tokio::test(flavor = "multi_thread")]
async fn a_stopping_server_sends_term_to_each_job_and_not_to_its_children() {
    let site = site().await;
    let dir = tempfile::tempdir().unwrap();
    let at = dir.path();
    let job = at.join("job.sh");
    std::fs::write(
        &job,
        format!(
            "#!/bin/sh\n\
             cd '{dir}'\n\
             sh -c 'trap \"echo child >> signals; exit 0\" TERM; echo $$ > child; \
             while :; do sleep 0.05; done' &\n\
             trap 'echo job >> signals; exit 0' TERM\n\
             echo $$ > job\n\
             while :; do sleep 0.05; done\n",
            dir = at.display()
        ),
    )
    .unwrap();
    std::fs::set_permissions(&job, std::os::unix::fs::PermissionsExt::from_mode(0o755)).unwrap();
    let dispatch = ProcessDispatch::new(command(&site, &job));
    dispatch.start(JobId(1)).unwrap();
    until("the job and its child are up", async || {
        at.join("job").exists() && at.join("child").exists()
    })
    .await;
    let child: u32 = std::fs::read_to_string(at.join("child"))
        .unwrap()
        .trim()
        .parse()
        .unwrap();

    tokio::task::spawn_blocking(move || dispatch.shutdown(Duration::from_secs(5)))
        .await
        .unwrap();

    let signals = std::fs::read_to_string(at.join("signals")).unwrap_or_default();
    assert_eq!(signals, "job\n", "only the job got a TERM");
    until("the job's child is gone after the grace", async || {
        std::fs::read_to_string(format!("/proc/{child}/stat"))
            .map_or(true, |stat| stat.split_whitespace().nth(2) == Some("Z"))
    })
    .await;
}
