//! The proposal on each held-out shape, through the preview route, with the
//! shape's pinned edits (`edits/<shape>.json`): its structure is taken into
//! the draft's plan and placed against the frozen v3 plan's placement, as
//! `import_shapes::proposal` does for BHC006. This is the overfitting test:
//! nobody who built the proposer saw these shapes.
//!
//! The click meter (`omeka-api`'s `import::clicks`) then reads each shape
//! from the as-found proposal, with no pinned edit: the offered clicks a
//! person needs, and what is left after them.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;

use serde_json::{Value, json};

use omeka_api::import::archive::ArchiveRoots;
use omeka_api::import::catalog_load::Owners;
use omeka_api::import::clicks::{Click, Metered, meter};
use omeka_api::import::derive::Sheets;
use omeka_api::import::placement::{Difference, ListFiles, Placement, differences};
use omeka_api::import::plan::{
    Auto, Delimiter, Edit, EncodingChoice, ImportPlan, ListFile, Quote, ReadOptions, RecordKind,
    RowKind, SheetId, SheetPlan,
};
use omeka_api::import::preview::proposal::{Evidence, FoundList, Proposal, propose};
use omeka_api::import::preview::{Caches, Derived, derive_import};
use omeka_api::import::scan::source_dirs;
use omeka_db::ids::ImportId;
use omeka_db::with_db;
use omeka_media::store::{FileStore, Store};
use omeka_media::validation::FileValidation;

use super::{
    Shape, accession_register, add_terms, born_digital, materialize, negative_archive,
    oral_history, paper_archive, placement_of, run, v3_plan, v3_plan_of,
};
use crate::imports_fixture::{self, Fixture, fixture};

/// How far a person is from each held-out shape's frozen outcome, starting
/// from the proposal: `unapplied` counts the pinned edits the proposal does
/// not offer (a click the person needs and cannot make), and `left` counts
/// the differences that remain after every pinned edit (what no click
/// fixes). Both only fall. A measured number above its entry is a
/// regression; one below it fails too, so the commit that lowers a number
/// lowers its entry and the table always states the truth.
///
/// The pinned edits were written before the proposal offered corrections,
/// so the two numbers after them are the meter's: `clicks` counts the
/// offered clicks the meter takes from the as-found proposal (both of its
/// phases, each untick one more), and `after` counts the differences left
/// after them. `after` is held like `left`. `clicks` is held exactly but is
/// no ratchet: fewer clicks for the same `after` is better, and so are more
/// clicks that lower `after`.
#[rustfmt::skip]
const DISTANCE: &[Distance] = &[
    Distance { shape: "accession_register", unapplied: 1, left: 39, clicks: 1, after: 40 },
    Distance { shape: "born_digital",       unapplied: 0, left: 4,  clicks: 2, after: 1 },
    Distance { shape: "negative_archive",   unapplied: 2, left: 10, clicks: 1, after: 3 },
    Distance { shape: "oral_history",       unapplied: 0, left: 0,  clicks: 1, after: 0 },
    Distance { shape: "paper_archive",      unapplied: 0, left: 4,  clicks: 2, after: 2 },
];

#[derive(Debug, PartialEq, Eq)]
struct Distance {
    shape: &'static str,
    unapplied: usize,
    left: usize,
    clicks: usize,
    after: usize,
}

fn pinned_edits(name: &str) -> Value {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/heldout/edits")
        .join(format!("{name}.json"));
    let pinned: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    pinned["edits"].clone()
}

/// A scanned draft of a shape, and its plan as found: one series over the
/// whole source, as v3, with no edit.
struct Asked {
    fixture: Fixture,
    id: Value,
    as_found: Value,
}

async fn asked(shape: &Shape, slots: &[&str]) -> Asked {
    let fixture = fixture().await;
    add_terms(&fixture, slots).await;
    materialize(shape, fixture.archive.path());
    let draft = fixture.draft().await;
    let id = draft["id"].clone();
    let scanned = fixture
        .call("POST", &format!("/api/romeka/imports/{id}/scan"), None)
        .await;
    assert_eq!(scanned.status, 200, "{}", scanned.body);
    let levels = json!([imports_fixture::v3_series("all", "")]);
    let as_found = imports_fixture::v3_plan(&draft["plan"], levels);
    Asked {
        fixture,
        id,
        as_found,
    }
}

