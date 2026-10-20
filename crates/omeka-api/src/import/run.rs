//! The run: from a plan to items, sets, templates, fields and media.
//!
//! The caller opens the undo window first, so a run that stops part way can be
//! undone. [`Work::load`] decides every write before the first one, and every
//! term must resolve then, so a plan that names an unknown term writes nothing.
//! Each new set, template and field commits with its `romeka_import_resource`
//! row. Then each item stores its files, and commits the item, its values, set
//! links, media and media values with their rows in one transaction. A file
//! that cannot be stored, or a transaction that fails, fails only that item:
//! its stored files are removed and the run goes on. After each commit the
//! item is indexed and joins its synced sites, as an API create does.

use std::collections::BTreeMap;

use omeka_db::DbError;
use omeka_db::ids::{ImportId, ItemSetId, PropertyId, ResourceId, TemplateId, UserId};
use omeka_db::repos::{ImportRepo, ImportWriteRepo, NewImportMedia, SettingRepo, TemplateRepo};
use omeka_db::rows::{
    NewItem, NewItemSet, NewProperty, NewResource, NewTemplate, NewTemplateProperty, NewWholeItem,
    ValueContent,
};
use omeka_media::store::FileStore;
use omeka_media::validation::FileValidation;
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

use super::archive::ArchiveFolder;
use super::catalog::{Catalog, starter};
use super::media_files;
use super::plan::ImportPlan;
use super::work::{ItemWork, SetRef, TemplateRef, Work, WorkError};
use crate::errors::ApiError;
use crate::jsonld::BaseUrl;
use crate::read::Api;
use crate::site_items;
use crate::write::fulltext;

mod lookups;

pub(super) use lookups::find_property;
use lookups::{
    TemplateUse, existing_templates, find_class, literals, local_vocabulary, property,
    resolve_terms, title,
};

/// The vocabulary new fields go in, as the admin's "Create a new field" makes it.
pub const LOCAL_PREFIX: &str = "local";

/// What a run needs besides the database and the store.
#[derive(Debug, Clone)]
pub struct RunInput {
    pub import_id: ImportId,
    /// The owner of every resource the run creates.
    pub owner: Option<UserId>,
    pub folder: ArchiveFolder,
    pub plan: ImportPlan,
    pub catalog: Catalog,
    /// The installation's origin; the `local` vocabulary's namespace sits under it.
    pub base_url: BaseUrl,
}

/// One step of a run or an undo, for the job log.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Progress<'a> {
    pub done: usize,
    pub total: usize,
    pub line: &'a str,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Count {
    pub done: usize,
    pub total: usize,
}

/// `romeka_import.summary`, kept current as the run goes. An undo adds its
/// [`super::undo::UndoReport`] as `undo` when it ends. A member missing from
/// a stored summary reads as empty.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ImportSummary {
    pub items: usize,
    /// Originals stored.
    pub files: usize,
    /// Media whose thumbnails were made.
    pub thumbnails: usize,
    /// Rows the run does not create: existing, skipped, left out with
    /// problems, or failed.
    pub left: usize,
    /// Rows the run skips because an item already holds their identifier.
    pub existing: usize,
    /// Rows whose files or transaction failed.
    pub failed: usize,
    pub new_sets: Vec<String>,
    pub new_types: Vec<String>,
    pub new_fields: Vec<String>,
    /// The item sets the run created or added items to, in first-use order.
    pub item_set_ids: Vec<u64>,
    /// The titles of the existing item sets the run added items to.
    pub existing_sets: Vec<String>,
    /// Each failed row and why it failed.
    pub failures: Vec<Failure>,
    pub progress: Count,
}

/// A row the run tried to create and could not.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Failure {
    pub identifier: String,
    pub reason: String,
}

