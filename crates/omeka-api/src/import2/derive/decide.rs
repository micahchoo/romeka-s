//! The `leave`, `set`, `match` and `flag` decisions: which record each one
//! targets, and which are orphans (`docs/import-v2.md`, "Decisions").
//!
//! A decision targets the record of its kind that holds its entry: for a
//! file, its media record or its group's item record; for a row, the record
//! the row made or joined. `leave` has no kind: a file leaves its item, a
//! row the record it made or joined.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use super::super::decisions::{Decision, EntryRef, RowRef};
use super::super::grouping::Grouping;
use super::super::join::SheetIn;
use super::super::model::{Anomaly, LeftBy, Orphan, OrphanReason, RecordId};
use super::super::plan::{ImportPlanV2, RecordKind, SheetId, SlotName};
use super::super::sheet::key_column;
use super::super::slots::SetDecisions;
use super::records::Draft;

pub(crate) struct Decided {
    pub sets: SetDecisions,
    pub matches: HashMap<RecordId, Option<i64>>,
}

/// Every row of every sheet read, for the orphan test.
fn rows_read(sheets: &[SheetIn<'_>]) -> HashSet<RowRef> {
    sheets
        .iter()
        .filter_map(|sheet| Some((&sheet.plan.id, sheet.sheet?)))
        .flat_map(|(id, sheet)| {
            sheet.rows.iter().map(|row| RowRef {
                sheet: id.clone(),
                key: row.key.clone(),
            })
        })
        .collect()
}

/// Sheets keyed by line whose bytes changed since the plan stored a hash.
fn line_keys_changed(sheets: &[SheetIn<'_>]) -> BTreeSet<SheetId> {
    sheets
        .iter()
        .filter(|sheet| key_column(sheet.plan).is_none())
        .filter(|sheet| match (&sheet.plan.hash, sheet.sheet) {
            (Some(stored), Some(read)) => *stored != read.hash,
            _ => false,
        })
        .map(|sheet| sheet.plan.id.clone())
        .collect()
}

/// `entries` holds each draft's entries, by the draft's place.
pub(crate) fn apply(
    plan: &ImportPlanV2,
    grouping: &Grouping<'_>,
    sheets: &[SheetIn<'_>],
    drafts: &mut [Draft],
    entries: &[Vec<EntryRef>],
    orphans: &mut Vec<Orphan>,
    anomalies: &mut Vec<Anomaly>,
) -> Decided {
    let wanted: HashSet<EntryRef> = plan
        .decisions
        .iter()
        .filter(|decision| !matches!(decision, Decision::Move { .. }))
        .map(Decision::entry)
        .collect();
    let mut holders: HashMap<&EntryRef, Vec<RecordId>> = HashMap::new();
    if !wanted.is_empty() {
        for (at, held) in entries.iter().enumerate() {
            let id = RecordId(u32::try_from(at).unwrap_or(u32::MAX));
            for entry in held.iter().filter(|entry| wanted.contains(*entry)) {
                holders.entry(entry).or_default().push(id);
            }
        }
    }
    let rows = rows_read(sheets);
    let changed = line_keys_changed(sheets);
    let mut decided = Decided {
        sets: HashMap::new(),
        matches: HashMap::new(),
    };
    let mut set_by: BTreeMap<(RecordId, SlotName), Vec<(usize, EntryRef)>> = BTreeMap::new();

    for (decision, choice) in plan.decisions.iter().enumerate() {
        if matches!(choice, Decision::Move { .. }) {
            continue;
        }
        let entry = choice.entry();
        let orphan = |reason| Orphan { decision, reason };
        let present = match &entry {
            EntryRef::File(file) => grouping.by_ref.contains_key(file),
            EntryRef::Row(row) => rows.contains(row),
        };
        if !present {
            orphans.push(orphan(OrphanReason::MissingEntry));
            continue;
        }
        if let EntryRef::Row(row) = &entry
            && changed.contains(&row.sheet)
        {
            orphans.push(orphan(OrphanReason::LineKeyChanged));
            continue;
        }
        let held = holders.get(&entry).map(Vec::as_slice).unwrap_or_default();
        let of_kind = |kind: RecordKind| {
            held.iter()
                .copied()
                .find(|id| drafts[id.index()].kind == kind)
        };
        let target = match choice {
            Decision::Leave { .. } => match &entry {
                EntryRef::File(_) => {
                    of_kind(RecordKind::Item).or_else(|| of_kind(RecordKind::Media))
                }
                EntryRef::Row(_) => held.first().copied(),
            },
            Decision::Set { target, .. } | Decision::Match { target, .. } => of_kind(*target),
            Decision::Flag { .. } | Decision::Move { .. } => continue,
        };
        let Some(target) = target else {
            orphans.push(orphan(OrphanReason::WrongTarget));
            continue;
        };
        match choice {
            Decision::Leave { .. } => {
                drafts[target.index()].left = Some(LeftBy::Decision(decision))
            }
            Decision::Set { slot, .. } => {
                decided
                    .sets
                    .entry(target)
                    .or_default()
                    .entry(slot.clone())
                    .or_default()
                    .push(decision);
                set_by
                    .entry((target, slot.clone()))
                    .or_default()
                    .push((decision, entry));
            }
            Decision::Match { resource, .. } => {
                decided.matches.insert(target, *resource);
            }
            Decision::Flag { .. } | Decision::Move { .. } => {}
        }
    }

    for ((record, slot), by) in set_by {
        let entries: BTreeSet<&EntryRef> = by.iter().map(|(_, entry)| entry).collect();
        if entries.len() > 1 {
            anomalies.push(Anomaly::DecisionOverlap {
                record,
                slot,
                decisions: by.iter().map(|(decision, _)| *decision).collect(),
            });
        }
    }
    decided
}
