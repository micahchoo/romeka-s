//! Step 7 of the derive: the sheet findings, where each entry landed, the
//! anomalies, the new types' fields, and the digest.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use super::super::decisions::{EntryRef, FileRef, RowRef};
use super::super::flags::FlagBook;
use super::super::grouping::Grouping;
use super::super::join::{Joined, RowAt, SheetIn};
use super::super::model::{
    Anomaly, EntryState, Fate, Flag, LeftBy, Model, NewSet, NewTypeFields, Orphan, Parent,
    PatternState, Record, RecordId, Subject, TemplateRef,
};
use super::super::plan::{FlagKind, ImportPlan, RecordKind, RowKind, SlotName, Term};
use super::super::sheet::{HeaderNote, SheetError};
use super::super::slots::named;
use super::{Index, Sheets};

/// `encoding-unsupported`, unread sheets, changed bytes, header notes and
/// unmapped columns.
pub(super) fn sheet_findings(
    plan: &ImportPlan,
    sheets: &Sheets,
    book: &mut FlagBook<'_>,
    anomalies: &mut Vec<Anomaly>,
) {
    for (_, sheet_plan) in plan.structure.lists() {
        let id = sheet_plan.id.clone();
        let sheet = match sheets.get(&id) {
            None => {
                anomalies.push(Anomaly::SheetUnread {
                    sheet: id,
                    detail: "missing".to_string(),
                });
                continue;
            }
            Some(Err(SheetError::EncodingUnsupported(name))) => {
                book.raise(
                    FlagKind::EncodingUnsupported,
                    Subject::Sheet(id),
                    None,
                    Some((*name).to_string()),
                    &[],
                );
                continue;
            }
            Some(Err(error)) => {
                anomalies.push(Anomaly::SheetUnread {
                    sheet: id,
                    detail: error.to_string(),
                });
                continue;
            }
            Some(Ok(sheet)) => sheet,
        };
        if let Some(stored) = &sheet_plan.hash
            && *stored != sheet.hash
        {
            anomalies.push(Anomaly::SheetChanged {
                sheet: id.clone(),
                stored: stored.clone(),
                now: sheet.hash.clone(),
            });
        }
        if !sheet.notes.is_empty() {
            let notes = sheet
                .notes
                .iter()
                .map(|note| match note {
                    HeaderNote::Duplicate { header, indexes } => {
                        format!("duplicate {header:?} at {indexes:?}")
                    }
                    HeaderNote::Empty { index } => format!("empty at {index}"),
                })
                .collect();
            anomalies.push(Anomaly::HeaderNotes {
                sheet: id.clone(),
                notes,
            });
        }
        let mut used: BTreeSet<u32> = sheet_plan
            .columns
            .iter()
            .map(|column| column.column.index)
            .collect();
        used.extend(sheet_plan.join.iter().map(|join| join.column.index));
        used.extend(sheet_plan.item_key.iter().map(|column| column.index));
        if let RowKind::Column(kind) = &sheet_plan.row_kind {
            used.insert(kind.column.index);
        }
        let columns: Vec<(u32, String)> = sheet
            .columns
            .iter()
            .filter(|column| column.mappable && !used.contains(&column.index))
            .map(|column| (column.index, column.header.clone()))
            .collect();
        if !columns.is_empty() {
            anomalies.push(Anomaly::UnmappedColumns { sheet: id, columns });
        }
    }
}

pub(super) struct Parts<'a, 'i> {
    pub plan: &'a ImportPlan,
    pub grouping: &'a Grouping<'i>,
    pub sheets: &'a [SheetIn<'i>],
    pub joined: &'a Joined,
    pub file_left: &'a [Option<LeftBy>],
    pub file_record: &'a [Option<RecordId>],
    pub index: &'a Index,
    pub records: Vec<Record>,
    pub new_sets: Vec<NewSet>,
    pub flags: Vec<Flag>,
    pub anomalies: Vec<Anomaly>,
    pub orphans: Vec<Orphan>,
    /// How many of `plan.decisions` are the structure's corrections, read
    /// ahead of the plan's own.
    pub corrections: usize,
}

