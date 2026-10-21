//! The outcome of the three frozen shapes, pinned as text before the plan's
//! v3 structure. The shapes need a scanned draft and the install's terms,
//! so they are read through the preview route here; `omeka-api`'s
//! `derive/tests/outcome_pins.rs` pins S1 to S8 in the same text, which
//! this file includes.

#[path = "../../../omeka-api/src/import/derive/tests/outcome_pins/text.rs"]
pub(super) mod text;

use std::path::Path;

use serde_json::Value;

use super::{Shape, bhc006, born_digital, oral_history, run};
use crate::imports_fixture::fixture;

async fn outcome_text(shape: &Shape, slots: &[&str], plan_for: fn(&mut Value)) -> String {
    let fixture = fixture().await;
    let preview = run(&fixture, shape, slots, plan_for).await;
    let records = records_of(&preview);
    outcome_of_records(&preview, &records)
}

/// Every record of a preview read by [`run`], any kind.
pub(super) fn records_of(preview: &Value) -> Vec<Value> {
    ["items", "media", "itemSets"]
        .iter()
        .flat_map(|kind| preview[*kind].as_array().cloned().unwrap_or_default())
        .collect()
}

/// The outcome text of `records` with the preview's new sets and flags.
pub(super) fn outcome_of_records(preview: &Value, records: &[Value]) -> String {
    let new_sets = preview["check"]["newSets"].as_array().unwrap().clone();
    let flags: Vec<(Value, Value, usize)> = preview["flags"]
        .as_array()
        .unwrap()
        .iter()
        .map(|count| {
            let n = usize::try_from(count["count"].as_u64().unwrap()).unwrap();
            (count["kind"].clone(), count["policy"].clone(), n)
        })
        .collect();
    text::outcome(records, &new_sets, &flags)
}

#[tokio::test]
async fn lifted_plans_keep_their_outcome() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/import_shapes/outcome_pins");
    let pins = [
        (
            "bhc006",
            outcome_text(&bhc006::shape(), &bhc006::SLOTS, bhc006::plan_for_bhc006).await,
        ),
        (
            "oral_history",
            outcome_text(
                &oral_history::shape(),
                &oral_history::SLOTS,
                oral_history::plan_for_oral_history,
            )
            .await,
        ),
        (
            "born_digital",
            outcome_text(
                &born_digital::shape(),
                &[],
                born_digital::plan_for_born_digital,
            )
            .await,
        ),
    ];
    for (name, outcome) in pins {
        text::assert_pinned(&dir, name, &outcome);
    }
}
