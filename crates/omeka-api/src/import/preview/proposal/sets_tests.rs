#![allow(clippy::expect_used, clippy::panic)]

use serde_json::json;

use super::bhc006_tests::{proposal, proposal_with_lists};
use super::{
    Evidence, FoundList, NodeEvidence, Proposal, ScanText, SeriesNode, SetCounts, Share, propose,
};
use crate::import::derive::tests::scenarios::{S1_TO_S8, Scenario};
use crate::import::derive::tests::support::{
    JPEG, catalog, col, index, join_on, plan, rule, sheet_plan,
};
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
            NodeEvidence::Kept(_) | NodeEvidence::Series(_) | NodeEvidence::Own(_) => None,
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
        sheets: &Sheets::new(),
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
            description: None,
        },
        Edit::Title {
            from: SetFrom::Folder {
                scope: scope("Artifacts"),
            },
            title: "Objects".into(),
            description: None,
        },
    ];
    let scenario = s1();
    let evidence = Evidence {
        index: &scenario.index,
        lists: &Vec::<FoundList>::new(),
        catalog: &scenario.catalog,
        sources: &scenario.plan.sources,
        structure: &scenario.plan.structure,
        sheets: &Sheets::new(),
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
        sheets: &Sheets::new(),
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
            NodeEvidence::Kept(_) | NodeEvidence::Series(_) | NodeEvidence::Own(_) => None,
        })
        .collect();
    assert_eq!(
        around,
        [(&kind, vec![("Artifact", 313)])],
        "a set level around the series"
    );
}

#[test]
fn a_split_that_leaves_most_items_alone_is_another_choice() {
    let found = proposal(&[]);
    let extras = series(&found, "Extras");
    let place = |at: u8| SetFrom::NamePlace {
        scope: scope("Extras"),
        place: at,
    };
    let splits: Vec<&SetFrom> = extras.splits.iter().map(|split| &split.value).collect();
    assert_eq!(splits, [&place(0)], "three sets, none of one item");
    let other = extras
        .set
        .others
        .iter()
        .find(|other| other.value.as_ref() == Some(&place(1)))
        .expect("place 1 is still a choice, under More…");
    let alone = other
        .counts
        .sets
        .iter()
        .filter(|(_, items)| *items == 1)
        .count();
    assert_eq!(
        (other.counts.sets.len(), alone),
        (148, 147),
        "148 sets, 147 of them one item"
    );

    let taken = proposal(&[Edit::AddSet { from: place(1) }]);
    assert!(taken.edits[0].applied, "an addSet still takes it");
}

#[test]
fn a_column_whose_items_hold_several_values_is_not_a_split() {
    let files = [
        ("AF_001_front.jpg", JPEG),
        ("AF_001_back.jpg", JPEG),
        ("AF_002_front.jpg", JPEG),
        ("AF_002_back.jpg", JPEG),
        ("AF_003_front.jpg", JPEG),
        ("AF_003_back.jpg", JPEG),
        ("AF_004_front.jpg", JPEG),
        ("AF_004_back.jpg", JPEG),
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
    // A row per file, two files per item: a tag on each, the place the
    // same on both.
    let text = "File,Place,Tag\n\
        AF_001_front.jpg,Pune,red\nAF_001_back.jpg,Pune,blue\n\
        AF_002_front.jpg,Pune,red\nAF_002_back.jpg,Pune,blue\n\
        AF_003_front.jpg,Goa,red\nAF_003_back.jpg,Goa,green\n\
        AF_004_front.jpg,Goa,blue\nAF_004_back.jpg,Goa,green\n";
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
        sheets: &Sheets::new(),
    };
    let found = propose(&evidence, &[]);
    let node = series(&found, "s1");
    assert_eq!(node.items.chosen.counts.items, 4, "a front and a back");
    let splits: Vec<Option<&str>> = node
        .splits
        .iter()
        .map(|split| header(&split.value))
        .collect();
    assert_eq!(splits, [Some("Place")], "each item has one place");
    let tag = node
        .set
        .others
        .iter()
        .find(|other| other.value.as_ref().and_then(header) == Some("Tag"))
        .expect("the tags are still a choice, under More…");
    assert_eq!(
        sets(&tag.counts),
        [("blue", 3), ("red", 3), ("green", 2)],
        "every item is in two of its sets"
    );
}

