//! What the records need before the first one is written: the plan's new
//! fields, its new item types, and the sets records name by a title no set
//! has (`docs/import-v2.md`, "Slot rules a worker must not guess").
//!
//! Each gets a ledger row under its own key ([`ledger`]), so a retry finds it
//! instead of making it again. The row commits in the transaction that makes
//! the field, type or set, so no crash leaves one the ledger does not know.

use omeka_db::Backend;
use omeka_db::ids::{ItemSetId, PropertyId, TemplateId, UserId};
use omeka_db::repos::{ImportResourceRepo, ImportWrites, VocabularyRepo};
use omeka_db::rows::{
    ImportChange, ImportTarget, NewImportFields, NewImportItemSet, NewImportResource, NewProperty,
    NewTemplate, NewTemplateProperty, NewVocabulary, ValueContent,
};
use sqlx::{ColumnIndex, Decode, Encode, Executor, IntoArguments, Type};

use super::{Run, RunError};
use crate::import::ledger::{set_key, term_key, type_key};
use crate::import::plan::{NewTypeFrom, Term};
use crate::import::starters::starter;
use crate::import::values::Visibility;

/// The vocabulary the plan's new fields go in.
const LOCAL_PREFIX: &str = "local";
const LOCAL_NAMESPACE: &str = "urn:romeka:local#";

/// One field of a type the run makes.
struct NewField {
    term: Term,
    required: bool,
    data_types: Vec<String>,
}