pub(super) fn finish(parts: Parts<'_, '_>) -> Model {
    let Parts {
        plan,
        grouping,
        sheets,
        joined,
        file_left,
        file_record,
        index,
        mut records,
        new_sets,
        flags,
        mut anomalies,
        mut orphans,
        corrections,
    } = parts;

    for at in 0..records.len() {
        if let Some(Parent::Record(item)) = records[at].parent
            && matches!(records[item.index()].fate, Fate::Left(_))
            && !matches!(records[at].fate, Fate::Left(_))
        {
            records[at].fate = Fate::Left(LeftBy::Parent);
        }
    }
    media_lists(&mut records);
    let entries = entry_states(grouping, sheets, joined, file_left, file_record, &records);
    shape_anomalies(&records, &mut anomalies);
    let unused: Vec<EntryRef> = entries
        .iter()
        .filter(|entry| {
            entry.record.is_none()
                && !matches!(entry.left, Some(LeftBy::Decision(_) | LeftBy::Outside))
        })
        .map(|entry| entry.entry.clone())
        .collect();
    if !unused.is_empty() {
        anomalies.push(Anomaly::UnusedEntries { entries: unused });
    }
    for source in &index.sources {
        let walk = &source.walk;
        let found = &walk.anomalies;
        let decoded: Vec<String> = walk.decoded_names().map(|(raw, _)| raw).collect();
        let any = found.symlinks + found.dot + found.special > 0
            || !found.unreadable.is_empty()
            || !found.unusable_names.is_empty()
            || !found.collisions.is_empty()
            || !decoded.is_empty();
        if any {
            anomalies.push(Anomaly::Walk {
                source: source.id.clone(),
                symlinks: found.symlinks,
                dot: found.dot,
                special: found.special,
                unreadable: found.unreadable.clone(),
                unusable_names: found.unusable_names.clone(),
                collisions: found.collisions.clone(),
                decoded,
            });
        }
    }
    let mut entries = entries;
    renumber(
        corrections,
        plan.decisions.len() - corrections,
        &mut records,
        &mut entries,
        &mut anomalies,
        &mut orphans,
    );
    orphans.sort_by_key(|orphan| orphan.decision);
    let new_types = new_type_fields(plan, &records);
    Model {
        pattern: grouping
            .patterns
            .first()
            .cloned()
            .unwrap_or(PatternState::Missing),
        patterns: grouping.patterns.clone(),
        records,
        entries,
        new_sets,
        new_types,
        flags,
        anomalies,
        orphans,
        digest: String::new(),
    }
    .seal()
}

/// Every decision index as the plan numbers it. Derive reads `corrections`
/// corrections ahead of the plan's `own` decisions, so a decision's place
/// there is `corrections` past its place in the plan. A correction is
/// numbered past the plan's decisions, in plan order, so no index names a
/// decision of the plan that it is not.
fn renumber(
    corrections: usize,
    own: usize,
    records: &mut [Record],
    entries: &mut [EntryState],
    anomalies: &mut [Anomaly],
    orphans: &mut [Orphan],
) {
    if corrections == 0 {
        return;
    }
    let number = |at: &mut usize| {
        *at = if *at < corrections {
            own + *at
        } else {
            *at - corrections
        };
    };
    let left = |by: &mut Option<LeftBy>| {
        if let Some(LeftBy::Decision(at)) = by {
            number(at);
        }
    };
    for record in records {
        if let Fate::Left(LeftBy::Decision(at)) = &mut record.fate {
            number(at);
        }
    }
    for entry in entries {
        left(&mut entry.left);
    }
    for anomaly in anomalies {
        if let Anomaly::DecisionOverlap { decisions, .. } = anomaly {
            decisions.iter_mut().for_each(number);
        }
    }
    for orphan in orphans {
        number(&mut orphan.decision);
    }
}

/// Each item's kept media, in the order its files were sorted, row media
/// last in natural order of their media order or key.
fn media_lists(records: &mut [Record]) {
    let mut row_media: BTreeMap<RecordId, Vec<RecordId>> = BTreeMap::new();
    for record in records.iter() {
        if let (Some(Parent::Record(item)), RecordKind::Media) = (&record.parent, record.kind)
            && !records[item.index()].media.contains(&record.id)
        {
            row_media.entry(*item).or_default().push(record.id);
        }
    }
    for (item, mut media) in row_media {
        media.sort_by(|a, b| {
            let key = |id: &RecordId| {
                let record = &records[id.index()];
                record
                    .resolved
                    .media_order
                    .clone()
                    .unwrap_or_else(|| record.origin.key())
            };
            super::super::grouping::natural_cmp(&key(a), &key(b))
        });
        records[item.index()].media.extend(media);
    }
    for at in 0..records.len() {
        let kept: Vec<RecordId> = records[at]
            .media
            .iter()
            .copied()
            .filter(|media| !matches!(records[media.index()].fate, Fate::Left(_)))
            .collect();
        records[at].media = kept;
    }
}

