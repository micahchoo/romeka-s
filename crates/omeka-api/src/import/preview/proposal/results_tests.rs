//! What the proposal says of each edit of the plan, and of a tree it does
//! not propose.

#![allow(clippy::expect_used)]

use indexmap::IndexMap;

use super::bhc006_tests::proposal_with_lists;
use super::{Evidence, FoundList, NotApplied, Proposal, RefusedTree, propose};
use crate::import::derive::Sheets;
use crate::import::derive::tests::support;
use crate::import::plan::{
    ColumnRef, Combine, CutEdit, Edit, FolderScope, ListFile, PlanError, RecordKind, RelPath,
    SlotName, SlotRule, SourceId, SourceRef, Structure, Term,
};
use crate::import::preview::candidates::automatic;
use crate::import::sheet;

fn scope(folder: &str) -> FolderScope {
    FolderScope {
        source: SourceId("s1".into()),
        folder: RelPath::parse(folder).expect("a folder"),
    }
}

fn file(path: &str) -> ListFile {
    ListFile {
        source: SourceId("s1".into()),
        file: RelPath::parse(path).expect("a path"),
    }
}

#[test]
fn an_edit_on_a_folder_that_is_gone_says_so() {
    let list = |path: &str| Edit::List {
        scope: scope("Artifacts"),
        list: Some(file(path)),
    };
    let edits = [
        Edit::LeaveOut {
            scope: scope("no such folder"),
        },
        Edit::Grain {
            list: file("no such list.xlsx"),
            describes: crate::import::plan::RecordKind::Media,
        },
        // No count makes items of the first two places of these names.
        Edit::Items {
            scope: scope("Artifacts"),
            cut: CutEdit::Places { places: 2 },
        },
        list("Artifacts/BHC006_GMT_AF.csv"),
        list("BHC006_GMT_AF.xlsx"),
    ];
    let proposal = proposal_with_lists(&edits);
    let said: Vec<(bool, Option<NotApplied>)> = proposal
        .edits
        .iter()
        .map(|edit| (edit.applied, edit.reason))
        .collect();
    assert_eq!(
        said,
        [
            (false, Some(NotApplied::Gone)),
            (false, Some(NotApplied::Gone)),
            (false, Some(NotApplied::NotOffered)),
            (false, Some(NotApplied::Replaced)),
            (true, None),
        ]
    );
}

const KEYWORDS: &str = "File,Keywords\n\
BHC006_GMT_AF_013a.jpg,\"Fossil, Front View\"\n\
BHC006_GMT_AF_013b.jpg,\"Fossil, Top View\"\n\
BHC006_GMT_AF_014a.jpg,\"Brass, Lamp\"\n";

/// The proposal for a folder `AF` of three photos described by `AF/af.csv`
/// and a folder `Other`, on the plan `structure`.
fn proposal_on(structure: &Structure) -> Proposal {
    let index = support::index(&[
        ("AF/BHC006_GMT_AF_013a.jpg", support::JPEG),
        ("AF/BHC006_GMT_AF_013b.jpg", support::JPEG),
        ("AF/BHC006_GMT_AF_014a.jpg", support::JPEG),
        ("Other/a.jpg", support::JPEG),
    ]);
    let plan = support::plan(|_| {});
    let catalog = support::catalog();
    let file = file("AF/af.csv");
    let read = automatic(&file.source, &file.file).read;
    let lists = vec![FoundList {
        sheet: sheet::read(KEYWORDS.as_bytes(), &read, None),
        file,
    }];
    let evidence = Evidence {
        index: &index,
        lists: &lists,
        catalog: &catalog,
        sources: &plan.sources,
        structure,
        sheets: &Sheets::new(),
    };
    propose(&evidence, &[])
}

