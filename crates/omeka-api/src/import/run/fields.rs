//! A record's slots as the rows `omeka_db::ImportWrites` takes: values with
//! property ids, and the named slots with resource ids.
//!
//! A value that links to another record of this import is not written here;
//! [`super::links`] writes it once every record exists.

use std::collections::HashMap;

use omeka_db::Backend;
use omeka_db::DbError;
use omeka_db::ids::{
    ClassId, ItemId, ItemSetId, MediaId, PropertyId, ResourceId, SiteId, TemplateId, UserId,
};
use omeka_db::repos::{
    ClassFilter, ClassRepo, Paging, PropertyFilter, PropertyRepo, ResourceRepo, VocabularyFilter,
    VocabularyRepo,
};
use omeka_db::rows::{
    ImportFields, ImportTarget, ItemSlots, NewImportFields, Slot, ValueContent, resource_type,
};
use sqlx::{ColumnIndex, Decode, Encode, Executor, IntoArguments, Pool, Type};

use crate::import::model::{
    Fate, Link, Model, OwnerRef, Record, RecordId, SetRef, SlotValue, TemplateRef,
};
use crate::import::plan::{Action, ImportPlan, Owner, RecordKind, SlotName, SlotRules, Term};
use crate::import::values::{ValueType, Visibility};

/// The install's ids for what the model names by term, and what the run
/// made before the records.
#[derive(Debug, Clone, Default)]
pub struct Lookup {
    pub properties: HashMap<Term, PropertyId>,
    pub classes: HashMap<Term, ClassId>,
    /// The data type of a link to each existing resource the model links to.
    pub link_types: HashMap<i64, &'static str>,
    /// One per plan `newTypes` entry, in plan order.
    pub new_types: Vec<Option<TemplateId>>,
    /// One per [`Model::new_sets`] entry.
    pub new_sets: Vec<Option<ItemSetId>>,
}

pub(super) fn id<T: From<u64>>(raw: i64) -> Option<T> {
    u64::try_from(raw).ok().map(T::from)
}

/// The resource a matched record acts on.
pub(super) fn matched(record: &Record) -> Option<ImportTarget> {
    let Fate::Act(raw) = record.fate else {
        return None;
    };
    Some(match record.kind {
        RecordKind::Item => ImportTarget::Item(id::<ItemId>(raw)?),
        RecordKind::Media => ImportTarget::Media(id::<MediaId>(raw)?),
        RecordKind::ItemSet => ImportTarget::ItemSet(id::<ItemSetId>(raw)?),
    })
}

impl Lookup {
    /// The vocabulary and every resource the model links to.
    pub async fn load<DB>(pool: &Pool<DB>, model: &Model) -> Result<Self, DbError>
    where
        DB: Backend,
        for<'c> &'c mut DB::Connection: Executor<'c, Database = DB>,
        for<'q> DB::Arguments<'q>: IntoArguments<'q, DB>,
        usize: ColumnIndex<DB::Row>,
        for<'x> bool: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
        for<'x> f64: Decode<'x, DB> + Type<DB>,
        for<'x> i64: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
        for<'x> Option<i64>: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
        for<'x> String: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
        for<'x> Option<String>: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
        for<'x> Vec<u8>: Decode<'x, DB> + Type<DB>,
        for<'x> Option<Vec<u8>>: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
        for<'x> &'x str: Encode<'x, DB> + Type<DB> + ColumnIndex<DB::Row>,
    {
        let all = Paging::default();
        let prefixes: HashMap<_, _> = VocabularyRepo::<DB>::new(pool)
            .list(&VocabularyFilter::default(), &all)
            .await?
            .into_iter()
            .map(|row| (row.id, row.prefix))
            .collect();
        let term_of = |vocabulary, local: &str| {
            Term::parse(&format!("{}:{local}", prefixes.get(&vocabulary)?)).ok()
        };
        let properties = PropertyRepo::<DB>::new(pool)
            .list(&PropertyFilter::default(), &all)
            .await?
            .into_iter()
            .filter_map(|row| Some((term_of(row.vocabulary_id, &row.local_name)?, row.id)))
            .collect();
        let classes = ClassRepo::<DB>::new(pool)
            .list(&ClassFilter::default(), &all)
            .await?
            .into_iter()
            .filter_map(|row| Some((term_of(row.vocabulary_id, &row.local_name)?, row.id)))
            .collect();
        let mut link_types = HashMap::new();
        let resources = ResourceRepo::<DB>::new(pool);
        let linked = model
            .kept()
            .flat_map(|record| record.slots.values().flatten())
            .filter_map(|value| match value.link {
                Some(Link::Existing(raw)) => Some(raw),
                _ => None,
            });
        for raw in linked {
            if link_types.contains_key(&raw) {
                continue;
            }
            let Some(resource) = id::<ResourceId>(raw) else {
                continue;
            };
            let kind = resources
                .get(resource)
                .await?
                .map_or("resource", |row| link_type(&row.resource_type));
            link_types.insert(raw, kind);
        }
        Ok(Self {
            properties,
            classes,
            link_types,
            new_types: Vec::new(),
            new_sets: Vec::new(),
        })
    }
}

