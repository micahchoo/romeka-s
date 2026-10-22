//! Detector 2 and the pattern writer (design section 2, "Detector 2", and
//! section 3).

#![allow(clippy::expect_used, clippy::panic)]

use super::bhc006_tests;
use super::shape::{Key, KeyShape, places};
use super::{CutRule, Evidence, FoundList, ItemCut, NodeEvidence, Proposal, SeriesNode, propose};
use crate::import::decisions::EntryRef;
use crate::import::derive::tests::scenarios::{S1_TO_S8, Scenario};
use crate::import::derive::tests::support::{JPEG, TIFF, catalog, index, plan};
use crate::import::derive::{NoTags, Sheets, derive};
use crate::import::grouping::pattern::FilePattern;
use crate::import::placement::{ListFiles, Placement};
use crate::import::plan::{CutEdit, Edit, FolderScope, ImportPlan, RelPath, SetFrom, SourceId};
use crate::import::preview::facts::Bucket;

fn of(scenario: &Scenario, edits: &[Edit]) -> Proposal {
    let evidence = Evidence {
        index: &scenario.index,
        lists: &Vec::<FoundList>::new(),
        catalog: &scenario.catalog,
        sources: &scenario.plan.sources,
        structure: &scenario.plan.structure,
    };
    propose(&evidence, edits)
}

fn shape(name: &str) -> Scenario {
    let (_, build) = S1_TO_S8
        .iter()
        .find(|(shape, _)| *shape == name)
        .expect("a shape");
    build()
}

fn series<'p>(proposal: &'p Proposal, folder: &str) -> &'p SeriesNode {
    proposal
        .nodes
        .iter()
        .find_map(|node| match node {
            NodeEvidence::Series(series) if series.scope.scope.folder.as_str() == folder => {
                Some(series)
            }
            _ => None,
        })
        .unwrap_or_else(|| panic!("a series at {folder:?}: {:?}", proposal.nodes))
}

fn scope(folder: &str) -> FolderScope {
    FolderScope {
        source: SourceId("s1".into()),
        folder: RelPath::parse(folder).expect("folder"),
    }
}

fn per_item(series: &SeriesNode) -> Vec<(Bucket, usize)> {
    series
        .items
        .chosen
        .counts
        .per_item
        .iter()
        .filter(|bucket| bucket.count > 0)
        .map(|bucket| (bucket.files, bucket.count))
        .collect()
}

#[test]
fn bhc006_artifacts_cut_after_the_number() {
    let proposal = bhc006_tests::proposal(&[]);
    let artifacts = series(&proposal, "Artifacts");
    let chosen = &artifacts.items.chosen;
    assert_eq!(
        chosen.value,
        ItemCut::Places {
            places: 5,
            rule: CutRule::RecurringParts
        }
    );
    assert_eq!(chosen.counts.items, 318);
    assert_eq!((chosen.counts.files.n, chosen.counts.files.of), (692, 696));
    assert_eq!(
        per_item(artifacts),
        [
            (Bucket::One, 43),
            (Bucket::Two, 226),
            (Bucket::Three, 22),
            (Bucket::FourToSix, 25),
            (Bucket::SevenOrMore, 2),
        ]
    );
    let parts: Vec<(&str, usize)> = chosen
        .counts
        .parts
        .iter()
        .take(5)
        .map(|(part, items)| (part.0.as_str(), *items))
        .collect();
    assert_eq!(
        parts,
        [("a", 279), ("b", 273), ("c", 45), ("d", 27), ("e", 14)]
    );
    assert_eq!(chosen.counts.names_ending_here, 38);
    assert_eq!(chosen.counts.other_shapes, 4);
    let rival = artifacts.items.rival.as_ref().expect("a rival");
    assert_eq!((&rival.value, rival.counts.items), (&ItemCut::Whole, 696));

    // The four of another shape make no item; the preview lists them as
    // files the pattern does not match.
    let plan = ImportPlan {
        structure: proposal.structure.clone(),
        ..bhc006_tests::draft()
    };
    let model = derive(
        &bhc006_tests::index(),
        &Sheets::new(),
        &plan,
        &bhc006_tests::install(),
        &NoTags,
    );
    let unmatched: Vec<&str> = model
        .entries
        .iter()
        .filter(|entry| entry.unmatched)
        .filter_map(|entry| match &entry.entry {
            EntryRef::File(file) => Some(file.path.as_str()),
            EntryRef::Row(_) => None,
        })
        .filter(|path| path.starts_with("Artifacts/") && !path.ends_with(".json"))
        .collect();
    assert_eq!(
        unmatched,
        [
            "Artifacts/0accebf6883150506f6ce8587d7fa7bd735a472b.jpg",
            "Artifacts/img20241004_17295799.tif",
            "Artifacts/img20241004_17321887.tif",
            "Artifacts/img20241006_15124931.tif",
        ]
    );
}

