//! Step 6 of the derive: each record matched to the resource it acts on,
//! media rows attached to their item, and `resource` values linked
//! (`docs/import-v2.md`, "The derive", "Values").
//!
//! Keys compare NFC and trimmed, in Rust, on the catalog's loaded values.
//! `importSets` means, for items and item sets, the existing sets this
//! import's records name; for media, the media of the matched item.

use std::collections::{BTreeSet, HashMap};

use super::catalog::{Catalog, Keys};
use super::decisions::EntryRef;
use super::flags::FlagBook;
use super::model::{Anomaly, Fate, LeftBy, Link, Parent, Record, RecordId, SetRef, Subject};
use super::plan::{
    Action, FlagKind, FlagPolicy, ImportPlan, MatchKey, MatchRule, NamedScope, RecordKind, Scope,
    SlotName, Term,
};
use super::slots::named::key_form;
use super::values::{ResourceBy, ValueType};

/// One kind's existing resources, by match key.
struct Existing {
    by_key: HashMap<String, Vec<i64>>,
    by_identifier: HashMap<String, Vec<i64>>,
    ids: BTreeSet<i64>,
    /// An item's sets, a media's item's sets, a set itself.
    sets: HashMap<i64, Vec<i64>>,
    /// A media's item.
    item: HashMap<i64, i64>,
}

impl Existing {
    fn new<'c>(
        rule: &MatchRule,
        resources: impl Iterator<Item = (i64, &'c Keys, Vec<i64>, Option<i64>)>,
    ) -> Self {
        let identifier = Term::dcterms_identifier();
        let mut existing = Self {
            by_key: HashMap::new(),
            by_identifier: HashMap::new(),
            ids: BTreeSet::new(),
            sets: HashMap::new(),
            item: HashMap::new(),
        };
        for (id, keys, sets, item) in resources {
            existing.ids.insert(id);
            if let MatchKey::Term(term) = &rule.key {
                for key in keys.get(term).into_iter().flatten() {
                    existing.by_key.entry(key_form(key)).or_default().push(id);
                }
            }
            for key in keys.get(&identifier).into_iter().flatten() {
                existing
                    .by_identifier
                    .entry(key_form(key))
                    .or_default()
                    .push(id);
            }
            existing.sets.insert(id, sets);
            if let Some(item) = item {
                existing.item.insert(id, item);
            }
        }
        for ids in existing
            .by_key
            .values_mut()
            .chain(existing.by_identifier.values_mut())
        {
            ids.sort_unstable();
            ids.dedup();
        }
        existing
    }

    fn find(&self, key: &MatchKey, text: &str) -> Vec<i64> {
        match key {
            MatchKey::InternalId => text
                .trim()
                .parse::<i64>()
                .ok()
                .filter(|id| self.ids.contains(id))
                .into_iter()
                .collect(),
            MatchKey::Term(_) => self
                .by_key
                .get(&key_form(text))
                .cloned()
                .unwrap_or_default(),
        }
    }
}

pub(crate) struct Matching<'a> {
    plan: &'a ImportPlan,
    items: Existing,
    media: Existing,
    sets: Existing,
    /// Existing sets this import's records name.
    import_sets: BTreeSet<i64>,
}

impl<'a> Matching<'a> {
    pub(crate) fn new(plan: &'a ImportPlan, catalog: &Catalog, records: &[Record]) -> Self {
        let rules = &plan.action.match_by;
        let item_sets: HashMap<i64, Vec<i64>> = catalog
            .items
            .iter()
            .map(|item| (item.id, item.sets.clone()))
            .collect();
        let items = Existing::new(
            &rules.item,
            catalog
                .items
                .iter()
                .map(|item| (item.id, &item.keys, item.sets.clone(), None)),
        );
        let media = Existing::new(
            &rules.media,
            catalog.media.iter().map(|media| {
                let sets = item_sets.get(&media.item).cloned().unwrap_or_default();
                (media.id, &media.keys, sets, Some(media.item))
            }),
        );
        let sets = Existing::new(
            &rules.item_set,
            catalog
                .item_sets
                .iter()
                .map(|set| (set.id, &set.keys, vec![set.id], None)),
        );
        let import_sets = records
            .iter()
            .filter(|record| !matches!(record.fate, Fate::Left(_)))
            .flat_map(|record| record.resolved.sets.iter())
            .filter_map(|set| match set {
                SetRef::Existing(id) => Some(*id),
                _ => None,
            })
            .collect();
        Self {
            plan,
            items,
            media,
            sets,
            import_sets,
        }
    }

