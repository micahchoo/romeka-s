//! The named slots read as what they name: sets, template, class, owner,
//! visibility, sites, and the single texts (`docs/import-v2.md`, "Slot rules
//! a worker must not guess").

use std::collections::{BTreeMap, HashMap, HashSet};

use unicode_normalization::UnicodeNormalization;

use super::super::decisions::EntryRef;
use super::super::flags::FlagBook;
use super::super::model::{
    LeftBy, NewSet, OwnerRef, RecordId, Resolved, SetRef, SlotValue, Subject, TemplateRef,
};
use super::super::plan::{
    FlagKind, FlagPolicy, ImportPlan, NewTypeFrom, Owner, SetNames, SlotName, Term,
};
use super::super::values::{ResourceBy, ValueType, Visibility, fold_title};
use super::Ctx;

/// One `sets` value of a record, as its checks left it, and how it names
/// its set.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SetName {
    pub text: String,
    pub reading: Reading,
}

/// How a `sets` value names its set: the set source's shape, never the
/// text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Reading {
    /// The set with this title, made with the description when missing.
    Title { description: Option<String> },
    /// The install's set with this id.
    Id,
    /// The item key of one of this import's item-set records.
    Key,
    /// v2's reading: an id that exists, else a record key, else a title.
    Text,
}

impl Reading {
    pub(crate) fn of_names(names: SetNames) -> Self {
        match names {
            SetNames::Title => Self::Title { description: None },
            SetNames::Id => Self::Id,
            SetNames::Key => Self::Key,
            SetNames::Text => Self::Text,
        }
    }

    /// A `set` decision's value names a set by its type: an internal id is
    /// an id, a literal a title. Any other type keeps v2's reading.
    pub(crate) fn of_decision(value_type: &ValueType) -> Self {
        match value_type {
            ValueType::Literal => Self::Title { description: None },
            ValueType::Resource(ResourceBy::InternalId) => Self::Id,
            _ => Self::Text,
        }
    }
}

/// What a `sets` value can name: the install's sets, this import's
/// item-set records, and the new sets values have made so far.
pub(crate) struct SetBook {
    ids: HashSet<i64>,
    by_key: HashMap<String, RecordId>,
    by_title: HashMap<String, Vec<SetRef>>,
    new_by_title: HashMap<String, usize>,
    pub new_sets: Vec<NewSet>,
}

/// Keys compare NFC and trimmed.
pub(crate) fn key_form(text: &str) -> String {
    text.trim().nfc().collect()
}

impl SetBook {
    /// `records` are this import's item-set records with their item key and
    /// title.
    pub(crate) fn new(
        ctx: &Ctx<'_, '_>,
        records: &[(RecordId, Option<String>, Option<String>)],
    ) -> Self {
        let mut by_title: HashMap<String, Vec<SetRef>> = HashMap::new();
        let mut sets: Vec<_> = ctx.catalog.item_sets.iter().collect();
        sets.sort_by_key(|set| set.id);
        for set in &sets {
            by_title
                .entry(fold_title(&set.title))
                .or_default()
                .push(SetRef::Existing(set.id));
        }
        let mut by_key = HashMap::new();
        for (id, key, title) in records {
            if let Some(key) = key {
                by_key.entry(key_form(key)).or_insert(*id);
            }
            if let Some(title) = title {
                by_title
                    .entry(fold_title(title))
                    .or_default()
                    .push(SetRef::Record(*id));
            }
        }
        Self {
            ids: sets.iter().map(|set| set.id).collect(),
            by_key,
            by_title,
            new_by_title: HashMap::new(),
            new_sets: Vec::new(),
        }
    }

