//! The change a bulk run applies, parsed once at the boundary.
//!
//! The wire form is also the stored form (`romeka_bulk.change_json`), so a run reads back exactly
//! what its preview checked:
//!
//! ```json
//! { "kind": "edit", "isPublic": false, "owner": 3, "class": 12, "template": null,
//!   "itemSets": { "add": [4], "remove": [5] }, "sites": { "add": [1], "remove": [] } }
//! ```
//!
//! A member that is absent leaves the field alone. `class` and `template` take an id to set or
//! `null` to clear. `values` lists value edits ([`super::values`]). An edit that names nothing,
//! or adds and removes the same set or site, is refused. A delete is `{"kind": "delete"}`: it
//! moves each item to the trash (`super::trash`). `{"kind": "delete", "of": "media"}` and `"of":
//! "item_sets"` move each media, or each set, alone; `"of": "items"` is the item delete, stored
//! without `of`.

use std::collections::BTreeSet;

use omeka_db::ids::{ClassId, ItemSetId, SiteId, TemplateId, UserId};
use omeka_db::rows::{BulkKind, Described, TrashKind};
use serde::{Deserialize, Deserializer};
use serde_json::{Value, json};

use super::values::{OpWire, ValueEdits};

/// What a run does to each item it selected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BulkChange {
    Edit(BulkEdit),
    /// Move each target, a record of this kind, to the trash.
    Delete(TrashKind),
}

/// A field set to a value, or emptied.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Assign<T> {
    Set(T),
    Clear,
}

/// Memberships to add and to remove. The two never share a member.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Membership<T: Ord> {
    add: BTreeSet<T>,
    remove: BTreeSet<T>,
}

impl<T: Ord + Copy> Membership<T> {
    fn new(add: BTreeSet<T>, remove: BTreeSet<T>) -> Result<Self, T> {
        match add.intersection(&remove).next() {
            Some(both) => Err(*both),
            None => Ok(Self { add, remove }),
        }
    }

    pub fn add(&self) -> &BTreeSet<T> {
        &self.add
    }

    pub fn remove(&self) -> &BTreeSet<T> {
        &self.remove
    }

    fn is_empty(&self) -> bool {
        self.add.is_empty() && self.remove.is_empty()
    }

    fn applied(&self, before: &BTreeSet<T>) -> BTreeSet<T> {
        before
            .difference(&self.remove)
            .chain(self.add.iter())
            .copied()
            .collect()
    }
}

/// The describing fields an edit sets on every item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BulkEdit {
    pub is_public: Option<bool>,
    pub owner: Option<UserId>,
    pub class: Option<Assign<ClassId>>,
    pub template: Option<Assign<TemplateId>>,
    pub item_sets: Membership<ItemSetId>,
    pub sites: Membership<SiteId>,
    pub values: Option<ValueEdits>,
}

impl BulkEdit {
    /// The fields an item holds after the edit, from those it holds before.
    #[must_use]
    pub fn apply(&self, before: &Described) -> Described {
        Described {
            is_public: self.is_public.unwrap_or(before.is_public),
            owner: self.owner.or(before.owner),
            class: assigned(self.class, before.class),
            template: assigned(self.template, before.template),
            item_sets: self.item_sets.applied(&before.item_sets),
            sites: self.sites.applied(&before.sites),
        }
    }
}

fn assigned<T: Copy>(assign: Option<Assign<T>>, before: Option<T>) -> Option<T> {
    match assign {
        None => before,
        Some(Assign::Set(id)) => Some(id),
        Some(Assign::Clear) => None,
    }
}

impl BulkChange {
    pub fn kind(&self) -> BulkKind {
        match self {
            Self::Edit(_) => BulkKind::Edit,
            Self::Delete(_) => BulkKind::Delete,
        }
    }

    /// Parse a change from the request body or the stored column.
    ///
    /// # Errors
    /// Why the change is refused, in words for the request's 400.
    pub fn parse(value: &Value) -> Result<Self, String> {
        let wire: Wire =
            serde_json::from_value(value.clone()).map_err(|error| error.to_string())?;
        match wire {
            Wire::Edit(edit) => edit.into_edit().map(Self::Edit),
            Wire::Delete(DeleteWire { of }) => {
                Ok(Self::Delete(of.map_or(TrashKind::Item, Of::kind)))
            }
        }
    }

