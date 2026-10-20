//! Importing a folder again: rows whose identifier an item already holds are
//! `existing`. They are never created, their problems count nowhere, and the
//! left-out rows list them with the reason `exists`.

#![allow(clippy::expect_used, clippy::unwrap_used)]

use omeka_db::ids::ImportId;
use omeka_db::repos::{ImportRepo, ImportResourceRepo};
use omeka_db::rows::{ImportKind, ImportTarget, NewImport};
use serde_json::{Value, json};

use super::archive::{ArchiveFolder, ArchiveRoot};
use super::catalog::{self, Catalog};
use super::left::left_out_csv;
use super::plan::ImportPlan;
use super::preview::PreviewRequest;
use super::run::{ImportSummary, RunInput, run};
use super::run_tests::{Fixture, fixture, plan};
use super::tags::FsTagReader;
use crate::jsonld::BaseUrl;

fn folder(fixture: &Fixture) -> ArchiveFolder {
    ArchiveRoot::new(&fixture.archive)
        .expect("root")
        .folder("Box")
        .expect("folder")
}

/// The ids of the items an import created, in creation order.
async fn created_items(fixture: &Fixture, import: ImportId) -> Vec<i64> {
    ImportResourceRepo::new(&fixture.pool)
        .list(import, Some(ImportKind::Item))
        .await
        .expect("records")
        .iter()
        .filter_map(|row| match row.target {
            ImportTarget::Item(id) => Some(id.0 as i64),
            _ => None,
        })
        .collect()
}

/// A new draft over `Box`, run with the catalog as it is now.
async fn run_again(fixture: &Fixture) -> (ImportId, ImportSummary) {
    let import = ImportRepo::new(&fixture.pool)
        .create_draft(&NewImport {
            owner_id: None,
            folder: "Box".to_string(),
            plan: plan().to_json(),
        })
        .await
        .expect("draft")
        .id;
    let input = RunInput {
        import_id: import,
        owner: None,
        folder: folder(fixture),
        plan: plan(),
        catalog: catalog::load(&fixture.pool).await.expect("catalog"),
        base_url: BaseUrl::parse("https://archive.example").expect("base"),
    };
    let summary = run(&fixture.pool, &fixture.store, input, |_| {})
        .await
        .expect("second run");
    (import, summary)
}

fn preview(fixture: &Fixture, plan: &ImportPlan, catalog: &Catalog, request: Value) -> Value {
    let request: PreviewRequest = serde_json::from_value(request).expect("request");
    let preview = super::preview(&folder(fixture), plan, catalog, &request).expect("preview");
    serde_json::to_value(preview).expect("json")
}

async fn item_count(fixture: &Fixture) -> i64 {
    sqlx::query_scalar("SELECT COUNT(*) FROM item")
        .fetch_one(&fixture.pool)
        .await
        .expect("count")
}

#[tokio::test]
async fn a_second_run_over_the_same_folder_creates_no_items_and_reports_every_row_existing() {
    let fixture = fixture().await;
    let (_, first, _) = fixture.run().await;
    assert_eq!(first.items, 3);
    assert_eq!(first.existing, 0, "nothing existed before the first run");

    let (second, summary) = run_again(&fixture).await;
    assert_eq!(summary.items, 0, "{summary:?}");
    assert_eq!(summary.existing, 3);
    assert_eq!(summary.left, 3, "existing rows are left out");
    assert!(summary.new_sets.is_empty() && summary.new_types.is_empty());
    assert_eq!(item_count(&fixture).await, 3, "no item was made twice");
    assert!(created_items(&fixture, second).await.is_empty());
}

#[tokio::test]
async fn the_preview_marks_existing_rows_and_counts_them_apart_from_problems() {
    let fixture = fixture().await;
    let (first, _, _) = fixture.run().await;
    let items = created_items(&fixture, first).await;
    std::fs::copy(
        fixture.archive.join("Box/Photos/P_001.jpg"),
        fixture.archive.join("Box/Photos/P_002.jpg"),
    )
    .expect("a new photograph");
    let mut value = plan().to_json();
    value["edits"] = json!({ "L_001": { "dcterms:date": "1900-02-31" } });
    let plan = ImportPlan::from_json(&value).expect("plan");
    let catalog = catalog::load(&fixture.pool).await.expect("catalog");

    let all = preview(
        &fixture,
        &plan,
        &catalog,
        json!({ "include": ["rows", "check"], "rows": { "limit": 200 } }),
    );
    let check = &all["check"];
    assert_eq!(check["existing"], 3);
    assert_eq!(check["create"], 1, "only P_002 is new");
    assert_eq!(
        check["withProblems"], 0,
        "an existing row's bad date counts nowhere"
    );
    assert_eq!(check["skipped"], 0);
    assert_eq!(check["canRun"], true);
    let rows = &all["rows"];
    assert_eq!(rows["existing"], 3);
    assert_eq!(rows["withProblems"], 0);
    assert_eq!(rows["kinds"], json!([]));
    let marks: Vec<(String, Value)> = rows["page"]
        .as_array()
        .expect("page")
        .iter()
        .map(|row| {
            (
                row["identifier"].as_str().unwrap().to_string(),
                row["existing"].clone(),
            )
        })
        .collect();
    assert_eq!(
        marks,
        [
            ("L_001".to_string(), json!({ "itemId": items[0] })),
            ("L_002".to_string(), json!({ "itemId": items[1] })),
            ("P_001".to_string(), json!({ "itemId": items[2] })),
            ("P_002".to_string(), Value::Null),
        ]
    );

    let only = preview(
        &fixture,
        &plan,
        &catalog,
        json!({ "include": ["rows"], "rows": { "existingOnly": true } }),
    );
    assert_eq!(only["rows"]["filtered"], 3);
    let problems = preview(
        &fixture,
        &plan,
        &catalog,
        json!({ "include": ["rows"], "rows": { "problemsOnly": true } }),
    );
    assert_eq!(problems["rows"]["filtered"], 0);
    let dates = preview(
        &fixture,
        &plan,
        &catalog,
        json!({ "include": ["rows"], "rows": { "kind": "date" } }),
    );
    assert_eq!(dates["rows"]["filtered"], 0);
}

#[tokio::test]
async fn left_csv_gives_existing_rows_the_reason_exists_but_not_the_rows_this_import_made() {
    let fixture = fixture().await;
    let (first, _, _) = fixture.run().await;
    let mut value = plan().to_json();
    value["skip"] = json!(["P_001"]);
    let skipping = ImportPlan::from_json(&value).expect("plan");

    let draft = catalog::load(&fixture.pool).await.expect("catalog");
    let csv = left_out_csv(&folder(&fixture), &skipping, &draft, &[], &FsTagReader).expect("csv");
    let reasons: Vec<&str> = csv
        .lines()
        .skip(1)
        .map(|line| line.rsplit(',').next().unwrap())
        .collect();
    assert_eq!(reasons, ["exists", "exists", "exists"], "{csv}");

    let own = catalog::load_for_import(&fixture.pool, first)
        .await
        .expect("catalog");
    assert!(own.existing.is_empty(), "the first import made all three");
    let csv = left_out_csv(&folder(&fixture), &skipping, &own, &[], &FsTagReader).expect("csv");
    assert!(csv.ends_with("\nP_001,Harbour,,,,B-1,skipped\n"), "{csv}");
    assert_eq!(csv.lines().count(), 2, "{csv}");
}
