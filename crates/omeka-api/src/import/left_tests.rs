//! What a run's summary tells the admin after it ends: the sets it filled,
//! why rows failed, the undo's report, and the left-out rows as CSV.

#![allow(clippy::expect_used, clippy::unwrap_used)]

use omeka_db::repos::{ImportRepo, SettingRepo};
use serde_json::json;

use super::archive::ArchiveRoot;
use super::left::left_out_csv;
use super::plan::ImportPlan;
use super::run::Failure;
use super::run_tests::{fixture, plan};
use super::tags::FsTagReader;

const REFUSE_SECOND_LETTER: &str = "CREATE TRIGGER refuse BEFORE INSERT ON value
     WHEN NEW.value = 'Second letter' BEGIN SELECT RAISE(ABORT, 'value refused'); END";

#[tokio::test]
async fn the_summary_names_the_sets_the_run_filled_and_why_a_row_failed() {
    let fixture = fixture().await;
    sqlx::query(REFUSE_SECOND_LETTER)
        .execute(&fixture.pool)
        .await
        .expect("trigger");
    let (_, summary, _) = fixture.run().await;

    let catalog = super::catalog::load(&fixture.pool).await.expect("catalog");
    let letters = catalog.item_sets["Letters"] as u64;
    assert_eq!(summary.item_set_ids, [letters, fixture.photos.0]);
    assert_eq!(summary.existing_sets, ["Photos"]);
    assert_eq!(summary.failures.len(), 1);
    let failure = &summary.failures[0];
    assert_eq!(failure.identifier, "L_002");
    assert!(failure.reason.contains("value refused"), "{failure:?}");
}

/// The run stores a file the way an upload does, so the install's file
/// validation settings apply: a TIFF master is refused once `tif` leaves the
/// extension list, and its item fails with upstream's message.
#[tokio::test]
async fn a_file_the_validation_settings_refuse_fails_its_item() {
    let fixture = fixture().await;
    SettingRepo::new(&fixture.pool)
        .set("extension_whitelist", &json!(["jpg"]))
        .await
        .expect("setting");
    let (_, summary, _) = fixture.run().await;

    assert_eq!(summary.failures.len(), 1, "{:?}", summary.failures);
    let failure = &summary.failures[0];
    assert_eq!(failure.identifier, "L_001");
    assert!(
        failure.reason.contains(
            "Error validating \"L_001_back.tif\". Cannot store files with the resolved extension \"tif\"."
        ),
        "{failure:?}"
    );
}

#[tokio::test]
async fn the_undo_keeps_its_report_in_the_summary_beside_the_run() {
    let fixture = fixture().await;
    let (import, _, _) = fixture.run().await;
    fixture.undo(import).await;

    let stored = ImportRepo::new(&fixture.pool)
        .get(import)
        .await
        .expect("get")
        .expect("import")
        .summary
        .expect("summary");
    assert_eq!(stored["items"], 3, "the run's counts stay");
    assert_eq!(
        stored["undo"],
        json!({ "items": 3, "media": 4, "itemSets": 1, "templates": 1, "fields": 1, "kept": [] })
    );
}

#[tokio::test]
async fn the_left_out_csv_lists_problem_skipped_and_failed_rows_with_their_reason() {
    let fixture = fixture().await;
    let failures = [
        Failure {
            identifier: "L_002".to_string(),
            reason: "value refused".to_string(),
        },
        Failure {
            identifier: "X_404".to_string(),
            reason: "gone".to_string(),
        },
    ];
    let mut value = plan().to_json();
    value["skip"] = json!(["P_001"]);
    value["edits"] = json!({ "L_001": { "dcterms:created": "1900-02-31" } });
    let plan = ImportPlan::from_json(&value).expect("plan");
    let folder = ArchiveRoot::new(&fixture.archive)
        .expect("root")
        .folder("Box")
        .expect("folder");

    let csv = left_out_csv(&folder, &plan, &fixture.catalog, &failures, &FsTagReader).expect("csv");
    assert_eq!(
        csv,
        "dcterms:identifier,dcterms:title,dcterms:creator,bibo:recipient,dcterms:created,\
         dcterms:spatial,dcterms:description,local:shelfMark,#reason\n\
         L_001,First letter,,,1900-02-31,,,A-1,problems: date (dcterms:created)\n\
         L_002,Second letter,,,,,,A-2,failed: value refused\n\
         P_001,Harbour,,,,,,B-1,skipped\n\
         X_404,,,,,,,,failed: gone\n",
        "the Letter type's fields are grid columns; a failure the folder no longer makes keeps its line"
    );
}