    /// The stored form, which [`Self::parse`] reads back.
    #[must_use]
    pub fn to_json(&self) -> Value {
        let edit = match self {
            Self::Edit(edit) => edit,
            Self::Delete(TrashKind::Item) => return json!({ "kind": "delete" }),
            Self::Delete(kind) => return json!({ "kind": "delete", "of": kind.resource_name() }),
        };
        let mut out = json!({ "kind": "edit" });
        if let Some(is_public) = edit.is_public {
            out["isPublic"] = json!(is_public);
        }
        if let Some(owner) = edit.owner {
            out["owner"] = json!(owner.0);
        }
        if let Some(class) = edit.class {
            out["class"] = assign_json(class);
        }
        if let Some(template) = edit.template {
            out["template"] = assign_json(template);
        }
        if !edit.item_sets.is_empty() {
            out["itemSets"] = membership_json(&edit.item_sets);
        }
        if !edit.sites.is_empty() {
            out["sites"] = membership_json(&edit.sites);
        }
        if let Some(values) = &edit.values {
            out["values"] = values.to_json();
        }
        out
    }
}

fn assign_json<T: Into<u64>>(assign: Assign<T>) -> Value {
    match assign {
        Assign::Set(id) => json!(id.into()),
        Assign::Clear => Value::Null,
    }
}

fn membership_json<T: Ord + Copy + Into<u64>>(members: &Membership<T>) -> Value {
    let ids = |set: &BTreeSet<T>| set.iter().map(|id| (*id).into()).collect::<Vec<u64>>();
    json!({ "add": ids(&members.add), "remove": ids(&members.remove) })
}

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum Wire {
    Edit(EditWire),
    Delete(DeleteWire),
}

/// A delete names its kind and, for media or sets, what it deletes.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DeleteWire {
    #[serde(default)]
    of: Option<Of>,
}

/// What a delete deletes, by the resource name of the API.
#[derive(Deserialize, Clone, Copy)]
#[serde(rename_all = "snake_case")]
enum Of {
    Items,
    Media,
    ItemSets,
}

impl Of {
    fn kind(self) -> TrashKind {
        match self {
            Self::Items => TrashKind::Item,
            Self::Media => TrashKind::Media,
            Self::ItemSets => TrashKind::ItemSet,
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct EditWire {
    #[serde(default)]
    is_public: Option<bool>,
    #[serde(default)]
    owner: Option<Id>,
    #[serde(default, deserialize_with = "present")]
    class: Option<Option<Id>>,
    #[serde(default, deserialize_with = "present")]
    template: Option<Option<Id>>,
    #[serde(default)]
    item_sets: MembershipWire,
    #[serde(default)]
    sites: MembershipWire,
    #[serde(default)]
    values: Vec<OpWire>,
}

#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct MembershipWire {
    #[serde(default)]
    add: BTreeSet<Id>,
    #[serde(default)]
    remove: BTreeSet<Id>,
}

/// A positive id; `0` names no row.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct Id(pub(super) u64);

impl<'de> Deserialize<'de> for Id {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        match u64::deserialize(deserializer)? {
            0 => Err(serde::de::Error::custom("an id is a positive integer")),
            id => Ok(Self(id)),
        }
    }
}

/// A member that is there, `null` included, as `Some`; an absent one stays `None` by `default`.
fn present<'de, D: Deserializer<'de>, T: Deserialize<'de>>(
    deserializer: D,
) -> Result<Option<Option<T>>, D::Error> {
    Option::<T>::deserialize(deserializer).map(Some)
}

impl EditWire {
    fn into_edit(self) -> Result<BulkEdit, String> {
        let members = |wire: MembershipWire, name: &str| {
            Membership::new(ids(wire.add), ids(wire.remove))
                .map_err(|both: u64| format!("\"{name}\" adds and removes {both}"))
        };
        let edit = BulkEdit {
            is_public: self.is_public,
            owner: self.owner.map(|id| UserId(id.0)),
            class: self.class.map(|id| assign(id, ClassId)),
            template: self.template.map(|id| assign(id, TemplateId)),
            item_sets: retype(members(self.item_sets, "itemSets")?, ItemSetId),
            sites: retype(members(self.sites, "sites")?, SiteId),
            values: ValueEdits::from_wire(self.values)?,
        };
        let names_nothing = edit.is_public.is_none()
            && edit.owner.is_none()
            && edit.class.is_none()
            && edit.template.is_none()
            && edit.item_sets.is_empty()
            && edit.sites.is_empty()
            && edit.values.is_none();
        if names_nothing {
            return Err("the edit changes no field".to_string());
        }
        Ok(edit)
    }
}

