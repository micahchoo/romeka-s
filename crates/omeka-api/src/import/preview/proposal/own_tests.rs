//! An `own` edit: the series the person wrote by hand, kept and counted.

#![allow(clippy::expect_used)]

use super::{Evidence, NodeEvidence, NotApplied, bhc006_tests, propose};
use crate::import::derive::{NoTags, Sheets, derive};
use crate::import::plan::{
    CutEdit, Edit, FolderScope, ImportPlan, RecordKind, RelPath, SeriesScope, SetFrom, SourceId,
};

fn scope(folder: &str) -> FolderScope {
    FolderScope {
        source: SourceId("s1".into()),
        folder: RelPath::parse(folder).expect("a folder"),
    }
}

#[test]
fn an_own_series_is_kept_as_written_and_counted() {
    // The person's plan: the frozen BHC006 tree. Its Artifacts series has a
    // hand pattern, a list read by id `af` and column rules no detector
    // writes.
    let frozen = ImportPlan::from_json(include_str!("../../fixtures/v3/bhc006.json"))
        .expect("the frozen plan");
    let person = ImportPlan {
        structure: frozen.structure,
        ..bhc006_tests::draft()
    };
    let artifacts = scope("Artifacts");
    let (at, written) = person
        .structure
        .all_series()
        .find(|(_, series)| *series.scope() == SeriesScope::Folders(vec![artifacts.clone()]))
        .expect("the person's Artifacts series");
    let index = bhc006_tests::index();
    let lists = bhc006_tests::lists();
    let catalog = bhc006_tests::install();
    let evidence = Evidence {
        index: &index,
        lists: &lists,
        catalog: &catalog,
        sources: &person.sources,
        structure: &person.structure,
        sheets: &Sheets::new(),
    };
    let edits = [
        Edit::Own {
            scope: artifacts.clone(),
        },
        Edit::Items {
            scope: artifacts.clone(),
            cut: CutEdit::Whole,
        },
    ];
    let proposal = propose(&evidence, &edits);

    let (_, kept) = proposal
        .structure
        .all_series()
        .find(|(_, series)| series.id() == written.id())
        .expect("the written series is in the proposal");
    assert_eq!(kept, written, "the series as the person wrote it");

    let sheets: Sheets = person
        .structure
        .lists()
        .filter_map(|(_, sheet)| {
            let list = lists
                .iter()
                .find(|list| list.file.source == sheet.source && list.file.file == sheet.file)?;
            Some((sheet.id.clone(), list.sheet.clone()))
        })
        .collect();
    let model = derive(&index, &sheets, &person, &catalog, &NoTags);
    let made = model
        .kept()
        .filter(|record| record.kind == RecordKind::Item && record.series == at)
        .count();
    assert!(made > 0, "the person's series makes items");
    let own = proposal
        .nodes
        .iter()
        .find_map(|node| match node {
            NodeEvidence::Own(own) => Some(own),
            NodeEvidence::Set(_) | NodeEvidence::Kept(_) | NodeEvidence::Series(_) => None,
        })
        .expect("the written series' node");
    assert_eq!(own.level, *written.id());
    assert_eq!(own.scope.scope, artifacts);
    assert_eq!(own.items.items, made, "the items the person's series makes");

    let said: Vec<(bool, Option<NotApplied>)> = proposal
        .edits
        .iter()
        .map(|edit| (edit.applied, edit.reason))
        .collect();
    assert_eq!(
        said,
        [(true, None), (false, Some(NotApplied::NotOffered))],
        "nothing is offered on a written series"
    );
    assert!(
        proposal.nodes.iter().any(|node| matches!(
            node,
            NodeEvidence::Series(series) if series.scope.scope == scope("Photo Narratives (PN)")
        )),
        "the other series are still proposed"
    );

    let draft = propose(&evidence, &edits[1..]);
    assert!(
        draft
            .structure
            .all_series()
            .all(|(_, series)| series != written),
        "without the edit the proposer writes Artifacts itself"
    );
}

#[test]
fn a_series_written_by_hand_keeps_the_set_levels_around_it() {
    // Probe P3 (P5 design section 0): the person takes Documents' name
    // place 4, saves that tree, then writes Documents by hand.
    let documents = scope("Documents (D)");
    let split = Edit::AddSet {
        from: SetFrom::NamePlace {
            scope: documents.clone(),
            place: 4,
        },
    };
    let taken = bhc006_tests::proposal(std::slice::from_ref(&split));
    assert!(taken.edits[0].applied, "the split is offered");
    let person = ImportPlan {
        structure: taken.structure,
        ..bhc006_tests::draft()
    };
    let index = bhc006_tests::index();
    let catalog = bhc006_tests::install();
    let evidence = Evidence {
        index: &index,
        lists: &Vec::new(),
        catalog: &catalog,
        sources: &person.sources,
        structure: &person.structure,
        sheets: &Sheets::new(),
    };
    let proposal = propose(&evidence, &[split, Edit::Own { scope: documents }]);

    let level = |structure: &serde_json::Value| -> serde_json::Value {
        structure["levels"]
            .as_array()
            .expect("levels")
            .iter()
            .find(|level| level["id"] == "Documents (D)4")
            .cloned()
            .expect("the set level Documents (D)4")
    };
    let before = level(&serde_json::to_value(&person.structure).expect("json"));
    assert_eq!(before["kind"], "set");
    assert_eq!(before["levels"][0]["id"], "Documents (D)");
    let after = level(&serde_json::to_value(&proposal.structure).expect("json"));
    assert_eq!(
        after, before,
        "the level, its set and the series as written"
    );
    assert!(
        proposal.matches_plan,
        "nothing the person wrote is lost: {:?}",
        proposal.edits
    );
}

