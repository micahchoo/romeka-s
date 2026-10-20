//! The undo report as the admin's TypeScript client reads it: each outcome an
//! undo can end in is driven once, and the import's history row (which holds
//! `undoReport`) is the fixture `web/admin/tests/fixtures/import/undo-report-<name>.json`.
//!
//! The server's wire test cannot drive an undo job, so these files are written
//! here. `UPDATE_FIXTURES=1 cargo test -p omeka-cli --test import_undo_wire`
//! rewrites them; without it a file that differs fails the test. The server's
//! recorder leaves the `undo-report-` files alone.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod import_site;

use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use import_site::{Site, jpeg, rule, site, tiff};
use omeka_api::import2::undo::{
    ApiRemoval, Force, Removal, Removed, UNDO_JOB, UndoEnd, undo_import,
};
use omeka_db::ids::ImportId;
use omeka_db::repos::{ImportRepo, JobRepo, StartOutcome};
use omeka_db::rows::{ImportStatus, ImportTarget, NewJob};
use serde_json::{Map, Value, json};

const BHC_PATTERN: &str = r"^(?<set>[^/]+)/(?:[^/]+/)*(?<identifier>BHC_006_[A-Z]{2}_\d{3})(?:_(?<part>[a-z0-9]+))?\.(?:jpg|tif)$";
const STEADY_TIME: &str = "2026-10-21T09:00:00+00:00";

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
}

fn bhc_plan(plan: &mut Value) {
    plan["grouping"]["pattern"] = json!(BHC_PATTERN);
    plan["slots"]["item"]["sets"] = rule(json!([{ "from": "group", "name": "set" }]), "first");
}

fn sorted(value: Value) -> Value {
    match value {
        Value::Object(members) => Value::Object(
            members
                .into_iter()
                .collect::<std::collections::BTreeMap<_, _>>()
                .into_iter()
                .map(|(key, value)| (key, sorted(value)))
                .collect::<Map<_, _>>(),
        ),
        Value::Array(items) => Value::Array(items.into_iter().map(sorted).collect()),
        other => other,
    }
}

fn steady(value: &mut Value) {
    match value {
        Value::Object(members) => {
            for (key, member) in members.iter_mut() {
                if ["created", "finished", "died"].contains(&key.as_str()) && member.is_string() {
                    *member = json!(STEADY_TIME);
                } else {
                    steady(member);
                }
            }
        }
        Value::Array(items) => items.iter_mut().for_each(steady),
        _ => {}
    }
}

