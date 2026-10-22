//! The held-out shapes' pinned outcomes: the text their v2 plans pinned
//! before the plan's v3 structure (`outcome_pins/<shape>.txt`, never
//! rewritten here), and the design's v3 plans of them, which must make the
//! same text and the outcome each shape freezes. A plan of one series keeps
//! the v2 record keys, so no allowance applies.

use std::path::PathBuf;

use super::{Made, assert_same, born_digital, oral_history, outcome_text, text, v3_outcome};

const ORAL_HISTORY: &str =
    include_str!("../../../omeka-api/src/import/fixtures/v3/oral_history.json");
const BORN_DIGITAL: &str =
    include_str!("../../../omeka-api/src/import/fixtures/v3/born_digital.json");

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