    fn existing(&self, kind: RecordKind) -> &Existing {
        match kind {
            RecordKind::Item => &self.items,
            RecordKind::Media => &self.media,
            RecordKind::ItemSet => &self.sets,
        }
    }

    fn rule(&self, kind: RecordKind) -> &MatchRule {
        let rules = &self.plan.action.match_by;
        match kind {
            RecordKind::Item => &rules.item,
            RecordKind::Media => &rules.media,
            RecordKind::ItemSet => &rules.item_set,
        }
    }

    /// Whether resource `id` of `kind` is in scope; `item` is the matched
    /// item of a media record.
    fn in_scope(&self, kind: RecordKind, id: i64, item: Option<i64>) -> bool {
        let existing = self.existing(kind);
        let sets = existing
            .sets
            .get(&id)
            .map(Vec::as_slice)
            .unwrap_or_default();
        match &self.rule(kind).scope {
            Scope::Named(NamedScope::All) => true,
            Scope::Named(NamedScope::ImportSets) if kind == RecordKind::Media => {
                item.is_some() && existing.item.get(&id).copied() == item
            }
            Scope::Named(NamedScope::ImportSets) => {
                sets.iter().any(|set| self.import_sets.contains(set))
            }
            Scope::Sets(scope) => sets.iter().any(|set| scope.sets.contains(set)),
        }
    }
}

/// The record's key: the first value of the match key's term, or, for
/// `internalId`, its item key.
fn key_of(record: &Record, key: &MatchKey) -> Option<String> {
    match key {
        MatchKey::InternalId => record.resolved.item_key.clone(),
        MatchKey::Term(term) => record
            .slots
            .get(&SlotName::Term(term.clone()))
            .and_then(|values| values.first())
            .map(|value| value.value.text.clone()),
    }
}

/// Attach media rows, then match every record that is not left out.
pub(crate) fn run(
    matching: &Matching<'_>,
    records: &mut [Record],
    entries: &[Vec<EntryRef>],
    chosen: &HashMap<RecordId, Option<i64>>,
    book: &mut FlagBook<'_>,
    anomalies: &mut Vec<Anomaly>,
) {
    let mut items_by_key: HashMap<String, RecordId> = HashMap::new();
    for record in records
        .iter()
        .filter(|record| record.kind == RecordKind::Item)
    {
        if let Some(key) = &record.resolved.item_key {
            items_by_key.entry(key.clone()).or_insert(record.id);
        }
    }
    for at in 0..records.len() {
        let record = &records[at];
        // A left record needs no parent: a refused file, or a decision, took
        // it out.
        if record.kind != RecordKind::Media
            || record.parent.is_some()
            || matches!(record.fate, Fate::Left(_))
        {
            continue;
        }
        let key = record.resolved.item_key.clone().unwrap_or_default();
        let parent = if let Some(item) = items_by_key.get(&key) {
            Parent::Record(*item)
        } else {
            let found: Vec<i64> = matching
                .items
                .find(&matching.rule(RecordKind::Item).key, &key)
                .into_iter()
                .filter(|id| matching.in_scope(RecordKind::Item, *id, None))
                .collect();
            match found.first() {
                Some(id) => Parent::Existing(*id),
                None => {
                    let policy = book.raise(
                        FlagKind::Unmatched,
                        Subject::Record(record.id),
                        Some(SlotName::ItemKey),
                        Some(key.clone()),
                        &entries[at],
                    );
                    if policy == Some(FlagPolicy::Leave) {
                        records[at].fate = Fate::Left(LeftBy::Flag(FlagKind::Unmatched));
                    }
                    Parent::Missing(key)
                }
            }
        };
        records[at].parent = Some(parent);
    }

    let action = matching.plan.action.action;
    for at in 0..records.len() {
        if matches!(records[at].fate, Fate::Left(_)) {
            continue;
        }
        if let Some(Parent::Record(parent)) = &records[at].parent
            && matches!(records[parent.index()].fate, Fate::Left(_))
        {
            records[at].fate = Fate::Left(LeftBy::Parent);
            continue;
        }
        let kind = records[at].kind;
        let item = match &records[at].parent {
            Some(Parent::Existing(id)) => Some(*id),
            Some(Parent::Record(parent)) => match records[parent.index()].fate {
                Fate::Act(id) => Some(id),
                _ => None,
            },
            _ => None,
        };
        let rule = matching.rule(kind);
        let key = key_of(&records[at], &rule.key);
        let everywhere = key
            .as_ref()
            .map(|key| matching.existing(kind).find(&rule.key, key))
            .unwrap_or_default();
        let mut found: Vec<i64> = everywhere
            .iter()
            .copied()
            .filter(|id| matching.in_scope(kind, *id, item))
            .collect();
        if found.is_empty() && !everywhere.is_empty() {
            anomalies.push(Anomaly::OutsideScope {
                record: records[at].id,
                key: key.clone().unwrap_or_default(),
                ids: everywhere,
            });
        }
        let decided = chosen.get(&records[at].id);
        if let Some(choice) = decided {
            found = choice.iter().copied().collect();
        }
        let id = records[at].id;
        let mut raise = |kind, detail: Option<String>| {
            book.raise(kind, Subject::Record(id), None, detail, &entries[at])
        };
        let fate = match (action, found.as_slice()) {
            (Action::Create, []) => Fate::Create,
            (Action::Create, ids) => match raise(FlagKind::Exists, Some(join_ids(ids))) {
                Some(FlagPolicy::Leave) => Fate::Left(LeftBy::Flag(FlagKind::Exists)),
                _ => Fate::Create,
            },
            (_, []) => match raise(FlagKind::Unmatched, key.clone()) {
                Some(FlagPolicy::Leave) | None => Fate::Left(LeftBy::Flag(FlagKind::Unmatched)),
                _ if action == Action::Delete => Fate::Left(LeftBy::Flag(FlagKind::Unmatched)),
                _ => Fate::Create,
            },
            (_, [one]) => Fate::Act(*one),
            (_, ids) => match raise(FlagKind::ManyMatches, Some(join_ids(ids))) {
                Some(FlagPolicy::Leave) | None => Fate::Left(LeftBy::Flag(FlagKind::ManyMatches)),
                _ => Fate::Act(ids[0]),
            },
        };
        records[at].matches = found;
        records[at].fate = fate;
    }
}