/// The plan a person confirms on the draft: the proposal's structure with
/// `edits`. Also gives the proposal the route answered.
async fn proposed(asked: &Asked, edits: &Value) -> (String, Value) {
    let mut tried = asked.as_found.clone();
    tried["edits"] = edits.clone();
    let answer = asked
        .fixture
        .preview(
            &asked.id,
            &json!({ "include": ["proposal"], "plan": tried }),
        )
        .await;
    assert_eq!(answer.status, 200, "{}", answer.body);
    let proposal = answer.json()["proposal"].clone();
    let mut confirmed = asked.as_found.clone();
    confirmed["structure"] = proposal["structure"].clone();
    confirmed["edits"] = edits.clone();
    (confirmed.to_string(), proposal)
}

/// How the preview reads a list it finds in a folder, as Simple adds one
/// (`preview/candidates.rs`, `automatic`). The meter's proposal must read
/// lists as the route's does, which `places_as_frozen` asserts.
fn as_simple_adds(file: &ListFile) -> SheetPlan {
    SheetPlan {
        id: SheetId(String::new()),
        source: file.source.clone(),
        file: file.file.clone(),
        hash: None,
        read: ReadOptions {
            encoding: EncodingChoice::Auto(Auto::Auto),
            delimiter: Delimiter::Auto,
            quote: Quote::Double,
            header_line: std::num::NonZeroU32::MIN,
            trim: true,
            sheet_name: None,
        },
        row_kind: RowKind::All(RecordKind::Item),
        join: None,
        item_key: None,
        columns: Vec::new(),
    }
}

/// What `derive_import` reads besides the plan, as the routes hand it.
struct Install<'a> {
    asked: &'a Asked,
    roots: ArchiveRoots,
    store: Arc<dyn Store>,
    caches: Arc<Caches>,
    files: FileValidation,
}

impl<'a> Install<'a> {
    async fn new(asked: &'a Asked) -> Self {
        let root = asked.fixture.archive.path().to_path_buf();
        Self {
            asked,
            roots: ArchiveRoots::new([("archive".to_string(), root)]).unwrap(),
            store: Arc::new(FileStore::new(asked.fixture.files.path(), "/files")),
            caches: Arc::new(Caches::default()),
            files: omeka_api::settings::file_validation(&asked.fixture.pool)
                .await
                .unwrap(),
        }
    }

    /// `plan` derived against the draft's scan, its catalog and its lists
    /// read for that plan, as the run route derives it.
    async fn derive(&self, plan: ImportPlan) -> Derived {
        let import = ImportId(self.asked.id.as_u64().unwrap());
        with_db!(&self.asked.fixture.pool, |db| derive_import(
            db,
            &self.roots,
            &self.store,
            &self.caches,
            import,
            Arc::new(plan),
            Owners::Anyone,
            self.files.clone(),
        )
        .await)
        .unwrap()
    }
}

/// The meter's judge on a held-out draft, as `omeka-api`'s own judges are
/// (`bhc006_tests::judge`): the proposal after some edits, from the as-found
/// evidence, and how the model its plan derives places the files against
/// the frozen outcome.
struct Judge<'a> {
    install: Install<'a>,
    want: &'a Placement,
    found: Derived,
    lists: Vec<FoundList>,
}

impl<'a> Judge<'a> {
    async fn new(asked: &'a Asked, want: &'a Placement) -> Self {
        let install = Install::new(asked).await;
        let base = ImportPlan::from_json(&asked.as_found.to_string()).unwrap();
        let dirs = source_dirs(&install.roots, &base.sources);
        let found = install.derive(base).await;
        let index = &found.stored.index;
        let lists = index
            .sources
            .iter()
            .flat_map(|source| source.walk.sheets.iter().map(move |entry| (source, entry)))
            .filter_map(|(source, entry)| {
                let file = ListFile {
                    source: source.id.clone(),
                    file: entry.path.clone(),
                };
                let disk = index.disk_path(&source.id, &entry.path);
                let read = install.caches.sheets.read(
                    &as_simple_adds(&file),
                    dirs.get(&source.id)?,
                    &disk,
                )?;
                Some(FoundList {
                    file,
                    sheet: read.sheet.clone(),
                })
            })
            .collect();
        Self {
            install,
            want,
            found,
            lists,
        }
    }

