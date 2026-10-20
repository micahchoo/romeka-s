//! Import v2 undo end to end (`docs/import-v2.md`, "Lifecycle and the
//! ledger", "Scenarios" S9 and S10): the routes over a real socket, then
//! `Romeka\Job\ImportUndo` through the job runner, then what the install and
//! the store hold.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod import_site;

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use import_site::{Site, col, jpeg, rule, sheet_plan, site, tiff};
use omeka_api::import2::undo::{
    ApiRemoval, Force, Removal, Removed, UNDO_JOB, UndoEnd, undo_import,
};
use omeka_db::ids::ImportId;
use omeka_db::repos::{ImportRepo, JobRepo, StartOutcome};
use omeka_db::rows::{ImportRow, ImportStatus, ImportTarget, NewJob};
use serde_json::{Value, json};

const BHC_PATTERN: &str = r"^(?<set>[^/]+)/(?:[^/]+/)*(?<identifier>BHC_006_[A-Z]{2}_\d{3})(?:_(?<part>[a-z0-9]+))?\.(?:jpg|tif)$";

/// The BHC_006 demo: four artifacts with two sides, four photographs, one
/// scan in three pages.
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

/// A revise of the demo from a register: new titles, the artifacts moved to
/// a set named "Revised", and every item given the Base Resource template
/// and a new site.
async fn revise(site: &Site) -> u64 {
    let made = site
        .send(
            "POST",
            "/api/sites",
            Some(json!({ "o:slug": "revised", "o:title": "Revised", "o:theme": "default" })),
        )
        .await;
    assert_eq!(made.status, 200, "{}", made.body);
    let site_id = made.json()["o:id"].as_u64().unwrap();
    let register: String = std::iter::once("Id,Title,Set\n".to_string())
        .chain((1..=4).map(|n| format!("BHC_006_AF_00{n},Artifact {n},Revised\n")))
        .collect();
    site.write("register.csv", register.as_bytes());
    site.draft(|plan| {
        bhc_plan(plan);
        plan["action"]["action"] = json!("revise");
        // The artifacts leave their set, so they are matched install-wide.
        plan["action"]["match"]["item"]["scope"] = json!("all");
        plan["sheets"] = json!([sheet_plan("register", &["Id", "Title", "Set"])]);
        plan["sheets"][0]["join"] = json!({
            "on": "identifier", "column": { "index": 0, "header": "Id" },
            "normalize": { "caseFold": false, "leadingZeros": false, "separators": false }
        });
        plan["slots"]["item"]["dcterms:title"] =
            rule(json!([col("register", 1, "Title")]), "first");
        plan["slots"]["item"]["sets"] = rule(json!([col("register", 2, "Set")]), "first");
        plan["slots"]["item"]["template"] = rule(
            json!([{ "from": "constant", "value": "Base Resource" }]),
            "first",
        );
        plan["slots"]["item"]["sites"] = rule(
            json!([{ "from": "constant", "value": site_id.to_string() }]),
            "first",
        );
    })
    .await
}

fn summary(row: &ImportRow) -> Value {
    row.summary.clone().unwrap_or(Value::Null)
}

fn report(row: &ImportRow) -> Value {
    summary(row)["undo"].clone()
}