#[test]
fn a_column_of_a_list_the_series_only_holds_for_a_free_set_is_not_a_split() {
    let files = [
        ("a.jpg", JPEG),
        ("b.jpg", JPEG),
        ("c.jpg", JPEG),
        ("d.jpg", JPEG),
    ];
    // The person's plan reads a box per item from `boxes.csv`, as a free
    // set; the proposer pairs `register.csv`, which names every file.
    let person = plan(|plan| {
        let mut boxes = sheet_plan("boxes", &["File", "Box"]);
        boxes["join"] = join_on("fileName", 0, "File", false, false, false);
        plan["sheets"] = json!([boxes]);
        plan["slots"]["item"]["sets"] = rule(json!([col("boxes", 1, "Box")]), "first");
    });
    let read = |file: &str| {
        let file = ListFile {
            source: SourceId("s1".into()),
            file: RelPath::parse(file).expect("a path"),
        };
        let plan = automatic(&file.source, &file.file);
        (file, plan.read)
    };
    let (register, register_read) = read("register.csv");
    let (boxes, boxes_read) = read("boxes.csv");
    let lists = vec![
        FoundList {
            file: register,
            sheet: sheet::read(
                "File,Place\na.jpg,Pune\nb.jpg,Pune\nc.jpg,Goa\nd.jpg,Goa\n".as_bytes(),
                &register_read,
                None,
            ),
        },
        FoundList {
            file: boxes.clone(),
            sheet: sheet::read(
                "File,Box\na.jpg,1\nb.jpg,1\nc.jpg,2\n".as_bytes(),
                &boxes_read,
                None,
            ),
        },
    ];
    let scenario = Scenario {
        index: index(&files),
        sheets: Sheets::new(),
        plan: person,
        catalog: catalog(),
    };
    let evidence = Evidence {
        index: &scenario.index,
        lists: &lists,
        catalog: &scenario.catalog,
        sources: &scenario.plan.sources,
        structure: &scenario.plan.structure,
        sheets: &Sheets::new(),
    };
    let found = propose(&evidence, &[]);
    let node = series(&found, "s1");
    assert_eq!(
        node.list
            .chosen
            .value
            .as_ref()
            .map(|list| list.file.as_str()),
        Some("register.csv")
    );
    let splits: Vec<Option<&str>> = node
        .splits
        .iter()
        .map(|split| header(&split.value))
        .collect();
    assert_eq!(splits, [Some("Place")], "the list the series reads");
    let other = node
        .set
        .others
        .iter()
        .find(|other| other.value.as_ref().and_then(header) == Some("Box"))
        .expect("the box is still a choice, under More…");
    assert_eq!(
        other.value,
        Some(SetFrom::Column {
            list: boxes,
            column: ColumnRef {
                index: 1,
                header: "Box".into(),
            },
        })
    );
    assert_eq!(sets(&other.counts), [("1", 2), ("2", 1)]);
}

/// The set of the S1 series `folder` in a proposal's structure, as JSON.
fn set_of(proposal: &Proposal, folder: &str) -> serde_json::Value {
    let json = serde_json::to_value(&proposal.structure).expect("json");
    json["levels"]
        .as_array()
        .expect("levels")
        .iter()
        .find(|level| level["id"] == folder)
        .map(|level| level["set"].clone())
        .expect("the series")
}

#[test]
fn an_existing_edit_names_the_set_by_its_id_and_survives_a_new_proposal() {
    let existing: Edit = serde_json::from_value(json!({
        "edit": "existing",
        "from": { "from": "folder", "scope": { "source": "s1", "folder": "Photographs" } },
        "id": 42
    }))
    .expect("the edit as the admin sends it");
    let s1 = S1_TO_S8[0].1();
    let propose_on = |plan: &ImportPlan, edits: &[Edit]| {
        let evidence = Evidence {
            index: &s1.index,
            lists: &Vec::<FoundList>::new(),
            catalog: &s1.catalog,
            sources: &plan.sources,
            structure: &plan.structure,
            sheets: &Sheets::new(),
        };
        propose(&evidence, edits)
    };
    let first = propose_on(&s1.plan, std::slice::from_ref(&existing));
    assert!(first.edits[0].applied, "{:?}", first.edits);
    assert_eq!(set_of(&first, "Photographs"), json!({ "id": 42 }));
    assert_eq!(
        series(&first, "Photographs").set.chosen.value,
        Some(SetFrom::Folder {
            scope: scope("Photographs")
        }),
        "the folder still says which items"
    );

    // The person saves that tree and clicks again elsewhere: the edit names
    // the folder, not a level id, so the new proposal keeps the set.
    let saved = ImportPlan {
        structure: first.structure.clone(),
        ..s1.plan.clone()
    };
    let leave = Edit::DropSet {
        from: SetFrom::Folder {
            scope: scope("Artifacts"),
        },
    };
    let second = propose_on(&saved, &[existing.clone(), leave]);
    assert!(second.edits.iter().all(|edit| edit.applied));
    assert_eq!(set_of(&second, "Photographs"), json!({ "id": 42 }));
    assert_eq!(set_of(&second, "Artifacts"), serde_json::Value::Null);

    // A later title is a later answer to the same question.
    let titled = Edit::Title {
        from: SetFrom::Folder {
            scope: scope("Photographs"),
        },
        title: "Photos, 1950s".into(),
        description: Some("Prints from the family album".into()),
    };
    let third = propose_on(&saved, &[existing, titled]);
    assert_eq!(
        set_of(&third, "Photographs"),
        json!({ "title": "Photos, 1950s", "description": "Prints from the family album" })
    );
    assert!(!third.edits[0].applied, "replaced by the title");
}

