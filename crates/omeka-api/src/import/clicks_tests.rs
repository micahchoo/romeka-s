//! The click meter on the archives the proposer was built on: BHC006 and
//! S1 to S8. Each finds by itself the pinned edits that change the draft,
//! and every one of them is offered.

#![allow(clippy::expect_used)]

use serde_json::json;

use super::clicks::{Click, meter};
use super::derive::tests::proposal_shapes::{judge, pinned};
use super::derive::tests::scenarios::S1_TO_S8;
use super::derive::tests::support;
use super::derive::{NoTags, Sheets, derive};
use super::placement::{Difference, ListFiles, Placement, differences};
use super::plan::{
    Auto, ColumnRef, Delimiter, Edit, EncodingChoice, FolderScope, ImportPlan, ListFile, Quote,
    ReadOptions, RecordKind, RelPath, SourceId,
};
use super::preview::proposal::{Evidence, FoundList, Proposal, RollUp, bhc006_tests, propose};
use super::sheet;

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
        [Click::Edit(extras), Click::Edit(narratives)],
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
        let needed: Vec<Click> = needed.into_iter().map(Click::Edit).collect();
        assert_eq!(metered.clicks, needed, "{shape}");
        assert_eq!(metered.gaps(), [].into(), "{shape}: {:#?}", metered.left);
    }
}

/// Files of two items, AF_013 and AF_014, and one row per file. `Keywords`
/// share values in each item, `View` is each file's own, and `Digitized by`
/// is the same on every row of an item.
const FILES: [&str; 5] = [
    "BHC006_GMT_AF_013a.jpg",
    "BHC006_GMT_AF_013b.jpg",
    "BHC006_GMT_AF_013c.jpg",
    "BHC006_GMT_AF_014a.jpg",
    "BHC006_GMT_AF_014b.jpg",
];
const ROWS: &str = "File,Keywords,View,Digitized by\n\
BHC006_GMT_AF_013a.jpg,\"Natural History, Fossil, Front View\",Front,Shreyas Srivatsa\n\
BHC006_GMT_AF_013b.jpg,\"Natural History, Fossil, Top View\",Top,Shreyas Srivatsa\n\
BHC006_GMT_AF_013c.jpg,\"Natural History, Fossil\",Back,Shreyas Srivatsa\n\
BHC006_GMT_AF_014a.jpg,\"Brass, Lamp\",Front,Dilip Patil\n\
BHC006_GMT_AF_014b.jpg,\"Brass, Lamp\",Back,Dilip Patil\n";

fn rows() -> FoundList {
    let file = ListFile {
        source: SourceId("s1".into()),
        file: RelPath::parse("af.csv").expect("a path"),
    };
    let read = ReadOptions {
        encoding: EncodingChoice::Auto(Auto::Auto),
        delimiter: Delimiter::Auto,
        quote: Quote::Double,
        header_line: std::num::NonZeroU32::MIN,
        trim: true,
        sheet_name: None,
    };
    FoundList {
        sheet: sheet::read(ROWS.as_bytes(), &read, None),
        file,
    }
}

/// The proposal after `edits`, on a draft that holds no slot rule.
fn proposal(edits: &[Edit]) -> Proposal {
    let index = support::index(&FILES.map(|file| (file, support::JPEG)));
    let draft = support::plan(|_| {});
    let lists = vec![rows()];
    let evidence = Evidence {
        index: &index,
        lists: &lists,
        catalog: &support::catalog(),
        sources: &draft.sources,
        structure: &draft.structure,
        sheets: &Sheets::new(),
    };
    propose(&evidence, edits)
}

fn placed(plan: &ImportPlan) -> Placement {
    let sheets: Sheets = plan
        .structure
        .lists()
        .map(|(_, list)| (list.id.clone(), rows().sheet))
        .collect();
    let index = support::index(&FILES.map(|file| (file, support::JPEG)));
    let model = derive(&index, &sheets, plan, &support::catalog(), &NoTags);
    Placement::of_model(&model, plan, &ListFiles::new(plan, &sheets))
}

fn column(index: u32, header: &str) -> ColumnRef {
    ColumnRef {
        index,
        header: header.into(),
    }
}

#[test]
fn a_column_on_the_other_level_is_one_offered_click() {
    // The wanted plan is the proposal with three fields: the keywords on the
    // item, the view and the digitizer on each file.
    let mut wanted = serde_json::to_value(support::plan(|_| {})).expect("the plan as JSON");
    wanted["structure"] =
        serde_json::to_value(&proposal(&[]).structure).expect("the structure as JSON");
    let series = &mut wanted["structure"]["levels"][0];
    assert_eq!(series["kind"], "series");
    let read = |index: u32, header: &str| {
        json!({ "sources": [{ "from": "column", "sheet": "af.csv",
                              "column": { "index": index, "header": header } }],
                "combine": "all" })
    };
    let mapped = |index: u32, header: &str, split: Option<&str>| {
        json!({ "column": { "index": index, "header": header },
                "options": { "split": split, "type": "literal", "lang": null, "private": false } })
    };
    series["lists"][0]["columns"] = json!([
        mapped(1, "Keywords", Some(",")),
        mapped(2, "View", None),
        mapped(3, "Digitized by", None),
    ]);
    series["slots"]["item"]["dcterms:subject"] = read(1, "Keywords");
    series["slots"]["media"]["dcterms:description"] = read(2, "View");
    series["slots"]["media"]["dcterms:contributor"] = read(3, "Digitized by");
    let wanted = ImportPlan::from_json(&wanted.to_string()).expect("the wanted plan");
    let want = placed(&wanted);

    let metered = meter(|edits| {
        let proposal = proposal(edits);
        let plan = ImportPlan {
            structure: proposal.structure.clone(),
            edits: edits.to_vec(),
            ..wanted.clone()
        };
        let got = placed(&plan).with_picks(&proposal);
        let found = differences(&want, &got);
        (proposal, found)
    });
    let file = rows().file;
    assert_eq!(
        metered.as_found, 1,
        "the keywords and the view sit where the proposal puts them"
    );
    assert_eq!(
        metered.clicks,
        [Click::Column {
            list: file,
            column: column(3, "Digitized by"),
            to: RollUp::File,
        }],
        "the digitizer is the same on each row of an item, so the proposal \
         puts it on the item; its rival segment is the click"
    );
    assert_eq!(metered.gaps(), [].into(), "{:#?}", metered.left);
}

#[test]
fn a_column_one_side_does_not_place_is_no_difference() {
    let found = proposal(&[]);
    let plan = ImportPlan {
        structure: found.structure.clone(),
        ..support::plan(|_| {})
    };
    let bare = placed(&plan);
    let picked = bare.clone().with_picks(&found);
    assert!(bare.columns.is_empty(), "the draft maps no column");
    assert_eq!(
        picked.columns.get(&(rows().file, column(2, "View"))),
        Some(&RollUp::File)
    );
    let unplaced: Vec<Difference> = differences(&picked, &bare);
    assert_eq!(unplaced, [], "a column only one side places is hand work");
}
