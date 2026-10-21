//! The [`Catalog`] of one plan, read from the install through the generic
//! repositories (`docs/import-v2.md`, "The derive").
//!
//! The derive compares in Rust, so this module only reads: the vocabulary,
//! the templates with their fields, the item sets, the users the caller may
//! name as owners, and, for every term [`Catalog::terms_needed`] names, the
//! literal values of items, media and item sets. A resource with no key
//! value is left out unless the plan matches its kind by internal id,
//! because nothing else in the derive can find it.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use omeka_db::Backend;
use omeka_db::DbError;
use omeka_db::ids::{ImportId, ItemSetId, UserId};
use omeka_db::repos::{
    ClassFilter, ClassRepo, ImportResourceRepo, ItemFilter, ItemRepo, ItemSetFilter, ItemSetRepo,
    MediaFilter, MediaRepo, Paging, PropertyFilter, PropertyRepo, TemplateRepo, UserFilter,
    UserRepo, ValueRewriteRepo, VocabularyFilter, VocabularyRepo,
};
use omeka_db::rows::{ImportTarget, resource_type};
use omeka_media::validation::FileValidation;
use sqlx::{ColumnIndex, Decode, Encode, Executor, IntoArguments, Pool, Type};

use super::catalog::{
    Catalog, ExistingItem, ExistingMedia, ExistingSet, Field, Keys, Template, User,
};
use super::plan::{ImportPlan, MatchKey, NewTypeFrom, Term};
use super::starters::starter;

/// The template the install calls "Base Resource". Upstream creates it at
/// install with this label and nothing marks it otherwise.
const BASE_RESOURCE: &str = "Base Resource";

/// Who the catalog is read for: owners are limited to what this caller may
/// name ("Owner" in "Slot rules a worker must not guess").
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Owners {
    /// A global admin may name any user.
    Anyone,
    /// A site admin may name only themself.
    Only(UserId),
}

impl Owners {
    /// The owners a caller of `role` may name: a global admin, or no user at
    /// all, any user; anyone else only themself.
    #[must_use]
    pub fn of(role: Option<&str>, user: Option<UserId>) -> Self {
        match (role, user) {
            (Some("global_admin"), _) | (_, None) => Self::Anyone,
            (_, Some(user)) => Self::Only(user),
        }
    }
}

fn raw(id: u64) -> i64 {
    i64::try_from(id).unwrap_or(i64::MAX)
}

