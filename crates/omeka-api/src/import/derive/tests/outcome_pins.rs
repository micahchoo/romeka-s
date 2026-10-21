//! The outcome of every existing plan scenario, pinned as text before the
//! plan's v3 structure, so each step of it proves it moved no record. The
//! three frozen archive shapes are pinned through the preview route in the
//! server's `import_shapes/outcome_pins.rs`.

mod text;

use std::path::Path;

use serde_json::{Value, json};

use super::scenarios::S1_TO_S8;
use crate::import::model::Model;

fn outcome_text(model: &Model) -> String {
    let records: Vec<Value> = model
        .records
        .iter()
        .map(|record| {
            let value = json!(record);
            assert_eq!(
                text::ledger_key(&value["origin"]),
                record.origin.key(),
                "the text names a record as the ledger does"
            );
            value
        })
        .collect();
    let new_sets: Vec<Value> = model.new_sets.iter().map(|set| json!(set)).collect();
    let flags: Vec<(Value, Value, usize)> = model
        .flags
        .iter()
        .map(|flag| (json!(flag.kind), json!(flag.policy), 1))
        .collect();
    text::outcome(&records, &new_sets, &flags)
}

#[test]
fn lifted_plans_keep_their_outcome() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/import/derive/tests/outcome_pins");
    for (name, scenario) in S1_TO_S8 {
        text::assert_pinned(&dir, name, &outcome_text(&scenario().derive()));
    }
}