fn join_ids(ids: &[i64]) -> String {
    ids.iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(", ")
}

/// Pass two: each `resource` value linked to an existing resource in its
/// kind's scope, else to one of this import's records, else `link-missing`.
pub(crate) fn link(
    matching: &Matching<'_>,
    records: &mut [Record],
    entries: &[Vec<EntryRef>],
    book: &mut FlagBook<'_>,
) {
    let mut by_key: HashMap<String, RecordId> = HashMap::new();
    let identifier = SlotName::Term(Term::dcterms_identifier());
    for record in records
        .iter()
        .filter(|record| !matches!(record.fate, Fate::Left(_)))
    {
        let identifiers = record
            .slots
            .get(&identifier)
            .into_iter()
            .flatten()
            .map(|value| key_form(&value.value.text));
        for key in record
            .resolved
            .item_key
            .clone()
            .into_iter()
            .chain(identifiers)
        {
            by_key.entry(key).or_insert(record.id);
        }
    }
    let kinds = [RecordKind::Item, RecordKind::ItemSet, RecordKind::Media];
    for at in 0..records.len() {
        if matches!(records[at].fate, Fate::Left(_)) {
            continue;
        }
        let id = records[at].id;
        for (slot, values) in &mut records[at].slots {
            values.retain_mut(|value| {
                let ValueType::Resource(by) = value.value.value_type else {
                    return true;
                };
                let text = key_form(&value.value.text);
                let found = match by {
                    ResourceBy::InternalId => text
                        .parse::<i64>()
                        .ok()
                        .filter(|id| {
                            kinds
                                .iter()
                                .any(|kind| matching.existing(*kind).ids.contains(id))
                        })
                        .map(Link::Existing),
                    ResourceBy::Identifier => kinds
                        .iter()
                        .find_map(|kind| {
                            let existing = matching.existing(*kind);
                            existing
                                .by_identifier
                                .get(&text)?
                                .iter()
                                .copied()
                                .find(|id| matching.in_scope(*kind, *id, None))
                        })
                        .map(Link::Existing)
                        .or_else(|| by_key.get(&text).copied().map(Link::Record)),
                };
                if let Some(found) = found {
                    value.link = Some(found);
                    return true;
                }
                let policy = book.raise(
                    FlagKind::LinkMissing,
                    Subject::Record(id),
                    Some(slot.clone()),
                    Some(value.value.text.clone()),
                    &entries[at],
                );
                value.value.value_type = ValueType::Literal;
                policy != Some(FlagPolicy::Leave)
            });
        }
        records[at].slots.retain(|_, values| !values.is_empty());
    }
}
