#![allow(clippy::expect_used, clippy::panic)]

use super::bhc006_tests::{proposal, proposal_with_lists};
use super::{
    Evidence, FoundList, NodeEvidence, Proposal, ScanText, SeriesNode, SetCounts, Share, propose,
};
use crate::import::derive::tests::scenarios::{S1_TO_S8, Scenario};
use crate::import::derive::tests::support::{JPEG, catalog, index, plan};
use crate::import::derive::{NoTags, Sheets, derive};
use crate::import::placement::{ListFiles, Placement};
use crate::import::plan::{
    ColumnRef, Edit, FolderScope, ImportPlan, ListFile, RelPath, SetFrom, SourceId,
};
use crate::import::preview::candidates::automatic;
use crate::import::sheet;

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
            NodeEvidence::Series(_) | NodeEvidence::Own(_) => None,
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

/// A proposal on media `files` in source `s1`, with no list.
fn of(files: &[(&str, &str)], edits: &[Edit]) -> (Scenario, Proposal) {
    let scenario = Scenario {
        index: index(files),
        sheets: Sheets::new(),
        plan: plan(|_| {}),
        catalog: catalog(),
    };
    let evidence = Evidence {
        index: &scenario.index,
        lists: &Vec::<FoundList>::new(),
        catalog: &scenario.catalog,
        sources: &scenario.plan.sources,
        structure: &scenario.plan.structure,
    };
    let proposal = propose(&evidence, edits);
    (scenario, proposal)
}

fn placed_sets(scenario: &Scenario, proposal: &Proposal) -> usize {
    let plan = ImportPlan {
        structure: proposal.structure.clone(),
        ..scenario.plan.clone()
    };
    let model = derive(
        &scenario.index,
        &Sheets::new(),
        &plan,
        &scenario.catalog,
        &NoTags,
    );
    Placement::of_model(&model, &plan, &ListFiles::new(&plan, &Sheets::new()))
        .sets
        .len()
}

const DRIVE: [(&str, &str); 5] = [
    ("Documents/tax 2019.pdf", "application/pdf"),
    ("Documents/lease.pdf", "application/pdf"),
    ("Pictures/beach.jpg", JPEG),
    ("Pictures/garden.jpg", JPEG),
    ("Music/song.mp3", "audio/mpeg"),
];

#[test]
fn folders_that_are_kinds_of_file_offer_their_sets() {
    let (scenario, found) = of(&DRIVE, &[]);
    for folder in ["Documents", "Music", "Pictures"] {
        let node = series(&found, folder);
        assert_eq!(
            node.set.chosen.value, None,
            "{folder} holds one kind of file"
        );
        assert_eq!(
            node.set
                .rival
                .as_ref()
                .and_then(|rival| rival.value.clone()),
            Some(SetFrom::Folder {
                scope: scope(folder)
            })
        );
    }
    let level = SetFrom::FolderLevel {
        scope: scope(""),
        depth: 1,
    };
    let offer = found
        .top_sets
        .iter()
        .find(|top| top.value == level)
        .expect("one set per folder, offered");
    assert_eq!(
        sets(&offer.counts),
        [("Documents", 2), ("Pictures", 2), ("Music", 1)]
    );
    assert_eq!(placed_sets(&scenario, &found), 0);

    let take = Edit::AddSet { from: level };
    let (scenario, taken) = of(&DRIVE, &[take.clone(), take]);
    assert!(taken.edits.iter().all(|edit| edit.applied), "twice is once");
    assert_eq!(
        series(&taken, "Music").set.chosen.value,
        Some(SetFrom::Folder {
            scope: scope("Music")
        })
    );
    assert!(
        taken.top_sets.is_empty(),
        "a taken offer is no longer offered"
    );
    assert_eq!(placed_sets(&scenario, &taken), 3);
}