#[test]
fn a_title_edit_without_a_description_reads_and_writes_as_before() {
    let before = json!({
        "edit": "title",
        "from": { "from": "folder", "scope": { "source": "s1", "folder": "Photographs" } },
        "title": "Photos, 1950s"
    });
    let edit: Edit = serde_json::from_value(before.clone()).expect("a title edit of P4");
    assert_eq!(serde_json::to_value(&edit).expect("json"), before);
}

#[test]
fn an_existing_edit_names_a_top_set_level_too() {
    let offered = proposal_with_lists(&[]).top_sets[0].value.clone();
    let proposal = proposal_with_lists(&[
        Edit::AddSet {
            from: offered.clone(),
        },
        Edit::Existing {
            from: offered,
            id: 7,
        },
    ]);
    assert!(proposal.edits.iter().all(|edit| edit.applied));
    let json = serde_json::to_value(&proposal.structure).expect("json");
    assert_eq!(json["levels"][0]["kind"], "set");
    assert_eq!(json["levels"][0]["id"], "Citizen Curators");
    assert_eq!(json["levels"][0]["set"], json!({ "id": 7 }));
}

/// The proposal for the replica, its lists read, on the plan `structure`
/// (JSON, as the admin saves it) after `edits`.
fn proposal_on(structure: serde_json::Value, edits: &[Edit]) -> Proposal {
    let mut plan = serde_json::to_value(super::bhc006_tests::draft()).expect("the draft as JSON");
    plan["structure"] = structure;
    let plan = ImportPlan::from_json(&plan.to_string()).expect("the plan the person saved");
    let index = super::bhc006_tests::index();
    let catalog = super::bhc006_tests::install();
    let lists = super::bhc006_tests::lists();
    let evidence = Evidence {
        index: &index,
        lists: &lists,
        catalog: &catalog,
        sources: &plan.sources,
        structure: &plan.structure,
        sheets: &Sheets::new(),
    };
    propose(&evidence, edits)
}

/// The `set` of the set level that holds the series `series`, as JSON.
fn level_around(structure: &serde_json::Value, series: &str) -> serde_json::Value {
    structure["levels"]
        .as_array()
        .expect("levels")
        .iter()
        .find(|level| level["kind"] == "set" && level["levels"][0]["id"] == series)
        .map(|level| level["set"].clone())
        .expect("a set level around the series")
}

#[test]
fn a_value_map_on_a_split_set_level_survives_the_next_proposal() {
    let split = Edit::AddSet {
        from: SetFrom::NamePlace {
            scope: scope("Documents (D)"),
            place: 4,
        },
    };
    let first = proposal_with_lists(std::slice::from_ref(&split));
    assert!(first.edits[0].applied, "{:?}", first.edits);
    let mut shown = serde_json::to_value(&first.structure).expect("json");
    let documents = shown["levels"]
        .as_array_mut()
        .expect("levels")
        .iter_mut()
        .find(|level| level["kind"] == "set" && level["levels"][0]["id"] == "Documents (D)")
        .expect("the split is a set level around the series");
    assert!(documents["set"]["each"].get("map").is_none(), "no map yet");
    // The person cleans the values of the split: one date becomes a title.
    documents["set"]["each"]["map"] = json!({ "20170616": "Donation of June 2017" });
    let mapped = level_around(&shown, "Documents (D)");

    // A click elsewhere: the video folder's set gets a title.
    let titled = Edit::Title {
        from: SetFrom::Folder {
            scope: scope("Video (V)"),
        },
        title: "Moving images".into(),
        description: None,
    };
    let next = proposal_on(shown, &[split, titled]);
    assert!(
        next.edits.iter().all(|edit| edit.applied),
        "{:?}",
        next.edits
    );
    let structure = serde_json::to_value(&next.structure).expect("json");
    assert_eq!(
        level_around(&structure, "Documents (D)"),
        mapped,
        "the person's map is still on the split"
    );
}
