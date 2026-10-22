//! Detector 1 on BHC006 and S1 (design section 2, "Detector 1").

#![allow(clippy::expect_used)]

use super::bhc006_tests;
use super::{Evidence, FoundList, NodeEvidence, OutsideFolder, OutsideReason, Proposal, propose};
use crate::import::derive::tests::scenarios::S1_TO_S8;
use crate::import::derive::tests::support::{JPEG, catalog, index, plan};
use crate::import::derive::{NoTags, Sheets, derive};
use crate::import::placement::{ListFiles, Placement};
use crate::import::plan::{Edit, FolderScope, ImportPlan, RelPath, SetFrom, SourceId};

const PDF: &str = "application/pdf";
use crate::import::preview::facts::MediaKind;

fn scope(folder: &str) -> FolderScope {
    FolderScope {
        source: SourceId("s1".into()),
        folder: RelPath::parse(folder).expect("folder"),
    }
}

fn series(proposal: &Proposal) -> Vec<(&str, usize, usize)> {
    proposal
        .nodes
        .iter()
        .filter_map(|node| match node {
            NodeEvidence::Series(series) => Some((
                series.scope.scope.folder.as_str(),
                series.scope.files,
                series.scope.media,
            )),
            NodeEvidence::Set(_) => None,
        })
        .collect()
}

fn outside(proposal: &Proposal) -> Vec<(&str, usize, usize, OutsideReason)> {
    proposal
        .outside
        .iter()
        .map(|OutsideFolder { counts, reason }| {
            (
                counts.scope.folder.as_str(),
                counts.files,
                counts.media,
                *reason,
            )
        })
        .collect()
}

fn s1() -> Proposal {
    let (_, build) = S1_TO_S8[0];
    let scenario = build();
    let evidence = Evidence {
        index: &scenario.index,
        lists: &Vec::<FoundList>::new(),
        catalog: &scenario.catalog,
        sources: &scenario.plan.sources,
        structure: &scenario.plan.structure,
    };
    propose(&evidence, &[])
}

#[test]
fn a_folder_with_nothing_to_show_is_outside() {
    let bhc006 = bhc006_tests::proposal(&[]);
    let tiny = bhc006
        .outside
        .iter()
        .find(|folder| folder.counts.scope == scope("tiny-iiif"))
        .expect("tiny-iiif is outside");
    assert_eq!(tiny.counts.files, 147);
    assert_eq!(tiny.reason, OutsideReason::NoMedia);
    assert!(
        tiny.counts
            .kinds
            .iter()
            .all(|(kind, _)| *kind == MediaKind::Other),
        "{:?}",
        tiny.counts.kinds
    );
    assert_eq!(outside(&bhc006).len(), 1, "only tiny-iiif");
    assert_eq!(
        series(&bhc006),
        [
            ("Artifacts", 1309, 696),
            ("Audio Interviews (AI)", 4, 3),
            ("Documents (D)", 31, 15),
            ("Extras", 293, 163),
            ("Photo Narratives (PN)", 379, 189),
            ("Video (V)", 20, 19),
        ]
    );

    let s1 = s1();
    assert_eq!(
        outside(&s1),
        [("_scratch", 2, 1, OutsideReason::NoMedia)],
        "notes.txt is accepted, but its kind is not one the pages show"
    );
    assert_eq!(
        series(&s1),
        [("Artifacts", 8, 8), ("Photographs", 4, 4), ("Scans", 3, 3)]
    );
}

#[test]
fn each_of_several_series_is_its_folders_set() {
    let s1 = s1();
    let sets: Vec<Option<&SetFrom>> = s1
        .nodes
        .iter()
        .filter_map(|node| match node {
            NodeEvidence::Series(series) => Some(series.set.chosen.value.as_ref()),
            NodeEvidence::Set(_) => None,
        })
        .collect();
    assert_eq!(
        sets,
        [
            Some(&SetFrom::Folder {
                scope: scope("Artifacts")
            }),
            Some(&SetFrom::Folder {
                scope: scope("Photographs")
            }),
            Some(&SetFrom::Folder {
                scope: scope("Scans")
            }),
        ]
    );
}

#[test]
fn a_left_out_folder_is_outside_by_the_edit_and_a_kept_one_is_a_series() {
    let leave = Edit::LeaveOut {
        scope: scope("Extras"),
    };
    let left = bhc006_tests::proposal(&[leave.clone(), leave]);
    assert!(left.edits.iter().all(|edit| edit.applied), "twice is once");
    assert_eq!(series(&left).len(), 5);
    assert!(outside(&left).contains(&("Extras", 293, 163, OutsideReason::Edit)));

    let kept = bhc006_tests::proposal(&[Edit::Keep {
        scope: scope("tiny-iiif"),
    }]);
    assert!(kept.edits[0].applied);
    assert_eq!(outside(&kept), []);
    assert_eq!(series(&kept).len(), 7);
}

