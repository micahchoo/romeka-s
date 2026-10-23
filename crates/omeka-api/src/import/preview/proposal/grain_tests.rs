#![allow(clippy::expect_used)]

use super::bhc006_tests::proposal_with_lists;
use super::{
    ColumnRollUp, Evidence, FoundList, ItemCut, NodeEvidence, Proposal, RollUp, RollUpCounts,
    SeriesNode, propose,
};
use crate::import::derive::Sheets;
use crate::import::derive::tests::scenarios::{S1_TO_S8, Scenario};
use crate::import::derive::tests::support;
use crate::import::plan::{
    ColumnOptions, ColumnPlan, ColumnRef, Combine, Edit, FolderScope, KindColumn, ListFile,
    RecordKind, RelPath, RowKind, SetFrom, SetNames, SetSource, SlotName, SlotRule, SourceId,
    SourceRef, Structure, Term,
};
use crate::import::preview::candidates::automatic;
use crate::import::sheet;
use crate::import::values::ValueType;
use indexmap::IndexMap;

fn first_series(proposal: &Proposal) -> &SeriesNode {
    proposal
        .nodes
        .iter()
        .find_map(|node| match node {
            NodeEvidence::Series(series) => Some(series),
            NodeEvidence::Set(_) | NodeEvidence::Kept(_) | NodeEvidence::Own(_) => None,
        })
        .expect("a series")
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

fn row_kind<'p>(proposal: &'p Proposal, file: &str) -> &'p RowKind {
    &proposal
        .structure
        .lists()
        .find(|(_, list)| list.file.as_str() == file)
        .expect("the list is in the plan")
        .1
        .row_kind
}

/// A scenario's proposal, its sheets as the lists of the walk.
fn proposal_of(shape: &str) -> Proposal {
    let (_, build) = S1_TO_S8
        .iter()
        .find(|(name, _)| *name == shape)
        .expect("a shape");
    let scenario: Scenario = build();
    let lists: Vec<FoundList> = scenario
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
        .collect();
    let evidence = Evidence {
        index: &scenario.index,
        lists: &lists,
        catalog: &scenario.catalog,
        sources: &scenario.plan.sources,
        structure: &scenario.plan.structure,
        sheets: &Sheets::new(),
    };
    propose(&evidence, &[])
}

#[test]
fn artifacts_rows_describe_files() {
    let proposal = proposal_with_lists(&[]);
    let artifacts = series(&proposal, "Artifacts");
    let grain = artifacts.grain.as_ref().expect("a file join has a grain");
    assert_eq!(grain.chosen.value, RecordKind::Media, "rows describe files");
    assert_eq!(
        (
            grain.chosen.counts.groups_with_many_rows,
            grain.chosen.counts.rows_naming_other_files_of_a_group,
        ),
        (272, 367),
        "272 items have rows for several of their files"
    );
    assert_eq!(
        grain.rival.as_ref().map(|rival| rival.value),
        Some(RecordKind::Item)
    );
    assert_eq!(
        row_kind(&proposal, "BHC006_GMT_AF.xlsx"),
        &RowKind::All(RecordKind::Media)
    );

    let narratives = series(&proposal, "Photo Narratives (PN)");
    let grain = narratives.grain.as_ref().expect("a file join has a grain");
    assert_eq!(grain.chosen.value, RecordKind::Item, "no item has two rows");
    assert_eq!(grain.chosen.counts.groups_with_many_rows, 0);
    assert_eq!(
        row_kind(&proposal, "BHC006_GMT_PN.xlsx"),
        &RowKind::All(RecordKind::Item)
    );
}