    fn judge(&self, edits: &[Edit]) -> (Proposal, Vec<Difference>) {
        let sheets = sheets_of(&self.found);
        let evidence = Evidence {
            index: &self.found.stored.index,
            lists: &self.lists,
            catalog: &self.found.catalog,
            sources: &self.found.plan.sources,
            structure: &self.found.plan.structure,
            sheets: &sheets,
        };
        let proposal = propose(&evidence, edits);
        let plan = ImportPlan {
            structure: proposal.structure.clone(),
            edits: edits.to_vec(),
            ..(*self.found.plan).clone()
        };
        // The meter's judge is a plain function; the derive reads the
        // database, so it runs on this test's runtime, which has threads.
        let derived = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(self.install.derive(plan.clone()))
        });
        let sheets = sheets_of(&derived);
        let got = Placement::of_model(&derived.model, &plan, &ListFiles::new(&plan, &sheets))
            .with_picks(&proposal);
        (proposal, differences(self.want, &got))
    }
}

fn sheets_of(derived: &Derived) -> Sheets {
    derived
        .sheets
        .iter()
        .map(|(id, read)| (id.clone(), read.sheet.clone()))
        .collect()
}

/// The differences by kind, the only form of them the gate prints.
fn kinds(found: &[Difference]) -> BTreeMap<&'static str, usize> {
    let mut kinds = BTreeMap::new();
    for difference in found {
        *kinds.entry(kind(difference)).or_default() += 1;
    }
    kinds
}

/// The meter's clicks by kind: an edit by its wire name, a column's rival
/// segment, an untick.
fn click_kinds(metered: &Metered) -> BTreeMap<String, usize> {
    let mut kinds = BTreeMap::new();
    for click in metered.clicks.iter().chain(&metered.corrections) {
        let name = match click {
            Click::Edit(edit) => serde_json::to_value(edit).unwrap()["edit"]
                .as_str()
                .unwrap()
                .to_string(),
            Click::Column { .. } => "column".to_string(),
            Click::Untick { .. } => "untick".to_string(),
        };
        *kinds.entry(name).or_default() += 1;
    }
    kinds
}

/// A difference's kind. The gate prints kinds and counts only: its output
/// reaches people who must not see a held-out shape's paths.
fn kind(difference: &Difference) -> &'static str {
    match difference {
        Difference::FilesSplit { .. } => "filesSplit",
        Difference::FilesMerged { .. } => "filesMerged",
        Difference::RowItem { .. } => "rowItem",
        Difference::Set { .. } => "set",
        Difference::List { .. } => "list",
        Difference::Row { .. } => "row",
        Difference::LeftOut { .. } => "leftOut",
        Difference::Column { .. } => "column",
    }
}

