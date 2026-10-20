//! The import actions end to end (`docs/import-v2.md`, "Actions", "Write
//! details", "Undo details", "Scenarios" S9 and S13): `update`, `replace`
//! and `delete` through the routes and the job runner, then their undo, and
//! the undo preview.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod import_site;

use std::time::{Duration, Instant};

use import_site::{Site, col, jpeg, rule, sheet_plan, site, tiff};
use omeka_api::import::undo::UNDO_JOB;
use omeka_db::ids::{ImportId, PropertyId};
use omeka_db::repos::{ImportRepo, JobRepo, ValueRepo};
use omeka_db::rows::{ImportChange, ImportRow, ImportStatus, NewJob};
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

/// A register sheet joined to the file groups by identifier, each named
/// column mapped to the term after it.
fn register(plan: &mut Value, columns: &[(&str, &str)]) {
    let mut headers = vec!["Id"];
    headers.extend(columns.iter().map(|(header, _)| *header));
    plan["sheets"] = json!([sheet_plan("register", &headers)]);
    plan["sheets"][0]["join"] = json!({
        "on": "identifier", "column": { "index": 0, "header": "Id" },
        "normalize": { "caseFold": false, "leadingZeros": false, "separators": false }
    });
    for (at, (header, term)) in columns.iter().enumerate() {
        plan["slots"]["item"][*term] =
            rule(json!([col("register", at as u32 + 1, header)]), "first");
    }
}

/// The demo created with titles, descriptions and subjects for the
/// artifacts, in sets named by folder.
async fn created(site: &Site) -> u64 {
    bhc_006(site);
    site.write(
        "register.csv",
        b"Id,Title,Description,Subject\n\
          BHC_006_AF_001,Artifact 1,A bowl,Pottery\n\
          BHC_006_AF_002,Artifact 2,A cup,Pottery\n\
          BHC_006_AF_003,Artifact 3,A jug,Pottery\n\
          BHC_006_AF_004,Artifact 4,A lid,Pottery\n",
    );
    let id = site
        .draft(|plan| {
            plan["grouping"]["pattern"] = json!(BHC_PATTERN);
            register(
                plan,
                &[
                    ("Title", "dcterms:title"),
                    ("Description", "dcterms:description"),
                    ("Subject", "dcterms:subject"),
                ],
            );
            plan["slots"]["item"]["sets"] =
                rule(json!([{ "from": "group", "name": "set" }]), "first");
        })
        .await;
    let row = site.run(id).await;
    assert_eq!(row.status, ImportStatus::Done, "{}", summary(&row));
    id
}