/// The import's row in the history, which carries no plan and so stays small.
async fn history_row(site: &Site, id: u64) -> Value {
    let listed = site
        .send("GET", "/api/romeka/imports?perPage=50", None)
        .await;
    assert_eq!(listed.status, 200, "{}", listed.body);
    listed.json()["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["id"] == id)
        .expect("the import is in the history")
        .clone()
}

fn record(name: &str, mut row: Value) {
    steady(&mut row);
    let text = serde_json::to_string_pretty(&sorted(row)).unwrap() + "\n";
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../web/admin/tests/fixtures/import")
        .join(format!("undo-report-{name}.json"));
    if std::env::var_os("UPDATE_FIXTURES").is_some() {
        std::fs::write(&path, text).unwrap();
    } else {
        assert_eq!(
            std::fs::read_to_string(&path).ok().as_deref(),
            Some(text.as_str()),
            "{name} is out of date; run with UPDATE_FIXTURES=1"
        );
    }
}

/// Press Undo with `force` and wait for the import to stop undoing.
async fn undo(site: &Site, id: u64, force: Option<Value>) {
    let body = force.map(|force| json!({ "force": force }));
    let started = site
        .send("POST", &format!("/api/romeka/imports/{id}/undo"), body)
        .await;
    assert_eq!(started.status, 200, "{}", started.body);
    let deadline = Instant::now() + Duration::from_secs(60);
    while site.row(id).await.status == ImportStatus::Undoing {
        assert!(Instant::now() < deadline, "import {id} stayed undoing");
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

/// Wait out the second the import wrote in: times are whole seconds.
async fn next_second() {
    tokio::time::sleep(Duration::from_millis(1100)).await;
}

async fn an_artifact(site: &Site) -> omeka_db::rows::ItemRow {
    for item in site.items().await {
        if site.media_of(&item).await.len() == 2 {
            return item;
        }
    }
    panic!("no artifact item");
}

async fn imported(site: &Site) -> u64 {
    bhc_006(site);
    let id = site.draft(bhc_plan).await;
    assert_eq!(site.run(id).await.status, ImportStatus::Done);
    id
}

#[tokio::test(flavor = "multi_thread")]
async fn an_edited_item_is_kept_then_forced() {
    let site = site().await;
    let id = imported(&site).await;
    let edited = an_artifact(&site).await;
    next_second().await;
    let patch = site
        .send(
            "PATCH",
            &format!("/api/items/{}", edited.id().0),
            Some(json!({ "o:is_public": true })),
        )
        .await;
    assert_eq!(patch.status, 200, "{}", patch.body);

    undo(&site, id, None).await;
    record("kept", history_row(&site, id).await);

    undo(
        &site,
        id,
        Some(json!([{ "kind": "item", "id": edited.id().0 }])),
    )
    .await;
    assert_eq!(site.row(id).await.status, ImportStatus::Undone);
    record("forced", history_row(&site, id).await);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_later_import_keeps_what_it_changed() {
    let site = site().await;
    let first = imported(&site).await;
    next_second().await;
    let second = site
        .draft(|plan| {
            bhc_plan(plan);
            plan["action"]["action"] = json!("revise");
            plan["action"]["match"]["item"]["scope"] = json!("all");
            plan["slots"]["item"]["dcterms:title"] = rule(
                json!([{ "from": "constant", "value": "Retitled" }]),
                "first",
            );
        })
        .await;
    assert_eq!(site.run(second).await.status, ImportStatus::Done);

    undo(&site, first, None).await;

    record("later-import", history_row(&site, first).await);
}

#[tokio::test(flavor = "multi_thread")]
async fn what_is_already_gone_is_skipped() {
    let site = site().await;
    let id = imported(&site).await;
    let gone = an_artifact(&site).await;
    let removed = site
        .send("DELETE", &format!("/api/items/{}", gone.id().0), None)
        .await;
    assert_eq!(removed.status, 200, "{}", removed.body);

    undo(&site, id, None).await;

    assert_eq!(site.row(id).await.status, ImportStatus::Undone);
    record("skipped", history_row(&site, id).await);
}

/// The API's deletes, failing on the `fail_at`-th call and asking for a stop
/// after the `stop_at`-th, as a dying worker and a cataloguer would.
struct Interfering<'a> {
    inner: ApiRemoval<'a>,
    pool: &'a sqlx::SqlitePool,
    import: u64,
    calls: AtomicUsize,
    fail_at: Option<usize>,
    stop_at: Option<usize>,
}

impl Removal for Interfering<'_> {
    fn remove<'a>(
        &'a self,
        target: ImportTarget,
    ) -> Pin<Box<dyn Future<Output = Result<Removed, String>> + Send + 'a>> {
        let call = self.calls.fetch_add(1, Ordering::SeqCst) + 1;
        if Some(call) == self.fail_at {
            return Box::pin(async { Err("the worker died".to_string()) });
        }
        if Some(call) == self.stop_at {
            return Box::pin(async move {
                // The stop route answers only for a running import, so the request is written as
                // the run's own stop would be.
                sqlx::query("UPDATE romeka_import SET stop_requested = 1 WHERE id = ?")
                    .bind(i64::try_from(self.import).unwrap())
                    .execute(self.pool)
                    .await
                    .map_err(|error| error.to_string())?;
                self.inner.remove(target).await
            });
        }
        self.inner.remove(target)
    }
}

/// Start an undo by hand and drive it with `removal`, then close its job as a
/// dead worker's would be.
async fn undo_by_hand(site: &Site, id: u64, fail_at: Option<usize>, stop_at: Option<usize>) {
    let imports = ImportRepo::new(&site.pool);
    assert_eq!(
        imports.start_undo(ImportId(id)).await.unwrap(),
        StartOutcome::Started
    );
    let job = JobRepo::new(&site.pool)
        .create(&NewJob {
            owner_id: None,
            pid: None,
            status: Some("in_progress".to_string()),
            class: UNDO_JOB.to_string(),
            args: Some(json!({ "importId": id })),
            log: None,
            started: None,
        })
        .await
        .unwrap();
    imports.set_job(ImportId(id), job.id).await.unwrap();
    let base = omeka_api::jsonld::BaseUrl::parse("http://localhost").unwrap();
    let api = omeka_api::read::Api::load_with_store(
        &site.pool,
        base,
        omeka_api::auth::Actor::new(omeka_db::ids::UserId(0), "global_admin"),
        Some(Arc::clone(&site.store) as Arc<dyn omeka_media::store::Store>),
    )
    .await
    .unwrap();
    let removal = Interfering {
        inner: ApiRemoval { api: &api },
        pool: &site.pool,
        import: id,
        calls: AtomicUsize::new(0),
        fail_at,
        stop_at,
    };
    let end = undo_import(&site.pool, &removal, ImportId(id), job.id, Force::Nothing)
        .await
        .unwrap();
    assert!(matches!(end, UndoEnd::Failed(_)), "{end:?}");
    JobRepo::new(&site.pool)
        .finish(job.id, "error")
        .await
        .unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn a_stop_ends_the_undo_after_the_record_in_hand() {
    let site = site().await;
    let id = imported(&site).await;

    undo_by_hand(&site, id, None, Some(3)).await;

    assert_eq!(site.row(id).await.status, ImportStatus::UndoFailed);
    record("stopped", history_row(&site, id).await);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_failed_delete_stops_the_undo_with_its_error() {
    let site = site().await;
    let id = imported(&site).await;

    undo_by_hand(&site, id, Some(10), None).await;

    assert_eq!(site.row(id).await.status, ImportStatus::UndoFailed);
    record("error", history_row(&site, id).await);
}