#[test]
fn a_sibling_shape_is_a_rival_only_when_it_joins_files() {
    let proposal = bhc006_tests::proposal(&[]);
    let documents = series(&proposal, "Documents (D)");
    assert_eq!(documents.items.chosen.value, ItemCut::Whole);
    assert_eq!(documents.items.chosen.counts.items, 15);
    let rival = documents.items.rival.as_ref().expect("a rival");
    assert_eq!(
        rival.value,
        ItemCut::Places {
            places: 5,
            rule: CutRule::SameAsSibling
        }
    );
    assert_eq!(
        rival.counts.items, 2,
        "as a choice it would make 2 items of 15"
    );
    let narratives = series(&proposal, "Photo Narratives (PN)");
    assert_eq!(narratives.items.chosen.counts.items, 189);
    assert!(
        narratives.items.rival.is_none(),
        "the Artifacts key keeps each narrative an item, so it is no other choice"
    );
}

#[test]
fn s1_scans_are_one_item_by_their_own_pages() {
    let scenario = shape("s1");
    let proposal = of(&scenario, &[]);
    let artifacts = series(&proposal, "Artifacts");
    assert_eq!(
        artifacts.items.chosen.value,
        ItemCut::Places {
            places: 4,
            rule: CutRule::RecurringParts
        }
    );
    assert_eq!(artifacts.items.chosen.counts.items, 4);
    let photographs = series(&proposal, "Photographs");
    assert_eq!(photographs.items.chosen.value, ItemCut::Whole);
    assert_eq!(photographs.items.chosen.counts.items, 4);
    let scans = series(&proposal, "Scans");
    assert_eq!(
        scans.items.chosen.value,
        ItemCut::Places {
            places: 4,
            rule: CutRule::RecurringInItem
        }
    );
    assert_eq!(scans.items.chosen.counts.items, 1);
    let rival = scans.items.rival.as_ref().expect("a rival");
    assert_eq!((&rival.value, rival.counts.items), (&ItemCut::Whole, 3));

    let take = Edit::Items {
        scope: scope("Scans"),
        cut: CutEdit::Places { places: 4 },
    };
    let taken = of(&scenario, &[take.clone(), take]);
    assert!(
        taken.edits.iter().all(|edit| edit.applied),
        "the pinned edit asks for what the proposal does; twice is once"
    );
    assert_eq!(series(&taken, "Scans").items.chosen.counts.items, 1);
    let each = of(
        &scenario,
        &[Edit::Items {
            scope: scope("Scans"),
            cut: CutEdit::Whole,
        }],
    );
    assert!(each.edits[0].applied, "the rival is offered");
    assert_eq!(series(&each, "Scans").items.chosen.counts.items, 3);
    let not_offered = of(
        &scenario,
        &[Edit::Items {
            scope: scope("Scans"),
            cut: CutEdit::Places { places: 5 },
        }],
    );
    assert!(!not_offered.edits[0].applied, "a cut no count offers");
    assert_eq!(series(&not_offered, "Scans").items.chosen.counts.items, 1);
}

#[test]
fn s2_names_end_after_the_number() {
    let proposal = of(&shape("s2"), &[]);
    let all = series(&proposal, "");
    assert_eq!(
        all.items.chosen.value,
        ItemCut::Places {
            places: 2,
            rule: CutRule::NamesEndHere
        }
    );
    assert_eq!(all.items.chosen.counts.items, 4);
    assert_eq!(all.items.chosen.counts.names_ending_here, 3);
}