/// Read the catalog `plan` derives against. `files` is the install's
/// file-validation settings, which the caller reads.
///
/// The item types and fields import `import`'s own ledger records are left
/// out: an earlier attempt of the same import made them from this plan, so a
/// retry derives as the first attempt did, with no `type-exists` flag.
///
/// # Errors
/// A [`DbError`] when a read fails.
pub async fn load<DB>(
    pool: &Pool<DB>,
    import: ImportId,
    plan: &ImportPlan,
    owners: Owners,
    files: FileValidation,
) -> Result<Catalog, DbError>
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
    let mut own_templates = HashSet::new();
    let mut own_properties = HashSet::new();
    for row in ImportResourceRepo::new(pool).list(import, None).await? {
        match row.target {
            ImportTarget::ResourceTemplate(id) => {
                own_templates.insert(id);
            }
            ImportTarget::Property(id) => {
                own_properties.insert(id);
            }
            _ => {}
        }
    }
    let prefixes: HashMap<_, _> = VocabularyRepo::new(pool)
        .list(&VocabularyFilter::default(), &all)
        .await?
        .into_iter()
        .map(|row| (row.id, row.prefix))
        .collect();
    let term_of = |vocabulary, local: &str| {
        prefixes
            .get(&vocabulary)
            .and_then(|prefix| Term::parse(&format!("{prefix}:{local}")).ok())
    };
    let mut properties = BTreeMap::new();
    let mut property_terms = HashMap::new();
    for row in PropertyRepo::new(pool)
        .list(&PropertyFilter::default(), &all)
        .await?
    {
        if own_properties.contains(&row.id) {
            continue;
        }
        if let Some(term) = term_of(row.vocabulary_id, &row.local_name) {
            property_terms.insert(row.id, term.clone());
            properties.insert(term, row.label);
        }
    }
    let classes = ClassRepo::new(pool)
        .list(&ClassFilter::default(), &all)
        .await?
        .into_iter()
        .filter_map(|row| Some((term_of(row.vocabulary_id, &row.local_name)?, row.label)))
        .collect();

    let template_repo = TemplateRepo::new(pool);
    let mut templates = Vec::new();
    let mut base_template = None;
    for row in template_repo.list().await? {
        if own_templates.contains(&row.id) {
            continue;
        }
        let fields = template_repo
            .list_properties(row.id)
            .await?
            .into_iter()
            .filter_map(|field| {
                Some(Field {
                    term: property_terms.get(&field.property_id)?.clone(),
                    required: field.is_required,
                    data_types: field.data_type.unwrap_or_default(),
                })
            })
            .collect();
        let id = raw(row.id.0);
        if base_template.is_none() && row.label == BASE_RESOURCE {
            base_template = Some(id);
        }
        templates.push(Template {
            id,
            label: row.label,
            fields,
        });
    }

    let users = match owners {
        Owners::Anyone => {
            UserRepo::new(pool)
                .list(&UserFilter::default(), &all)
                .await?
        }
        Owners::Only(user) => UserRepo::new(pool).get(user).await?.into_iter().collect(),
    }
    .into_iter()
    .map(|row| User {
        id: raw(row.id.0),
        email: row.email,
    })
    .collect();

    let mut keys: HashMap<i64, Keys> = HashMap::new();
    let mut kinds: HashMap<i64, &'static str> = HashMap::new();
    let wanted = Catalog::terms_needed(plan);
    let rewrites = ValueRewriteRepo::new(pool);
    for (property, term) in &property_terms {
        if !wanted.contains(term) {
            continue;
        }
        for value in rewrites.text_values("literal", Some(*property)).await? {
            let id = raw(value.resource_id.0);
            let kind = match value.resource_type.as_str() {
                resource_type::ITEM => resource_type::ITEM,
                resource_type::MEDIA => resource_type::MEDIA,
                resource_type::ITEM_SET => resource_type::ITEM_SET,
                _ => continue,
            };
            kinds.insert(id, kind);
            keys.entry(id)
                .or_default()
                .entry(term.clone())
                .or_default()
                .push(value.text);
        }
    }
    let keyed = |kind: &str| -> BTreeSet<i64> {
        kinds
            .iter()
            .filter(|(_, of)| **of == kind)
            .map(|(id, _)| *id)
            .collect()
    };
    let by_id = |key: &MatchKey| matches!(key, MatchKey::InternalId);
    let rules = &plan.action.match_by;

    let set_rows = ItemSetRepo::new(pool)
        .list(&ItemSetFilter::default(), &all)
        .await?;
    let item_repo = ItemRepo::new(pool);
    let mut sets_of: HashMap<i64, Vec<i64>> = HashMap::new();
    for set in &set_rows {
        let filter = ItemFilter {
            item_set_id: Some(ItemSetId(set.id().0)),
            ..ItemFilter::default()
        };
        for item in item_repo.list(&filter, &all).await? {
            sets_of
                .entry(raw(item.id().0))
                .or_default()
                .push(raw(set.id().0));
        }
    }
    let item_sets = set_rows
        .iter()
        .map(|set| {
            let id = raw(set.id().0);
            ExistingSet {
                id,
                title: set.resource.title.clone().unwrap_or_default(),
                keys: keys.get(&id).cloned().unwrap_or_default(),
            }
        })
        .collect();

    let mut item_ids = keyed(resource_type::ITEM);
    if by_id(&rules.item.key) {
        for item in item_repo.list(&ItemFilter::default(), &all).await? {
            item_ids.insert(raw(item.id().0));
        }
    }
    let items = item_ids
        .into_iter()
        .map(|id| ExistingItem {
            id,
            keys: keys.get(&id).cloned().unwrap_or_default(),
            sets: sets_of.get(&id).cloned().unwrap_or_default(),
        })
        .collect();

    let media_keyed = keyed(resource_type::MEDIA);
    let media = if media_keyed.is_empty() && !by_id(&rules.media.key) {
        Vec::new()
    } else {
        MediaRepo::new(pool)
            .list(&MediaFilter::default(), &all)
            .await?
            .into_iter()
            .map(|row| (raw(row.resource.id.0), raw(row.item_id.0)))
            .filter(|(id, _)| by_id(&rules.media.key) || media_keyed.contains(id))
            .map(|(id, item)| ExistingMedia {
                id,
                keys: keys.get(&id).cloned().unwrap_or_default(),
                item,
            })
            .collect()
    };

    Ok(Catalog {
        items,
        media,
        item_sets,
        templates,
        base_template,
        properties,
        classes,
        users,
        starters: starter_fields(plan),
        files,
    })
}