fn ids(set: BTreeSet<Id>) -> BTreeSet<u64> {
    set.into_iter().map(|id| id.0).collect()
}

fn assign<T>(id: Option<Id>, make: fn(u64) -> T) -> Assign<T> {
    match id {
        Some(id) => Assign::Set(make(id.0)),
        None => Assign::Clear,
    }
}

fn retype<T: Ord + Copy>(members: Membership<u64>, make: fn(u64) -> T) -> Membership<T> {
    Membership {
        add: members.add.into_iter().map(make).collect(),
        remove: members.remove.into_iter().map(make).collect(),
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    fn before() -> Described {
        Described {
            is_public: true,
            owner: Some(UserId(1)),
            class: Some(ClassId(7)),
            template: None,
            item_sets: [ItemSetId(4), ItemSetId(5)].into(),
            sites: [SiteId(1)].into(),
        }
    }

    #[test]
    fn an_edit_sets_the_fields_it_names_and_keeps_the_rest() {
        let change = BulkChange::parse(&json!({
            "kind": "edit",
            "isPublic": false,
            "class": null,
            "template": 3,
            "itemSets": { "add": [6], "remove": [4] },
        }))
        .unwrap();
        let BulkChange::Edit(edit) = &change else {
            panic!("an edit")
        };
        let after = edit.apply(&before());
        assert_eq!(
            after,
            Described {
                is_public: false,
                owner: Some(UserId(1)),
                class: None,
                template: Some(TemplateId(3)),
                item_sets: [ItemSetId(5), ItemSetId(6)].into(),
                sites: [SiteId(1)].into(),
            }
        );
        assert_eq!(BulkChange::parse(&change.to_json()).unwrap(), change);
    }

    #[test]
    fn an_edit_that_already_holds_is_no_change() {
        let change = BulkChange::parse(&json!({
            "kind": "edit", "isPublic": true, "itemSets": { "add": [4] }, "sites": { "remove": [2] },
        }))
        .unwrap();
        let BulkChange::Edit(edit) = change else {
            panic!("an edit")
        };
        assert_eq!(edit.apply(&before()), before());
    }

    #[test]
    fn a_delete_names_what_it_deletes_and_reads_back() {
        for (body, kind) in [
            (json!({ "kind": "delete" }), TrashKind::Item),
            (json!({ "kind": "delete", "of": "items" }), TrashKind::Item),
            (json!({ "kind": "delete", "of": "media" }), TrashKind::Media),
            (
                json!({ "kind": "delete", "of": "item_sets" }),
                TrashKind::ItemSet,
            ),
        ] {
            let change = BulkChange::parse(&body).unwrap();
            assert_eq!(change, BulkChange::Delete(kind), "{body}");
            assert_eq!(change.kind(), BulkKind::Delete);
            assert_eq!(BulkChange::parse(&change.to_json()).unwrap(), change);
        }
        assert_eq!(
            BulkChange::Delete(TrashKind::Item).to_json(),
            json!({ "kind": "delete" }),
            "an item delete is stored as it was before media and sets"
        );
    }

    #[test]
    fn a_change_that_names_nothing_or_contradicts_itself_is_refused() {
        for body in [
            json!({ "kind": "edit" }),
            json!({ "kind": "edit", "itemSets": { "add": [], "remove": [] } }),
            json!({ "kind": "edit", "values": [] }),
            json!({ "kind": "edit", "sites": { "add": [2], "remove": [2] } }),
            json!({ "kind": "edit", "owner": null }),
            json!({ "kind": "edit", "class": 0 }),
            json!({ "kind": "edit", "title": "x" }),
            json!({ "kind": "delete", "isPublic": false }),
            json!({ "kind": "delete", "of": "sites" }),
            json!({ "isPublic": true }),
        ] {
            assert!(BulkChange::parse(&body).is_err(), "{body}");
        }
    }
}