#[test]
fn s4_colliding_names_cut_at_their_folder() {
    let proposal = of(&shape("s4"), &[]);
    let boxes = series(&proposal, "");
    assert_eq!(boxes.items.chosen.value, ItemCut::Folder { depth: 2 });
    assert_eq!(boxes.items.chosen.counts.items, 3);
    let [split] = boxes.splits.as_slice() else {
        panic!("one offered split: {:?}", boxes.splits);
    };
    assert!(matches!(split.value, SetFrom::FolderLevel { depth: 1, .. }));
    let sets: Vec<(&str, usize)> = split
        .counts
        .sets
        .iter()
        .map(|(name, items)| (name.0.as_str(), *items))
        .collect();
    assert_eq!(sets, [("Box 1", 2), ("Box 2", 1)]);
}

#[test]
fn a_key_as_long_as_its_endings_is_a_shared_prefix() {
    let files = [
        ("Budget 2019.jpg", JPEG),
        ("Budget 2020.jpg", JPEG),
        ("Minutes 2019.jpg", JPEG),
        ("Minutes 2020.jpg", JPEG),
    ];
    let scenario = Scenario {
        index: index(&files),
        sheets: Sheets::new(),
        plan: plan(|_| {}),
        catalog: catalog(),
    };
    let proposal = of(&scenario, &[]);
    let all = series(&proposal, "");
    assert_eq!(all.items.chosen.value, ItemCut::Whole);
    assert_eq!(all.items.chosen.counts.items, 4);
}

#[test]
fn a_pattern_matches_the_names_its_key_shape_fits() {
    let shape = KeyShape::of(&places("AB_CD"));
    let pattern = FilePattern::compile(Some(&Key::Places(shape).pattern())).expect("compiles");
    let key = |path: &str| {
        pattern
            .groups(path)
            .map(|groups| groups["identifier"].text.clone())
    };
    assert_eq!(key("x/AB_CD_p1.jpg").as_deref(), Some("AB_CD"));
    assert_eq!(key("AB-CD.tif").as_deref(), Some("AB-CD"));
    assert_eq!(key("ABCD.jpg"), None, "one run of letters is one place");
    assert_eq!(key("AB_CD"), None, "a name with no extension");
    assert_eq!(key("12_CD.jpg"), None);

    let folder = FilePattern::compile(Some(
        &Key::Folder {
            depth: 2,
            lead: None,
        }
        .pattern(),
    ))
    .expect("compiles");
    let groups = folder.groups("Box 1/Folder 1/0001.tif").expect("a match");
    assert_eq!(groups["identifier"].text, "Box 1/Folder 1");
    assert_eq!(groups["part"].text, "0001");
    assert!(folder.groups("Box 1/0001.tif").is_none());
}

/// The items `proposal` makes, each as its files.
fn items_of(scenario: &Scenario, proposal: &Proposal) -> Vec<Vec<String>> {
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
        .items
        .values()
        .map(|files| {
            files
                .iter()
                .map(|(_, path)| path.as_str().to_owned())
                .collect()
        })
        .collect()
}

#[test]
fn a_restarted_counter_keeps_each_photo_an_item() {
    let files = [
        ("DCIM/2019-07-14/IMG_0001.JPG", JPEG),
        ("DCIM/2019-07-14/IMG_0002.JPG", JPEG),
        ("DCIM/2019-07-14/IMG_0003.JPG", JPEG),
        ("DCIM/2019-08-02/IMG_0004.JPG", JPEG),
        ("DCIM/2019-08-02/IMG_0005.JPG", JPEG),
        ("DCIM/2020-01-05/IMG_0001.JPG", JPEG),
        ("DCIM/2020-01-05/IMG_0002.JPG", JPEG),
    ];
    let scenario = Scenario {
        index: index(&files),
        sheets: Sheets::new(),
        plan: plan(|_| {}),
        catalog: catalog(),
    };
    let proposal = of(&scenario, &[]);
    let camera = series(&proposal, "");
    assert_eq!(
        camera.items.chosen.value,
        ItemCut::Whole,
        "2019-08-02 holds two names no other folder holds, so the counter only restarted"
    );
    assert_eq!(camera.items.chosen.counts.items, 7);
    assert_eq!(
        camera.items.rival.as_ref().map(|rival| &rival.value),
        Some(&ItemCut::Folder { depth: 2 })
    );
    assert!(
        items_of(&scenario, &proposal)
            .iter()
            .all(|files| files.len() == 1),
        "the same name in two folders is two photos"
    );

    let s4 = shape("s4");
    let boxes = of(&s4, &[]);
    assert_eq!(
        series(&boxes, "")
            .items
            .rival
            .as_ref()
            .map(|rival| (&rival.value, rival.counts.items)),
        Some((&ItemCut::Whole, 5)),
        "each folder restarts its pages, so the folder is the item; each file is the rival"
    );
}