#[test]
fn pn_rows_that_name_no_file_are_counted_for_both_grains() {
    let proposal = proposal_with_lists(&[]);
    let narratives = series(&proposal, "Photo Narratives (PN)");
    let grain = narratives.grain.as_ref().expect("a file join has a grain");
    let rival = grain.rival.as_ref().expect("rows may describe files");
    assert_eq!(
        (grain.chosen.value, rival.value),
        (RecordKind::Item, RecordKind::Media)
    );
    assert_eq!(
        (
            grain.chosen.counts.rows_naming_no_file,
            rival.counts.rows_naming_no_file
        ),
        (26, 26),
        "215 rows, 189 of them name a file"
    );
    assert_eq!(
        (grain.chosen.counts.items, rival.counts.items),
        (215, 189),
        "item rows that name no file make items; file rows are left out"
    );
    assert_eq!(
        grain.chosen.counts.items,
        narratives.items.chosen.counts.items
    );
}

#[test]
fn a_grain_click_makes_the_items_its_rival_counted() {
    let before = proposal_with_lists(&[]);
    for (level, file) in [
        ("Photo Narratives (PN)", "BHC006_GMT_PN.xlsx"),
        ("Artifacts", "BHC006_GMT_AF.xlsx"),
    ] {
        let rival = series(&before, level)
            .grain
            .as_ref()
            .and_then(|grain| grain.rival.clone())
            .expect("a rival grain");
        let after = proposal_with_lists(&[Edit::Grain {
            list: ListFile {
                source: SourceId("s1".into()),
                file: RelPath::parse(file).expect("a path"),
            },
            describes: rival.value,
        }]);
        let node = series(&after, level);
        let grain = node.grain.as_ref().expect("a grain");
        assert_eq!(grain.chosen.value, rival.value, "{level}");
        assert_eq!(
            (
                grain.chosen.counts.items,
                grain.chosen.counts.rows_naming_no_file
            ),
            (rival.counts.items, rival.counts.rows_naming_no_file),
            "{level}: the click makes what its segment counted"
        );
        assert_eq!(
            node.items.chosen.counts.items, rival.counts.items,
            "{level}"
        );
    }
}

#[test]
fn a_grain_edit_takes_the_rival() {
    let edit = Edit::Grain {
        list: ListFile {
            source: SourceId("s1".into()),
            file: RelPath::parse("BHC006_GMT_AF.xlsx").expect("a path"),
        },
        describes: RecordKind::Item,
    };
    let proposal = proposal_with_lists(&[edit.clone(), edit]);
    assert!(
        proposal.edits.iter().all(|edit| edit.applied),
        "twice is once"
    );
    assert_eq!(
        row_kind(&proposal, "BHC006_GMT_AF.xlsx"),
        &RowKind::All(RecordKind::Item)
    );
    let grain = series(&proposal, "Artifacts")
        .grain
        .as_ref()
        .expect("a grain");
    assert_eq!(grain.chosen.value, RecordKind::Item);
    assert_eq!(
        grain.rival.as_ref().map(|rival| rival.value),
        Some(RecordKind::Media)
    );
}

#[test]
fn a_register_that_names_every_file_makes_the_items_from_its_rows() {
    let proposal = proposal_of("s6");
    let node = first_series(&proposal);
    let ItemCut::Rows { list, key } = &node.items.chosen.value else {
        panic!("the rows are the items: {:?}", node.items.chosen.value);
    };
    assert_eq!(
        (list.file.as_str(), key.header.as_str()),
        ("objects.csv", "Id")
    );
    assert_eq!(node.items.chosen.counts.items, 2, "O1 and O2");
    assert_eq!(node.items.chosen.counts.files.n, 3);
    assert_eq!(
        node.items.rival.as_ref().map(|rival| &rival.value),
        Some(&ItemCut::Whole),
        "each name an item is the rival"
    );
    let (at, _) = proposal.structure.all_series().next().expect("a series");
    assert_eq!(proposal.structure.series(at).pattern(), None);
    let images = proposal
        .structure
        .lists()
        .find(|(_, list)| list.file.as_str() == "images.csv")
        .expect("images")
        .1;
    assert_eq!(images.row_kind, RowKind::All(RecordKind::Media));
    assert_eq!(
        images.item_key.as_ref().map(|key| key.header.as_str()),
        Some("Object")
    );
    assert!(proposal.unattached.is_empty());
}

