//! The derive: evidence, sheets, the plan and the install in, the import's
//! [`Model`] out (`docs/import-v2.md`, "The derive").
//!
//! [`derive`] is pure: it reads no disk and no database. The caller passes
//! the evidence index, the sheets it read, the plan, a [`Catalog`] of the
//! install, and a [`FileTags`] reader. The steps run in the contract's
//! order: group files, apply `move` decisions, join sheets, make records,
//! resolve slots, match, then flag, report and digest.

mod decide;
mod finish;
pub(crate) mod records;
#[cfg(test)]
mod tests;

use std::collections::{BTreeMap, BTreeSet};

use super::catalog::Catalog;
use super::decisions::Act;
use super::evidence::{FileEntry, Walk};
use super::flags::FlagBook;
use super::join::{self, SheetIn};
use super::matching::{self, Matching};
use super::model::{Fate, Model, Parent, Record, RecordId, SlotValue, Subject};
use super::plan::{FlagKind, ImportPlanV2, RecordKind, SheetId, SlotName, SourceId, TagId, Term};
use super::sheet::{Sheet, SheetError};
use super::slots::{self, Ctx, named};
use super::{grouping, grouping::natural_cmp};

/// The evidence index: each source's last scan.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Index {
    pub sources: Vec<IndexedSource>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexedSource {
    pub id: SourceId,
    pub walk: Walk,
}

/// Each plan sheet as the caller read it. A sheet the plan names and the
/// map lacks is a sheet the caller could not find.
pub type Sheets = BTreeMap<SheetId, Result<Sheet, SheetError>>;

/// File tags, read by the caller's cache (`tags::TagCache`). The derive
/// asks only for the tags a slot rule names.
pub trait FileTags {
    fn tag(&self, source: &SourceId, entry: &FileEntry, tag: TagId) -> Option<String>;
}

/// No file has tags: for plans that read none.
pub struct NoTags;

impl FileTags for NoTags {
    fn tag(&self, _: &SourceId, _: &FileEntry, _: TagId) -> Option<String> {
        None
    }
}

#[must_use]
pub fn derive(
    index: &Index,
    sheets: &Sheets,
    plan: &ImportPlanV2,
    catalog: &Catalog,
    tags: &dyn FileTags,
) -> Model {
    let mut book = FlagBook::new(plan);
    let mut anomalies = Vec::new();
    plan_flags(plan, catalog, &mut book);
    let sheet_ins: Vec<SheetIn<'_>> = plan
        .sheets
        .iter()
        .map(|sheet| SheetIn {
            plan: sheet,
            sheet: sheets.get(&sheet.id).and_then(|read| read.as_ref().ok()),
        })
        .collect();
    finish::sheet_findings(plan, sheets, &mut book, &mut anomalies);

    let grouping = grouping::group(index, plan, &sheet_ins);
    let mut orphans = grouping.orphans.clone();
    let mut joined = join::join(&sheet_ins, &grouping, &plan.decisions, &mut book);
    orphans.append(&mut joined.orphans);
    let mut made = records::make(
        &grouping,
        &joined,
        &sheet_ins,
        catalog,
        plan.anomalies.items_without_row,
        &mut book,
    );
    let entries: Vec<_> = made
        .drafts
        .iter()
        .map(|draft| draft.entries(&grouping, &sheet_ins))
        .collect();
    let decided = decide::apply(
        plan,
        &grouping,
        &sheet_ins,
        &mut made.drafts,
        &entries,
        &mut orphans,
        &mut anomalies,
    );

    let ctx = Ctx::new(plan, catalog, &grouping, &sheet_ins, tags);
    let ids: Vec<RecordId> = (0..made.drafts.len())
        .map(|at| RecordId(u32::try_from(at).unwrap_or(u32::MAX)))
        .collect();
    let of_kind = |kind: RecordKind, drafts: &[records::Draft]| -> Vec<RecordId> {
        ids.iter()
            .copied()
            .filter(|id| drafts[id.index()].kind == kind)
            .collect()
    };
    let (media_ids, set_ids, item_ids) = (
        of_kind(RecordKind::Media, &made.drafts),
        of_kind(RecordKind::ItemSet, &made.drafts),
        of_kind(RecordKind::Item, &made.drafts),
    );
    let mut resolved_slots: Vec<BTreeMap<SlotName, Vec<SlotValue>>> =
        vec![BTreeMap::new(); made.drafts.len()];
    // Media first: an item's tag rule reads its first file in media order.
    for id in &media_ids {
        resolved_slots[id.index()] = slots::resolve(
            &ctx,
            &made.drafts[id.index()],
            *id,
            &entries[id.index()],
            &decided.sets,
            &mut book,
        );
    }
    for id in &item_ids {
        let mut media = made.drafts[id.index()].media.clone();
        let key = |media: &RecordId| {
            media_order_key(&made.drafts[media.index()], &resolved_slots[media.index()])
        };
        media.sort_by(|a, b| natural_cmp(&key(a), &key(b)));
        let files = media
            .iter()
            .flat_map(|media| made.drafts[media.index()].files.clone())
            .collect();
        made.drafts[id.index()].files = files;
        made.drafts[id.index()].media = media;
    }
    // Item sets before items: a `sets` value can name an item-set record.
    for id in set_ids.iter().chain(&item_ids) {
        resolved_slots[id.index()] = slots::resolve(
            &ctx,
            &made.drafts[id.index()],
            *id,
            &entries[id.index()],
            &decided.sets,
            &mut book,
        );
    }

    let title =
        SlotName::Term(Term::parse("dcterms:title").unwrap_or_else(|_| Term::dcterms_identifier()));
    let first_text = |slots: &BTreeMap<SlotName, Vec<SlotValue>>, slot: &SlotName| {
        slots
            .get(slot)
            .and_then(|values| values.first())
            .map(|value| value.value.text.clone())
    };
    let set_records: Vec<_> = set_ids
        .iter()
        .map(|id| {
            let slots = &resolved_slots[id.index()];
            (
                *id,
                first_text(slots, &SlotName::ItemKey),
                first_text(slots, &title),
            )
        })
        .collect();
    let mut set_book = named::SetBook::new(&ctx, &set_records);

    let mut records: Vec<Record> = Vec::with_capacity(made.drafts.len());
    for (at, draft) in made.drafts.iter().enumerate() {
        let id = ids[at];
        let mut slots = std::mem::take(&mut resolved_slots[at]);
        let (resolved, named_left) =
            named::resolve(&ctx, id, &slots, &entries[at], &mut set_book, &mut book);
        let fields_left = slots::check_fields(
            &ctx,
            id,
            &entries[at],
            &mut slots,
            resolved.template,
            resolved.class.as_ref(),
            &mut book,
        );
        let left = draft.left.or(named_left).or(fields_left);
        records.push(Record {
            id,
            kind: draft.kind,
            origin: draft.origin.clone(),
            files: draft
                .files
                .iter()
                .map(|file| grouping.files[*file].file.clone())
                .collect(),
            rows: draft.rows.iter().map(|row| row.entry(&sheet_ins)).collect(),
            parent: draft.parent.map(Parent::Record),
            media: draft.media.clone(),
            slots,
            resolved,
            matches: Vec::new(),
            fate: left.map_or(Fate::Create, Fate::Left),
        });
    }

    let matching = Matching::new(plan, catalog, &records);
    matching::run(
        &matching,
        &mut records,
        &entries,
        &decided.matches,
        &mut book,
        &mut anomalies,
    );
    matching::link(&matching, &mut records, &entries, &mut book);

    finish::finish(finish::Parts {
        plan,
        grouping: &grouping,
        sheets: &sheet_ins,
        joined: &joined,
        file_left: &made.file_left,
        file_record: &made.file_record,
        index,
        records,
        new_sets: set_book.new_sets,
        flags: book.into_flags(),
        anomalies,
        orphans,
    })
}

