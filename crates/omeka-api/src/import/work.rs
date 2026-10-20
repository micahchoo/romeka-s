//! What a run writes, decided before the first write.
//!
//! [`Work::load`] reads the folder, derives the same [`Model`] the Check tab
//! reads, and turns the rows [`Model::to_create`] keeps into plain owned data:
//! the new sets, templates and fields, and per item its literals and files with
//! their media values. Every tag a value names is read here, so the writes that
//! follow touch the disk only to store files.

use std::collections::BTreeSet;
use std::path::PathBuf;

use super::archive::{ArchiveError, ArchiveFolder};
use super::catalog::{Catalog, TemplateField, starter_fields, starter_title};
use super::derive::{self, Issue, Model, Row, TypeSource};
use super::inputs::Inputs;
use super::plan::ImportPlan;
use super::tags::{FsTagReader, TagCache, TagReader};
use super::values::{IDENTIFIER_TERM, Sources};

/// One literal value: a term and its text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Literal {
    pub term: String,
    pub value: String,
}

/// An item set an item joins.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SetRef {
    Existing(i64),
    /// An index into [`Work::sets`].
    New(usize),
}

/// The template an item gets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TemplateRef {
    None,
    Existing(i64),
    /// An index into [`Work::types`].
    New(usize),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewTypeWork {
    pub name: String,
    pub fields: Vec<TemplateField>,
    /// The term of its title property.
    pub title: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewFieldWork {
    pub term: String,
    pub label: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileWork {
    /// Relative to the import folder.
    pub path: String,
    pub absolute: PathBuf,
    pub values: Vec<Literal>,
}

impl FileWork {
    /// The media's `source`: the file name, as upstream's upload ingester stores it.
    #[must_use]
    pub fn source(&self) -> &str {
        self.path.rsplit('/').next().unwrap_or(&self.path)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemWork {
    pub identifier: String,
    pub sets: Vec<SetRef>,
    pub template: TemplateRef,
    pub values: Vec<Literal>,
    pub files: Vec<FileWork>,
}

/// Everything a run writes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Work {
    /// New item set titles, in first-use order.
    pub sets: Vec<String>,
    /// The existing item sets items join: id and title, in first-use order.
    pub existing_sets: Vec<(i64, String)>,
    pub types: Vec<NewTypeWork>,
    pub fields: Vec<NewFieldWork>,
    pub items: Vec<ItemWork>,
    /// Rows of the model the run does not create: existing, skipped, or left
    /// out with problems.
    pub left: usize,
    /// Rows whose identifier an item already holds.
    pub existing: usize,
}

/// Why the folder and plan give no work.
#[derive(Debug, thiserror::Error)]
pub enum WorkError {
    #[error(transparent)]
    Archive(#[from] ArchiveError),
    /// The plan cannot run; the key is the preview's `error.key`.
    #[error("the plan cannot run: {}", .0.key())]
    Issue(Issue),
}

impl Work {
    /// Read the folder and decide the writes.
    ///
    /// # Errors
    /// [`WorkError::Archive`] when the folder cannot be walked,
    /// [`WorkError::Issue`] when the plan has an issue the preview would show.
    pub fn load(
        folder: &ArchiveFolder,
        plan: &ImportPlan,
        catalog: &Catalog,
    ) -> Result<Self, WorkError> {
        let inputs = Inputs::load(folder, plan)?;
        Self::from_inputs(plan, &inputs, &FsTagReader, catalog)
    }

    /// [`Self::load`] over inputs already read, with any tag reader.
    ///
    /// # Errors
    /// [`WorkError::Issue`] when the plan has an issue the preview would show.
    pub fn from_inputs(
        plan: &ImportPlan,
        inputs: &Inputs,
        reader: &dyn TagReader,
        catalog: &Catalog,
    ) -> Result<Self, WorkError> {
        let tags = TagCache::new(reader);
        let model = derive::derive(plan, inputs, &tags, catalog);
        if let Some(issue) = model.issues.first() {
            return Err(WorkError::Issue(issue.clone()));
        }
        let sources = Sources {
            plan,
            sheet: model.sheet.as_ref().and_then(|sheet| sheet.as_ref().ok()),
            files: &model.files,
            tags: &tags,
        };
        let create = model.to_create(plan);
        let mut work = Self {
            sets: Vec::new(),
            existing_sets: Vec::new(),
            types: Vec::new(),
            fields: model
                .new_fields
                .iter()
                .map(|(term, label)| NewFieldWork {
                    term: term.clone(),
                    label: label.clone(),
                })
                .collect(),
            items: Vec::with_capacity(create.len()),
            left: model.rows.len() - create.len(),
            existing: model
                .rows
                .iter()
                .filter(|row| row.existing.is_some())
                .count(),
        };
        for row in create {
            let item = work.item(&model, catalog, &sources, row);
            work.items.push(item);
        }
        Ok(work)
    }

    fn item(
        &mut self,
        model: &Model,
        catalog: &Catalog,
        sources: &Sources<'_>,
        row: &Row,
    ) -> ItemWork {
        let identifier = sources.resolve(&row.item, IDENTIFIER_TERM).value;
        let mut values = vec![Literal {
            term: IDENTIFIER_TERM.to_string(),
            value: identifier,
        }];
        values.extend(row.cells.iter().flat_map(|cell| {
            cell.value
                .written()
                .into_iter()
                .filter(|value| !value.trim().is_empty())
                .map(|value| Literal {
                    term: cell.term.clone(),
                    value: value.to_string(),
                })
        }));
        let files = row
            .item
            .files
            .iter()
            .map(|&index| {
                let file = &model.files[index];
                FileWork {
                    path: file.file.path.clone(),
                    absolute: file.file.absolute.clone(),
                    values: sources
                        .media_values(file)
                        .into_iter()
                        .map(|(term, value)| Literal { term, value })
                        .collect(),
                }
            })
            .collect();
        let sets = row
            .item
            .set_keys
            .iter()
            .filter_map(|key| model.sets.iter().find(|set| set.key == *key))
            .map(|set| self.set(catalog, set.name.trim()))
            .collect::<Vec<_>>();
        ItemWork {
            identifier: row.item.identifier.clone(),
            sets: dedupe(sets),
            template: self.template(&row.item_type.source),
            values,
            files,
        }
    }

    fn set(&mut self, catalog: &Catalog, name: &str) -> SetRef {
        if let Some(&id) = catalog.item_sets.get(name) {
            if !self.existing_sets.iter().any(|(known, _)| *known == id) {
                self.existing_sets.push((id, name.to_string()));
            }
            return SetRef::Existing(id);
        }
        let at = self.sets.iter().position(|known| known == name);
        SetRef::New(at.unwrap_or_else(|| {
            self.sets.push(name.to_string());
            self.sets.len() - 1
        }))
    }

    fn template(&mut self, source: &TypeSource) -> TemplateRef {
        match source {
            TypeSource::None => TemplateRef::None,
            TypeSource::Template { id, .. } => TemplateRef::Existing(*id),
            TypeSource::New(new) => {
                let name = new.name.trim();
                let at = self.types.iter().position(|known| known.name == name);
                TemplateRef::New(at.unwrap_or_else(|| {
                    self.types.push(NewTypeWork {
                        name: name.to_string(),
                        fields: starter_fields(new.starter),
                        title: starter_title(new.starter),
                    });
                    self.types.len() - 1
                }))
            }
        }
    }

    /// Every term the run writes or lists on a new template, new fields left out.
    #[must_use]
    pub fn existing_terms(&self) -> BTreeSet<&str> {
        let new: BTreeSet<&str> = self.fields.iter().map(|f| f.term.as_str()).collect();
        let items = self.items.iter().flat_map(|item| {
            item.values
                .iter()
                .chain(item.files.iter().flat_map(|file| &file.values))
                .map(|literal| literal.term.as_str())
        });
        let types = self
            .types
            .iter()
            .flat_map(|t| t.fields.iter().map(|f| f.term.as_str()));
        items
            .chain(types)
            .filter(|term| !new.contains(term))
            .collect()
    }

    /// The files the run stores.
    #[must_use]
    pub fn file_count(&self) -> usize {
        self.items.iter().map(|item| item.files.len()).sum()
    }
}

fn dedupe(sets: Vec<SetRef>) -> Vec<SetRef> {
    let mut out: Vec<SetRef> = Vec::with_capacity(sets.len());
    for set in sets {
        if !out.contains(&set) {
            out.push(set);
        }
    }
    out
}
