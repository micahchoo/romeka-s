#![allow(clippy::expect_used)]

use super::bhc006_tests::proposal_with_lists;
use super::{Evidence, FoundList, NodeEvidence, Proposal, SeriesNode, propose};
use crate::import::derive::tests::support::{JPEG, catalog, index, plan};
use crate::import::plan::{Edit, FolderScope, JoinOn, ListFile, RelPath, SourceId};
use crate::import::preview::candidates::{CandidateRead, automatic};
use crate::import::sheet;

fn file(path: &str) -> ListFile {
    ListFile {
        source: SourceId("s1".into()),
        file: RelPath::parse(path).expect("a path"),
    }
}

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

fn chosen(node: &SeriesNode) -> Option<&str> {
    node.list
        .chosen
        .value
        .as_ref()
        .map(|list| list.file.as_str())
}

#[test]
fn each_bhc006_list_goes_to_the_series_it_pairs() {
    let proposal = proposal_with_lists(&[]);

    let artifacts = series(&proposal, "Artifacts");
    assert_eq!(chosen(artifacts), Some("BHC006_GMT_AF.xlsx"));
    let counts = artifacts.list.chosen.counts.as_ref().expect("counts");
    let best = counts.best.as_ref().expect("a best column");
    assert_eq!(
        (best.header.as_str(), best.on, best.paired, counts.rows),
        ("Identifier", JoinOn::FileName, 680, 690),
        "680 of 690 rows name a file here, exactly"
    );
    let rival = artifacts.list.rival.as_ref().expect("a rival");
    assert_eq!(
        rival.value.as_ref().map(|list| list.file.as_str()),
        Some("Artifacts/BHC006_GMT_AF.csv")
    );
    let csv = rival.counts.as_ref().expect("counts");
    assert_eq!(
        csv.best.as_ref().map(|best| (best.paired, csv.rows)),
        Some((5, 5))
    );
    let shared = csv
        .overlaps
        .iter()
        .find(|overlap| overlap.file.as_str() == "BHC006_GMT_AF.xlsx")
        .expect("the CSV pairs items the workbook pairs");
    assert!(shared.shared_targets > 0, "{shared:?}");

    let narratives = series(&proposal, "Photo Narratives (PN)");
    assert_eq!(chosen(narratives), Some("BHC006_GMT_PN.xlsx"));
    let counts = narratives.list.chosen.counts.as_ref().expect("counts");
    let best = counts.best.as_ref().expect("a best column");
    assert_eq!(
        (best.paired, best.paired_normalized, counts.rows),
        (86, 189, 215),
        "86 rows exactly, 189 with the folds"
    );

    // Rule 3: no stated rule pairs a row of the D list, and detector 6's
    // suggestion pairs its rows in Documents only.
    assert_eq!(
        chosen(series(&proposal, "Documents (D)")),
        Some("BHC006_GMT_D.xlsx")
    );
    for id in ["Video (V)", "Audio Interviews (AI)", "Extras"] {
        assert_eq!(chosen(series(&proposal, id)), None, "{id} has no list");
    }

    let unattached: Vec<(&str, bool)> = proposal
        .unattached
        .iter()
        .map(|list| (list.file.as_str(), list.read == CandidateRead::Ok))
        .collect();
    assert_eq!(
        unattached,
        [
            ("Artifacts/BHC006_GMT_AF.ods", false),
            ("BHC006_CC_SECOND_AF.xlsx", true),
        ]
    );
    assert!(
        proposal.unattached.iter().all(|list| list.best.is_none()),
        "no stated rule pairs a row of them"
    );

    let lists: Vec<(&str, &str, bool)> = proposal
        .structure
        .lists()
        .map(|(at, list)| {
            let series = proposal.structure.series(at).id().0.as_str();
            let folds = list.join.as_ref().expect("a join").normalize;
            (series, list.file.as_str(), folds.leading_zeros)
        })
        .collect();
    assert_eq!(
        lists,
        [
            ("Artifacts", "BHC006_GMT_AF.xlsx", false),
            ("Documents (D)", "BHC006_GMT_D.xlsx", false),
            ("Photo Narratives (PN)", "BHC006_GMT_PN.xlsx", true),
        ],
        "a fold is on only where it pairs more rows"
    );
}

#[test]
fn a_list_edit_takes_the_rival_or_no_list() {
    let scope = FolderScope {
        source: SourceId("s1".into()),
        folder: RelPath::parse("Artifacts").expect("a folder"),
    };
    let take = |list: Option<ListFile>| {
        let edit = Edit::List {
            scope: scope.clone(),
            list,
        };
        proposal_with_lists(&[edit.clone(), edit])
    };

    let csv = take(Some(file("Artifacts/BHC006_GMT_AF.csv")));
    assert!(csv.edits.iter().all(|edit| edit.applied), "twice is once");
    let artifacts = series(&csv, "Artifacts");
    assert_eq!(chosen(artifacts), Some("Artifacts/BHC006_GMT_AF.csv"));
    assert_eq!(
        artifacts
            .list
            .rival
            .as_ref()
            .and_then(|rival| rival.value.as_ref())
            .map(|list| list.file.as_str()),
        Some("BHC006_GMT_AF.xlsx")
    );

    let none = take(None);
    assert!(none.edits.iter().all(|edit| edit.applied));
    assert_eq!(chosen(series(&none, "Artifacts")), None);
    assert_eq!(
        none.structure.lists().count(),
        2,
        "only PN and Documents keep their lists"
    );

    let not_offered = take(Some(file("BHC006_CC_SECOND_AF.xlsx")));
    assert!(not_offered.edits.iter().all(|edit| !edit.applied));
    assert_eq!(
        chosen(series(&not_offered, "Artifacts")),
        Some("BHC006_GMT_AF.xlsx")
    );
}

#[test]
fn a_list_that_pairs_two_series_alike_describes_neither() {
    let index = index(&[("Letters/ana.jpg", JPEG), ("Maps/harbour.jpg", JPEG)]);
    let plan = plan(|_| {});
    let list = file("register.csv");
    let read = automatic(&list.source, &list.file).read;
    let lists = vec![FoundList {
        sheet: sheet::read(b"File\nana.jpg\nharbour.jpg\n", &read, None),
        file: list,
    }];
    let catalog = catalog();
    let evidence = Evidence {
        index: &index,
        lists: &lists,
        catalog: &catalog,
        sources: &plan.sources,
        structure: &plan.structure,
    };
    let proposal = propose(&evidence, &[]);
    assert_eq!(
        proposal.structure.lists().count(),
        0,
        "a tie describes none"
    );
    let [unattached] = proposal.unattached.as_slice() else {
        panic!("{:?}", proposal.unattached);
    };
    assert_eq!(
        unattached.best.as_ref().map(|best| best.paired),
        Some(1),
        "its counts are shown"
    );
}
