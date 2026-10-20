//! Step 5 of the derive: each record's slots (`docs/import-v2.md`, "The
//! derive" and "Slot rules a worker must not guess").
//!
//! A slot's values come from its rule (source refs, `combine`, the value
//! map) or, when a `set` decision targets the record and slot, from the
//! latest such decision. Either way they then pass the same checks: empty
//! values dropped, `space-at-ends`, `encoding-suspect`, distinct by
//! `(type, text, lang)`, and `many-values` on a single-valued slot.
//! [`named`] reads the named slots as what they name; [`dates`] and the
//! required-field check run once the template is known.

pub mod dates;
pub(crate) mod named;
mod sources;

use std::collections::{BTreeMap, BTreeSet, HashMap};

use super::catalog::Catalog;
use super::decisions::{Decision, EntryRef};
use super::derive::FileTags;
use super::derive::records::Draft;
use super::flags::{FlagBook, looks_misdecoded};
use super::grouping::Grouping;
use super::join::{SheetIn, cell};
use super::model::{LeftBy, RecordId, SlotValue, Subject, TemplateRef};
use super::plan::{
    Combine, FlagKind, FlagPolicy, ImportPlanV2, RecordKind, SlotName, SlotRule, Term,
};
use super::values::{Value, ValueType, fold_title};

/// What every record's resolution reads.
pub(crate) struct Ctx<'a, 'i> {
    pub plan: &'a ImportPlanV2,
    pub catalog: &'a Catalog,
    pub grouping: &'a Grouping<'i>,
    pub sheets: &'a [SheetIn<'i>],
    pub tags: &'a dyn FileTags,
    /// Each rule's value map with its keys folded as titles; the first key
    /// wins when two fold alike.
    maps: HashMap<(RecordKind, SlotName), HashMap<String, String>>,
}

impl<'a, 'i> Ctx<'a, 'i> {
    pub(crate) fn new(
        plan: &'a ImportPlanV2,
        catalog: &'a Catalog,
        grouping: &'a Grouping<'i>,
        sheets: &'a [SheetIn<'i>],
        tags: &'a dyn FileTags,
    ) -> Self {
        let mut maps = HashMap::new();
        for kind in [RecordKind::Item, RecordKind::Media, RecordKind::ItemSet] {
            for (slot, rule) in rules(plan, kind) {
                if rule.map.is_empty() {
                    continue;
                }
                let mut folded = HashMap::new();
                for (key, to) in &rule.map {
                    folded.entry(fold_title(key)).or_insert_with(|| to.clone());
                }
                maps.insert((kind, slot.clone()), folded);
            }
        }
        Self {
            plan,
            catalog,
            grouping,
            sheets,
            tags,
            maps,
        }
    }
}

pub(crate) fn rules(
    plan: &ImportPlanV2,
    kind: RecordKind,
) -> &indexmap::IndexMap<SlotName, SlotRule> {
    match kind {
        RecordKind::Item => &plan.slots.item,
        RecordKind::Media => &plan.slots.media,
        RecordKind::ItemSet => &plan.slots.item_set,
    }
}

/// The `set` decisions that target each record, by slot, in plan order.
pub(crate) type SetDecisions = HashMap<RecordId, BTreeMap<SlotName, Vec<usize>>>;

/// A record's slots from its rules, its defaults and its `set` decisions.
pub(crate) fn resolve(
    ctx: &Ctx<'_, '_>,
    draft: &Draft,
    id: RecordId,
    entries: &[EntryRef],
    decided: &SetDecisions,
    book: &mut FlagBook<'_>,
) -> BTreeMap<SlotName, Vec<SlotValue>> {
    let rules = rules(ctx.plan, draft.kind);
    let identifier = SlotName::Term(Term::dcterms_identifier());
    let decided = decided.get(&id);
    let mut names: Vec<SlotName> = rules.keys().cloned().collect();
    names.extend(decided.into_iter().flat_map(BTreeMap::keys).cloned());
    if draft.kind == RecordKind::Item {
        names.push(identifier.clone());
    }
    names.push(SlotName::ItemKey);
    let names: BTreeSet<SlotName> = names.into_iter().collect();

    let mut slots = BTreeMap::new();
    // The identifier comes first: the item key defaults to it.
    let ordered = names
        .iter()
        .filter(|slot| **slot == identifier)
        .chain(names.iter().filter(|slot| **slot != identifier));
    for slot in ordered {
        let raw = if let Some(decisions) = decided.and_then(|decided| decided.get(slot)) {
            decisions
                .last()
                .map(|at| decision_values(&ctx.plan.decisions[*at]))
                .unwrap_or_default()
        } else if let Some(rule) = rules.get(slot) {
            from_rule(ctx, draft, slot, rule)
        } else if *slot == identifier {
            draft.identifier.iter().map(|text| literal(text)).collect()
        } else {
            default_item_key(ctx, draft, &slots, &identifier)
        };
        let values = check(raw, slot, id, entries, book);
        if !values.is_empty() {
            slots.insert(slot.clone(), values);
        }
    }
    slots
}

fn literal(text: &str) -> Value {
    Value {
        value_type: ValueType::Literal,
        text: text.to_string(),
        lang: None,
        label: None,
        is_public: true,
    }
}

/// An item's or item set's first identifier; a media row's item key cell.
fn default_item_key(
    ctx: &Ctx<'_, '_>,
    draft: &Draft,
    slots: &BTreeMap<SlotName, Vec<SlotValue>>,
    identifier: &SlotName,
) -> Vec<Value> {
    if draft.kind == RecordKind::Media {
        return draft
            .rows
            .iter()
            .filter_map(|row| {
                let column = ctx.sheets[row.sheet].plan.item_key.as_ref()?;
                Some(literal(cell(row.row(ctx.sheets), column.index)))
            })
            .take(1)
            .collect();
    }
    slots
        .get(identifier)
        .and_then(|values| values.first())
        .map(|first| literal(&first.value.text))
        .into_iter()
        .collect()
}