/// Press Undo with `force` and wait for the import to stop undoing.
async fn undo(site: &Site, id: u64, force: Option<Value>) -> ImportRow {
    let body = force.map(|force| json!({ "force": force }));
    let started = site
        .send("POST", &format!("/api/romeka/imports/{id}/undo"), body)
        .await;
    assert_eq!(started.status, 200, "{}", started.body);
    let deadline = Instant::now() + Duration::from_secs(60);
    loop {
        let row = site.row(id).await;
        if row.status != ImportStatus::Undoing {
            return row;
        }
        assert!(Instant::now() < deadline, "import {id} stayed undoing");
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

/// Every item as the API shows it, by id, less `o:modified`, which a restore
/// moves.
async fn items_shown(site: &Site) -> Vec<Value> {
    let mut shown = Vec::new();
    for item in site.items().await {
        let read = site
            .send("GET", &format!("/api/items/{}", item.id().0), None)
            .await;
        assert_eq!(read.status, 200, "{}", read.body);
        let mut document = read.json();
        document.as_object_mut().unwrap().remove("o:modified");
        shown.push(document);
    }
    shown
}

fn originals(site: &Site) -> usize {
    site.stored_files()
        .iter()
        .filter(|path| path.starts_with("original/"))
        .count()
}

/// Wait out the second the import wrote in: times are whole seconds.
async fn next_second() {
    tokio::time::sleep(Duration::from_millis(1100)).await;
}

fn kept_of<'r>(report: &'r Value, kind: &str, id: u64) -> Option<&'r Value> {
    report["kept"]
        .as_array()
        .unwrap()
        .iter()
        .find(|kept| kept["kind"] == kind && kept["id"] == id)
}

fn reasons(kept: &Value) -> Vec<String> {
    kept["reasons"]
        .as_array()
        .unwrap()
        .iter()
        .map(|reason| reason["reason"].as_str().unwrap().to_string())
        .collect()
}

#[tokio::test(flavor = "multi_thread")]
async fn s9_undo_of_a_revise_restores_every_item_exactly() {
    let site = site().await;
    bhc_006(&site);
    let first = site.draft(bhc_plan).await;
    assert_eq!(site.run(first).await.status, ImportStatus::Done);
    let before = items_shown(&site).await;
    let second = revise(&site).await;
    let revised = site.run(second).await;
    assert_eq!(revised.status, ImportStatus::Done, "{}", summary(&revised));
    let artifact = site.item("Artifact 1").await;
    assert_ne!(items_shown(&site).await, before, "the revise changed items");
    let shown = site
        .send("GET", &format!("/api/items/{}", artifact.id().0), None)
        .await
        .json();
    assert!(shown["o:resource_template"].is_object(), "{shown}");
    assert_eq!(shown["o:site"].as_array().map(Vec::len), Some(1), "{shown}");
    assert_eq!(site.item_sets().await.len(), 4, "the revise made a set");

    let row = undo(&site, second, None).await;

    assert_eq!(row.status, ImportStatus::Undone, "{}", summary(&row));
    let report = report(&row);
    assert_eq!(report["restored"]["items"], 9, "{report}");
    assert_eq!(report["removed"]["itemSets"], 1, "the set it made");
    assert_eq!(report["kept"], json!([]));
    assert_eq!(
        items_shown(&site).await,
        before,
        "values, template, sets, sites and primary media as before"
    );
    assert!(site.ledger(second).await.is_empty());
    assert_eq!(site.item_sets().await.len(), 3);
    assert!(
        site.items()
            .await
            .iter()
            .any(|item| item.id() == artifact.id())
    );
    assert_eq!(originals(&site), 15, "a revise added no file to remove");
}

#[tokio::test(flavor = "multi_thread")]
async fn s10_an_edited_item_is_kept_and_listed_then_forced() {
    let site = site().await;
    bhc_006(&site);
    let id = site.draft(bhc_plan).await;
    assert_eq!(site.run(id).await.status, ImportStatus::Done);
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

    let row = undo(&site, id, None).await;

    assert_eq!(row.status, ImportStatus::PartlyUndone, "kept rows stay");
    let report = report(&row);
    let kept = kept_of(&report, "item", edited.id().0).expect("the edited item is kept");
    assert_eq!(reasons(kept), ["edited"]);
    assert_eq!(kept["reasons"][0]["key"], "server.importUndoEdited");
    let media = site.media_of(&edited).await;
    assert_eq!(media.len(), 2, "the kept item keeps its media");
    for one in &media {
        let kept = kept_of(&report, "media", one.id().0).unwrap();
        assert_eq!(reasons(kept), ["itemKept"]);
    }
    let sets = site.item_sets().await;
    assert_eq!(sets.len(), 1, "the set holding the kept item stays");
    assert_eq!(
        reasons(kept_of(&report, "itemSet", sets[0].id().0).unwrap()),
        ["memberKept"]
    );
    assert_eq!(site.items().await.len(), 1, "the rest are undone");
    assert_eq!(report["removed"]["items"], 8);
    assert_eq!(report["removed"]["media"], 13);
    assert_eq!(report["removed"]["itemSets"], 2);
    assert_eq!(originals(&site), 2, "the removed media's files are gone");
    assert_eq!(site.ledger(id).await.len(), 4);

    let forced = undo(
        &site,
        id,
        Some(json!([{ "kind": "item", "id": edited.id().0 }])),
    )
    .await;

    assert_eq!(forced.status, ImportStatus::Undone, "{}", summary(&forced));
    assert_eq!(
        report_of(&forced)["forced"],
        json!([{ "kind": "item", "id": edited.id().0 }])
    );
    assert!(site.items().await.is_empty());
    assert!(site.media().await.is_empty());
    assert!(site.item_sets().await.is_empty());
    assert_eq!(originals(&site), 0);
    assert!(site.ledger(id).await.is_empty());
}