    /// The set one `sets` value names, made when it is a title no set has;
    /// `None` when it names no set. `leave` takes a flag whose policy
    /// leaves the record out.
    fn find(
        &mut self,
        named: &SetName,
        visibility: Visibility,
        id: RecordId,
        entries: &[EntryRef],
        book: &mut FlagBook<'_>,
        leave: &mut impl FnMut(FlagKind),
    ) -> Option<SetRef> {
        let text = named.text.as_str();
        let mut raise = |book: &mut FlagBook<'_>, kind| {
            let policy = book.raise(
                kind,
                Subject::Record(id),
                Some(SlotName::Sets),
                Some(text.to_string()),
                entries,
            );
            if policy == Some(FlagPolicy::Leave) {
                leave(kind);
            }
        };
        let by_id = || {
            text.trim()
                .parse::<i64>()
                .ok()
                .filter(|set| self.ids.contains(set))
                .map(SetRef::Existing)
        };
        let by_key = || {
            self.by_key
                .get(&key_form(text))
                .copied()
                .map(SetRef::Record)
        };
        match &named.reading {
            Reading::Id => {
                let found = by_id();
                if found.is_none() {
                    raise(book, FlagKind::UnknownSet);
                }
                found
            }
            Reading::Key => {
                let found = by_key();
                if found.is_none() {
                    raise(book, FlagKind::UnknownSet);
                }
                found
            }
            Reading::Title { description } => Some(
                self.by_title(text, &mut |kind| raise(book, kind))
                    .unwrap_or_else(|| self.made(text, description.as_ref(), visibility)),
            ),
            Reading::Text => {
                let found = by_id().or_else(by_key);
                let found = found.or_else(|| self.by_title(text, &mut |kind| raise(book, kind)));
                Some(found.unwrap_or_else(|| self.made(text, None, visibility)))
            }
        }
    }

    /// The set whose title compares equal; the first, flagged
    /// `many-matches`, when several have it.
    fn by_title(&self, text: &str, raise: &mut impl FnMut(FlagKind)) -> Option<SetRef> {
        match self.by_title.get(&fold_title(text)).map(Vec::as_slice) {
            Some([one]) => Some(*one),
            Some([first, ..]) => {
                raise(FlagKind::ManyMatches);
                Some(*first)
            }
            _ => None,
        }
    }

    /// The new set with this title, made once per title. A description
    /// given by a later value fills one the first value left empty.
    fn made(&mut self, text: &str, description: Option<&String>, visibility: Visibility) -> SetRef {
        let title = fold_title(text);
        let index = *self.new_by_title.entry(title).or_insert_with(|| {
            self.new_sets.push(NewSet {
                title: text.trim().to_string(),
                description: None,
                visibility,
                is_open: true,
            });
            self.new_sets.len() - 1
        });
        let new = &mut self.new_sets[index];
        if new.description.is_none() {
            new.description = description.cloned();
        }
        SetRef::New(index)
    }
}

fn texts(slots: &BTreeMap<SlotName, Vec<SlotValue>>, slot: &SlotName) -> Vec<String> {
    slots
        .get(slot)
        .map(|values| {
            values
                .iter()
                .map(|value| value.value.text.clone())
                .collect()
        })
        .unwrap_or_default()
}

fn first(slots: &BTreeMap<SlotName, Vec<SlotValue>>, slot: &SlotName) -> Option<String> {
    texts(slots, slot).into_iter().next()
}