/// The fields of each starter the plan's new types name.
fn starter_fields(plan: &ImportPlan) -> BTreeMap<super::plan::StarterId, Vec<Field>> {
    plan.new_types
        .iter()
        .filter_map(|new_type| match new_type.from {
            NewTypeFrom::Starter(from) => Some(from.starter),
            NewTypeFrom::Mapping(_) => None,
        })
        .map(|id| {
            let fields = starter(id)
                .fields
                .iter()
                .filter_map(|field| {
                    Some(Field {
                        term: Term::parse(field.term).ok()?,
                        required: field.required,
                        data_types: field.data_types.iter().map(ToString::to_string).collect(),
                    })
                })
                .collect();
            (id, fields)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::unwrap_used)]

    use omeka_db::repos::{ItemRepo, ItemSetRepo, ValueRepo};
    use omeka_db::rows::{NewItem, NewItemSet, NewProperty, NewTemplate, NewValue, NewVocabulary};
    use serde_json::json;

    use super::*;
    use crate::import::plan::{RelPath, SourceId, SourcePlan, StarterId, default_plan};

    fn plan(edit: impl FnOnce(&mut serde_json::Value)) -> ImportPlan {
        let plan = default_plan(vec![SourcePlan::Folder {
            id: SourceId("s1".into()),
            root: "archive".into(),
            path: RelPath::parse("").unwrap(),
        }])
        .unwrap();
        let mut value = serde_json::to_value(plan).unwrap();
        edit(&mut value);
        ImportPlan::from_json(&value.to_string()).unwrap()
    }

    #[tokio::test]
    async fn the_catalog_reads_keys_sets_templates_users_and_starter_fields() {
        let pool = crate::testing::temp_db().await;
        let admin = crate::testing::test_user(&pool, "a@example.com", "global_admin", true).await;
        let site = crate::testing::test_user(&pool, "s@example.com", "site_admin", true).await;
        let set = ItemSetRepo::new(&pool)
            .create(&NewItemSet::default())
            .await
            .unwrap();
        let item = ItemRepo::new(&pool)
            .create(&NewItem::default())
            .await
            .unwrap();
        ItemRepo::new(&pool)
            .add_to_item_set(item.id(), set.id())
            .await
            .unwrap();
        let dcterms = VocabularyRepo::new(&pool)
            .create(&NewVocabulary {
                owner_id: None,
                namespace_uri: "http://purl.org/dc/terms/".into(),
                prefix: "dcterms".into(),
                label: "Dublin Core".into(),
                comment: None,
            })
            .await
            .unwrap();
        let identifier = PropertyRepo::new(&pool)
            .create(&NewProperty {
                owner_id: None,
                vocabulary_id: dcterms.id,
                local_name: "identifier".into(),
                label: "Identifier".into(),
                comment: None,
            })
            .await
            .unwrap();
        TemplateRepo::new(&pool)
            .create(&NewTemplate {
                owner_id: None,
                resource_class_id: None,
                title_property_id: None,
                description_property_id: None,
                label: "Base Resource".into(),
            })
            .await
            .unwrap();
        ValueRepo::new(&pool)
            .create(&NewValue {
                resource_id: item.resource.id,
                property_id: identifier.id,
                value_resource_id: None,
                value_annotation_id: None,
                r#type: "literal".into(),
                lang: None,
                value: Some("BHC_006_AF_001".into()),
                uri: None,
                is_public: true,
            })
            .await
            .unwrap();
        let plan = plan(|plan| {
            plan["newTypes"] =
                json!([{ "label": "Letters", "from": { "starter": "letter" }, "class": null }]);
        });

        let catalog = load(
            &pool,
            ImportId(1),
            &plan,
            Owners::Anyone,
            FileValidation::Disabled,
        )
        .await
        .unwrap();

        assert_eq!(catalog.items.len(), 1);
        assert_eq!(
            catalog.items[0].keys[&Term::dcterms_identifier()],
            ["BHC_006_AF_001"]
        );
        assert_eq!(catalog.items[0].sets, [raw(set.id().0)]);
        assert_eq!(catalog.item_sets.len(), 1);
        assert!(catalog.properties.contains_key(&Term::dcterms_identifier()));
        assert!(catalog.base_template.is_some());
        assert_eq!(catalog.users.len(), 2);
        assert_eq!(
            catalog.starters[&StarterId::Letter][0].term.as_str(),
            "dcterms:title"
        );
        let mine = load(
            &pool,
            ImportId(1),
            &plan,
            Owners::Only(site.id),
            FileValidation::Disabled,
        )
        .await
        .unwrap();
        assert_eq!(mine.users.len(), 1, "a site admin names only themself");
        assert_ne!(mine.users[0].id, raw(admin.id.0));
    }

    #[tokio::test]
    async fn a_retry_reads_no_type_or_field_its_own_import_made() {
        let pool = crate::testing::temp_db().await;
        let local = VocabularyRepo::new(&pool)
            .create(&NewVocabulary {
                owner_id: None,
                namespace_uri: "urn:romeka:local#".into(),
                prefix: "local".into(),
                label: "Local".into(),
                comment: None,
            })
            .await
            .unwrap();
        let field = PropertyRepo::new(&pool)
            .create(&NewProperty {
                owner_id: None,
                vocabulary_id: local.id,
                local_name: "shelf".into(),
                label: "Shelf".into(),
                comment: None,
            })
            .await
            .unwrap();
        let letters = TemplateRepo::new(&pool)
            .create(&NewTemplate {
                owner_id: None,
                resource_class_id: None,
                title_property_id: None,
                description_property_id: None,
                label: "Letters".into(),
            })
            .await
            .unwrap();
        let imports = omeka_db::repos::ImportRepo::new(&pool);
        let new = omeka_db::rows::NewImport {
            owner_id: None,
            sources: json!([]),
            plan: json!({}),
            copied_from: None,
        };
        let own = imports.create_draft(&new).await.unwrap().id;
        let other = imports.create_draft(&new).await.unwrap().id;
        for (record, target) in [
            ("type:Letters", ImportTarget::ResourceTemplate(letters.id)),
            ("term:Shelf", ImportTarget::Property(field.id)),
        ] {
            ImportResourceRepo::record(
                &pool,
                own,
                &omeka_db::rows::NewImportResource {
                    target,
                    record: record.into(),
                    change: omeka_db::rows::ImportChange::Created,
                },
            )
            .await
            .unwrap();
        }
        let plan = plan(|plan| {
            plan["newTypes"] =
                json!([{ "label": "Letters", "from": { "starter": "letter" }, "class": null }]);
        });
        let shelf = Term::local("Shelf").unwrap();

        let retry = load(&pool, own, &plan, Owners::Anyone, FileValidation::Disabled)
            .await
            .unwrap();
        let elsewhere = load(
            &pool,
            other,
            &plan,
            Owners::Anyone,
            FileValidation::Disabled,
        )
        .await
        .unwrap();

        assert!(retry.templates.iter().all(|t| t.label != "Letters"));
        assert!(!retry.properties.contains_key(&shelf));
        assert!(elsewhere.templates.iter().any(|t| t.label == "Letters"));
        assert!(elsewhere.properties.contains_key(&shelf));
    }
}