fn decision_values(decision: &Decision) -> Vec<Value> {
    let Decision::Set { values, .. } = decision else {
        return Vec::new();
    };
    values
        .iter()
        .map(|value| Value {
            value_type: value.value_type.clone(),
            text: value.text.clone(),
            lang: value.lang.clone(),
            label: value.label.clone(),
            is_public: true,
        })
        .collect()
}

fn from_rule(ctx: &Ctx<'_, '_>, draft: &Draft, slot: &SlotName, rule: &SlotRule) -> Vec<Value> {
    let mut values = Vec::new();
    for source in &rule.sources {
        let found = sources::values(ctx, draft, source);
        if found.is_empty() {
            continue;
        }
        values.extend(found);
        if rule.combine == Combine::First {
            break;
        }
    }
    let Some(map) = ctx.maps.get(&(draft.kind, slot.clone())) else {
        return values;
    };
    // A value mapped to "" is then empty, and the checks drop it.
    for value in &mut values {
        if let Some(to) = map.get(&fold_title(&value.text)) {
            value.text.clone_from(to);
        }
    }
    values
}

/// The checks every slot's values pass, whatever gave them.
fn check(
    raw: Vec<Value>,
    slot: &SlotName,
    id: RecordId,
    entries: &[EntryRef],
    book: &mut FlagBook<'_>,
) -> Vec<SlotValue> {
    let mut values: Vec<Value> = Vec::with_capacity(raw.len());
    for mut value in raw {
        if value.text.trim().is_empty() {
            continue;
        }
        let raise = |book: &mut FlagBook<'_>, kind, text: &str| {
            book.raise(
                kind,
                Subject::Record(id),
                Some(slot.clone()),
                Some(text.to_string()),
                entries,
            )
        };
        if value.text.trim() != value.text {
            match raise(book, FlagKind::SpaceAtEnds, &value.text) {
                Some(FlagPolicy::Leave) => continue,
                Some(FlagPolicy::Repair) => value.text = value.text.trim().to_string(),
                _ => {}
            }
        }
        if looks_misdecoded(&value.text)
            && raise(book, FlagKind::EncodingSuspect, &value.text) == Some(FlagPolicy::Leave)
        {
            continue;
        }
        let duplicate = values.iter().any(|seen| {
            seen.value_type == value.value_type
                && seen.text == value.text
                && seen.lang == value.lang
        });
        if !duplicate {
            values.push(value);
        }
    }
    if !slot.is_multi() && values.len() > 1 {
        let rest: Vec<&str> = values[1..]
            .iter()
            .map(|value| value.text.as_str())
            .collect();
        let policy = book.raise(
            FlagKind::ManyValues,
            Subject::Record(id),
            Some(slot.clone()),
            Some(rest.join("; ")),
            entries,
        );
        values.truncate(usize::from(policy != Some(FlagPolicy::Leave)));
    }
    values
        .into_iter()
        .map(|value| SlotValue { value, link: None })
        .collect()
}

/// `date-form` on date values and `empty-required` on required fields, once
/// the record's template is known. Returns why the record is left out.
pub(crate) fn check_fields(
    ctx: &Ctx<'_, '_>,
    id: RecordId,
    entries: &[EntryRef],
    slots: &mut BTreeMap<SlotName, Vec<SlotValue>>,
    template: Option<TemplateRef>,
    class: Option<&Term>,
    book: &mut FlagBook<'_>,
) -> Option<LeftBy> {
    let fields = match template {
        Some(TemplateRef::Existing(template)) => ctx
            .catalog
            .template(template)
            .map(|template| template.fields.clone())
            .unwrap_or_default(),
        _ => Vec::new(),
    };
    for (slot, values) in slots.iter_mut() {
        let SlotName::Term(term) = slot else {
            continue;
        };
        let field_is_date = fields.iter().any(|field| {
            &field.term == term && field.data_types.iter().any(|t| dates::is_date_type(t))
        });
        let term_is_date = dates::DATE_TERMS.contains(&term.as_str()) || field_is_date;
        values.retain(|value| {
            let is_date = term_is_date
                || matches!(&value.value.value_type, ValueType::DataType(name) if dates::is_date_type(name));
            let checked = matches!(value.value.value_type, ValueType::Literal | ValueType::DataType(_));
            if !is_date || !checked || dates::is_edtf(&value.value.text) {
                return true;
            }
            book.raise(
                FlagKind::DateForm,
                Subject::Record(id),
                Some(slot.clone()),
                Some(value.value.text.clone()),
                entries,
            ) != Some(FlagPolicy::Leave)
        });
    }
    slots.retain(|_, values| !values.is_empty());

    let mut required: Vec<Term> = fields
        .iter()
        .filter(|field| field.required)
        .map(|field| field.term.clone())
        .collect();
    if let Some(TemplateRef::New(new_type)) = template
        && named::from_mapping(ctx.plan, new_type)
    {
        required.push(named::required_term(ctx.plan, new_type, class));
    }
    let mut left = None;
    for term in required {
        let slot = SlotName::Term(term);
        if slots.contains_key(&slot) {
            continue;
        }
        let policy = book.raise(
            FlagKind::EmptyRequired,
            Subject::Record(id),
            Some(slot),
            None,
            entries,
        );
        if policy == Some(FlagPolicy::Leave) && left.is_none() {
            left = Some(LeftBy::Flag(FlagKind::EmptyRequired));
        }
    }
    left
}