fn report_of(row: &ImportRow) -> Value {
    report(row)
}

/// One of the demo's artifacts: an item with a front and a back.
async fn an_artifact(site: &Site) -> omeka_db::rows::ItemRow {
    for item in site.items().await {
        if site.media_of(&item).await.len() == 2 {
            return item;
        }
    }
    panic!("no artifact item");
}

#[tokio::test(flavor = "multi_thread")]
async fn an_undo_keeps_what_a_later_import_changed_and_names_it() {
    let site = site().await;
    bhc_006(&site);
    let first = site.draft(bhc_plan).await;
    assert_eq!(site.run(first).await.status, ImportStatus::Done);
    next_second().await;
    let second = revise(&site).await;
    assert_eq!(site.run(second).await.status, ImportStatus::Done);

    let row = undo(&site, first, None).await;

    assert_eq!(row.status, ImportStatus::PartlyUndone);
    let report = report(&row);
    let artifact = site.item("Artifact 1").await;
    let kept = kept_of(&report, "item", artifact.id().0).unwrap();
    assert!(
        kept["reasons"].as_array().unwrap().contains(&json!({
            "reason": "laterImport", "import": second,
            "key": "server.importUndoLaterImport",
            "detail": format!("Import {second} changed this after this import did.")
        })),
        "{kept}"
    );
    assert_eq!(site.items().await.len(), 9, "every item was revised later");
    assert_eq!(report["removed"]["items"], 0);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_stopped_run_is_undone_from_what_it_wrote() {
    let site = site().await;
    bhc_006(&site);
    let id = site.draft(bhc_plan).await;
    let gate = site.store.hold_after(4);
    site.start(id).await;
    let gate = tokio::task::spawn_blocking(move || {
        gate.reached.recv_timeout(Duration::from_secs(30)).unwrap();
        gate
    })
    .await
    .unwrap();
    let stop = site
        .send("POST", &format!("/api/romeka/imports/{id}/stop"), None)
        .await;
    assert_eq!(stop.status, 202, "{}", stop.body);
    drop(gate.release);
    let stopped = site.ended(id).await;
    assert_eq!(stopped.status, ImportStatus::RunFailed);
    let written = site.items().await.len();
    assert!((1..9).contains(&written), "{written} items before the stop");

    let row = undo(&site, id, None).await;

    assert_eq!(row.status, ImportStatus::Undone, "{}", summary(&row));
    assert_eq!(report(&row)["removed"]["items"], written);
    assert!(site.items().await.is_empty());
    assert!(site.media().await.is_empty());
    assert!(site.item_sets().await.is_empty(), "the sets it named too");
    assert_eq!(originals(&site), 0);
    assert_eq!(summary(&row)["stopped"], true, "the run's summary stays");
}

/// The API's deletes, failing on the `fail_at`-th call, as a worker that
/// dies in the middle would leave things.
struct FailingRemoval<'a> {
    inner: ApiRemoval<'a>,
    calls: AtomicUsize,
    fail_at: usize,
}