fn flat(files: &[(&str, &str)]) -> (Scenario, Proposal) {
    let scenario = Scenario {
        index: index(files),
        sheets: Sheets::new(),
        plan: plan(|_| {}),
        catalog: catalog(),
    };
    let proposal = of(&scenario, &[]);
    (scenario, proposal)
}

#[test]
fn one_item_alone_in_its_series_proves_its_cut_by_its_own_pages() {
    let (_, ledger) = flat(&[
        ("Ledger_1907_004_p1.tif", TIFF),
        ("Ledger_1907_004_p2.tif", TIFF),
        ("Ledger_1907_004_p10.tif", TIFF),
    ]);
    let pages = &series(&ledger, "").items.chosen;
    assert_eq!(
        pages.value,
        ItemCut::Places {
            places: 3,
            rule: CutRule::RecurringInItem
        },
        "after the number, every file shares `p` and only the page differs"
    );
    assert_eq!(pages.counts.items, 1);

    let (_, narratives) = flat(&[
        ("Ledger_1907_PN_0100.tif", TIFF),
        ("Ledger_1907_PN_0101.tif", TIFF),
        ("Ledger_1907_PN_0102.tif", TIFF),
    ]);
    let each = &series(&narratives, "").items.chosen;
    assert_eq!(
        (&each.value, each.counts.items),
        (&ItemCut::Whole, 3),
        "a key that ends in letters is a shared prefix of numbered codes"
    );

    let (_, tapes) = flat(&[
        ("Tape_2017_V1.jpg", JPEG),
        ("Tape_2017_V2.jpg", JPEG),
        ("Tape_2017_AI01.jpg", JPEG),
    ]);
    let each = &series(&tapes, "").items.chosen;
    assert_eq!(
        (&each.value, each.counts.items),
        (&ItemCut::Whole, 3),
        "parts that differ before their last place are two kinds of code"
    );
}

#[test]
fn every_cut_the_counts_prove_is_a_choice() {
    let proposal = bhc006_tests::proposal(&[]);
    let artifacts = series(&proposal, "Artifacts");
    let others: Vec<(&ItemCut, usize)> = artifacts
        .items
        .others
        .iter()
        .map(|other| (&other.value, other.counts.items))
        .collect();
    assert_eq!(
        others,
        [(
            &ItemCut::Places {
                places: 6,
                rule: CutRule::RecurringParts
            },
            649
        )],
        "the next valid cut past the chosen one and the rival"
    );

    let taken = bhc006_tests::proposal(&[Edit::Items {
        scope: scope("Artifacts"),
        cut: CutEdit::Places { places: 6 },
    }]);
    assert!(taken.edits[0].applied, "a choice past the rival applies");
    let artifacts = series(&taken, "Artifacts");
    assert_eq!(artifacts.items.chosen.counts.items, 649);
    let rival = artifacts.items.rival.as_ref().expect("a rival");
    assert_eq!(
        (&rival.value, rival.counts.items),
        (
            &ItemCut::Places {
                places: 5,
                rule: CutRule::RecurringParts
            },
            318
        ),
        "the draft's cut is the rival, one click back"
    );
    let others: Vec<&ItemCut> = artifacts
        .items
        .others
        .iter()
        .map(|other| &other.value)
        .collect();
    assert_eq!(others, [&ItemCut::Whole]);
}