#[test]
fn a_set_level_kept_around_a_written_series_is_a_node_with_the_sets_it_makes() {
    // P5 unit C3: the admin draws the kept level as the proposal sends it,
    // with the sets the split made before Documents was written by hand.
    let documents = scope("Documents (D)");
    let split = Edit::AddSet {
        from: SetFrom::NamePlace {
            scope: documents.clone(),
            place: 4,
        },
    };
    let taken = bhc006_tests::proposal(std::slice::from_ref(&split));
    let offered = taken
        .nodes
        .iter()
        .find_map(|node| match node {
            NodeEvidence::Set(set) if set.level.0 == "Documents (D)4" => {
                Some(set.counts.sets.clone())
            }
            _ => None,
        })
        .expect("the split's set level, counted");
    assert_eq!(offered.len(), 2, "name place 4 makes two sets");
    let person = ImportPlan {
        structure: taken.structure,
        ..bhc006_tests::draft()
    };
    let index = bhc006_tests::index();
    let catalog = bhc006_tests::install();
    let evidence = Evidence {
        index: &index,
        lists: &Vec::new(),
        catalog: &catalog,
        sources: &person.sources,
        structure: &person.structure,
        sheets: &Sheets::new(),
    };
    let proposal = propose(&evidence, &[split, Edit::Own { scope: documents }]);

    let at = proposal
        .nodes
        .iter()
        .position(
            |node| matches!(node, NodeEvidence::Kept(kept) if kept.level.0 == "Documents (D)4"),
        )
        .expect("a kept node for the level");
    let NodeEvidence::Kept(kept) = &proposal.nodes[at] else {
        unreachable!("found above");
    };
    let held = serde_json::to_value(&person.structure).expect("json")["levels"]
        .as_array()
        .expect("levels")
        .iter()
        .find(|level| level["id"] == "Documents (D)4")
        .map(|level| level["set"].clone())
        .expect("the plan's level");
    assert_eq!(
        serde_json::to_value(&kept.set).expect("json"),
        held,
        "the set the plan writes"
    );
    assert_eq!(
        kept.counts.as_ref().map(|counts| &counts.sets),
        Some(&offered),
        "the sets the split made, counted on the written series"
    );
    assert!(
        matches!(&proposal.nodes[at + 1], NodeEvidence::Own(own) if own.level.0 == "Documents (D)"),
        "the kept level comes just before the series it holds"
    );
}

#[test]
fn a_suggested_folder_takes_back_a_list_moved_away_so_move_here_writes_both() {
    // "Move here" (P5 unit A3): the person reads Documents' list into
    // Artifacts and writes Artifacts by hand. While Documents stays
    // suggested, the proposer gives it the list again, so the list would be
    // read twice. The admin's Move here therefore writes both folders.
    let mut structure = serde_json::to_value(&bhc006_tests::proposal_with_lists(&[]).structure)
        .expect("the structure as JSON");
    let levels = structure["levels"].as_array_mut().expect("levels");
    let at = |id: &str| {
        levels
            .iter()
            .position(|level| level["id"] == id)
            .expect("a suggested series")
    };
    let (artifacts, documents) = (at("Artifacts"), at("Documents (D)"));
    let moved = levels[documents]["lists"][0].clone();
    assert_eq!(
        moved["file"], "BHC006_GMT_D.xlsx",
        "the list Documents reads"
    );
    levels[documents]["lists"] = serde_json::json!([]);
    levels[artifacts]["lists"]
        .as_array_mut()
        .expect("lists")
        .push(moved);
    let mut plan = serde_json::to_value(bhc006_tests::draft()).expect("the draft as JSON");
    plan["structure"] = structure;
    let person = ImportPlan::from_json(&plan.to_string()).expect("the plan after Move here");

    let index = bhc006_tests::index();
    let lists = bhc006_tests::lists();
    let catalog = bhc006_tests::install();
    let evidence = Evidence {
        index: &index,
        lists: &lists,
        catalog: &catalog,
        sources: &person.sources,
        structure: &person.structure,
        sheets: &Sheets::new(),
    };
    let readers = |edits: &[Edit]| -> Vec<String> {
        propose(&evidence, edits)
            .structure
            .all_series()
            .filter(|(_, series)| {
                series
                    .lists()
                    .iter()
                    .any(|list| list.file.as_str() == "BHC006_GMT_D.xlsx")
            })
            .map(|(_, series)| series.id().0.clone())
            .collect()
    };
    let own = |folder: &str| Edit::Own {
        scope: scope(folder),
    };

    assert_eq!(
        readers(&[own("Artifacts")]),
        ["Artifacts", "Documents (D)"],
        "the suggested holder takes the list back"
    );
    assert_eq!(
        readers(&[own("Documents (D)"), own("Artifacts")]),
        ["Artifacts"],
        "with both folders written, only the folder it was moved to reads it"
    );
}