#[derive(Debug, thiserror::Error)]
pub enum RunError {
    #[error(transparent)]
    Work(#[from] WorkError),
    #[error("the plan names {0:?}, which is not a property here")]
    UnknownTerm(String),
    #[error("a template labelled {0:?} already exists")]
    TemplateExists(String),
    #[error(transparent)]
    Db(#[from] DbError),
    #[error(transparent)]
    Api(#[from] ApiError),
    #[error("the blocking pool failed: {0}")]
    Blocking(String),
}

/// Run an import. Returns the final summary, which is also stored.
///
/// # Errors
/// [`RunError::Work`], [`RunError::UnknownTerm`] or
/// [`RunError::TemplateExists`] before any write; [`RunError::Db`] or
/// [`RunError::Api`] when the database fails outside one item's transaction.
pub async fn run<F>(
    pool: &SqlitePool,
    store: &FileStore,
    input: RunInput,
    progress: F,
) -> Result<ImportSummary, RunError>
where
    F: FnMut(Progress<'_>) + Send,
{
    let RunInput {
        import_id,
        owner,
        folder,
        plan,
        catalog,
        base_url,
    } = input;
    let work = blocking(move || Work::load(&folder, &plan, &catalog)).await??;
    let mut terms = resolve_terms(pool, &work).await?;
    for new in &work.types {
        if TemplateRepo::new(pool)
            .find_by_label(&new.name)
            .await?
            .is_some()
        {
            return Err(RunError::TemplateExists(new.name.clone()));
        }
    }
    let validation = crate::settings::file_validation(&SettingRepo::new(pool)).await?;
    let mut runner = Runner {
        pool,
        store,
        validation,
        writer: ImportWriteRepo::new(pool, import_id),
        import_id,
        owner,
        summary: ImportSummary {
            left: work.left,
            existing: work.existing,
            progress: Count {
                done: 0,
                total: work.items.len(),
            },
            ..ImportSummary::default()
        },
        progress,
    };
    let sets = runner.create_sets(&work, &terms).await?;
    let types = runner.create_types(&work, &terms).await?;
    runner.create_fields(&work, &base_url, &mut terms).await?;
    let templates = existing_templates(pool, &work).await?;
    let api =
        Api::load_with_store(pool, base_url, super::system_actor(), Some(store.clone())).await?;
    for set in &sets {
        fulltext::reindex(&api, "item_sets", ResourceId(set.0)).await?;
    }
    let made = Made {
        sets,
        existing_sets: work.existing_sets,
        types,
        templates,
        terms,
    };
    for item in work.items {
        runner.item(&api, &made, item).await?;
    }
    Ok(runner.summary)
}

/// What the setup made, for the items to refer to.
struct Made {
    sets: Vec<ItemSetId>,
    existing_sets: Vec<(i64, String)>,
    /// The new templates, by [`TemplateRef::New`] index.
    types: Vec<TemplateUse>,
    /// Each existing template an item uses, by id.
    templates: BTreeMap<i64, TemplateUse>,
    terms: BTreeMap<String, PropertyId>,
}

struct Runner<'a, F> {
    pool: &'a SqlitePool,
    store: &'a FileStore,
    validation: FileValidation,
    writer: ImportWriteRepo<'a>,
    import_id: ImportId,
    owner: Option<UserId>,
    summary: ImportSummary,
    progress: F,
}

impl<F> Runner<'_, F>
where
    F: FnMut(Progress<'_>) + Send,
{
    async fn report(&mut self, line: &str) -> Result<(), RunError> {
        ImportRepo::new(self.pool)
            .set_summary(self.import_id, &summary_json(&self.summary))
            .await?;
        (self.progress)(Progress {
            done: self.summary.progress.done,
            total: self.summary.progress.total,
            line,
        });
        Ok(())
    }

    fn resource(&self, title: Option<String>) -> NewResource {
        NewResource {
            owner_id: self.owner,
            title,
            ..NewResource::default()
        }
    }

    async fn create_sets(
        &mut self,
        work: &Work,
        terms: &BTreeMap<String, PropertyId>,
    ) -> Result<Vec<ItemSetId>, RunError> {
        let title = property(terms, "dcterms:title")?;
        let mut ids = Vec::with_capacity(work.sets.len());
        for name in &work.sets {
            let new = NewItemSet {
                resource: self.resource(Some(name.clone())),
                is_open: false,
            };
            let row = self
                .writer
                .create_item_set(&new, &[ValueContent::literal(title, name.clone())])
                .await?;
            ids.push(row.id());
            self.summary.item_set_ids.push(row.id().0);
            self.summary.new_sets.push(name.clone());
            self.report(&format!("created item set {name:?} ({})", row.id().0))
                .await?;
        }
        Ok(ids)
    }

    async fn create_types(
        &mut self,
        work: &Work,
        terms: &BTreeMap<String, PropertyId>,
    ) -> Result<Vec<TemplateUse>, RunError> {
        let mut made = Vec::with_capacity(work.types.len());
        for new in &work.types {
            let starter = starter(new.starter);
            let mut fields = Vec::with_capacity(starter.fields.len());
            for (position, field) in (1_i64..).zip(starter.fields) {
                fields.push(NewTemplateProperty {
                    resource_template_id: TemplateId(0),
                    property_id: property(terms, field.term)?,
                    alternate_label: Some(field.label.to_string()),
                    alternate_comment: None,
                    position: Some(position),
                    data_type: Some(field.data_types.iter().map(|t| (*t).to_string()).collect()),
                    is_required: field.required,
                    is_private: false,
                    default_lang: None,
                });
            }
            let title = new
                .title
                .as_deref()
                .map(|term| property(terms, term))
                .transpose()?;
            // A vocabulary this install lacks leaves the class off, as the
            // admin does when it lacks the starter's class.
            let class = find_class(self.pool, starter.class_term).await?;
            let template = NewTemplate {
                owner_id: self.owner,
                resource_class_id: class,
                title_property_id: title,
                description_property_id: None,
                label: new.name.clone(),
            };
            let row = self.writer.create_template(&template, &fields).await?;
            made.push(TemplateUse {
                id: row.id,
                class,
                title,
            });
            self.summary.new_types.push(new.name.clone());
            self.report(&format!("created item type {:?} ({})", new.name, row.id.0))
                .await?;
        }
        Ok(made)
    }

    /// New fields whose term does not exist yet, in the `local` vocabulary,
    /// which is made on first use.
    async fn create_fields(
        &mut self,
        work: &Work,
        base_url: &BaseUrl,
        terms: &mut BTreeMap<String, PropertyId>,
    ) -> Result<(), RunError> {
        for field in &work.fields {
            if let Some(id) = find_property(self.pool, &field.term).await? {
                terms.insert(field.term.clone(), id);
                continue;
            }
            let vocabulary = local_vocabulary(self.pool, base_url).await?;
            let local_name = field
                .term
                .strip_prefix("local:")
                .unwrap_or(&field.term)
                .to_string();
            let row = self
                .writer
                .create_property(&NewProperty {
                    owner_id: self.owner,
                    vocabulary_id: vocabulary,
                    local_name,
                    label: field.label.clone(),
                    comment: None,
                })
                .await?;
            terms.insert(field.term.clone(), row.id);
            self.summary.new_fields.push(field.label.clone());
            self.report(&format!("created field {} ({})", field.term, row.id.0))
                .await?;
        }
        Ok(())
    }

    async fn item(&mut self, api: &Api<'_>, made: &Made, item: ItemWork) -> Result<(), RunError> {
        let store = self.store.clone();
        let owner = self.owner;
        let files = item.files.clone();
        let validation = self.validation.clone();
        let stored =
            blocking(move || media_files::store_all(&store, &validation, &files, owner)).await?;
        let line = match stored {
            Ok(media) => self.commit(api, made, &item, media).await?,
            Err(message) => self.fail(&item.identifier, message),
        };
        self.summary.progress.done += 1;
        self.report(&line).await
    }

    /// Count a failed row and keep why, for the left-out rows. The log line.
    fn fail(&mut self, identifier: &str, reason: String) -> String {
        let line = format!("failed {identifier}: {reason}");
        self.summary.failed += 1;
        self.summary.left += 1;
        self.summary.failures.push(Failure {
            identifier: identifier.to_string(),
            reason,
        });
        line
    }

    /// Note the sets a committed item joined.
    fn joined(&mut self, made: &Made, sets: &[SetRef]) {
        for set in sets {
            let (id, existing) = match set {
                SetRef::Existing(id) => (
                    *id as u64,
                    made.existing_sets
                        .iter()
                        .find(|(known, _)| known == id)
                        .map(|(_, name)| name),
                ),
                SetRef::New(index) => (made.sets[*index].0, None),
            };
            if !self.summary.item_set_ids.contains(&id) {
                self.summary.item_set_ids.push(id);
            }
            if let Some(name) = existing
                && !self.summary.existing_sets.contains(name)
            {
                self.summary.existing_sets.push(name.clone());
            }
        }
    }

    /// One transaction for the item and its media; the index and site sync after.
    async fn commit(
        &mut self,
        api: &Api<'_>,
        made: &Made,
        item: &ItemWork,
        media: Vec<omeka_db::rows::NewMedia>,
    ) -> Result<String, RunError> {
        let template = match item.template {
            TemplateRef::None => None,
            TemplateRef::Existing(id) => made.templates.get(&id).copied(),
            TemplateRef::New(index) => Some(made.types[index]),
        };
        let values = literals(&made.terms, &item.values)?;
        let title = title(template, &made.terms, &values);
        let new = NewWholeItem {
            item: NewItem {
                resource: NewResource {
                    resource_template_id: template.map(|t| t.id),
                    resource_class_id: template.and_then(|t| t.class),
                    ..self.resource(title)
                },
                primary_media_id: None,
            },
            item_sets: item
                .sets
                .iter()
                .map(|set| match set {
                    SetRef::Existing(id) => ItemSetId(*id as u64),
                    SetRef::New(index) => made.sets[*index],
                })
                .collect(),
            values,
        };
        let mut entries = Vec::with_capacity(media.len());
        for (row, file) in media.iter().zip(&item.files) {
            entries.push(NewImportMedia {
                media: row.clone(),
                values: literals(&made.terms, &file.values)?,
            });
        }
        let imported = match self.writer.create_item(&new, &entries).await {
            Ok(imported) => imported,
            Err(error) => {
                media_files::discard(self.store, &media);
                return Ok(self.fail(&item.identifier, error.to_string()));
            }
        };
        self.joined(made, &item.sets);
        for row in &imported.media {
            fulltext::reindex(api, "media", row.resource.id).await?;
        }
        fulltext::reindex(api, "items", imported.item.resource.id).await?;
        site_items::attach_new_matches(api, imported.item.id(), &[]).await?;
        let thumbnails = imported.media.iter().filter(|m| m.has_thumbnails).count();
        self.summary.items += 1;
        self.summary.files += imported.media.iter().filter(|m| m.has_original).count();
        self.summary.thumbnails += thumbnails;
        Ok(format!(
            "created {} (item {}, {} files)",
            item.identifier,
            imported.item.id().0,
            imported.media.len()
        ))
    }
}

fn summary_json(summary: &ImportSummary) -> serde_json::Value {
    serde_json::to_value(summary).unwrap_or(serde_json::Value::Null)
}

async fn blocking<T, R>(task: T) -> Result<R, RunError>
where
    T: FnOnce() -> R + Send + 'static,
    R: Send + 'static,
{
    tokio::task::spawn_blocking(task)
        .await
        .map_err(|error| RunError::Blocking(error.to_string()))
}
