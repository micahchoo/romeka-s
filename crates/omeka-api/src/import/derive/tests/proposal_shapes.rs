//! The proposer judged on S1 to S8 (design section 4): each scenario is the
//! proposer's evidence, a person makes the shape's pinned edits, and the
//! placement oracle compares what the proposal imports with what the
//! scenario's own plan imports.

use serde_json::json;

use super::scenarios::{S1_TO_S8, Scenario};
use super::support::{JPEG, TEXT, TIFF, catalog, index, plan};
use crate::import::derive::{NoTags, Sheets, derive};
use crate::import::placement::{Difference, ListFiles, Placement, differences};
use crate::import::plan::{
    CutEdit, Edit, FolderScope, ImportPlan, ListFile, RelPath, SetFrom, SourceId,
};
use crate::import::preview::proposal::{Evidence, FoundList, NodeEvidence, Proposal, propose};

/// The shapes that pass today. The table changes in the commit that makes a
/// shape pass, and only then.
const PASSING: &[&str] = &["s1", "s2", "s4", "s5"];

fn scope(folder: &str) -> FolderScope {
    FolderScope {
        source: SourceId("s1".into()),
        folder: RelPath::parse(folder).expect("folder"),
    }
}

/// The edits a person makes on each shape's proposal: each a click the
/// proposal offers (design section 4, "Expected trees"). The count is part
/// of the pin.
fn pinned(shape: &str) -> Vec<Edit> {
    match shape {
        // Three pages of one scan are one item.
        "s1" => vec![Edit::Items {
            scope: scope("Scans"),
            cut: CutEdit::Places { places: 4 },
        }],
        // `AF` and `PH` name sets, which no count can say.
        "s3" => vec![Edit::AddSet {
            from: SetFrom::NamePlace {
                scope: scope(""),
                place: 2,
            },
        }],
        _ => Vec::new(),
    }
}

/// Each sheet of the scenario as a list file of the walk.
fn found_lists(scenario: &Scenario) -> Vec<FoundList> {
    scenario
        .plan
        .structure
        .lists()
        .filter_map(|(_, sheet)| {
            Some(FoundList {
                file: ListFile {
                    source: sheet.source.clone(),
                    file: sheet.file.clone(),
                },
                sheet: scenario.sheets.get(&sheet.id)?.clone(),
            })
        })
        .collect()
}

fn placed(scenario: &Scenario, plan: &ImportPlan, sheets: &Sheets) -> Placement {
    let model = derive(&scenario.index, sheets, plan, &scenario.catalog, &NoTags);
    Placement::of_model(&model, plan, &ListFiles::new(plan, sheets))
}

/// The proposal after `edits`, and how its placement differs from the
/// scenario's.
fn judge(scenario: &Scenario, edits: &[Edit]) -> (Proposal, Vec<Difference>) {
    let lists = found_lists(scenario);
    let evidence = Evidence {
        index: &scenario.index,
        lists: &lists,
        catalog: &scenario.catalog,
        sources: &scenario.plan.sources,
        structure: &scenario.plan.structure,
    };
    let proposal = propose(&evidence, edits);
    let want = placed(scenario, &scenario.plan, &scenario.sheets);
    let plan = ImportPlan {
        structure: proposal.structure.clone(),
        edits: edits.to_vec(),
        ..scenario.plan.clone()
    };
    let sheets: Sheets = plan
        .structure
        .lists()
        .filter_map(|(_, sheet)| {
            let found = lists
                .iter()
                .find(|list| list.file.source == sheet.source && list.file.file == sheet.file)?;
            Some((sheet.id.clone(), found.sheet.clone()))
        })
        .collect();
    let got = placed(scenario, &plan, &sheets);
    let found = differences(&want, &got);
    (proposal, found)
}

#[test]
fn the_ratchet_holds() {
    let mut passing = Vec::new();
    let mut report = Vec::new();
    for (shape, build) in S1_TO_S8 {
        let edits = pinned(shape);
        let (proposal, found) = judge(&build(), &edits);
        let applied = proposal.edits.iter().filter(|edit| edit.applied).count();
        if found.is_empty() && applied == edits.len() {
            passing.push(shape);
        }
        report.push(format!(
            "{shape}: {} differences, {applied} of {} edits applied",
            found.len(),
            edits.len()
        ));
    }
    assert_eq!(
        passing,
        PASSING,
        "move a shape into PASSING in the commit that makes it pass\n{}",
        report.join("\n")
    );
}

#[test]
fn an_archive_as_found_gets_no_level() {
    let files = [
        ("Letter to Ana.jpg", JPEG),
        ("harbour 1952.tif", TIFF),
        ("notes.txt", TEXT),
    ];
    let scenario = Scenario {
        index: index(&files),
        sheets: Sheets::new(),
        plan: plan(|_| {}),
        catalog: catalog(),
    };
    let (proposal, found) = judge(&scenario, &[]);
    assert_eq!(found, [], "places as the default plan does");
    let want = placed(&scenario, &scenario.plan, &scenario.sheets);
    assert_eq!(want.items.len(), 3, "each name is an item");

    let structure = &proposal.structure;
    assert!(
        structure
            .all_series()
            .all(|(at, _)| structure.memberships(at).is_empty()),
        "no set level, no series set and no free set"
    );
    let tree = json!(structure);
    assert!(
        tree["levels"]
            .as_array()
            .expect("levels")
            .iter()
            .all(|level| level["kind"] == "series"),
        "{tree}"
    );
    assert!(proposal.nodes.iter().all(|node| matches!(
        node,
        NodeEvidence::Series(series) if series.set.chosen.value.is_none() && series.list.chosen.value.is_none()
    )));
}
