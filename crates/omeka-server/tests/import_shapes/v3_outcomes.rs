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
//! - File flags of outside files, BHC006 only. A file in no series' scope
//!   raises no file flag. Each pinned flag count is lowered by exactly the
//!   flags the v2 run raised on the files the v3 plan puts outside, and the
//!   v3 text must then match.
//!
//! The run reads the shape from the draft's own source, so the v3 plan
//! takes the draft's `sources`, as the v2 plan does.

use std::collections::{BTreeMap, BTreeSet};
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

fn plan_for(fixture_text: &'static str) -> impl FnOnce(&mut Value) {
    move |draft_plan: &mut Value| {
        let mut plan: Value = serde_json::from_str(fixture_text).unwrap();
        plan["sources"] = draft_plan["sources"].clone();
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

/// What a v3 plan made of a shape.
struct Made {
    text: String,
    /// Group items whose key names their series.
    series_keys: usize,
    /// The file paths the preview leaves out as `outside`.
    outside: BTreeSet<String>,
}

async fn v3_outcome(shape: &Shape, slots: &[&str], fixture_text: &'static str) -> Made {
    let fixture = fixture().await;
    let preview = run(&fixture, shape, slots, plan_for(fixture_text)).await;
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

/// The flags, as `(kind, policy)` words, that the v2 BHC006 run raised on
/// these files.
async fn v2_flags_on(paths: &BTreeSet<String>) -> BTreeMap<(String, String), usize> {
    let fixture = fixture().await;
    let shape = bhc006::shape();
    let preview = run(&fixture, &shape, &bhc006::SLOTS, bhc006::plan_for_bhc006).await;
    let mut counts = BTreeMap::new();
    for entry in preview["entries"].as_array().unwrap() {
        let on_path = entry
            .pointer("/entry/file/path")
            .and_then(Value::as_str)
            .is_some_and(|path| paths.contains(path));
        if !on_path {
            continue;
        }
        for flag in entry["flags"].as_array().unwrap() {
            let policy = flag["policy"].as_str().unwrap_or("blocking");
            let kind = flag["kind"].as_str().unwrap().to_string();
            *counts.entry((kind, policy.to_string())).or_insert(0) += 1;
        }
    }
    counts
}

/// The outside-flags allowance on the pinned text: each `flag` line lowered
/// by the count `gone` names, and dropped at zero.
fn without_flags(pinned: &str, gone: &BTreeMap<(String, String), usize>) -> String {
    let mut used = 0;
    let mut out = String::new();
    for line in pinned.lines() {
        let words: Vec<&str> = line.split(' ').collect();
        let ["flag", kind, policy, count] = words.as_slice() else {
            out.push_str(line);
            out.push('\n');
            continue;
        };
        let less = gone
            .get(&((*kind).to_string(), (*policy).to_string()))
            .copied()
            .unwrap_or(0);
        used += less;
        let left = count.parse::<usize>().unwrap() - less;
        if left > 0 {
            out.push_str(&format!("flag {kind} {policy} {left}\n"));
        }
    }
    assert_eq!(
        used,
        gone.values().sum::<usize>(),
        "every gone flag had a pinned line"
    );
    out
}

#[tokio::test]
async fn v3_plans_match_the_frozen_outcomes() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/import_shapes/outcome_pins");
    let bhc = v3_outcome(&bhc006::shape(), &bhc006::SLOTS, BHC006).await;
    assert!(bhc.series_keys > 0, "BHC006's five series name their keys");
    let extras = bhc006::shape()
        .files
        .into_iter()
        .map(|(path, _)| path)
        .filter(|path| path.starts_with("Extras/") || path.starts_with("tiny-iiif/"));
    for path in extras {
        assert!(bhc.outside.contains(&path), "{path} is in no scope");
    }
    let oral = v3_outcome(&oral_history::shape(), &oral_history::SLOTS, ORAL_HISTORY).await;
    let born = v3_outcome(&born_digital::shape(), &[], BORN_DIGITAL).await;
    for one_series in [&oral, &born] {
        assert_eq!(one_series.series_keys, 0, "one series keeps group:<key>");
    }

    let mut failures = Vec::new();
    let gone = v2_flags_on(&bhc.outside).await;
    assert_eq!(
        gone,
        BTreeMap::from([(("not-media".to_string(), "leave".to_string()), 276)]),
        "the v2 run raised not-media on 276 files the v3 plan puts outside, \
         so the pinned 1097 becomes 821, and no other flag moves"
    );
    for (name, made) in [
        ("bhc006", bhc),
        ("oral_history", oral),
        ("born_digital", born),
    ] {
        let pinned = std::fs::read_to_string(dir.join(format!("{name}.txt"))).unwrap();
        let pinned = if name == "bhc006" {
            without_flags(&pinned, &gone)
        } else {
            pinned
        };
        if pinned != made.text {
            failures.push(format!("{name}: {}", text::block_diff(&pinned, &made.text)));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