#[test]
fn a_list_with_no_file_to_pair_is_records_keyed_by_its_first_unique_column() {
    let proposal = proposal_of("s7");
    let node = first_series(&proposal);
    assert!(matches!(
        &node.items.chosen.value,
        ItemCut::Rows { key, .. } if key.header == "Id"
    ));
    assert_eq!(node.items.chosen.counts.items, 1, "L1");
    assert_eq!(
        row_kind(&proposal, "r.csv"),
        &RowKind::All(RecordKind::Item)
    );
}

#[test]
fn a_kind_column_keys_each_kind_and_links_them() {
    let proposal = proposal_of("s8");
    let (at, _) = proposal.structure.all_series().next().expect("a series");
    let (_, mixed) = proposal.structure.lists().next().expect("the list");
    assert!(matches!(
        &mixed.row_kind,
        RowKind::Column(KindColumn { column }) if column.header == "Kind"
    ));
    assert_eq!(
        mixed.item_key.as_ref().map(|key| key.header.as_str()),
        Some("Item"),
        "media rows name their item"
    );
    let series = proposal.structure.series(at);
    for kind in [RecordKind::Item, RecordKind::ItemSet] {
        assert!(
            series.rules(kind).contains_key(&SlotName::ItemKey),
            "{kind:?} rows are keyed by Id"
        );
    }
    assert!(matches!(
        proposal.structure.memberships(at).as_slice(),
        [SetSource::Each(each)] if each.each.names == SetNames::Key
    ));
    assert_eq!(first_series(&proposal).items.chosen.counts.items, 2);
}

/// The proposal for media `files` in source `s1` and the CSV `lists` at its
/// top, read as Simple adds a list.
fn proposal_from(files: &[(&str, &str)], lists: &[(&str, &str)]) -> Proposal {
    proposal_after(files, lists, &support::plan(|_| {}).structure, &[])
}

/// [`proposal_from`] on the person's `structure`, with `edits`.
fn proposal_after(
    files: &[(&str, &str)],
    lists: &[(&str, &str)],
    structure: &Structure,
    edits: &[Edit],
) -> Proposal {
    let scenario = Scenario {
        index: support::index(files),
        sheets: Sheets::new(),
        plan: support::plan(|_| {}),
        catalog: support::catalog(),
    };
    let lists: Vec<FoundList> = lists
        .iter()
        .map(|(path, text)| {
            let file = ListFile {
                source: SourceId("s1".into()),
                file: RelPath::parse(path).expect("a path"),
            };
            let read = automatic(&file.source, &file.file).read;
            FoundList {
                sheet: sheet::read(text.as_bytes(), &read, None),
                file,
            }
        })
        .collect();
    let evidence = Evidence {
        index: &scenario.index,
        lists: &lists,
        catalog: &scenario.catalog,
        sources: &scenario.plan.sources,
        structure,
        sheets: &Sheets::new(),
    };
    propose(&evidence, edits)
}

#[test]
fn a_column_of_row_numbers_is_never_a_reference() {
    let files = [
        ("a.jpg", support::JPEG),
        ("b.jpg", support::JPEG),
        ("c.jpg", support::JPEG),
    ];
    let photos = "Sl No,File\n1,a.jpg\n2,b.jpg\n3,c.jpg\n";
    let people = "Sl No,Name\n4,Ana\n3,Bo\n2,Cy\n1,Di\n";
    let proposal = proposal_from(&files, &[("photos.csv", photos), ("people.csv", people)]);
    let node = first_series(&proposal);
    assert_eq!(
        node.items.chosen.value,
        ItemCut::Whole,
        "the photos' row numbers do not name the people's rows"
    );
    assert_eq!(node.items.chosen.counts.items, 3);
    let (_, photos) = proposal
        .structure
        .lists()
        .find(|(_, list)| list.file.as_str() == "photos.csv")
        .expect("the photos describe the series");
    assert_eq!(photos.item_key, None);
    assert!(
        proposal
            .structure
            .lists()
            .all(|(_, list)| list.file.as_str() != "people.csv"),
        "the people pair nothing and stay unattached"
    );
}