fn entry_states(
    grouping: &Grouping<'_>,
    sheets: &[SheetIn<'_>],
    joined: &Joined,
    file_left: &[Option<LeftBy>],
    file_record: &[Option<RecordId>],
    records: &[Record],
) -> Vec<EntryState> {
    let fate_left = |record: Option<RecordId>| match record.map(|id| records[id.index()].fate) {
        Some(Fate::Left(by)) => Some(by),
        _ => None,
    };
    let mut entries: Vec<EntryState> = grouping
        .files
        .iter()
        .enumerate()
        .map(|(at, file)| EntryState {
            entry: EntryRef::File(file.file.clone()),
            record: file_record[at],
            left: file_left[at].or_else(|| fate_left(file_record[at])),
            unmatched: file.series.is_some() && file.groups.is_none(),
            example: file.entry.sniffed != "application/octet-stream",
        })
        .collect();
    let mut by_row: HashMap<&RowRef, RecordId> = HashMap::new();
    for record in records {
        for row in &record.rows {
            by_row.entry(row).or_insert(record.id);
        }
    }
    for (at_sheet, sheet) in sheets.iter().enumerate() {
        let Some(read) = sheet.sheet else {
            continue;
        };
        for at_row in 0..read.rows.len() {
            let at = RowAt {
                sheet: at_sheet,
                row: at_row,
            };
            let row = at.entry(sheets);
            let record = by_row.get(&row).copied();
            entries.push(EntryState {
                entry: EntryRef::Row(row),
                record,
                left: joined
                    .skipped
                    .get(&at)
                    .map(|kind| LeftBy::Flag(*kind))
                    .or_else(|| fate_left(record)),
                unmatched: false,
                example: false,
            });
        }
    }
    entries
}

fn shape_anomalies(records: &[Record], anomalies: &mut Vec<Anomaly>) {
    let folder_of = |file: &FileRef| {
        let path = file.path.as_str();
        let folder = path.rsplit_once('/').map_or("", |(folder, _)| folder);
        format!("{}:{folder}", file.source.0)
    };
    let items: Vec<&Record> = records
        .iter()
        .filter(|record| {
            record.kind == RecordKind::Item
                && !matches!(record.fate, Fate::Left(_))
                && !record.files.is_empty()
        })
        .collect();
    for record in &items {
        let folders: BTreeSet<String> = record.files.iter().map(folder_of).collect();
        if folders.len() > 1 {
            anomalies.push(Anomaly::SpanFolders {
                record: record.id,
                folders: folders.into_iter().collect(),
            });
        }
    }
    let mut counts: Vec<usize> = items.iter().map(|record| record.files.len()).collect();
    counts.sort_unstable();
    let median = counts.get(counts.len() / 2).copied().unwrap_or(0);
    for record in &items {
        let files = record.files.len();
        if files > 5 && files > 4 * median {
            anomalies.push(Anomaly::LargeGroup {
                record: record.id,
                files,
                median,
            });
        }
    }
}

/// One field per term a record using the type writes; the required field
/// first.
fn new_type_fields(plan: &ImportPlan, records: &[Record]) -> Vec<NewTypeFields> {
    (0..plan.new_types.len())
        .filter(|new_type| named::from_mapping(plan, *new_type))
        .map(|new_type| {
            let using: Vec<&Record> = records
                .iter()
                .filter(|record| {
                    !matches!(record.fate, Fate::Left(_))
                        && record.resolved.template == Some(TemplateRef::New(new_type))
                })
                .collect();
            let class = using
                .iter()
                .find_map(|record| record.resolved.class.as_ref());
            let required = named::required_term(plan, new_type, class);
            let written: BTreeSet<&Term> = using
                .iter()
                .flat_map(|record| record.slots.keys())
                .filter_map(|slot| match slot {
                    SlotName::Term(term) => Some(term),
                    _ => None,
                })
                .collect();
            let mut fields = vec![required.clone()];
            fields.extend(
                written
                    .into_iter()
                    .filter(|term| **term != required)
                    .cloned(),
            );
            NewTypeFields {
                new_type,
                fields,
                required,
            }
        })
        .collect()
}