/// A media record's sort key: its `mediaOrder`, else its origin (its path).
fn media_order_key(draft: &records::Draft, slots: &BTreeMap<SlotName, Vec<SlotValue>>) -> String {
    slots
        .get(&SlotName::MediaOrder)
        .and_then(|values| values.first())
        .map_or_else(|| draft.origin.key(), |value| value.value.text.clone())
}

/// The blocking plan flags: `unknown-term`, `template-missing`,
/// `type-exists`.
fn plan_flags(plan: &ImportPlanV2, catalog: &Catalog, book: &mut FlagBook<'_>) {
    let new_terms: BTreeSet<Term> = plan
        .new_terms
        .iter()
        .filter_map(|term| Term::local(&term.label))
        .collect();
    let decided = plan
        .decisions
        .iter()
        .filter_map(|decision| match decision.act()? {
            Act::Set { slot, .. } => Some(slot),
            _ => None,
        });
    let named: BTreeSet<&Term> = plan
        .slots
        .rules()
        .map(|(slot, _)| slot)
        .chain(decided)
        .filter_map(|slot| match slot {
            SlotName::Term(term) => Some(term),
            _ => None,
        })
        .collect();
    for term in named {
        if !catalog.properties.contains_key(term) && !new_terms.contains(term) {
            book.raise(
                FlagKind::UnknownTerm,
                Subject::Plan,
                Some(SlotName::Term(term.clone())),
                Some(term.to_string()),
                &[],
            );
        }
    }
    if let Some(template) = plan.defaults.template
        && catalog.template(template).is_none()
    {
        book.raise(
            FlagKind::TemplateMissing,
            Subject::Plan,
            Some(SlotName::Template),
            Some(template.to_string()),
            &[],
        );
    }
    let existing: BTreeSet<String> = catalog
        .templates
        .iter()
        .map(|template| super::values::fold_title(&template.label))
        .collect();
    for new_type in &plan.new_types {
        if existing.contains(&super::values::fold_title(&new_type.label)) {
            book.raise(
                FlagKind::TypeExists,
                Subject::Plan,
                Some(SlotName::Template),
                Some(new_type.label.clone()),
                &[],
            );
        }
    }
}
