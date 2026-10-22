//! The design's v3 plan of BHC006 (`omeka-api/src/import/fixtures/v3/`),
//! run through the preview and compared with the outcome its v2 plan pinned
//! (`outcome_pins/bhc006.txt`, never rewritten here).
//!
//! Two differences are allowed, each named where it is applied:
//!
//! - Record keys. A plan of several series keys a group item
//!   `group:<series>/<key>`, where v2 wrote `group:<key>`. The full keys must
//!   be unique; the `<series>/` part is then dropped from the v3 side
//!   (`harness::v3_outcome`).
//! - File flags of outside files. A file in no series' scope raises no file
//!   flag. Each pinned flag count is lowered by exactly the flags the v2 run
//!   raised on the files the v3 plan puts outside, and the v3 text must then
//!   match.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use serde_json::Value;

use super::{bhc006, run, text, v3_outcome};
use crate::imports_fixture::fixture;

const BHC006: &str = include_str!("../../../omeka-api/src/import/fixtures/v3/bhc006.json");

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

    let gone = v2_flags_on(&bhc.outside).await;
    assert_eq!(
        gone,
        BTreeMap::from([(("not-media".to_string(), "leave".to_string()), 276)]),
        "the v2 run raised not-media on 276 files the v3 plan puts outside, \
         so the pinned 1097 becomes 821, and no other flag moves"
    );
    let pinned = std::fs::read_to_string(dir.join("bhc006.txt")).unwrap();
    let pinned = without_flags(&pinned, &gone);
    assert!(
        pinned == bhc.text,
        "bhc006: {}",
        text::block_diff(&pinned, &bhc.text)
    );
}