#[test]
fn a_tree_the_plan_check_refuses_is_not_proposed_and_the_proposal_says_why() {
    let first = proposal_on(&support::plan(|_| {}).structure);
    assert!(first.refused.is_none(), "{:?}", first.refused);
    // A rule on each file reads Keywords, a column the list does not map:
    // the tree the proposer makes from this plan carries the rule.
    let mut plan = first.structure.clone();
    let (at, list) = plan
        .lists()
        .find(|(_, list)| list.file.as_str() == "AF/af.csv")
        .map(|(at, list)| (at, list.id.clone()))
        .expect("the list describes the series");
    plan.rules_mut(at, RecordKind::Media).insert(
        SlotName::Term(Term::parse("dcterms:subject").expect("a term")),
        SlotRule {
            sources: vec![SourceRef::Column {
                sheet: list.clone(),
                column: ColumnRef {
                    index: 1,
                    header: "Keywords".into(),
                },
            }],
            combine: Combine::First,
            map: IndexMap::new(),
        },
    );
    let next = proposal_on(&plan);
    let refused = next.refused.as_ref().expect("the proposal says why");
    assert_eq!(refused.tree, RefusedTree::Lists);
    assert_eq!(
        refused.reason,
        PlanError::MissingColumn {
            sheet: list.0,
            index: 1
        }
        .to_string()
    );
    assert_eq!(next.structure, plan, "the plan's own tree stands");
    assert!(next.matches_plan);
    assert!(next.nodes.is_empty(), "no evidence of a tree not proposed");
    let json = serde_json::to_value(&next).expect("json");
    assert_eq!(json["refused"]["tree"], "lists");
    assert!(
        serde_json::to_value(&first)
            .expect("json")
            .get("refused")
            .is_none(),
        "absent when the tree is proposed"
    );
}

#[test]
fn a_row_whose_line_holds_another_cell_does_not_apply() {
    use super::fixes_tests::{list_file, only_series, path, typo};
    use super::{Stale, StaleWhy};
    use crate::import::plan::{RowCell, RowPair};
    let archive = typo();
    let pair = |line: u32, cell: &str| RowPair {
        row: RowCell {
            line,
            cell: cell.into(),
        },
        file: path("2019.4.12.tif"),
    };
    let edit = Edit::RowIs {
        list: list_file("register.csv"),
        pairs: vec![pair(7, "2019.4.21"), pair(8, "2019.4.21")],
    };
    let proposal = archive.propose(std::slice::from_ref(&edit));
    let result = &proposal.edits[0];
    assert_eq!(
        (result.applied, result.reason),
        (false, Some(NotApplied::Gone))
    );
    assert_eq!(
        result.stale,
        [Stale {
            at: 0,
            why: StaleWhy::RowChanged
        }],
        "line 7 holds 2019.4.6"
    );
    assert_eq!(
        only_series(&proposal).items.chosen.counts.items,
        7,
        "the pair that still holds applies"
    );
    let json = serde_json::to_value(&proposal.edits).expect("json");
    assert_eq!(json[0]["stale"][0]["why"], "rowChanged");
}

#[test]
fn a_correction_whose_file_is_gone_does_not_apply() {
    use super::fixes_tests::{list_file, path, scope, sides, typo};
    use super::{Stale, StaleWhy};
    use crate::import::plan::{ItemJoin, RowCell, RowPair};
    let join = Edit::OneItem {
        scope: scope("Objects"),
        joins: vec![ItemJoin {
            item: path("Objects/2019.4.2_front.tif"),
            with: path("Objects/2019.4.9_front.tif"),
        }],
    };
    let proposal = sides().propose(std::slice::from_ref(&join));
    assert_eq!(
        (proposal.edits[0].applied, proposal.edits[0].reason),
        (false, Some(NotApplied::Gone))
    );
    assert_eq!(
        proposal.edits[0].stale,
        [Stale {
            at: 0,
            why: StaleWhy::FileGone
        }]
    );
    assert!(proposal.structure.corrections().next().is_none());

    let pair = Edit::RowIs {
        list: list_file("register.csv"),
        pairs: vec![RowPair {
            row: RowCell {
                line: 8,
                cell: "2019.4.21".into(),
            },
            file: path("2019.4.13.tif"),
        }],
    };
    let proposal = typo().propose(std::slice::from_ref(&pair));
    assert_eq!(
        proposal.edits[0].stale,
        [Stale {
            at: 0,
            why: StaleWhy::FileGone
        }]
    );
    let json = serde_json::to_value(&typo().propose(&[]).edits).expect("json");
    assert_eq!(
        json,
        serde_json::json!([]),
        "no stale member when nothing is stale"
    );
}
