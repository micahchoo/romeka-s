//! The design's v3 plans of the three frozen shapes
//! (`omeka-api/src/import/fixtures/v3/`), run through the preview and
//! compared with the outcomes their v2 plans pinned
//! (`outcome_pins/<shape>.txt`, never rewritten here).
//!
//! Two differences are allowed, each named where it is applied:
//!
//! - Record keys. A plan of several series keys a group item
//!   `group:<series>/<key>`, where v2 wrote `group:<key>`. The full keys must
//!   be unique; the `<series>/` part is then dropped from the v3 side.
//! - Set memberships, until U5. The engine refuses set levels and a
//!   series' own set, so a fixture that has them runs without them, and
//!   both sides are compared with their `slot sets` and `set` lines
//!   removed. U5 deletes this allowance.
//!
//! The run reads the shape from the draft's own source, so the v3 plan
//! takes the draft's `sources`, as the v2 plan does.

use std::collections::BTreeSet;
use std::path::Path;

use serde_json::Value;

use super::outcome_pins::{outcome_of_records, records_of, text};
use super::{Shape, bhc006, born_digital, oral_history, run};
use crate::imports_fixture::fixture;

const BHC006: &str = include_str!("../../../omeka-api/src/import/fixtures/v3/bhc006.json");
const ORAL_HISTORY: &str =
    include_str!("../../../omeka-api/src/import/fixtures/v3/oral_history.json");
const BORN_DIGITAL: &str =
    include_str!("../../../omeka-api/src/import/fixtures/v3/born_digital.json");

/// Whether a fixture needs what U5 builds: set levels or a series' set.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Sets {
    Kept,
    /// The set-memberships allowance. U5 deletes it.
    DroppedUntilU5,
}

/// The levels without their sets: each set level is replaced by the
/// levels under it, and each series loses its own set.
fn without_sets(levels: &[Value]) -> Vec<Value> {
    levels
        .iter()
        .flat_map(|level| match level["kind"].as_str() {
            Some("set") => without_sets(level["levels"].as_array().unwrap()),
            _ => {
                let mut series = level.clone();
                series["set"] = Value::Null;
                vec![series]
            }
        })
        .collect()
}

fn plan_for(fixture_text: &'static str, sets: Sets) -> impl FnOnce(&mut Value) {
    move |draft_plan: &mut Value| {
        let mut plan: Value = serde_json::from_str(fixture_text).unwrap();
        plan["sources"] = draft_plan["sources"].clone();
        if sets == Sets::DroppedUntilU5 {
            let levels = plan["structure"]["levels"].as_array().unwrap().clone();
            plan["structure"]["levels"] = Value::Array(without_sets(&levels));
        }
        *draft_plan = plan;
    }
}

/// The record-key allowance: assert the ledger keys are unique, then write
/// each `group:<series>/<key>` origin as the `group:<key>` v2 wrote. Gives
/// how many origins it rewrote.
fn strip_series(records: &mut [Value]) -> usize {
    let keys: Vec<String> = records
        .iter()
        .map(|record| text::ledger_key(&record["origin"]))
        .collect();
    let unique: BTreeSet<&String> = keys.iter().collect();
    assert_eq!(unique.len(), keys.len(), "two records share a ledger key");
    let mut rewrote = 0;
    for record in records {
        let pattern = record
            .pointer_mut("/origin/group/pattern")
            .filter(|pattern| pattern.is_object());
        if let Some(pattern) = pattern {
            *pattern = pattern["text"].clone();
            rewrote += 1;
        }
    }
    rewrote
}

/// The set-memberships allowance on one side's text.
fn without_set_lines(text: &str) -> String {
    text.lines()
        .filter(|line| !line.starts_with("  slot sets ") && !line.starts_with("  set "))
        .map(|line| format!("{line}\n"))
        .collect()
}

/// What a v3 plan made of a shape.
struct Made {
    text: String,
    /// Group items whose key names their series.
    series_keys: usize,
    /// The file paths the preview leaves out as `outside`.
    outside: BTreeSet<String>,
}

async fn v3_outcome(shape: &Shape, slots: &[&str], fixture_text: &'static str, sets: Sets) -> Made {
    let fixture = fixture().await;
    let preview = run(&fixture, shape, slots, plan_for(fixture_text, sets)).await;
    let outside = preview["entries"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|entry| entry["left"] == "outside")
        .filter_map(|entry| entry.pointer("/entry/file/path")?.as_str())
        .map(str::to_string)
        .collect();
    let mut records = records_of(&preview);
    let series_keys = strip_series(&mut records);
    Made {
        text: outcome_of_records(&preview, &records),
        series_keys,
        outside,
    }
}

#[tokio::test]
async fn v3_plans_match_the_frozen_outcomes() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/import_shapes/outcome_pins");
    let bhc = v3_outcome(
        &bhc006::shape(),
        &bhc006::SLOTS,
        BHC006,
        Sets::DroppedUntilU5,
    )
    .await;
    assert!(bhc.series_keys > 0, "BHC006's five series name their keys");
    let extras = bhc006::shape()
        .files
        .into_iter()
        .map(|(path, _)| path)
        .filter(|path| path.starts_with("Extras/") || path.starts_with("tiny-iiif/"));
    for path in extras {
        assert!(bhc.outside.contains(&path), "{path} is in no scope");
    }
    let oral = v3_outcome(
        &oral_history::shape(),
        &oral_history::SLOTS,
        ORAL_HISTORY,
        Sets::DroppedUntilU5,
    )
    .await;
    let born = v3_outcome(&born_digital::shape(), &[], BORN_DIGITAL, Sets::Kept).await;
    for one_series in [&oral, &born] {
        assert_eq!(one_series.series_keys, 0, "one series keeps group:<key>");
    }

    let mut failures = Vec::new();
    let shapes = [
        ("bhc006", bhc, Sets::DroppedUntilU5),
        ("oral_history", oral, Sets::DroppedUntilU5),
        ("born_digital", born, Sets::Kept),
    ];
    for (name, made, sets) in shapes {
        let pinned = std::fs::read_to_string(dir.join(format!("{name}.txt"))).unwrap();
        let (pinned, actual) = match sets {
            Sets::Kept => (pinned, made.text),
            Sets::DroppedUntilU5 => (without_set_lines(&pinned), without_set_lines(&made.text)),
        };
        if pinned != actual {
            failures.push(format!("{name}: {}", text::block_diff(&pinned, &actual)));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
