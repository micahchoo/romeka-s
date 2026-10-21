//! What the boundary refuses across members: the list under "The boundary
//! refuses" in `docs/import-v2.md`, less the path rules, which [`RelPath`]
//! enforces member by member.
//!
//! [`RelPath`]: super::RelPath

use std::collections::{BTreeMap, BTreeSet};

use super::structure::{SeriesScope, Structure};
use super::{
    Action, Combine, ImportPlan, NotYet, PlanError, RecordKind, RelPath, SeriesIx, SetSource,
    SheetId, SlotName, SourceId, SourceRef, Term,
};
use crate::import::decisions::{Change, Decision, EntryRef};
use crate::import::values::fold_title;

pub(super) fn plan(plan: &ImportPlan) -> Result<(), PlanError> {
    ids(plan)?;
    slots(plan)?;
    new_types(plan)?;
    new_terms(plan)?;
    decisions(plan)?;
    tree(plan)?;
    not_yet(plan)
}

/// Only what no rescan can mend: a missing entry is an orphan, not this.
fn decisions(plan: &ImportPlan) -> Result<(), PlanError> {
    for (at, decision) in plan.decisions.iter().enumerate() {
        let Decision::Many { entries, change } = decision else {
            continue;
        };
        if entries.is_empty() {
            return Err(PlanError::EmptyMany(at));
        }
        if matches!(change, Change::Move { .. })
            && entries
                .iter()
                .any(|entry| matches!(entry, EntryRef::Row(_)))
        {
            return Err(PlanError::ManyMovesRow(at));
        }
    }
    Ok(())
}

fn ids(plan: &ImportPlan) -> Result<(), PlanError> {
    let mut sources = BTreeSet::new();
    for source in &plan.sources {
        if !sources.insert(source.id()) {
            return Err(PlanError::DuplicateSource(source.id().0.clone()));
        }
    }
    let mut sheets = BTreeSet::new();
    for (_, sheet) in plan.structure.lists() {
        if !sheets.insert(&sheet.id) {
            return Err(PlanError::DuplicateSheet(sheet.id.0.clone()));
        }
        if !sources.contains(&sheet.source) {
            return Err(PlanError::MissingSource {
                sheet: sheet.id.0.clone(),
                missing: sheet.source.0.clone(),
            });
        }
        let mut columns = BTreeSet::new();
        for column in &sheet.columns {
            if !columns.insert(column.column.index) {
                return Err(PlanError::DuplicateColumn {
                    sheet: sheet.id.0.clone(),
                    index: column.column.index,
                });
            }
        }
    }
    Ok(())
}

fn slots(plan: &ImportPlan) -> Result<(), PlanError> {
    let every_rule = || {
        plan.structure
            .all_series()
            .flat_map(|(_, series)| series.every_rule())
    };
    if plan.action.action == Action::Delete && every_rule().next().is_some() {
        return Err(PlanError::SlotRulesOnDelete);
    }
    let kinds = [RecordKind::Item, RecordKind::Media, RecordKind::ItemSet];
    let by_kind = plan
        .structure
        .all_series()
        .flat_map(|(_, series)| kinds.map(|kind| (kind, series.rules(kind))));
    for (kind, rules) in by_kind {
        for (slot, rule) in rules {
            if rule.combine == Combine::All && !slot.is_multi() {
                return Err(PlanError::CombineAll {
                    kind,
                    slot: slot.to_string(),
                });
            }
        }
    }
    for (at, series) in plan.structure.all_series() {
        for (slot, rule) in series.every_rule() {
            map_keys(&slot.to_string(), &rule.map)?;
            for (owner, sheet) in column_refs(plan, &rule.sources)? {
                if owner != at {
                    return Err(PlanError::ForeignList {
                        series: series.id.0.clone(),
                        sheet: sheet.0.clone(),
                    });
                }
            }
        }
    }
    Ok(())
}

fn map_keys(slot: &str, map: &indexmap::IndexMap<String, String>) -> Result<(), PlanError> {
    let mut keys: BTreeMap<String, &str> = BTreeMap::new();
    for key in map.keys() {
        if let Some(first) = keys.insert(fold_title(key), key) {
            return Err(PlanError::MapKeysCollide {
                slot: slot.to_string(),
                first: first.to_string(),
                second: key.clone(),
            });
        }
    }
    Ok(())
}

/// Each list the column refs read, with the series that holds it; refused
/// when a list or its column is missing.
fn column_refs<'p>(
    plan: &ImportPlan,
    sources: &'p [SourceRef],
) -> Result<Vec<(SeriesIx, &'p SheetId)>, PlanError> {
    let mut owners = Vec::new();
    for source in sources {
        let SourceRef::Column { sheet, column } = source else {
            continue;
        };
        let Some((owner, read)) = plan.structure.list(sheet) else {
            return Err(PlanError::MissingSheet(sheet.0.clone()));
        };
        if read.column(column.index).is_none() {
            return Err(PlanError::MissingColumn {
                sheet: sheet.0.clone(),
                index: column.index,
            });
        }
        owners.push((owner, sheet));
    }
    Ok(owners)
}