/// Upstream's data type for a link to a resource of this entity class.
fn link_type(entity: &str) -> &'static str {
    match entity {
        resource_type::ITEM => "resource:item",
        resource_type::ITEM_SET => "resource:itemset",
        resource_type::MEDIA => "resource:media",
        _ => "resource",
    }
}

/// The data type of a link to a record of this kind.
pub(super) fn record_link_type(kind: RecordKind) -> &'static str {
    match kind {
        RecordKind::Item => "resource:item",
        RecordKind::ItemSet => "resource:itemset",
        RecordKind::Media => "resource:media",
    }
}

/// One stored value: `literal`, `uri` (the text is the URI and the label
/// its value, upstream's rule), a link, or a named data type whose text is
/// the value.
pub(super) fn content(
    property: PropertyId,
    value: &SlotValue,
    link: Option<(ResourceId, &str)>,
) -> ValueContent {
    let text = &value.value;
    let mut content = ValueContent {
        lang: text.lang.clone(),
        is_public: text.is_public,
        ..ValueContent::literal(property, text.text.clone())
    };
    match (&text.value_type, link) {
        (ValueType::Resource(_), Some((target, data_type))) => {
            content.r#type = data_type.to_string();
            content.value = None;
            content.lang = None;
            content.value_resource_id = Some(target);
        }
        (ValueType::Uri, _) => {
            content.r#type = "uri".to_string();
            content.uri = Some(text.text.clone());
            content.value = text.label.clone();
        }
        (ValueType::DataType(name), _) => content.r#type = name.clone(),
        (ValueType::Literal | ValueType::Resource(_), _) => {}
    }
    content
}

/// Whether the record's slot holds a value: what `revise` and `append`
/// write.
pub(super) fn has(record: &Record, slot: &SlotName) -> bool {
    record
        .slots
        .get(slot)
        .is_some_and(|values| !values.is_empty())
}

/// The record's term values, in slot order, less the links to this
/// import's records.
pub(super) fn values(lookup: &Lookup, record: &Record) -> Vec<ValueContent> {
    let mut out = Vec::new();
    for (slot, values) in &record.slots {
        let SlotName::Term(term) = slot else {
            continue;
        };
        let Some(property) = lookup.properties.get(term) else {
            continue;
        };
        for value in values {
            let link = match value.link {
                Some(Link::Record(_)) => continue,
                Some(Link::Existing(raw)) => id::<ResourceId>(raw).map(|target| {
                    (
                        target,
                        lookup.link_types.get(&raw).copied().unwrap_or("resource"),
                    )
                }),
                None => None,
            };
            out.push(content(*property, value, link));
        }
    }
    out
}

/// The run's view of a record's named slots.
pub(super) struct Named<'a> {
    pub lookup: &'a Lookup,
    pub plan: &'a ImportPlan,
    pub runner: Option<UserId>,
    /// What this run or an earlier attempt wrote for each item-set record.
    pub set_records: &'a HashMap<RecordId, ItemSetId>,
}

