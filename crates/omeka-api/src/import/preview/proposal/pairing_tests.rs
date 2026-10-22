#![allow(clippy::expect_used)]

use std::collections::BTreeMap;
use std::num::NonZeroU8;

use super::bhc006_tests::{draft, index, install, lists, proposal_with_lists};
use super::pairing::Media;
use super::{Evidence, NodeEvidence, Proposal, SeriesNode, Share};
use crate::import::derive::{NoTags, Sheets, derive};
use crate::import::plan::{
    Edit, ImportPlan, ListFile, PairRule, RecordKind, RelPath, RowKind, SourceId,
};

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

const DOCUMENTS: &str = "BHC006_GMT_D.xlsx";

#[test]
fn documents_pair_by_the_last_two_places() {
    let proposal = proposal_with_lists(&[]);
    let documents = series(&proposal, "Documents (D)");
    let offered: Vec<(&PairRule, Share, usize)> = documents
        .pairing
        .iter()
        .map(|pick| (&pick.value, pick.counts.rows, pick.counts.more))
        .collect();
    assert_eq!(
        offered,
        [(
            &PairRule::LastPlaces { places: 2 },
            Share { n: 15, of: 15 },
            15
        )],
        "each row to one file, all in Documents"
    );
    assert_eq!(
        documents
            .list
            .chosen
            .value
            .as_ref()
            .map(|list| list.file.as_str()),
        Some(DOCUMENTS),
        "the suggestion attaches the list that pairs no row by a stated rule"
    );
    assert!(
        proposal
            .unattached
            .iter()
            .all(|list| list.file.as_str() != DOCUMENTS)
    );
    let (_, list) = proposal
        .structure
        .lists()
        .find(|(_, list)| list.file.as_str() == DOCUMENTS)
        .expect("in the plan");
    assert_eq!(
        list.row_kind,
        RowKind::All(RecordKind::Media),
        "its rows name files"
    );
    assert_eq!(
        list.join.as_ref().map(|join| join.column.header.as_str()),
        Some("Identifier"),
        "joined by file name on the column the suggestion reads"
    );
    assert_eq!(
        list.join.as_ref().map(|join| join.normalize.last_places),
        Some(None),
        "nothing pairs until the person takes the offer"
    );

    // The last place alone is refused: it pairs rows into other series.
    let index = index();
    let catalog = install();
    let plan = draft();
    let found = lists();
    let evidence = Evidence {
        index: &index,
        lists: &found,
        catalog: &catalog,
        sources: &plan.sources,
        structure: &plan.structure,
    };
    let sheet = found
        .iter()
        .find(|list| list.file.file.as_str() == DOCUMENTS)
        .and_then(|list| list.sheet.as_ref().ok())
        .expect("the D list reads");
    let column = sheet
        .columns
        .iter()
        .find(|column| column.header == "Identifier")
        .expect("Identifier")
        .index;
    let names: Vec<&str> = proposal
        .structure
        .all_series()
        .map(|(_, series)| series.id().0.as_str())
        .collect();
    let mut media = Media::of(&evidence, &proposal.structure);
    let by_series = |last: usize, media: &mut Media<'_>| -> BTreeMap<&str, usize> {
        media
            .count(sheet, column, last)
            .series
            .into_iter()
            .map(|(at, rows)| (at.map_or("outside", |at| names[at]), rows))
            .collect()
    };
    let one = by_series(1, &mut media);
    assert_eq!(one.get("Documents (D)"), Some(&15));
    assert_eq!(
        (one.get("Photo Narratives (PN)"), one.get("Video (V)")),
        (Some(&9), Some(&10)),
        "{one:?}"
    );
    assert_eq!(
        by_series(2, &mut media),
        BTreeMap::from([("Documents (D)", 15)])
    );
}

#[test]
fn taking_the_offer_states_it_as_the_join_fold() {
    let pair = Edit::Pair {
        list: ListFile {
            source: SourceId("s1".into()),
            file: RelPath::parse(DOCUMENTS).expect("a path"),
        },
        rule: PairRule::LastPlaces { places: 2 },
    };
    let proposal = proposal_with_lists(&[pair.clone(), pair]);
    assert!(
        proposal.edits.iter().all(|edit| edit.applied),
        "twice is once"
    );
    assert!(series(&proposal, "Documents (D)").pairing.is_empty());
    let (_, list) = proposal
        .structure
        .lists()
        .find(|(_, list)| list.file.as_str() == DOCUMENTS)
        .expect("in the plan");
    let normalize = list.join.as_ref().expect("a join").normalize;
    assert_eq!(normalize.last_places.map(NonZeroU8::get), Some(2));

    let plan = ImportPlan {
        structure: proposal.structure.clone(),
        ..draft()
    };
    let found = lists();
    let sheets: Sheets = plan
        .structure
        .lists()
        .filter_map(|(_, list)| {
            let read = found.iter().find(|one| one.file.file == list.file)?;
            Some((list.id.clone(), read.sheet.clone()))
        })
        .collect();
    let model = derive(&index(), &sheets, &plan, &install(), &NoTags);
    let paired: Vec<&str> = model
        .kept()
        .filter(|record| record.kind == RecordKind::Media)
        .filter(|record| record.rows.iter().any(|row| row.sheet == list.id))
        .filter_map(|record| Some(record.files.first()?.path.as_str()))
        .collect();
    assert_eq!(paired.len(), 15, "{paired:?}");
    assert!(paired.iter().all(|path| path.starts_with("Documents (D)/")));
}
