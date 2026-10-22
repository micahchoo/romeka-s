#![allow(clippy::expect_used)]

use super::bhc006_tests::{proposal, proposal_with_lists};
use super::{NodeEvidence, Proposal, ScanText, SeriesNode, SetCounts, Share};
use crate::import::plan::{Edit, FolderScope, RelPath, SetFrom, SourceId};

fn series<'p>(proposal: &'p Proposal, id: &str) -> &'p SeriesNode {
    proposal
        .nodes
        .iter()
        .find_map(|node| match node {
            NodeEvidence::Series(series) if series.level.0 == id => Some(series),
            _ => None,
        })
        .expect("the series")
}

fn header(from: &SetFrom) -> Option<&str> {
    match from {
        SetFrom::Column { column, .. } => Some(&column.header),
        _ => None,
    }
}

fn sets(counts: &SetCounts) -> Vec<(&str, usize)> {
    counts
        .sets
        .iter()
        .map(|(ScanText(name), items)| (name.as_str(), *items))
        .collect()
}

fn scope(folder: &str) -> FolderScope {
    FolderScope {
        source: SourceId("s1".into()),
        folder: RelPath::parse(folder).expect("a folder"),
    }
}

#[test]
fn a_set_with_the_items_of_its_series_is_never_offered() {
    let proposal = proposal_with_lists(&[]);
    let artifacts = series(&proposal, "Artifacts");
    assert_eq!(
        artifacts.set.chosen.value,
        Some(SetFrom::Folder {
            scope: scope("Artifacts")
        }),
        "the folder is the only set chosen"
    );
    let rival = artifacts.set.rival.as_ref().expect("a title rival");
    assert_eq!(
        rival.value.as_ref().and_then(header),
        Some("Item Type"),
        "one value per list: its set is the series"
    );
    assert_eq!(
        sets(&rival.counts),
        [("Artifact", 313)],
        "the 313 of 318 items that have a row"
    );
    assert_eq!(rival.counts.rows, Some(Share { n: 690, of: 690 }));

    for node in &proposal.nodes {
        let NodeEvidence::Series(node) = node else {
            continue;
        };
        assert!(
            node.splits
                .iter()
                .all(|split| header(&split.value) != Some("Item Type")),
            "{}: Item Type is never a split",
            node.level.0
        );
    }
    let top: Vec<&str> = proposal
        .top_sets
        .iter()
        .filter_map(|top| header(&top.value))
        .collect();
    assert_eq!(top, ["Collection", "Curator"], "each holds every item");
}

#[test]
fn a_name_place_of_few_values_is_an_offered_split() {
    let proposal = proposal(&[]);
    let documents = series(&proposal, "Documents (D)");
    let places: Vec<(&SetFrom, Vec<(&str, usize)>)> = documents
        .splits
        .iter()
        .map(|split| (&split.value, sets(&split.counts)))
        .collect();
    assert_eq!(
        places,
        [(
            &SetFrom::NamePlace {
                scope: scope("Documents (D)"),
                place: 4,
            },
            vec![("20170616", 11), ("20171013", 4)],
        )],
        "a donation date, offered and never chosen"
    );
    assert_eq!(
        documents.set.chosen.value,
        Some(SetFrom::Folder {
            scope: scope("Documents (D)")
        })
    );

    let video = series(&proposal, "Video (V)");
    let splits: Vec<Vec<(&str, usize)>> = video
        .splits
        .iter()
        .map(|split| sets(&split.counts))
        .collect();
    assert_eq!(splits, [vec![("V", 13), ("AI", 6)]], "a format code");
}

#[test]
fn an_added_split_is_a_set_level_and_an_added_top_set_holds_everything() {
    let split = Edit::AddSet {
        from: SetFrom::NamePlace {
            scope: scope("Documents (D)"),
            place: 4,
        },
    };
    let offered = proposal_with_lists(&[]).top_sets[0].value.clone();
    let top = Edit::AddSet { from: offered };
    let proposal = proposal_with_lists(&[split.clone(), top.clone(), split, top]);
    assert!(
        proposal.edits.iter().all(|edit| edit.applied),
        "twice is once"
    );
    let levels: Vec<&str> = proposal
        .nodes
        .iter()
        .filter_map(|node| match node {
            NodeEvidence::Set(set) => Some(set.level.0.as_str()),
            NodeEvidence::Series(_) => None,
        })
        .collect();
    assert_eq!(levels.len(), 2, "the top set and the split: {levels:?}");
    assert_eq!(levels[0], "Citizen Curators");
    assert!(series(&proposal, "Documents (D)").splits.is_empty());
    assert!(
        proposal
            .top_sets
            .iter()
            .all(|top| header(&top.value) != Some("Collection"))
    );
    let json = serde_json::to_value(&proposal.structure).expect("json");
    assert_eq!(json["levels"].as_array().expect("levels").len(), 1);
    assert_eq!(json["levels"][0]["kind"], "set");
}