impl Named<'_> {
    pub fn owner(&self, record: &Record) -> Option<UserId> {
        let owner = match &record.resolved.owner {
            OwnerRef::Is(owner) => *owner,
            OwnerRef::Unknown(_) => self.plan.defaults.owner,
        };
        match owner {
            Owner::Runner => self.runner,
            Owner::User(raw) => id(raw),
        }
    }

    pub fn template(&self, record: &Record) -> Option<TemplateId> {
        match record.resolved.template? {
            TemplateRef::Existing(raw) => id(raw),
            TemplateRef::New(at) => self.lookup.new_types.get(at).copied().flatten(),
        }
    }

    pub fn class(&self, record: &Record) -> Option<ClassId> {
        self.lookup
            .classes
            .get(record.resolved.class.as_ref()?)
            .copied()
    }

    pub fn sets(&self, record: &Record) -> Vec<ItemSetId> {
        let mut sets: Vec<ItemSetId> = record
            .resolved
            .sets
            .iter()
            .filter_map(|set| match set {
                SetRef::Existing(raw) => id(*raw),
                SetRef::Record(at) => self.set_records.get(at).copied(),
                SetRef::New(at) => self.lookup.new_sets.get(*at).copied().flatten(),
            })
            .collect();
        sets.dedup();
        sets
    }

    pub fn sites(record: &Record) -> Vec<SiteId> {
        record
            .resolved
            .sites
            .iter()
            .filter_map(|raw| id(*raw))
            .collect()
    }

    /// What a created resource carries.
    pub fn created(&self, record: &Record) -> NewImportFields {
        NewImportFields {
            values: values(self.lookup, record),
            template: self.template(record),
            class: self.class(record),
            owner: self.owner(record),
            is_public: record.resolved.visibility == Visibility::Public,
        }
    }

    fn rules(&self, kind: RecordKind) -> &SlotRules {
        let structure = &self.plan.structure;
        structure.series(structure.sole()).rules(kind)
    }

    /// Whether `update` and `replace` write every slot the plan maps, not
    /// only the slots with values.
    fn whole(&self) -> bool {
        matches!(self.plan.action.action, Action::Update | Action::Replace)
    }

    /// A named slot as a matched record writes it. It is mapped when the
    /// record holds a value for it, so `revise` leaves the others as stored;
    /// under `update` and `replace` also when the plan has a rule for it, so
    /// an empty cell clears the slot.
    fn mapped<T>(&self, record: &Record, slot: SlotName, value: T) -> Slot<T> {
        let ruled = self.whole() && self.rules(record.kind).contains_key(&slot);
        if ruled || has(record, &slot) {
            Slot::Mapped(value)
        } else {
            Slot::Unmapped
        }
    }

    /// What a matched resource gets. Under `update` and `replace` the owner
    /// and the visibility are always mapped: the plan's defaults give them
    /// when no rule does (`docs/import-v2.md`, "Write details"). A default
    /// item type maps the item type too, as CSVImport writes its default
    /// `o:resource_template` into every row it updates.
    pub fn changed(&self, record: &Record, kind: RecordKind) -> ImportFields {
        let rules = self.rules(kind);
        let mapped_terms = rules
            .keys()
            .filter_map(|slot| match slot {
                SlotName::Term(term) => self.lookup.properties.get(term).copied(),
                _ => None,
            })
            .collect();
        ImportFields {
            values: values(self.lookup, record),
            mapped_terms,
            template: if self.whole() && self.plan.defaults.template.is_some() {
                Slot::Mapped(self.template(record))
            } else {
                self.mapped(record, SlotName::Template, self.template(record))
            },
            class: self.mapped(record, SlotName::Class, self.class(record)),
            owner: if self.whole() {
                Slot::Mapped(self.owner(record))
            } else {
                self.mapped(record, SlotName::Owner, self.owner(record))
            },
            is_public: if self.whole() {
                Slot::Mapped(record.resolved.visibility == Visibility::Public)
            } else {
                self.mapped(
                    record,
                    SlotName::Visibility,
                    record.resolved.visibility == Visibility::Public,
                )
            },
        }
    }

    pub fn item_slots(&self, record: &Record) -> ItemSlots {
        ItemSlots {
            item_sets: self.mapped(record, SlotName::Sets, self.sets(record)),
            sites: self.mapped(record, SlotName::Sites, Self::sites(record)),
        }
    }
}