#[test]
fn a_dropped_set_and_a_titled_set_are_replayed() {
    let s1 = || S1_TO_S8[0].1();
    let edits = [
        Edit::DropSet {
            from: SetFrom::Folder {
                scope: scope("Artifacts"),
            },
        },
        Edit::Title {
            from: SetFrom::Folder {
                scope: scope("Photographs"),
            },
            title: "Photos, 1950s".into(),
        },
        Edit::Title {
            from: SetFrom::Folder {
                scope: scope("Artifacts"),
            },
            title: "Objects".into(),
        },
    ];
    let scenario = s1();
    let evidence = Evidence {
        index: &scenario.index,
        lists: &Vec::<FoundList>::new(),
        catalog: &scenario.catalog,
        sources: &scenario.plan.sources,
        structure: &scenario.plan.structure,
    };
    let proposal = propose(&evidence, &edits);
    let applied: Vec<bool> = proposal.edits.iter().map(|edit| edit.applied).collect();
    assert_eq!(
        applied,
        [true, true, false],
        "a dropped set has no title to change"
    );
    assert_eq!(series(&proposal, "Artifacts").set.chosen.value, None);
    let photographs = &series(&proposal, "Photographs").set.chosen;
    assert_eq!(sets(&photographs.counts), [("Photos, 1950s", 4)]);
    let json = serde_json::to_value(&proposal.structure).expect("json");
    let titles: Vec<&serde_json::Value> = json["levels"]
        .as_array()
        .expect("levels")
        .iter()
        .map(|level| &level["set"]["title"])
        .collect();
    assert_eq!(
        titles,
        [
            &serde_json::Value::Null,
            &serde_json::json!("Photos, 1950s"),
            &serde_json::json!("Scans")
        ]
    );
}

#[test]
fn a_set_from_any_column_of_the_series_list_applies() {
    let files = [
        ("a.jpg", JPEG),
        ("b.jpg", JPEG),
        ("c.jpg", JPEG),
        ("d.jpg", JPEG),
    ];
    let scenario = Scenario {
        index: index(&files),
        sheets: Sheets::new(),
        plan: plan(|_| {}),
        catalog: catalog(),
    };
    let register = ListFile {
        source: SourceId("s1".into()),
        file: RelPath::parse("register.csv").expect("a path"),
    };
    let text =
        "File,Place,Note\na.jpg,Pune,torn\nb.jpg,Pune,faded\nc.jpg,Goa,signed\nd.jpg,Goa,dated\n";
    let read = automatic(&register.source, &register.file).read;
    let lists = vec![FoundList {
        file: register.clone(),
        sheet: sheet::read(text.as_bytes(), &read, None),
    }];
    let evidence = Evidence {
        index: &scenario.index,
        lists: &lists,
        catalog: &scenario.catalog,
        sources: &scenario.plan.sources,
        structure: &scenario.plan.structure,
    };
    let note = SetFrom::Column {
        list: register,
        column: ColumnRef {
            index: 2,
            header: "Note".into(),
        },
    };
    let draft = propose(&evidence, &[]);
    let node = series(&draft, "s1");
    let splits: Vec<Option<&str>> = node
        .splits
        .iter()
        .map(|split| header(&split.value))
        .collect();
    assert_eq!(splits, [Some("Place")], "two places of two items each sort");
    let [other] = node.set.others.as_slice() else {
        panic!("one other choice: {:?}", node.set.others);
    };
    assert_eq!(
        (other.value.as_ref(), sets(&other.counts)),
        (
            Some(&note),
            vec![("dated", 1), ("faded", 1), ("signed", 1), ("torn", 1)]
        ),
        "a column of one item per value sorts nothing, and is still a choice"
    );

    let taken = propose(&evidence, &[Edit::AddSet { from: note.clone() }]);
    assert!(taken.edits[0].applied, "{:?}", taken.edits);
    let node = series(&taken, "s1");
    assert_eq!(node.set.chosen.value.as_ref(), Some(&note));
    assert!(
        node.set.others.is_empty(),
        "a taken column is no longer offered"
    );
}

#[test]
fn the_rival_title_of_a_series_set_applies() {
    let draft = proposal_with_lists(&[]);
    let kind = series(&draft, "Artifacts")
        .set
        .rival
        .as_ref()
        .and_then(|rival| rival.value.clone())
        .expect("Item Type, one value in every row");
    let proposal = proposal_with_lists(&[Edit::AddSet { from: kind.clone() }]);
    assert!(proposal.edits[0].applied);
    let artifacts = series(&proposal, "Artifacts");
    assert_eq!(
        artifacts.set.chosen.value,
        Some(SetFrom::Folder {
            scope: scope("Artifacts")
        }),
        "the folder stays the series' own set"
    );
    assert!(
        artifacts.set.rival.is_none(),
        "a taken column is no longer offered"
    );
    let around: Vec<(&SetFrom, Vec<(&str, usize)>)> = proposal
        .nodes
        .iter()
        .filter_map(|node| match node {
            NodeEvidence::Set(set) => Some((&set.from, sets(&set.counts))),
            NodeEvidence::Series(_) | NodeEvidence::Own(_) => None,
        })
        .collect();
    assert_eq!(
        around,
        [(&kind, vec![("Artifact", 313)])],
        "a set level around the series"
    );
}
