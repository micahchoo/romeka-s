//! The click meter on the archives the proposer was built on: BHC006 and
//! S1 to S8. Each finds by itself the pinned edits that change the draft,
//! and every one of them is offered.

#![allow(clippy::expect_used)]

use super::clicks::meter;
use super::derive::tests::proposal_shapes::{judge, pinned};
use super::derive::tests::scenarios::S1_TO_S8;
use super::plan::{Edit, FolderScope, ListFile, RecordKind, RelPath, SourceId};
use super::preview::proposal::bhc006_tests;

#[test]
fn bhc006_needs_two_offered_clicks() {
    let metered = meter(bhc006_tests::judge());
    let extras = Edit::LeaveOut {
        scope: FolderScope {
            source: SourceId("s1".into()),
            folder: RelPath::parse("Extras").expect("a folder"),
        },
    };
    let narratives = Edit::Grain {
        list: ListFile {
            source: SourceId("s1".into()),
            file: RelPath::parse("BHC006_GMT_PN.xlsx").expect("a path"),
        },
        describes: RecordKind::Media,
    };
    assert_eq!(
        metered.clicks,
        [extras, narratives],
        "the edits `import_shapes/proposal.rs` pins, found without reading them; {} differences as found",
        metered.as_found
    );
    assert_eq!(metered.gaps(), [].into(), "{:#?}", metered.left);
}

#[test]
fn s1_to_s8_need_their_pinned_clicks() {
    for (shape, build) in S1_TO_S8 {
        let scenario = build();
        let (draft, _) = judge(&scenario, &[]);
        // A pinned edit that asks for what the draft chooses needs no click.
        let needed: Vec<Edit> = pinned(shape)
            .into_iter()
            .filter(|edit| {
                judge(&scenario, std::slice::from_ref(edit)).0.structure != draft.structure
            })
            .collect();
        let metered = meter(|edits| judge(&scenario, edits));
        assert_eq!(metered.clicks, needed, "{shape}");
        assert_eq!(metered.gaps(), [].into(), "{shape}: {:#?}", metered.left);
    }
}