async fn places_as_frozen(name: &str, shape: &Shape, slots: &[&str], plan_text: &'static str) {
    let frozen = fixture().await;
    let preview = run(&frozen, shape, slots, v3_plan(plan_text)).await;
    let want = placement_of(&preview, &v3_plan_of(plan_text), frozen.archive.path());

    let edits = pinned_edits(name);
    let asked = asked(shape, slots).await;
    let (text, proposal) = proposed(&asked, &edits).await;
    let replayed = proposal["edits"].as_array().unwrap();
    assert_eq!(
        replayed.len(),
        edits.as_array().unwrap().len(),
        "{name}: every pinned edit is replayed"
    );
    let unapplied = replayed
        .iter()
        .filter(|result| result["applied"] != true)
        .count();

    let plan = ImportPlan::from_json(&text).unwrap();
    let saved = fixture().await;
    let got_preview = run(&saved, shape, slots, |draft_plan: &mut Value| {
        let mut plan: Value = serde_json::from_str(&text).unwrap();
        plan["sources"] = draft_plan["sources"].clone();
        *draft_plan = plan;
    })
    .await;
    let got = placement_of(&got_preview, &plan, saved.archive.path());
    let found = differences(&want, &got);
    let by_kind = kinds(&found);
    eprintln!(
        "{name}: {unapplied} unapplied, {} left {by_kind:?}",
        found.len()
    );

    let started = std::time::Instant::now();
    let judge = Judge::new(&asked, &want).await;
    let (pinned, after_pins) = judge.judge(&plan.edits);
    assert_eq!(
        serde_json::to_value(&pinned).unwrap(),
        proposal,
        "{name}: the meter's proposal is the route's"
    );
    assert_eq!(
        kinds(&after_pins),
        by_kind,
        "{name}: the meter's judge places the pinned plan as the run route does"
    );
    let (_, as_found) = judge.judge(&[]);
    let metered = meter(|edits| judge.judge(edits));
    eprintln!(
        "{name}: as found {} {:?}; meter {}; clicks {:?}; {:.1?}",
        as_found.len(),
        kinds(&as_found),
        metered.line(),
        click_kinds(&metered),
        started.elapsed()
    );

    let entry = DISTANCE
        .iter()
        .find(|entry| entry.shape == name)
        .unwrap_or_else(|| panic!("{name}: no DISTANCE entry"));
    let measured = Distance {
        shape: entry.shape,
        unapplied,
        left: found.len(),
        clicks: metered.clicks.len() + metered.corrections.len(),
        after: metered.left.len(),
    };
    let gaps = metered.gaps();
    assert!(
        measured.unapplied <= entry.unapplied
            && measured.left <= entry.left
            && measured.after <= entry.after,
        "{name}: regressed to {} unapplied, {} left {by_kind:?}, {} after {gaps:?}; DISTANCE says {}, {}, {}",
        measured.unapplied,
        measured.left,
        measured.after,
        entry.unapplied,
        entry.left,
        entry.after
    );
    assert!(
        measured.unapplied == entry.unapplied
            && measured.left == entry.left
            && measured.after == entry.after,
        "{name}: improved to {} unapplied, {} left {by_kind:?}, {} after {gaps:?}; lower its DISTANCE entry from {}, {}, {}",
        measured.unapplied,
        measured.left,
        measured.after,
        entry.unapplied,
        entry.left,
        entry.after
    );
    assert_eq!(
        &measured, entry,
        "{name}: the meter takes {} clicks for its {} after; record them in DISTANCE, which says {}",
        measured.clicks, measured.after, entry.clicks
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn oral_history_proposal_places_as_frozen() {
    places_as_frozen(
        "oral_history",
        &oral_history::shape(),
        &oral_history::SLOTS,
        include_str!("fixtures/oral_history.json"),
    )
    .await;
}

#[tokio::test(flavor = "multi_thread")]
async fn born_digital_proposal_places_as_frozen() {
    places_as_frozen(
        "born_digital",
        &born_digital::shape(),
        &[],
        include_str!("fixtures/born_digital.json"),
    )
    .await;
}

#[tokio::test(flavor = "multi_thread")]
async fn paper_archive_proposal_places_as_frozen() {
    places_as_frozen(
        "paper_archive",
        &paper_archive::shape(),
        &paper_archive::SLOTS,
        paper_archive::PLAN,
    )
    .await;
}

#[tokio::test(flavor = "multi_thread")]
async fn accession_register_proposal_places_as_frozen() {
    places_as_frozen(
        "accession_register",
        &accession_register::shape(),
        &accession_register::SLOTS,
        accession_register::PLAN,
    )
    .await;
}

#[tokio::test(flavor = "multi_thread")]
async fn negative_archive_proposal_places_as_frozen() {
    places_as_frozen(
        "negative_archive",
        &negative_archive::shape(),
        &negative_archive::SLOTS,
        negative_archive::PLAN,
    )
    .await;
}
