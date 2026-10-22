//! The held-out shapes' pinned outcomes (`outcome_pins/<shape>.txt`, never
//! rewritten here) and their v3 plans (`fixtures/`). Two shapes had v2
//! plans, whose text was pinned before the plan's v3 structure; the paper
//! archive has only its v3 plan, written with the shape. Each v3 plan must
//! make the pinned text and the outcome its shape freezes. A plan of one
//! series keeps the v2 record keys, so no allowance applies.

use std::path::PathBuf;

use omeka_api::import::plan::{ImportPlan, PlanVersion};
use serde_json::Value;

use super::{
    Made, accession_register, assert_same, born_digital, negative_archive, oral_history,
    outcome_text, paper_archive, text, v3_outcome,
};

const ORAL_HISTORY: &str = include_str!("fixtures/oral_history.json");
const BORN_DIGITAL: &str = include_str!("fixtures/born_digital.json");

/// Every held-out shape by its name, with its v3 plan.
const PLANS: [(&str, &str); 5] = [
    ("oral_history", ORAL_HISTORY),
    ("born_digital", BORN_DIGITAL),
    ("paper_archive", paper_archive::PLAN),
    ("accession_register", accession_register::PLAN),
    ("negative_archive", negative_archive::PLAN),
];

fn pins() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/heldout/outcome_pins")
}

fn assert_same_text(name: &str, made: &Made) {
    let pinned = std::fs::read_to_string(pins().join(format!("{name}.txt"))).unwrap();
    assert_eq!(made.series_keys, 0, "one series keeps group:<key>");
    assert!(
        pinned == made.text,
        "{name}: {}",
        text::block_diff(&pinned, &made.text)
    );
}

#[tokio::test]
async fn lifted_plans_keep_their_outcome() {
    let oral = outcome_text(
        &oral_history::shape(),
        &oral_history::SLOTS,
        oral_history::plan_for_oral_history,
    )
    .await;
    text::assert_pinned(&pins(), "oral_history", &oral);
    let born = outcome_text(
        &born_digital::shape(),
        &[],
        born_digital::plan_for_born_digital,
    )
    .await;
    text::assert_pinned(&pins(), "born_digital", &born);
}

#[tokio::test]
async fn oral_history_v3_plan_makes_the_frozen_outcome() {
    let made = v3_outcome(&oral_history::shape(), &oral_history::SLOTS, ORAL_HISTORY).await;
    assert_same_text("oral_history", &made);
    assert_same(&made.outcome, &oral_history::expected());
}

#[tokio::test]
async fn born_digital_v3_plan_makes_the_frozen_outcome() {
    let made = v3_outcome(&born_digital::shape(), &[], BORN_DIGITAL).await;
    assert_same_text("born_digital", &made);
    assert_same(&made.outcome, &born_digital::expected());
}

#[tokio::test]
async fn paper_archive_v3_plan_makes_the_frozen_outcome() {
    let made = v3_outcome(
        &paper_archive::shape(),
        &paper_archive::SLOTS,
        paper_archive::PLAN,
    )
    .await;
    assert_same(&made.outcome, &paper_archive::expected());
    assert_eq!(made.series_keys, 0, "one series keeps group:<key>");
    text::assert_pinned(&pins(), "paper_archive", &made.text);
}

#[tokio::test]
async fn accession_register_v3_plan_makes_the_frozen_outcome() {
    let made = v3_outcome(
        &accession_register::shape(),
        &accession_register::SLOTS,
        accession_register::PLAN,
    )
    .await;
    assert_same(&made.outcome, &accession_register::expected());
    assert_eq!(made.series_keys, 0, "one series keeps group:<key>");
    text::assert_pinned(&pins(), "accession_register", &made.text);
}

#[tokio::test]
async fn negative_archive_v3_plan_makes_the_frozen_outcome() {
    let made = v3_outcome(
        &negative_archive::shape(),
        &negative_archive::SLOTS,
        negative_archive::PLAN,
    )
    .await;
    assert_same(&made.outcome, &negative_archive::expected());
    assert_eq!(made.series_keys, 0, "one series keeps group:<key>");
    text::assert_pinned(&pins(), "negative_archive", &made.text);
}

/// The held-out v3 plans are not in `omeka-api`'s fixtures, so its round
/// trip does not see them; this one does.
#[test]
fn the_held_out_plans_parse_and_write_back() {
    for (name, plan_text) in [
        ("oral_history", ORAL_HISTORY),
        ("born_digital", BORN_DIGITAL),
    ] {
        let plan =
            ImportPlan::from_json(plan_text).unwrap_or_else(|error| panic!("{name}: {error}"));
        assert_eq!(plan.version, PlanVersion::V3, "{name}");
        let compact = serde_json::from_str::<Value>(plan_text)
            .unwrap()
            .to_string();
        assert_eq!(
            plan.to_json(),
            compact,
            "{name}: written back byte for byte"
        );
    }
}

/// The pinned edits are data until the plan has an `edits` member: each is
/// an `Edit` of the design, on the wire, with the reason the archivist makes
/// it.
#[test]
fn the_pinned_edits_are_edits() {
    const KINDS: [&str; 9] = [
        "leaveOut", "keep", "items", "list", "grain", "addSet", "dropSet", "pair", "title",
    ];
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/heldout/edits");
    for (name, _) in PLANS {
        let text = std::fs::read_to_string(dir.join(format!("{name}.json"))).unwrap();
        let pinned: Value = serde_json::from_str(&text).unwrap();
        let edits = pinned["edits"].as_array().unwrap();
        let because = pinned["because"].as_array().unwrap();
        assert_eq!(edits.len(), because.len(), "{name}: one reason per edit");
        for edit in edits {
            let kind = edit["edit"].as_str().unwrap_or_default();
            assert!(KINDS.contains(&kind), "{name}: {edit} is no edit");
        }
    }
}