impl<DB> Run<'_, DB>
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
    pub(super) async fn prelude(&mut self) -> Result<(), RunError> {
        self.new_terms().await?;
        self.new_types().await?;
        self.new_sets().await
    }

    /// The owner of what the run makes for the whole import.
    fn default_owner(&self) -> Option<UserId> {
        self.runner.owner(self.derived.plan.defaults.owner)
    }

    /// Record what `tx` made under `key` and commit both.
    async fn record_made(
        &mut self,
        mut tx: sqlx::Transaction<'static, DB>,
        key: String,
        target: ImportTarget,
    ) -> Result<(), RunError> {
        ImportResourceRepo::<DB>::record(
            &mut *tx,
            self.import,
            &NewImportResource {
                target,
                record: key.clone(),
                change: ImportChange::Created,
            },
        )
        .await?;
        tx.commit().await?;
        self.written.add(key, target);
        Ok(())
    }

    async fn new_terms(&mut self) -> Result<(), RunError> {
        let plan = std::sync::Arc::clone(&self.derived.plan);
        for new in &plan.new_terms {
            let Some(term) = Term::local(&new.label) else {
                continue;
            };
            let key = term_key(&new.label);
            if let Some(ImportTarget::Property(id)) = self.written.target(&key) {
                self.lookup.properties.insert(term, id);
                continue;
            }
            if self.lookup.properties.contains_key(&term) {
                continue;
            }
            let vocabulary = self.local_vocabulary().await?;
            let local_name = term
                .as_str()
                .strip_prefix("local:")
                .unwrap_or(term.as_str())
                .to_string();
            let mut tx = self.pool.begin().await?;
            let row = ImportWrites::<DB>::new(&mut tx)
                .create_property(&NewProperty {
                    owner_id: self.default_owner(),
                    vocabulary_id: vocabulary,
                    local_name,
                    label: new.label.clone(),
                    comment: None,
                })
                .await?;
            self.record_made(tx, key, ImportTarget::Property(row.id))
                .await?;
            self.lookup.properties.insert(term, row.id);
        }
        Ok(())
    }

    async fn local_vocabulary(&self) -> Result<omeka_db::ids::VocabularyId, RunError> {
        let vocabularies = VocabularyRepo::<DB>::new(self.pool);
        if let Some(row) = vocabularies.get_by_prefix(LOCAL_PREFIX).await? {
            return Ok(row.id);
        }
        Ok(vocabularies
            .create(&NewVocabulary {
                owner_id: self.default_owner(),
                namespace_uri: LOCAL_NAMESPACE.to_string(),
                prefix: LOCAL_PREFIX.to_string(),
                label: "Local".to_string(),
                comment: Some("Fields made by imports.".to_string()),
            })
            .await?
            .id)
    }

    async fn new_types(&mut self) -> Result<(), RunError> {
        let plan = std::sync::Arc::clone(&self.derived.plan);
        let mut made = Vec::with_capacity(plan.new_types.len());
        for (at, new) in plan.new_types.iter().enumerate() {
            let key = type_key(&new.label);
            if let Some(ImportTarget::ResourceTemplate(id)) = self.written.target(&key) {
                made.push(Some(id));
                continue;
            }
            let fields = self.type_fields(at, &new.from);
            let property = |term: &Term| self.lookup.properties.get(term).copied();
            let title = fields
                .iter()
                .find(|field| field.required)
                .and_then(|field| property(&field.term));
            let class = new
                .class
                .as_ref()
                .and_then(|term| self.lookup.classes.get(term).copied());
            let mut position = 0;
            let mut rows = Vec::with_capacity(fields.len());
            for field in &fields {
                let Some(property) = property(&field.term) else {
                    continue;
                };
                position += 1;
                rows.push(new_field(property, field, position));
            }
            let new_template = NewTemplate {
                owner_id: self.default_owner(),
                resource_class_id: class,
                title_property_id: title,
                description_property_id: None,
                label: new.label.clone(),
            };
            let mut tx = self.pool.begin().await?;
            let row = ImportWrites::<DB>::new(&mut tx)
                .create_template(&new_template, &rows)
                .await?;
            self.record_made(tx, key, ImportTarget::ResourceTemplate(row.id))
                .await?;
            made.push(Some(row.id));
        }
        self.lookup.new_types = made;
        Ok(())
    }

    /// A mapping type's fields are the ones the derive lists; a starter's
    /// are its table's.
    fn type_fields(&self, at: usize, from: &NewTypeFrom) -> Vec<NewField> {
        match from {
            NewTypeFrom::Starter(from) => starter(from.starter)
                .fields
                .iter()
                .filter_map(|field| {
                    Some(NewField {
                        term: Term::parse(field.term).ok()?,
                        required: field.required,
                        data_types: field.data_types.iter().map(ToString::to_string).collect(),
                    })
                })
                .collect(),
            NewTypeFrom::Mapping(_) => self
                .derived
                .model
                .new_types
                .iter()
                .find(|fields| fields.new_type == at)
                .map(|fields| {
                    fields
                        .fields
                        .iter()
                        .map(|term| NewField {
                            term: term.clone(),
                            required: *term == fields.required,
                            data_types: Vec::new(),
                        })
                        .collect()
                })
                .unwrap_or_default(),
        }
    }

    /// Each set records name by a title no set has: open, with the
    /// visibility of the first record that named it, titled as written,
    /// and described when its set source has a description.
    async fn new_sets(&mut self) -> Result<(), RunError> {
        let property = |text: &str| {
            Term::parse(text)
                .map(|term| self.lookup.properties.get(&term).copied())
                .map_err(|error| RunError::State(format!("{text}: {error}")))
        };
        let title = property("dcterms:title")?;
        let description = property("dcterms:description")?;
        let mut made = Vec::with_capacity(self.derived.model.new_sets.len());
        for set in &self.derived.model.new_sets {
            let key = set_key(&set.title);
            if let Some(ImportTarget::ItemSet(id)) = self.written.target(&key) {
                made.push(Some(id));
                continue;
            }
            let described = description.zip(set.description.clone());
            let values = title
                .map(|property: PropertyId| ValueContent::literal(property, set.title.clone()))
                .into_iter()
                .chain(described.map(|(property, text)| ValueContent::literal(property, text)))
                .collect();
            let new = NewImportItemSet {
                fields: NewImportFields {
                    values,
                    template: None,
                    class: None,
                    owner: self.default_owner(),
                    is_public: set.visibility == Visibility::Public,
                },
                is_open: set.is_open,
                sites: Vec::new(),
            };
            let mut tx = self.pool.begin().await?;
            let id: ItemSetId = ImportWrites::<DB>::new(&mut tx)
                .create_item_set(&new)
                .await?;
            self.record_made(tx, key, ImportTarget::ItemSet(id)).await?;
            made.push(Some(id));
        }
        self.lookup.new_sets = made;
        Ok(())
    }
}

/// One field row; [`ImportWrites::create_template`] names its template.
fn new_field(property: PropertyId, field: &NewField, position: i64) -> NewTemplateProperty {
    NewTemplateProperty {
        resource_template_id: TemplateId(0),
        property_id: property,
        alternate_label: None,
        alternate_comment: None,
        position: Some(position),
        data_type: (!field.data_types.is_empty()).then(|| field.data_types.clone()),
        is_required: field.required,
        is_private: false,
        default_lang: None,
    }
}
