//! The proposal on each held-out shape, through the preview route, with the
//! shape's pinned edits (`edits/<shape>.json`): its structure is taken into
//! the draft's plan and placed against the frozen v3 plan's placement, as
//! `import_shapes::proposal` does for BHC006. This is the overfitting test:
//! nobody who built the proposer saw these shapes.

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use serde_json::{Value, json};

use omeka_api::import::placement::{Difference, FileAt, ItemAt, differences};
use omeka_api::import::plan::ImportPlan;

use super::{
    Shape, accession_register, add_terms, born_digital, materialize, negative_archive,
    oral_history, paper_archive, placement_of, run, v3_plan, v3_plan_of,
};
use crate::imports_fixture::{self, fixture};

/// The held-out shapes whose proposal places as frozen with every pinned
/// edit applied. A listed shape that stops placing so fails, and so does an
/// unlisted one that starts: the list moves by hand, with the reason in the
/// commit.
const HELDOUT_PASSING: &[&str] = &["oral_history"];

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

fn file(at: &FileAt) -> String {
    at.1.as_str().to_string()
}

fn item(at: &ItemAt) -> String {
    match at {
        ItemAt::File(at) => file(at),
        ItemAt::Row((list, line)) => format!("{}:{line}", list.file.as_str()),
    }
}

fn files(set: &BTreeSet<FileAt>) -> String {
    let names: Vec<String> = set.iter().map(file).collect();
    format!("[{}]", names.join(", "))
}

fn items(set: Option<&BTreeSet<ItemAt>>) -> String {
    set.map_or_else(
        || "none".to_string(),
        |set| {
            let names: Vec<String> = set.iter().map(item).collect();
            format!("[{}]", names.join(", "))
        },
    )
}

fn side(want: bool) -> &'static str {
    if want { "want" } else { "got" }
}

/// A difference's kind, and the paths that show it, on one line.
fn described(difference: &Difference) -> (&'static str, String) {
    match difference {
        Difference::FilesSplit { want, got } => {
            let got: Vec<String> = got.iter().map(files).collect();
            (
                "filesSplit",
                format!("want {} got {}", files(want), got.join(" ")),
            )
        }
        Difference::FilesMerged { want, got } => {
            let want: Vec<String> = want.iter().map(files).collect();
            (
                "filesMerged",
                format!("want {} got {}", want.join(" "), files(got)),
            )
        }
        Difference::RowItem {
            item: at,
            want_made,
        } => (
            "rowItem",
            format!("{} made only by {}", item(at), side(*want_made)),
        ),
        Difference::Set { want, got } => (
            "set",
            format!("want {} got {}", items(want.as_ref()), items(got.as_ref())),
        ),
        Difference::List { list } => ("list", list.file.as_str().to_string()),
        Difference::Row { row: (list, line) } => ("row", format!("{}:{line}", list.file.as_str())),
        Difference::LeftOut {
            file: at,
            want_left_out,
        } => (
            "leftOut",
            format!("{} left out only by {}", file(at), side(*want_left_out)),
        ),
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
    let unapplied: Vec<String> = replayed
        .iter()
        .filter(|result| result["applied"] != true)
        .map(|result| format!("unapplied: {}", result["edit"]))
        .collect();

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
    let mut lines = unapplied.clone();
    for difference in &found {
        let (kind, text) = described(difference);
        *by_kind.entry(kind).or_default() += 1;
        lines.push(format!("{kind}: {text}"));
    }
    let report = format!(
        "{name}: {} unapplied edits, {} differences {by_kind:?}\n  {}",
        unapplied.len(),
        found.len(),
        lines.join("\n  ")
    );
    eprintln!("{report}");
    assert_eq!(
        unapplied.is_empty() && found.is_empty(),
        HELDOUT_PASSING.contains(&name),
        "HELDOUT_PASSING is wrong for {report}"
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