/// A series' slots hold no `item.sets` rule on the wire. Checked before
/// [`Structure::fold`] puts the one `text` free set there.
pub(super) fn no_series_sets(structure: &Structure) -> Result<(), PlanError> {
    for (_, series) in structure.all_series() {
        if series.slots.item.contains_key(&SlotName::Sets) {
            return Err(PlanError::SeriesSets(series.id.0.clone()));
        }
    }
    Ok(())
}

/// The rules the tree adds: a series at all, level ids, scopes, and the
/// set sources.
fn tree(plan: &ImportPlan) -> Result<(), PlanError> {
    let structure = &plan.structure;
    if structure.all_series().next().is_none() {
        return Err(PlanError::NoSeries);
    }
    let mut ids = BTreeSet::new();
    let level_ids = structure
        .set_levels()
        .into_iter()
        .map(|set| &set.id)
        .chain(structure.all_series().map(|(_, series)| &series.id));
    for id in level_ids {
        if !ids.insert(id) {
            return Err(PlanError::DuplicateLevel(id.0.clone()));
        }
    }

    let sources: BTreeSet<_> = plan.sources.iter().map(|source| source.id()).collect();
    let mut scopes = BTreeMap::new();
    for (_, series) in structure.all_series() {
        let taken: Vec<Option<(&SourceId, &RelPath)>> = match &series.scope {
            SeriesScope::All(_) => vec![None],
            SeriesScope::Folders(folders) => folders
                .iter()
                .map(|scope| Some((&scope.source, &scope.folder)))
                .collect(),
        };
        for scope in taken {
            if let Some((source, _)) = scope
                && !sources.contains(source)
            {
                return Err(PlanError::ScopeSource {
                    series: series.id.0.clone(),
                    missing: source.0.clone(),
                });
            }
            if let Some(first) = scopes.insert(scope, &series.id) {
                return Err(PlanError::DuplicateScope {
                    first: first.0.clone(),
                    second: series.id.0.clone(),
                });
            }
        }
    }

    let level_sets = structure
        .set_levels()
        .into_iter()
        .map(|set| (set.id.0.clone(), &set.set));
    let series_sets = structure
        .all_series()
        .filter_map(|(_, series)| Some((series.id.0.clone(), series.set.as_ref()?)));
    let free_sets = structure
        .free_sets
        .iter()
        .enumerate()
        .map(|(at, set)| (format!("free set {}", at + 1), set));
    for (place, set) in level_sets.chain(series_sets).chain(free_sets) {
        match set {
            SetSource::Title(title) if title.title.trim().is_empty() => {
                return Err(PlanError::EmptySetTitle(place));
            }
            SetSource::Each(each) => {
                map_keys("sets", &each.each.map)?;
                column_refs(plan, &each.each.sources)?;
            }
            SetSource::Id(_) | SetSource::Title(_) => {}
        }
    }
    Ok(())
}

/// The temporary rule: a plan the engine cannot run yet is refused in the
/// person's terms. U5 deletes it with the set cases.
fn not_yet(plan: &ImportPlan) -> Result<(), PlanError> {
    let structure = &plan.structure;
    if let Some(set) = structure.set_levels().first() {
        return Err(PlanError::NotYet(NotYet::SetLevel(set.id.0.clone())));
    }
    for (_, series) in structure.all_series() {
        if series.set.is_some() {
            return Err(PlanError::NotYet(NotYet::SeriesSet(series.id.0.clone())));
        }
    }
    if !structure.free_sets.is_empty() {
        return Err(PlanError::NotYet(NotYet::FreeSets));
    }
    Ok(())
}

fn new_types(plan: &ImportPlan) -> Result<(), PlanError> {
    let mut titles = BTreeSet::new();
    for new_type in &plan.new_types {
        let title = fold_title(&new_type.label);
        if title.is_empty() {
            return Err(PlanError::EmptyTypeLabel(new_type.label.clone()));
        }
        if !titles.insert(title) {
            return Err(PlanError::DuplicateNewType(new_type.label.clone()));
        }
    }
    Ok(())
}

fn new_terms(plan: &ImportPlan) -> Result<(), PlanError> {
    let mut made: BTreeMap<Term, &str> = BTreeMap::new();
    for new_term in &plan.new_terms {
        let Some(term) = Term::local(&new_term.label) else {
            return Err(PlanError::EmptyTermLabel(new_term.label.clone()));
        };
        if let Some(first) = made.get(&term) {
            return Err(PlanError::TermsCollide {
                first: (*first).to_string(),
                second: new_term.label.clone(),
                term: term.to_string(),
            });
        }
        made.insert(term, &new_term.label);
    }
    Ok(())
}

pub(super) fn existing_labels<'a>(
    plan: &ImportPlan,
    templates: impl IntoIterator<Item = &'a str>,
) -> Result<(), PlanError> {
    let existing: BTreeSet<String> = templates.into_iter().map(fold_title).collect();
    match plan
        .new_types
        .iter()
        .find(|new_type| existing.contains(&fold_title(&new_type.label)))
    {
        Some(new_type) => Err(PlanError::TypeExists(new_type.label.clone())),
        None => Ok(()),
    }
}
