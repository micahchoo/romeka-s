//! A date field the files fill with several values: the run writes each one,
//! so each one is checked as a date.

#![allow(clippy::expect_used)]

use std::collections::BTreeMap;

use serde_json::json;

use super::archive::ArchiveFile;
use super::derive::derive;
use super::problems::ProblemKind;
use super::tags::{TagCache, TagReader};
use super::tests::{catalog, plan, sample};

/// AF_001's two sides were shot on two good days; AF_002's back carries an
/// impossible month.
struct TwoDays;

impl TagReader for TwoDays {
    fn read(&self, file: &ArchiveFile) -> BTreeMap<&'static str, String> {
        let date = match file.path.as_str() {
            "Artifacts/BHC_006_AF_001_front.jpg" | "Artifacts/BHC_006_AF_002_front.jpg" => {
                "2019:05:14 11:02:10"
            }
            "Artifacts/BHC_006_AF_001_back.jpg" => "2019:06:01 09:00:00",
            "Artifacts/BHC_006_AF_002_back.jpg" => "2019:13:01 09:00:00",
            _ => return BTreeMap::new(),
        };
        BTreeMap::from([("DateTimeOriginal", date.to_string())])
    }
}

#[test]
fn each_date_the_files_give_is_checked_on_its_own() {
    let plan = plan(|p| {
        p["meta"] = json!([
            { "tag": "DateTimeOriginal", "term": "dcterms:created", "on": "item", "from": "all" }
        ]);
    });
    let sample = sample(&plan);
    let model = derive(&plan, &sample.inputs, &TagCache::new(&TwoDays), &catalog());
    let created = |identifier: &str| {
        let row = model
            .rows
            .iter()
            .find(|row| row.item.identifier == identifier)
            .expect("row");
        row.cells
            .iter()
            .find(|cell| cell.term == "dcterms:created")
            .map(|cell| (cell.value.value.clone(), cell.problems.clone()))
            .expect("cell")
    };
    assert_eq!(
        created("BHC_006_AF_001"),
        ("2019-06-01 ; 2019-05-14".to_string(), vec![]),
        "two good dates are no date problem"
    );
    assert_eq!(
        created("BHC_006_AF_002"),
        (
            "2019-13-01 ; 2019-05-14".to_string(),
            vec![ProblemKind::Date]
        ),
        "one bad date among good ones is"
    );
}