/// A second import over the demo with `action`, from a register of titles
/// and descriptions.
async fn rewrite(site: &Site, action: &str, register_csv: &str) -> u64 {
    site.write("register.csv", register_csv.as_bytes());
    site.draft(|plan| {
        plan["grouping"]["pattern"] = json!(BHC_PATTERN);
        plan["action"]["action"] = json!(action);
        plan["action"]["match"]["item"]["scope"] = json!("all");
        register(
            plan,
            &[
                ("Title", "dcterms:title"),
                ("Description", "dcterms:description"),
            ],
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

/// Every item as the API shows it, `o:modified` included: a restore puts it
/// back too.
async fn items_shown(site: &Site) -> Vec<Value> {
    let mut shown = Vec::new();
    for item in site.items().await {
        shown.push(item_shown(site, item.id().0).await);
    }
    shown
}

async fn item_shown(site: &Site, id: u64) -> Value {
    let read = site.send("GET", &format!("/api/items/{id}"), None).await;
    assert_eq!(read.status, 200, "{}", read.body);
    read.json()
}

fn texts(item: &Value, term: &str) -> Vec<String> {
    item[term]
        .as_array()
        .map(|values| {
            values
                .iter()
                .filter_map(|value| value["@value"].as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default()
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

#[tokio::test(flavor = "multi_thread")]
async fn s9_update_clears_empty_cells_and_its_undo_restores_exactly() {
    let site = site().await;
    created(&site).await;
    let artifact = site.item("Artifact 1").await.id().0;
    let cup = site.item("Artifact 2").await.id().0;
    let public = site
        .send(
            "PATCH",
            &format!("/api/items/{cup}"),
            Some(json!({ "o:is_public": true })),
        )
        .await;
    assert_eq!(public.status, 200, "{}", public.body);
    let before = items_shown(&site).await;
    next_second().await;
    site.write(
        "register.csv",
        b"Id,Title,Description,Set\n\
          BHC_006_AF_001,Bowl,,\n\
          BHC_006_AF_002,Cup,A small cup,Artifacts\n",
    );
    let update = site
        .draft(|plan| {
            plan["grouping"]["pattern"] = json!(BHC_PATTERN);
            plan["action"]["action"] = json!("update");
            plan["action"]["match"]["item"]["scope"] = json!("all");
            register(
                plan,
                &[
                    ("Title", "dcterms:title"),
                    ("Description", "dcterms:description"),
                    ("Set", "sets"),
                ],
            );
        })
        .await;

    let row = site.run(update).await;

    assert_eq!(row.status, ImportStatus::Done, "{}", summary(&row));
    let bowl = item_shown(&site, artifact).await;
    assert_eq!(texts(&bowl, "dcterms:title"), ["Bowl"]);
    assert!(
        texts(&bowl, "dcterms:description").is_empty(),
        "an empty cell clears: {bowl}"
    );
    assert_eq!(
        texts(&bowl, "dcterms:subject"),
        ["Pottery"],
        "an unmapped term stays"
    );
    assert_eq!(bowl["o:item_set"], json!([]), "an empty set cell clears");
    let cup = item_shown(&site, cup).await;
    assert_eq!(texts(&cup, "dcterms:description"), ["A small cup"]);
    assert_eq!(cup["o:item_set"].as_array().map(Vec::len), Some(1));
    assert_eq!(
        cup["o:is_public"], false,
        "the default visibility is written, as upstream writes its default"
    );
    assert!(
        site.items()
            .await
            .iter()
            .all(|item| item.resource.title.as_deref() != Some("Artifact 3")),
        "a mapped title with no cell clears too"
    );
    assert_eq!(site.media().await.len(), 15, "update adds no media");

    let undone = undo(&site, update, None).await;

    assert_eq!(undone.status, ImportStatus::Undone, "{}", summary(&undone));
    assert_eq!(report(&undone)["restored"]["items"], 9);
    assert_eq!(items_shown(&site).await, before, "exactly, o:modified too");
}

#[tokio::test(flavor = "multi_thread")]
async fn s9_replace_writes_only_the_slots_keeps_media_and_its_undo_restores_exactly() {
    let site = site().await;
    created(&site).await;
    let artifact = site.item("Artifact 1").await;
    let media = site.media_of(&artifact).await;
    let before = items_shown(&site).await;
    next_second().await;
    let replace = rewrite(
        &site,
        "replace",
        "Id,Title,Description\nBHC_006_AF_001,Bowl,\n",
    )
    .await;

    let row = site.run(replace).await;

    assert_eq!(row.status, ImportStatus::Done, "{}", summary(&row));
    let bowl = item_shown(&site, artifact.id().0).await;
    assert_eq!(texts(&bowl, "dcterms:title"), ["Bowl"]);
    assert!(
        texts(&bowl, "dcterms:subject").is_empty(),
        "every value went"
    );
    assert_eq!(
        texts(&bowl, "dcterms:identifier"),
        ["BHC_006_AF_001"],
        "the identifier is a slot of the record, so it is written again"
    );
    assert_eq!(bowl["o:item_set"], json!([]), "unmapped sets are cleared");
    assert_eq!(site.media_of(&artifact).await, media, "existing media kept");
    assert_eq!(originals(&site), 15);

    let undone = undo(&site, replace, None).await;

    assert_eq!(undone.status, ImportStatus::Undone, "{}", summary(&undone));
    assert_eq!(items_shown(&site).await, before, "exactly, o:modified too");
}

#[tokio::test(flavor = "multi_thread")]
async fn undoing_b_then_a_keeps_nothing_of_a_as_edited() {
    let site = site().await;
    created(&site).await;
    let before = items_shown(&site).await;
    next_second().await;
    let a = rewrite(
        &site,
        "update",
        "Id,Title,Description\nBHC_006_AF_001,Bowl A,\n",
    )
    .await;
    assert_eq!(site.run(a).await.status, ImportStatus::Done);
    next_second().await;
    let b = rewrite(
        &site,
        "update",
        "Id,Title,Description\nBHC_006_AF_001,Bowl B,\n",
    )
    .await;
    assert_eq!(site.run(b).await.status, ImportStatus::Done);
    next_second().await;

    let undone_b = undo(&site, b, None).await;
    assert_eq!(
        undone_b.status,
        ImportStatus::Undone,
        "{}",
        summary(&undone_b)
    );
    next_second().await;
    let undone_a = undo(&site, a, None).await;

    assert_eq!(
        undone_a.status,
        ImportStatus::Undone,
        "{}",
        summary(&undone_a)
    );
    assert_eq!(report(&undone_a)["kept"], json!([]));
    assert_eq!(items_shown(&site).await, before);
}

#[tokio::test(flavor = "multi_thread")]
async fn s13_a_delete_needs_its_count_takes_media_and_files_and_is_never_undone() {
    let site = site().await;
    created(&site).await;
    assert_eq!(originals(&site), 15);
    let delete = site
        .draft(|plan| {
            plan["grouping"]["pattern"] = json!(BHC_PATTERN);
            plan["action"]["action"] = json!("delete");
            plan["action"]["match"]["item"]["scope"] = json!("all");
        })
        .await;
    let check = site.check(delete).await;
    assert_eq!(check["deletes"], 9, "{check}");
    let run = format!("/api/romeka/imports/{delete}/run");
    for confirm in [None, Some(8)] {
        let mut body = json!({ "digest": check["digest"] });
        if let Some(confirm) = confirm {
            body["confirm"] = json!(confirm);
        }
        let refused = site.send("POST", &run, Some(body)).await;
        assert_eq!(refused.status, 409, "{}", refused.body);
        assert_eq!(refused.json()["code"], "confirm");
        assert_eq!(refused.json()["deletes"], 9);
    }
    assert_eq!(site.row(delete).await.status, ImportStatus::Draft);

    let started = site
        .send(
            "POST",
            &run,
            Some(json!({ "digest": check["digest"], "confirm": 9 })),
        )
        .await;
    assert_eq!(started.status, 200, "{}", started.body);
    let row = site.ended(delete).await;

    assert_eq!(row.status, ImportStatus::Done, "{}", summary(&row));
    assert!(site.items().await.is_empty());
    assert!(site.media().await.is_empty(), "an item takes its media");
    assert_eq!(originals(&site), 0, "and their files");
    assert_eq!(site.item_sets().await.len(), 3, "sets are not matched");
    let ledger = site.ledger(delete).await;
    assert_eq!(ledger.len(), 9);
    assert!(ledger.iter().all(|row| row.change == ImportChange::Deleted));

    let undo = site
        .send("POST", &format!("/api/romeka/imports/{delete}/undo"), None)
        .await;
    assert_eq!(undo.status, 409, "{}", undo.body);
    assert_eq!(undo.json()["code"], "cannotUndo");
    assert_eq!(
        undo.json()["errors"]["error"],
        "An import that deleted items, media or item sets cannot be undone.",
        "{}",
        undo.body
    );
    let preview = site
        .send(
            "GET",
            &format!("/api/romeka/imports/{delete}/undo/preview"),
            None,
        )
        .await;
    assert_eq!(preview.status, 409, "{}", preview.body);
    assert_eq!(site.row(delete).await.status, ImportStatus::Done);
}

#[tokio::test(flavor = "multi_thread")]
async fn the_undo_preview_answers_what_the_undo_then_does() {
    let site = site().await;
    let id = created(&site).await;
    let edited = site.item("Artifact 2").await.id().0;
    next_second().await;
    let patch = site
        .send(
            "PATCH",
            &format!("/api/items/{edited}"),
            Some(json!({ "o:is_public": true })),
        )
        .await;
    assert_eq!(patch.status, 200, "{}", patch.body);
    let ledger = site.ledger(id).await;
    let uri = format!("/api/romeka/imports/{id}/undo/preview");

    let preview = site.send("GET", &uri, None).await;

    assert_eq!(preview.status, 200, "{}", preview.body);
    let preview = preview.json();
    let kept = preview["kept"].as_array().unwrap();
    assert!(
        kept.contains(&json!({ "kind": "item", "id": edited, "reasons": [{
            "reason": "edited", "key": "server.importUndoEdited",
            "detail": "This was edited after the import wrote it."
        }] })),
        "{preview}"
    );
    assert_eq!(
        kept.len(),
        4,
        "the item, its two media and its set: {preview}"
    );
    assert_eq!(preview["remove"]["items"], 8);
    assert_eq!(site.ledger(id).await, ledger, "the preview wrote nothing");
    let forced = site.send("GET", &format!("{uri}?force=all"), None).await;
    assert_eq!(forced.status, 200, "{}", forced.body);
    assert_eq!(forced.json()["kept"], json!([]));
    assert_eq!(forced.json()["remove"]["items"], 9);
    assert_eq!(
        forced.json()["forced"],
        json!([{ "kind": "item", "id": edited }])
    );

    let row = undo(&site, id, None).await;

    assert_eq!(row.status, ImportStatus::PartlyUndone, "{}", summary(&row));
    let report = report(&row);
    for member in ["kept", "skipped", "forced"] {
        assert_eq!(report[member], preview[member], "{member}");
    }
    assert_eq!(report["removed"], preview["remove"]);
    assert_eq!(report["restored"], preview["restore"]);
}

#[tokio::test(flavor = "multi_thread")]
async fn the_stop_route_stops_an_undo() {
    let site = site().await;
    let id = created(&site).await;
    let imports = ImportRepo::new(&site.pool);
    imports.start_undo(ImportId(id)).await.unwrap();
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

    let stop = site
        .send("POST", &format!("/api/romeka/imports/{id}/stop"), None)
        .await;

    assert_eq!(stop.status, 202, "{}", stop.body);
    assert!(imports.stop_requested(ImportId(id)).await.unwrap());
    imports
        .finish(ImportId(id), omeka_db::rows::ImportEnding::UndoFailed)
        .await
        .unwrap();
    let ended = site
        .send("POST", &format!("/api/romeka/imports/{id}/stop"), None)
        .await;
    assert_eq!(ended.status, 409, "{}", ended.body);
    assert_eq!(ended.json()["code"], "notRunning");
}

async fn property(site: &Site, term: &str) -> PropertyId {
    let (prefix, local) = term.split_once(':').unwrap();
    let vocabulary = omeka_db::repos::VocabularyRepo::new(&site.pool)
        .get_by_prefix(prefix)
        .await
        .unwrap()
        .unwrap();
    omeka_db::repos::PropertyRepo::new(&site.pool)
        .find(vocabulary.id, local)
        .await
        .unwrap()
        .unwrap()
        .id
}

fn letters_plan(plan: &mut Value) {
    plan["sheets"] = json!([sheet_plan("letters", &["Id", "Title", "Related"])]);
    plan["sheets"][0]["columns"][2]["options"]["type"] = json!({ "resource": "identifier" });
    plan["slots"]["item"]["dcterms:identifier"] = rule(json!([col("letters", 0, "Id")]), "first");
    plan["slots"]["item"]["dcterms:title"] = rule(json!([col("letters", 1, "Title")]), "first");
    plan["slots"]["item"]["dcterms:relation"] =
        rule(json!([col("letters", 2, "Related")]), "first");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_revise_replaces_a_term_that_holds_only_links_to_its_own_records() {
    let site = site().await;
    site.write(
        "letters.csv",
        b"Id,Title,Related\nL1,First letter,\nL2,Reply,L1\n",
    );
    let first = site.draft(letters_plan).await;
    assert_eq!(site.run(first).await.status, ImportStatus::Done);
    site.write("letters.csv", b"Id,Title,Related\nL2,Reply,L3\nL3,Third,\n");
    let revise = site
        .draft(|plan| {
            letters_plan(plan);
            plan["action"]["action"] = json!("revise");
            plan["action"]["unmatched"] = json!("create");
            plan["action"]["match"]["item"]["scope"] = json!("all");
        })
        .await;

    let row = site.run(revise).await;

    assert_eq!(row.status, ImportStatus::Done, "{}", summary(&row));
    let relation = property(&site, "dcterms:relation").await;
    let reply = site.item("Reply").await;
    let third = site.item("Third").await;
    let links = ValueRepo::new(&site.pool)
        .list_for_property(reply.resource.id, relation)
        .await
        .unwrap();
    assert_eq!(
        links
            .iter()
            .map(|link| link.value_resource_id)
            .collect::<Vec<_>>(),
        [Some(third.resource.id)],
        "the link to the new record replaces the old link"
    );

    let undone = undo(&site, revise, None).await;

    assert_eq!(undone.status, ImportStatus::Undone, "{}", summary(&undone));
    let links = ValueRepo::new(&site.pool)
        .list_for_property(reply.resource.id, relation)
        .await
        .unwrap();
    assert_eq!(
        links[0].value_resource_id,
        Some(site.item("First letter").await.resource.id)
    );
    assert_eq!(links.len(), 1);
}

#[tokio::test(flavor = "multi_thread")]
async fn update_and_replace_write_the_default_item_type_as_upstream_does() {
    let site = site().await;
    created(&site).await;
    let templates = site.send("GET", "/api/resource_templates", None).await;
    let template = templates.json()[0]["o:id"].as_u64().unwrap();
    for action in ["replace", "update"] {
        let patch = site
            .send(
                "PATCH",
                &format!("/api/items/{}", site.item("Artifact 1").await.id().0),
                Some(json!({ "o:resource_template": null })),
            )
            .await;
        assert_eq!(patch.status, 200, "{}", patch.body);
        site.write("register.csv", b"Id,Title\nBHC_006_AF_001,Artifact 1\n");
        let id = site
            .draft(|plan| {
                plan["grouping"]["pattern"] = json!(BHC_PATTERN);
                plan["action"]["action"] = json!(action);
                plan["action"]["match"]["item"]["scope"] = json!("all");
                plan["defaults"]["template"] = json!(template);
                register(plan, &[("Title", "dcterms:title")]);
            })
            .await;
        assert_eq!(site.run(id).await.status, ImportStatus::Done, "{action}");
        let shown = item_shown(&site, site.item("Artifact 1").await.id().0).await;
        assert_eq!(
            shown["o:resource_template"]["o:id"], template,
            "{action}: {shown}"
        );
    }
}
