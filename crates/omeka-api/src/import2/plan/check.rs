//! What the boundary refuses across members: the list under "The boundary
//! refuses" in `docs/import-v2.md`, less the path rules, which [`RelPath`]
//! enforces member by member.
//!
//! [`RelPath`]: super::RelPath

use std::collections::{BTreeMap, BTreeSet};

use super::{Action, Combine, ImportPlanV2, PlanError, RecordKind, SourceRef, Term};
use crate::import2::values::fold_title;

pub(super) fn plan(plan: &ImportPlanV2) -> Result<(), PlanError> {
    ids(plan)?;
    slots(plan)?;
    new_types(plan)?;
    new_terms(plan)
}

fn ids(plan: &ImportPlanV2) -> Result<(), PlanError> {
    let mut sources = BTreeSet::new();
    for source in &plan.sources {
        if !sources.insert(source.id()) {
            return Err(PlanError::DuplicateSource(source.id().0.clone()));
        }
    }
    let mut sheets = BTreeSet::new();
    for sheet in &plan.sheets {
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

fn slots(plan: &ImportPlanV2) -> Result<(), PlanError> {
    if plan.action.action == Action::Delete && plan.slots.rules().next().is_some() {
        return Err(PlanError::SlotRulesOnDelete);
    }
    let kinds = [
        (RecordKind::Item, &plan.slots.item),
        (RecordKind::Media, &plan.slots.media),
        (RecordKind::ItemSet, &plan.slots.item_set),
    ];
    for (kind, rules) in kinds {
        for (slot, rule) in rules {
            if rule.combine == Combine::All && !slot.is_multi() {
                return Err(PlanError::CombineAll {
                    kind,
                    slot: slot.to_string(),
                });
            }
        }
    }
    for (_, rule) in plan.slots.rules() {
        for source in &rule.sources {
            let SourceRef::Column { sheet, column } = source else {
                continue;
            };
            let Some(read) = plan.sheets.iter().find(|s| &s.id == sheet) else {
                return Err(PlanError::MissingSheet(sheet.0.clone()));
            };
            if read.column(column.index).is_none() {
                return Err(PlanError::MissingColumn {
                    sheet: sheet.0.clone(),
                    index: column.index,
                });
            }
        }
    }
    Ok(())
}

fn new_types(plan: &ImportPlanV2) -> Result<(), PlanError> {
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

fn new_terms(plan: &ImportPlanV2) -> Result<(), PlanError> {
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
    plan: &ImportPlanV2,
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