/// The named slots of one record, and why the record is left out.
pub(crate) fn resolve(
    ctx: &Ctx<'_, '_>,
    id: RecordId,
    slots: &BTreeMap<SlotName, Vec<SlotValue>>,
    set_names: &[SetName],
    entries: &[EntryRef],
    sets: &mut SetBook,
    book: &mut FlagBook<'_>,
) -> (Resolved, Option<LeftBy>) {
    let plan = ctx.plan;
    let mut left = None;
    let mut leave = |kind| {
        if left.is_none() {
            left = Some(LeftBy::Flag(kind));
        }
    };
    let visibility = first(slots, &SlotName::Visibility)
        .and_then(|text| Visibility::from_cell(&text))
        .unwrap_or(plan.defaults.visibility);

    let mut set_refs: Vec<SetRef> = Vec::new();
    for named in set_names {
        let Some(set) = sets.find(named, visibility, id, entries, book, &mut leave) else {
            continue;
        };
        if !set_refs.contains(&set) {
            set_refs.push(set);
        }
    }

    let class = first(slots, &SlotName::Class).and_then(|text| {
        let text = text.trim();
        Term::parse(text)
            .ok()
            .filter(|term| ctx.catalog.classes.contains_key(term))
            .or_else(|| {
                let wanted = fold_title(text);
                ctx.catalog
                    .classes
                    .iter()
                    .find(|(_, label)| fold_title(label) == wanted)
                    .map(|(term, _)| term.clone())
            })
    });

    let fallback = || {
        plan.defaults
            .template
            .filter(|template| ctx.catalog.template(*template).is_some())
            .or(ctx.catalog.base_template)
            .map(TemplateRef::Existing)
    };
    let template = match first(slots, &SlotName::Template) {
        None => fallback(),
        Some(text) => match template_named(ctx, plan, &text) {
            Some(found) => Some(found),
            None => {
                let policy = book.raise(
                    FlagKind::UnknownType,
                    Subject::Record(id),
                    Some(SlotName::Template),
                    Some(text),
                    entries,
                );
                if policy == Some(FlagPolicy::Leave) {
                    leave(FlagKind::UnknownType);
                }
                fallback()
            }
        },
    };

    let owner = match first(slots, &SlotName::Owner) {
        None => OwnerRef::Is(plan.defaults.owner),
        Some(text) => owner_named(ctx, &text),
    };
    let sites = match slots.get(&SlotName::Sites) {
        None => plan.defaults.sites.clone(),
        Some(values) => {
            let mut sites: Vec<i64> = values
                .iter()
                .filter_map(|value| value.value.text.trim().parse().ok())
                .collect();
            sites.dedup();
            sites
        }
    };
    let resolved = Resolved {
        sets: set_refs,
        template,
        class,
        owner,
        visibility,
        sites,
        media_order: first(slots, &SlotName::MediaOrder),
        ingester: first(slots, &SlotName::Ingester),
        media_source: first(slots, &SlotName::MediaSource),
        item_key: first(slots, &SlotName::ItemKey).map(|text| key_form(&text)),
    };
    (resolved, left)
}

/// A template by id or label, then a new type by label, titles compared
/// folded.
fn template_named(ctx: &Ctx<'_, '_>, plan: &ImportPlan, text: &str) -> Option<TemplateRef> {
    if let Ok(id) = text.trim().parse::<i64>()
        && ctx.catalog.template(id).is_some()
    {
        return Some(TemplateRef::Existing(id));
    }
    let wanted = fold_title(text);
    let mut templates: Vec<_> = ctx.catalog.templates.iter().collect();
    templates.sort_by_key(|template| template.id);
    templates
        .iter()
        .find(|template| fold_title(&template.label) == wanted)
        .map(|template| TemplateRef::Existing(template.id))
        .or_else(|| {
            plan.new_types
                .iter()
                .position(|new_type| fold_title(&new_type.label) == wanted)
                .map(TemplateRef::New)
        })
}

fn owner_named(ctx: &Ctx<'_, '_>, text: &str) -> OwnerRef {
    let text = text.trim();
    if text == "runner" {
        return OwnerRef::Is(Owner::Runner);
    }
    let by_id = text
        .parse::<i64>()
        .ok()
        .filter(|id| ctx.catalog.users.iter().any(|user| user.id == *id));
    let by_email = || {
        ctx.catalog
            .users
            .iter()
            .find(|user| user.email.eq_ignore_ascii_case(text))
            .map(|user| user.id)
    };
    match by_id.or_else(by_email) {
        Some(id) => OwnerRef::Is(Owner::User(id)),
        None => OwnerRef::Unknown(text.to_string()),
    }
}

/// The field a new type from the mapping requires: `dcterms:title`, or
/// `foaf:name` when the type's class, or the record's, is `foaf:Person`.
pub(crate) fn required_term(plan: &ImportPlan, new_type: usize, class: Option<&Term>) -> Term {
    let person = |term: &Term| term.as_str() == "foaf:Person";
    let class_is_person = plan
        .new_types
        .get(new_type)
        .and_then(|new_type| new_type.class.as_ref())
        .or(class)
        .is_some_and(person);
    let term = if class_is_person {
        "foaf:name"
    } else {
        "dcterms:title"
    };
    Term::parse(term).unwrap_or_else(|_| Term::dcterms_identifier())
}

/// Whether a new type takes its fields from the mapping.
pub(crate) fn from_mapping(plan: &ImportPlan, new_type: usize) -> bool {
    plan.new_types
        .get(new_type)
        .is_some_and(|new_type| matches!(new_type.from, NewTypeFrom::Mapping(_)))
}

#[cfg(test)]
#[path = "named_tests.rs"]
mod tests;