#[test]
fn a_records_list_keyed_by_row_numbers_takes_its_next_unique_column() {
    let records = "Sl No,Id,Title\n1,L1,Letter\n2,L2,Reply\n";
    let proposal = proposal_from(&[], &[("r.csv", records)]);
    assert!(matches!(
        &first_series(&proposal).items.chosen.value,
        ItemCut::Rows { key, .. } if key.header == "Id"
    ));
}

/// AF_013's three real `Keywords` rows, AF_014's two alike, and a `View`
/// column in which each file has its own value.
const VIEWS: &str = "File,Keywords,View\n\
BHC006_GMT_AF_013a.jpg,\"Natural History, Fossil, Moti Shankh, Front View, Whorl\",Front\n\
BHC006_GMT_AF_013b.jpg,\"Natural History, Fossil, Moti Shankh, Top View\",Top\n\
BHC006_GMT_AF_013c.jpg,\"Natural History, Fossil, Moti Shankh, Siphonal Canal\",Back\n\
BHC006_GMT_AF_014a.jpg,\"Brass, Lamp\",Front\n\
BHC006_GMT_AF_014b.jpg,\"Brass, Lamp\",Back\n";

fn column<'n>(node: &'n SeriesNode, header: &str) -> &'n ColumnRollUp {
    node.columns
        .iter()
        .find(|column| column.header == header)
        .expect("the column has a roll-up")
}

#[test]
fn shared_values_go_on_the_item_and_disjoint_ones_on_each_file() {
    let files = [
        ("BHC006_GMT_AF_013a.jpg", support::JPEG),
        ("BHC006_GMT_AF_013b.jpg", support::JPEG),
        ("BHC006_GMT_AF_013c.jpg", support::JPEG),
        ("BHC006_GMT_AF_014a.jpg", support::JPEG),
        ("BHC006_GMT_AF_014b.jpg", support::JPEG),
    ];
    let proposal = proposal_from(&files, &[("af.csv", VIEWS)]);
    let node = first_series(&proposal);
    assert_eq!(node.items.chosen.counts.items, 2, "AF_013 and AF_014");
    assert_eq!(
        node.grain.as_ref().map(|grain| grain.chosen.value),
        Some(RecordKind::Media)
    );
    let on_item = RollUpCounts { items: 2, files: 0 };
    let on_files = RollUpCounts { items: 0, files: 5 };

    let keywords = column(node, "Keywords");
    assert_eq!(
        (
            keywords.varies.groups,
            keywords.varies.disjoint,
            keywords.varies.of
        ),
        (1, 0, 2),
        "AF_013's rows differ, cut at their commas, and share three values"
    );
    assert_eq!(
        (keywords.pick.chosen.value, keywords.pick.chosen.counts),
        (RollUp::Item, on_item)
    );
    assert_eq!(
        keywords
            .pick
            .rival
            .as_ref()
            .map(|rival| (rival.value, rival.counts)),
        Some((RollUp::File, on_files))
    );

    let view = column(node, "View");
    assert_eq!(
        (view.varies.groups, view.varies.disjoint, view.varies.of),
        (2, 2, 2),
        "each file has its own view"
    );
    assert_eq!(
        (view.pick.chosen.value, view.pick.chosen.counts),
        (RollUp::File, on_files)
    );
    assert_eq!(
        view.pick
            .rival
            .as_ref()
            .map(|rival| (rival.value, rival.counts)),
        Some((RollUp::Item, on_item))
    );
}