#[test]
fn an_edit_that_asks_for_what_the_proposal_does_is_applied() {
    let already = Edit::LeaveOut {
        scope: scope("tiny-iiif"),
    };
    let missing = Edit::LeaveOut {
        scope: scope("no such folder"),
    };
    let proposal = bhc006_tests::proposal(&[already, missing]);
    let applied: Vec<bool> = proposal.edits.iter().map(|edit| edit.applied).collect();
    assert_eq!(
        applied,
        [true, false],
        "the person got what they asked for; a folder that is not there is not found"
    );
    assert!(
        outside(&proposal)
            .iter()
            .any(|(folder, _, _, reason)| *folder == "tiny-iiif"
                && *reason == OutsideReason::NoMedia),
        "its counts still say why it is outside"
    );
}

#[test]
fn numbered_folders_are_one_series_with_their_level_offered() {
    let (_, build) = S1_TO_S8[3];
    let scenario = build();
    let evidence = Evidence {
        index: &scenario.index,
        lists: &Vec::<FoundList>::new(),
        catalog: &scenario.catalog,
        sources: &scenario.plan.sources,
        structure: &scenario.plan.structure,
    };
    let proposal = propose(&evidence, &[]);
    let [NodeEvidence::Series(boxes)] = proposal.nodes.as_slice() else {
        panic!("one series: {:?}", proposal.nodes);
    };
    assert_eq!(
        boxes.scope.scope,
        scope(""),
        "the only series takes the source"
    );
    assert_eq!(
        boxes.set.chosen.value, None,
        "a single series sorts nothing"
    );
    let [split] = boxes.splits.as_slice() else {
        panic!("one offered split: {:?}", boxes.splits);
    };
    assert_eq!(
        split.value,
        SetFrom::FolderLevel {
            scope: scope(""),
            depth: 1
        }
    );
}

/// A folder of two reports, one empty file, one file whose bytes are not
/// what its name says, and a sub-folder.
fn reports() -> crate::import::derive::tests::scenarios::Scenario {
    let mut index = index(&[
        ("Reports/annual 2019.pdf", PDF),
        ("Reports/annual 2020.pdf", PDF),
        ("Reports/annual 2021.pdf", PDF),
        ("Reports/draft.pdf", PDF),
        ("Reports/budget.pdf", JPEG),
        ("Reports/old/annual 2018.pdf", PDF),
    ]);
    for entry in &mut index.sources[0].walk.entries {
        if entry.path.as_str() == "Reports/draft.pdf" {
            entry.size = 0;
        }
    }
    crate::import::derive::tests::scenarios::Scenario {
        index,
        sheets: Sheets::new(),
        plan: plan(|_| {}),
        catalog: catalog(),
    }
}

fn left_out(
    scenario: &crate::import::derive::tests::scenarios::Scenario,
    proposal: &Proposal,
) -> Vec<String> {
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
    let placement = Placement::of_model(&model, &plan, &ListFiles::new(&plan, &Sheets::new()));
    placement
        .left_out
        .iter()
        .map(|(_, path)| path.as_str().to_owned())
        .collect()
}

#[test]
fn a_file_that_is_not_media_makes_no_item() {
    let scenario = reports();
    let of = |edits: &[Edit]| {
        let evidence = Evidence {
            index: &scenario.index,
            lists: &Vec::<FoundList>::new(),
            catalog: &scenario.catalog,
            sources: &scenario.plan.sources,
            structure: &scenario.plan.structure,
        };
        propose(&evidence, edits)
    };
    let found = of(&[]);
    assert_eq!(
        outside(&found),
        [
            ("Reports/budget.pdf", 1, 0, OutsideReason::NoMedia),
            ("Reports/draft.pdf", 1, 0, OutsideReason::NoMedia),
        ],
        "an empty file, and one whose bytes are another format, are not media"
    );
    assert_eq!(series(&found), [("", 4, 4)], "the series counts only media");
    assert_eq!(
        left_out(&scenario, &found),
        ["Reports/budget.pdf", "Reports/draft.pdf"]
    );

    let edits = [
        Edit::LeaveOut {
            scope: scope("Reports/annual 2021.pdf"),
        },
        Edit::LeaveOut {
            scope: scope("Reports/old"),
        },
        Edit::Keep {
            scope: scope("Reports/draft.pdf"),
        },
        Edit::LeaveOut {
            scope: scope("Reports/no such file.pdf"),
        },
    ];
    let edited = of(&edits);
    let applied: Vec<bool> = edited.edits.iter().map(|edit| edit.applied).collect();
    assert_eq!(applied, [true, true, true, false]);
    assert_eq!(
        left_out(&scenario, &edited),
        [
            "Reports/annual 2021.pdf",
            "Reports/budget.pdf",
            "Reports/old/annual 2018.pdf"
        ],
        "one file and one folder left out by an edit; the empty file kept"
    );
}
