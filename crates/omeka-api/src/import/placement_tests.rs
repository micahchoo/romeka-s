//! The placement oracle on S1 and S6.

#![allow(clippy::expect_used)]

use std::collections::BTreeSet;

use serde_json::json;

use super::derive::tests::scenarios::{S1_TO_S8, Scenario};
use super::derive::tests::support::{plan, rule};
use super::derive::{NoTags, derive};
use super::placement::{Difference, FileAt, ListFiles, Placement, differences};
use super::plan::{ImportPlan, ListFile, RelPath, SourceId};

fn scenario(name: &str) -> Scenario {
    let (_, build) = S1_TO_S8
        .iter()
        .find(|(shape, _)| *shape == name)
        .expect("a scenario");
    build()
}

fn placed(scenario: &Scenario, plan: &ImportPlan) -> Placement {
    let model = derive(
        &scenario.index,
        &scenario.sheets,
        plan,
        &scenario.catalog,
        &NoTags,
    );
    Placement::of_model(&model, plan, &ListFiles::new(plan, &scenario.sheets))
}

/// S1's plan with another pattern and these decisions.
fn s1_plan(pattern: &str, decisions: serde_json::Value) -> ImportPlan {
    plan(|plan| {
        plan["grouping"]["pattern"] = json!(pattern);
        plan["slots"]["item"]["sets"] = rule(json!([{ "from": "group", "name": "set" }]), "first");
        plan["decisions"] = decisions;
    })
}

fn at(path: &str) -> FileAt {
    (SourceId("s1".into()), RelPath::parse(path).expect("path"))
}

#[test]
fn regex_text_does_not_matter_but_a_moved_file_does() {
    let s1 = scenario("s1");
    let want = placed(&s1, &s1.plan);
    let other = r"^(?<set>[^/]+)/(?:.*/)?(?<identifier>[A-Z]+_\d+_[A-Z]+_\d+)(?:_(?<part>[^/.]+))?\.[^/.]+$";
    assert_eq!(
        differences(&want, &placed(&s1, &s1_plan(other, json!([])))),
        [],
        "another pattern that groups alike places alike"
    );

    let page = "Scans/BHC_006_SC_001_p10.tif";
    let moved = json!([{ "kind": "move", "file": { "file": { "source": "s1", "path": page } }, "to": "own" }]);
    let got = placed(&s1, &s1_plan(other, moved));
    let scan = |pages: &[&str]| -> BTreeSet<FileAt> {
        pages
            .iter()
            .map(|page| at(&format!("Scans/BHC_006_SC_001_{page}.tif")))
            .collect()
    };
    assert_eq!(
        differences(&want, &got),
        [Difference::FilesSplit {
            want: scan(&["p1", "p2", "p10"]),
            got: vec![scan(&["p1", "p2"]), scan(&["p10"])],
        }]
    );
}

#[test]
fn a_row_names_its_file_only_on_an_item_of_several() {
    let s6 = scenario("s6");
    let placement = placed(&s6, &s6.plan);
    let images = ListFile {
        source: SourceId("s1".into()),
        file: RelPath::parse("images.csv").expect("path"),
    };
    let files: Vec<Option<FileAt>> = (2..=4)
        .map(|line| placement.rows[&(images.clone(), line)].1.clone())
        .collect();
    assert_eq!(
        files,
        [Some(at("scans/s002.tif")), None, Some(at("scans/s001.tif"))],
        "O1 has two files, O2 one"
    );
    assert!(placement.left_out.is_empty());
}
