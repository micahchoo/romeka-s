//! The derive's share of the preview budget (`docs/import-v2.md`,
//! "Performance": 10,000 entries in 150 ms, 40,000 in 600 ms). Run with
//! `cargo test --release -p omeka-api import2::derive::tests::perf -- --ignored --nocapture`.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use serde_json::json;

use super::support::*;
use crate::import2::derive::{FileTags, derive};
use crate::import2::evidence::FileEntry;
use crate::import2::plan::{RecordKind, SourceId, TagId};

/// A warm tag cache: one hash lookup per tag asked.
struct Warm(HashMap<(String, TagId), String>);

impl FileTags for Warm {
    fn tag(&self, _: &SourceId, entry: &FileEntry, tag: TagId) -> Option<String> {
        self.0.get(&(entry.path.as_str().to_string(), tag)).cloned()
    }
}

struct Load {
    index: crate::import2::derive::Index,
    sheets: crate::import2::derive::Sheets,
    plan: crate::import2::plan::ImportPlanV2,
    tags: Warm,
}

fn load(items: usize) -> Load {
    let paths: Vec<String> = (0..items)
        .map(|n| format!("Box {}/OBJ_{n:06}.jpg", n / 500))
        .collect();
    let listed: Vec<(&str, &str)> = paths.iter().map(|path| (path.as_str(), JPEG)).collect();
    let index = index(&listed);
    let mut csv = String::from("Id,Title,Date\n");
    for n in 0..items {
        csv.push_str(&format!("OBJ_{n:06},Object {n},19{:02}\n", n % 100));
    }
    let plan = plan(|plan| {
        plan["grouping"]["pattern"] = json!(r"^(?:.*/)?(?<identifier>OBJ_\d+)\.jpg$");
        plan["sheets"] = json!([sheet_plan("register", &["Id", "Title", "Date"])]);
        plan["sheets"][0]["join"] = join_on("identifier", 0, "Id", true, true, true);
        plan["slots"]["item"]["dcterms:title"] =
            rule(json!([col("register", 1, "Title")]), "first");
        plan["slots"]["item"]["dcterms:date"] = rule(
            json!([col("register", 2, "Date"), { "from": "tag", "tag": "DateTimeOriginal", "files": "first" }]),
            "first",
        );
        plan["slots"]["item"]["dcterms:creator"] = rule(
            json!([{ "from": "tag", "tag": "Artist", "files": "all" }]),
            "all",
        );
        plan["slots"]["item"]["sets"] = rule(json!([{ "from": "folder", "level": 1 }]), "first");
    });
    let sheets = read_sheets(&plan, &[("register", csv.as_bytes())]);
    let tags = Warm(
        paths
            .iter()
            .flat_map(|path| {
                [
                    ((path.clone(), TagId::Artist), "R. Mehta".to_string()),
                    (
                        (path.clone(), TagId::DateTimeOriginal),
                        "2019:05:14 11:02:10".to_string(),
                    ),
                ]
            })
            .collect(),
    );
    Load {
        index,
        sheets,
        plan,
        tags,
    }
}

fn median(mut runs: Vec<Duration>) -> (Duration, Vec<Duration>) {
    let mut sorted = runs.clone();
    sorted.sort();
    (sorted[3], std::mem::take(&mut runs))
}

fn median_of_seven(items: usize) -> (Duration, Vec<Duration>) {
    let Load {
        index,
        sheets,
        plan,
        tags,
    } = load(items);
    let catalog = catalog();
    let mut runs = Vec::new();
    for _ in 0..7 {
        let started = Instant::now();
        let model = derive(&index, &sheets, &plan, &catalog, &tags);
        runs.push(started.elapsed());
        assert_eq!(
            model
                .kept()
                .filter(|record| record.kind == RecordKind::Item)
                .count(),
            items
        );
    }
    median(runs)
}

#[test]
#[ignore = "release-mode timing; run by hand"]
fn the_derive_of_10000_and_40000_items_fits_the_preview_budget() {
    for (items, budget) in [(10_000, 150), (40_000, 600)] {
        let (median, runs) = median_of_seven(items);
        println!("{items} items: median {median:?} of {runs:?} (budget {budget} ms)");
        assert!(
            median < Duration::from_millis(budget),
            "{items}: {median:?}"
        );
    }
}

/// The streamed digest against the buffered one it replaced, interleaved
/// so both see the same machine load.
#[test]
#[ignore = "release-mode timing; run by hand"]
fn the_streamed_digest_of_40000_items_against_the_buffered_one() {
    let Load {
        index,
        sheets,
        plan,
        tags,
    } = load(40_000);
    let model = derive(&index, &sheets, &plan, &catalog(), &tags);
    let (mut streamed, mut buffered) = (Vec::new(), Vec::new());
    for _ in 0..7 {
        let unsealed = model.clone();
        let started = Instant::now();
        let sealed = unsealed.seal();
        streamed.push(started.elapsed());
        let started = Instant::now();
        let bytes = serde_json::to_vec(&model).expect("json");
        let hex = omeka_media::store::sha256_hex(&bytes);
        buffered.push(started.elapsed());
        assert_eq!(sealed.digest, hex, "one digest either way");
        assert_eq!(sealed.digest, model.digest);
    }
    let (streamed, streamed_runs) = median(streamed);
    let (buffered, buffered_runs) = median(buffered);
    println!("digest of 40000 items: streamed median {streamed:?} of {streamed_runs:?}");
    println!("digest of 40000 items: buffered median {buffered:?} of {buffered_runs:?}");
}
