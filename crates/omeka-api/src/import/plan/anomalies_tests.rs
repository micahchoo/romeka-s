#![allow(clippy::expect_used)]

use serde_json::{Value, json};

use super::*;

const DEFAULT: &str = include_str!("../fixtures/default.json");

fn with(answers: Value) -> Result<ImportPlanV2, PlanError> {
    let mut value: Value = serde_json::from_str(DEFAULT).expect("fixture json");
    value["anomalies"] = answers;
    ImportPlanV2::from_json(&value.to_string())
}

#[test]
fn a_plan_without_anomalies_has_the_default_answers() {
    let plan = ImportPlanV2::from_json(DEFAULT).expect("a valid plan");
    assert_eq!(
        plan.anomalies,
        AnomalyAnswers {
            identical: IdenticalAnswer::KeepOne,
            unmatched_files: UnmatchedFilesAnswer::Leave,
            items_without_row: ItemsWithoutRowAnswer::Keep,
        }
    );
    let written: Value = serde_json::from_str(&plan.to_json()).expect("json");
    assert!(
        written.get("anomalies").is_none(),
        "the default answers are not written, so a stored plan keeps its bytes and digest"
    );
}

#[test]
fn each_answer_parses_and_round_trips_and_a_missing_one_is_its_default() {
    let plan = with(json!({ "identical": "keepAll", "unmatchedFiles": "own" })).expect("valid");
    assert_eq!(plan.anomalies.identical, IdenticalAnswer::KeepAll);
    assert_eq!(plan.anomalies.unmatched_files, UnmatchedFilesAnswer::Own);
    assert_eq!(
        plan.anomalies.items_without_row,
        ItemsWithoutRowAnswer::Keep
    );
    let written: Value = serde_json::from_str(&plan.to_json()).expect("json");
    assert_eq!(
        written["anomalies"],
        json!({ "identical": "keepAll", "unmatchedFiles": "own", "itemsWithoutRow": "keep" })
    );
    let leave = with(json!({ "itemsWithoutRow": "leave" })).expect("valid");
    assert_eq!(
        leave.anomalies.items_without_row,
        ItemsWithoutRowAnswer::Leave
    );
}

#[test]
fn an_unknown_answer_or_member_is_refused() {
    for answers in [
        json!({ "identical": "keepBoth" }),
        json!({ "unmatchedFiles": "keep" }),
        json!({ "itemsWithoutRow": "own" }),
        json!({ "duplicates": "keepOne" }),
        json!(null),
    ] {
        assert!(
            matches!(with(answers.clone()), Err(PlanError::Shape(_))),
            "{answers}"
        );
    }
}