#[test]
fn a_list_whose_rows_describe_items_has_no_roll_up() {
    let proposal = proposal_with_lists(&[]);
    assert!(
        series(&proposal, "Photo Narratives (PN)")
            .columns
            .is_empty()
    );
    assert!(!series(&proposal, "Artifacts").columns.is_empty());
}

#[test]
fn file_rows_of_items_of_one_file_have_no_roll_up() {
    let proposal = proposal_with_lists(&[Edit::Grain {
        list: ListFile {
            source: SourceId("s1".into()),
            file: RelPath::parse("BHC006_GMT_PN.xlsx").expect("a path"),
        },
        describes: RecordKind::Media,
    }]);
    let narratives = series(&proposal, "Photo Narratives (PN)");
    assert_eq!(
        narratives.grain.as_ref().map(|grain| grain.chosen.value),
        Some(RecordKind::Media)
    );
    assert!(
        narratives.columns.is_empty(),
        "each PN item has one file, so a column places alike on either level"
    );
}

#[test]
fn a_column_moved_to_each_file_stays_there_on_the_next_proposal() {
    let files = [
        ("AF/BHC006_GMT_AF_013a.jpg", support::JPEG),
        ("AF/BHC006_GMT_AF_013b.jpg", support::JPEG),
        ("AF/BHC006_GMT_AF_013c.jpg", support::JPEG),
        ("AF/BHC006_GMT_AF_014a.jpg", support::JPEG),
        ("AF/BHC006_GMT_AF_014b.jpg", support::JPEG),
        ("Other/a.jpg", support::JPEG),
        ("Other/b.jpg", support::JPEG),
    ];
    let lists = [("AF/af.csv", VIEWS)];
    let first = proposal_from(&files, &lists);
    assert_eq!(
        column(first_series(&first), "Keywords").pick.chosen.value,
        RollUp::Item
    );
    // The person gives Keywords a field, then clicks "On each file": the
    // shown structure with the rule on the media level.
    let mut shown = first.structure.clone();
    let (at, list) = shown
        .lists()
        .find(|(_, list)| list.file.as_str() == "AF/af.csv")
        .map(|(at, list)| (at, list.id.clone()))
        .expect("the list describes the series");
    let subject = SlotName::Term(Term::parse("dcterms:subject").expect("a term"));
    let keywords = ColumnRef {
        index: 1,
        header: "Keywords".into(),
    };
    shown
        .list_mut(&list)
        .expect("the list")
        .columns
        .push(ColumnPlan {
            column: keywords.clone(),
            options: ColumnOptions {
                split: Some(",".into()),
                value_type: ValueType::Literal,
                lang: None,
                private: false,
            },
        });
    shown.rules_mut(at, RecordKind::Media).insert(
        subject.clone(),
        SlotRule {
            sources: vec![SourceRef::Column {
                sheet: list,
                column: keywords.clone(),
            }],
            combine: Combine::First,
            map: IndexMap::new(),
        },
    );
    // A click elsewhere: the other folder's set gets a title.
    let next = proposal_after(
        &files,
        &lists,
        &shown,
        &[Edit::Title {
            from: SetFrom::Folder {
                scope: FolderScope {
                    source: SourceId("s1".into()),
                    folder: RelPath::parse("Other").expect("a folder"),
                },
            },
            title: "Loose photos".into(),
            description: None,
        }],
    );
    assert!(next.edits[0].applied, "the unrelated edit applies");
    let (at, list) = next
        .structure
        .lists()
        .find(|(_, list)| list.file.as_str() == "AF/af.csv")
        .map(|(at, list)| (at, list.id.clone()))
        .expect("the list still describes the series");
    let series = next.structure.series(at);
    let rule = series
        .rules(RecordKind::Media)
        .get(&subject)
        .expect("the subject rule is still on each file");
    assert_eq!(
        rule.sources,
        vec![SourceRef::Column {
            sheet: list,
            column: keywords,
        }]
    );
    assert!(
        series.rules(RecordKind::Item).get(&subject).is_none(),
        "and not on the item"
    );
}
