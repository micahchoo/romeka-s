//! The proposal on each held-out shape, through the preview route, with the
//! shape's pinned edits (`edits/<shape>.json`): its structure is taken into
//! the draft's plan and placed against the frozen v3 plan's placement, as
//! `import_shapes::proposal` does for BHC006. This is the overfitting test:
//! nobody who built the proposer saw these shapes.

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde_json::{Value, json};

use omeka_api::import::placement::{Difference, differences};
use omeka_api::import::plan::ImportPlan;

use super::{
    Shape, accession_register, add_terms, born_digital, materialize, negative_archive,
    oral_history, paper_archive, placement_of, run, v3_plan, v3_plan_of,
};
use crate::imports_fixture::{self, fixture};

/// How far a person is from each held-out shape's frozen outcome, starting
/// from the proposal: `unapplied` counts the pinned edits the proposal does
/// not offer (a click the person needs and cannot make), and `left` counts
/// the differences that remain after every pinned edit (what no click
/// fixes). Both only fall. A measured number above its entry is a
/// regression; one below it fails too, so the commit that lowers a number
/// lowers its entry and the table always states the truth.
#[rustfmt::skip]
const DISTANCE: &[Distance] = &[
    Distance { shape: "accession_register", unapplied: 1, left: 39 },
    Distance { shape: "born_digital",       unapplied: 0, left: 4 },
    Distance { shape: "negative_archive",   unapplied: 2, left: 10 },
    Distance { shape: "oral_history",       unapplied: 0, left: 0 },
    Distance { shape: "paper_archive",      unapplied: 0, left: 4 },
];

#[derive(Debug, PartialEq, Eq)]
struct Distance {
    shape: &'static str,
    unapplied: usize,
    left: usize,
}

fn pinned_edits(name: &str) -> Value {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/heldout/edits")
        .join(format!("{name}.json"));
    let pinned: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    pinned["edits"].clone()
}

/// The plan a person confirms on a draft of `shape`: the proposal's
/// structure with `edits`. Also gives the proposal the route answered.
async fn proposed(shape: &Shape, slots: &[&str], edits: &Value) -> (String, Value) {
    let asked = fixture().await;
    add_terms(&asked, slots).await;
    materialize(shape, asked.archive.path());
    let draft = asked.draft().await;
    let id = draft["id"].clone();
    let scanned = asked
        .call("POST", &format!("/api/romeka/imports/{id}/scan"), None)
        .await;
    assert_eq!(scanned.status, 200, "{}", scanned.body);
    let as_found = json!([imports_fixture::v3_series("all", "")]);
    let mut tried = imports_fixture::v3_plan(&draft["plan"], as_found);
    tried["edits"] = edits.clone();
    let answer = asked
        .preview(&id, &json!({ "include": ["proposal"], "plan": tried }))
        .await;
    assert_eq!(answer.status, 200, "{}", answer.body);
    let proposal = answer.json()["proposal"].clone();
    let mut confirmed = imports_fixture::v3_plan(&draft["plan"], json!([]));
    confirmed["structure"] = proposal["structure"].clone();
    confirmed["edits"] = edits.clone();
    (confirmed.to_string(), proposal)
}

/// A difference's kind. The gate prints kinds and counts only: its output
/// reaches people who must not see a held-out shape's paths.
fn kind(difference: &Difference) -> &'static str {
    match difference {
        Difference::FilesSplit { .. } => "filesSplit",
        Difference::FilesMerged { .. } => "filesMerged",
        Difference::RowItem { .. } => "rowItem",
        Difference::Set { .. } => "set",
        Difference::List { .. } => "list",
        Difference::Row { .. } => "row",
        Difference::LeftOut { .. } => "leftOut",
        Difference::Column { .. } => "column",
    }
}

async fn places_as_frozen(name: &str, shape: &Shape, slots: &[&str], plan_text: &'static str) {
    let frozen = fixture().await;
    let preview = run(&frozen, shape, slots, v3_plan(plan_text)).await;
    let want = placement_of(&preview, &v3_plan_of(plan_text), frozen.archive.path());

    let edits = pinned_edits(name);
    let (text, proposal) = proposed(shape, slots, &edits).await;
    let replayed = proposal["edits"].as_array().unwrap();
    assert_eq!(
        replayed.len(),
        edits.as_array().unwrap().len(),
        "{name}: every pinned edit is replayed"
    );
    let unapplied = replayed
        .iter()
        .filter(|result| result["applied"] != true)
        .count();

    let plan = ImportPlan::from_json(&text).unwrap();
    let saved = fixture().await;
    let got_preview = run(&saved, shape, slots, |draft_plan: &mut Value| {
        let mut plan: Value = serde_json::from_str(&text).unwrap();
        plan["sources"] = draft_plan["sources"].clone();
        *draft_plan = plan;
    })
    .await;
    let got = placement_of(&got_preview, &plan, saved.archive.path());
    let found = differences(&want, &got);

    let mut by_kind: BTreeMap<&str, usize> = BTreeMap::new();
    for difference in &found {
        *by_kind.entry(kind(difference)).or_default() += 1;
    }
    eprintln!(
        "{name}: {unapplied} unapplied, {} left {by_kind:?}",
        found.len()
    );
    let entry = DISTANCE
        .iter()
        .find(|entry| entry.shape == name)
        .unwrap_or_else(|| panic!("{name}: no DISTANCE entry"));
    let measured = Distance {
        shape: entry.shape,
        unapplied,
        left: found.len(),
    };
    assert!(
        measured.unapplied <= entry.unapplied && measured.left <= entry.left,
        "{name}: regressed to {} unapplied, {} left {by_kind:?}; DISTANCE says {}, {}",
        measured.unapplied,
        measured.left,
        entry.unapplied,
        entry.left
    );
    assert_eq!(
        &measured, entry,
        "{name}: improved to {} unapplied, {} left {by_kind:?}; lower its DISTANCE entry from {}, {}",
        measured.unapplied, measured.left, entry.unapplied, entry.left
    );
}

#[tokio::test]
async fn oral_history_proposal_places_as_frozen() {
    places_as_frozen(
        "oral_history",
        &oral_history::shape(),
        &oral_history::SLOTS,
        include_str!("fixtures/oral_history.json"),
    )
    .await;
}

#[tokio::test]
async fn born_digital_proposal_places_as_frozen() {
    places_as_frozen(
        "born_digital",
        &born_digital::shape(),
        &[],
        include_str!("fixtures/born_digital.json"),
    )
    .await;
}

#[tokio::test]
async fn paper_archive_proposal_places_as_frozen() {
    places_as_frozen(
        "paper_archive",
        &paper_archive::shape(),
        &paper_archive::SLOTS,
        paper_archive::PLAN,
    )
    .await;
}

#[tokio::test]
async fn accession_register_proposal_places_as_frozen() {
    places_as_frozen(
        "accession_register",
        &accession_register::shape(),
        &accession_register::SLOTS,
        accession_register::PLAN,
    )
    .await;
}

#[tokio::test]
async fn negative_archive_proposal_places_as_frozen() {
    places_as_frozen(
        "negative_archive",
        &negative_archive::shape(),
        &negative_archive::SLOTS,
        negative_archive::PLAN,
    )
    .await;
}