impl Removal for FailingRemoval<'_> {
    fn remove<'a>(
        &'a self,
        target: ImportTarget,
    ) -> Pin<Box<dyn Future<Output = Result<Removed, String>> + Send + 'a>> {
        let call = self.calls.fetch_add(1, Ordering::SeqCst) + 1;
        if call == self.fail_at {
            return Box::pin(async { Err("the worker died".to_string()) });
        }
        self.inner.remove(target)
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn an_interrupted_undo_converges_on_rerun() {
    let site = site().await;
    bhc_006(&site);
    let id = site.draft(bhc_plan).await;
    assert_eq!(site.run(id).await.status, ImportStatus::Done);
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
    let pool = omeka_db::DbPool::from(site.pool.clone());
    let api = omeka_api::read::Api::load_with_store(
        &pool,
        base,
        omeka_api::auth::Actor::new(omeka_db::ids::UserId(0), "global_admin"),
        Some(Arc::clone(&site.store) as Arc<dyn omeka_media::store::Store>),
    )
    .await
    .unwrap();
    let removal = FailingRemoval {
        inner: ApiRemoval { api: &api },
        calls: AtomicUsize::new(0),
        fail_at: 10,
    };

    let end = undo_import(&site.pool, &removal, ImportId(id), job.id, Force::Nothing)
        .await
        .unwrap();

    let UndoEnd::Failed(first) = end else {
        panic!("the failure ends the undo failed: {end:?}");
    };
    assert_eq!(first.removed.media, 9, "nine deletes before the failure");
    assert!(
        first
            .error
            .as_ref()
            .unwrap()
            .detail
            .contains("the worker died")
    );
    assert_eq!(site.row(id).await.status, ImportStatus::UndoFailed);
    assert_eq!(site.media().await.len(), 6);
    JobRepo::new(&site.pool)
        .finish(job.id, "error")
        .await
        .unwrap();

    let row = undo(&site, id, None).await;

    assert_eq!(row.status, ImportStatus::Undone, "{}", summary(&row));
    let report = report(&row);
    assert_eq!(report["removed"]["media"], 6, "the rest of the media");
    assert_eq!(report["removed"]["items"], 9);
    assert_eq!(report["removed"]["itemSets"], 3);
    assert_eq!(report["kept"], json!([]), "its own deletes touch nothing");
    assert!(site.items().await.is_empty() && site.media().await.is_empty());
    assert_eq!(originals(&site), 0);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_database_failure_in_a_record_rolls_back_and_removes_its_bytes() {
    let site = site().await;
    bhc_006(&site);
    sqlx::query(
        "CREATE TRIGGER refuse_media BEFORE INSERT ON media
         BEGIN SELECT RAISE(ABORT, 'media refused'); END",
    )
    .execute(&site.pool)
    .await
    .unwrap();
    let id = site.draft(bhc_plan).await;

    let row = site.run(id).await;

    assert_eq!(
        row.status,
        ImportStatus::RunFailed,
        "every record failed: {}",
        summary(&row)
    );
    let failures = summary(&row)["failures"].as_array().unwrap().clone();
    assert_eq!(failures.len(), 9, "each item with its media failed");
    assert!(
        failures
            .iter()
            .all(|failure| failure["key"] == "server.importRecordNotSaved")
    );
    assert!(
        site.items().await.is_empty(),
        "the item rolled back with its media"
    );
    assert_eq!(originals(&site), 0, "the stored bytes were removed");
    assert!(
        site.stored_files()
            .iter()
            .all(|path| path == &format!("imports/{id}/left.csv")),
        "{:?}",
        site.stored_files()
    );
    sqlx::query("DROP TRIGGER refuse_media")
        .execute(&site.pool)
        .await
        .unwrap();
    let undone = undo(&site, id, None).await;
    assert_eq!(undone.status, ImportStatus::Undone, "{}", summary(&undone));
    assert!(
        site.item_sets().await.is_empty(),
        "the sets made by title go"
    );
}
