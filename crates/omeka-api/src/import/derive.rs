//! The derive: one pure function from the plan and what was read to the model
//! every tab shows and the run writes.
//!
//! The model holds every file with its match, the candidate sets, the joined
//! spreadsheet, the grid columns, and one [`Row`] per item with its cells and
//! problems. Nothing here reads the disk except through the [`TagCache`], and
//! that only for tags a rule names.

use std::collections::{BTreeMap, BTreeSet};

use serde::Serialize;

use super::catalog::{Catalog, TemplateField, starter_fields};
use super::csv::sheet::{Sheet, SheetError};
use super::grouping::{self, MatchedFile, SetEntry};
use super::inputs::Inputs;
use super::pattern::{FilePattern, PatternError};
use super::plan::{ColumnTarget, CsvRowChoice, ImportPlan, NewType, TypeChoice};
use super::problems::{KindKey, ProblemKind, RowProblem, cell_problems};
use super::tags::TagCache;
use super::values::{IDENTIFIER_TERM, Item, Resolved, Sources};

/// A reason the plan cannot run, in the order the preview reports them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Issue {
    Pattern(PatternError),
    Sheet(SheetError),
    MissingTemplate(i64),
    /// The plan writes a term no vocabulary here has.
    UnknownTerm(String),
    /// A new item type's name is already a template's label.
    TypeExists(String),
}

impl Issue {
    #[must_use]
    pub fn key(&self) -> &'static str {
        match self {
            Self::Pattern(error) => error.key(),
            Self::Sheet(error) => error.key(),
            Self::MissingTemplate(_) => "templateMissing",
            Self::UnknownTerm(_) => "termUnknown",
            Self::TypeExists(_) => "typeExists",
        }
    }

    #[must_use]
    pub fn detail(&self) -> Option<String> {
        match self {
            Self::Pattern(error) => error.detail(),
            Self::Sheet(SheetError::Parse(error)) => Some(error.to_string()),
            Self::Sheet(SheetError::JoinColumn(column) | SheetError::Unreadable(column)) => {
                Some(column.clone())
            }
            Self::Sheet(_) => None,
            Self::MissingTemplate(id) => Some(id.to_string()),
            Self::UnknownTerm(term) => Some(term.clone()),
            Self::TypeExists(name) => Some(name.clone()),
        }
    }
}

/// The item type an item gets.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypeSource {
    None,
    Template { id: i64, label: String },
    New(NewType),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemType {
    pub source: TypeSource,
    pub fields: Vec<TemplateField>,
}

impl ItemType {
    #[must_use]
    pub fn name(&self) -> &str {
        match &self.source {
            TypeSource::None => "",
            TypeSource::Template { label, .. } => label,
            TypeSource::New(new) => &new.name,
        }
    }
}

/// One grid column.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Column {
    pub term: String,
    pub label: String,
    /// `type`, `csv:<header>` or `tag:<tag>`, one per source.
    pub sources: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cell {
    pub term: String,
    pub value: Resolved,
    pub problems: Vec<ProblemKind>,
}

/// One item as the grid shows it and the run writes it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    pub item: Item,
    pub item_type: ItemType,
    pub cells: Vec<Cell>,
    /// The values of the unmapped columns, in their order.
    pub extra: Vec<String>,
    pub row_problems: Vec<RowProblem>,
    pub skipped: bool,
    /// The item that already holds this row's identifier. Such a row is
    /// never created, and its problems count nowhere.
    pub existing: Option<i64>,
}

impl Row {
    /// Whether the run may create this row: not skipped and not existing.
    /// Only such rows count their problems.
    #[must_use]
    pub fn open(&self) -> bool {
        !self.skipped && self.existing.is_none()
    }

    #[must_use]
    pub fn problems(&self) -> usize {
        self.cells
            .iter()
            .map(|cell| cell.problems.len())
            .sum::<usize>()
            + self.row_problems.len()
    }

    #[must_use]
    pub fn kinds(&self) -> BTreeSet<KindKey> {
        let cells = self.cells.iter().flat_map(|cell| {
            cell.problems.iter().map(|kind| KindKey {
                kind: *kind,
                term: Some(cell.term.clone()),
            })
        });
        let rows = self.row_problems.iter().map(|problem| KindKey {
            kind: problem.kind(),
            term: None,
        });
        cells.chain(rows).collect()
    }
}

/// How the spreadsheet joined the files.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Join {
    pub joined: usize,
    pub rows_without_file: usize,
    pub items_without_row: usize,
}

#[derive(Debug, Clone)]
pub struct Model {
    pub issues: Vec<Issue>,
    pub group_names: Vec<String>,
    pub files: Vec<MatchedFile>,
    pub sets: Vec<SetEntry>,
    /// Every item the files make, kept or not, numbered by position.
    pub file_items: Vec<grouping::FileItem>,
    pub sheet: Option<Result<Sheet, SheetError>>,
    pub join: Join,
    pub columns: Vec<Column>,
    pub unmapped: Vec<String>,
    pub rows: Vec<Row>,
    /// New local fields: term and label.
    pub new_fields: Vec<(String, String)>,
}

/// Derive the model of `plan` over `inputs`.
#[must_use]
pub fn derive(plan: &ImportPlan, inputs: &Inputs, tags: &TagCache<'_>, catalog: &Catalog) -> Model {
    let pattern = FilePattern::compile(plan.pattern.as_deref());
    let grouping = grouping::group(plan, &inputs.files, pattern.as_ref().ok(), catalog);
    let sheet = match (&plan.csv, &inputs.csv) {
        (Some(csv), Some(Ok(bytes))) => Some(Sheet::read(bytes, csv)),
        (Some(_), Some(Err(message))) => Some(Err(SheetError::Unreadable(message.clone()))),
        _ => None,
    };
    let usable = sheet.as_ref().and_then(|sheet| sheet.as_ref().ok());
    let (items, join) = join_items(&grouping.items, usable);

    let mut issues = Vec::new();
    if let Err(error) = &pattern {
        issues.push(Issue::Pattern(error.clone()));
    }
    if let Some(Err(error)) = &sheet {
        issues.push(Issue::Sheet(error.clone()));
    }
    let types: Vec<ItemType> = items
        .iter()
        .map(|item| item_type(plan, catalog, item, &mut issues))
        .collect();
    let sources = Sources {
        plan,
        sheet: usable,
        files: &grouping.files,
        tags,
    };
    let existing: Vec<Option<i64>> = items
        .iter()
        .map(|item| {
            let identifier = sources.resolve(item, IDENTIFIER_TERM).value;
            catalog.existing.get(identifier.trim()).copied()
        })
        .collect();
    let new_fields = new_fields(plan, usable);
    let columns = columns(plan, catalog, usable, &types, &new_fields);
    let created_types: Vec<&ItemType> = types
        .iter()
        .zip(&existing)
        .filter(|(_, existing)| existing.is_none())
        .map(|(item_type, _)| item_type)
        .collect();
    issues.extend(catalog_issues(
        plan,
        catalog,
        &columns,
        &created_types,
        &new_fields,
    ));
    let unmapped: Vec<String> = usable.map_or_else(Vec::new, |sheet| {
        let mapped = plan.csv.as_ref().map(|csv| &csv.columns);
        sheet
            .head
            .iter()
            .enumerate()
            .filter(|(index, header)| {
                *index != sheet.join && !mapped.is_some_and(|columns| columns.contains_key(*header))
            })
            .map(|(_, header)| header.clone())
            .collect()
    });

    let rows = items
        .into_iter()
        .zip(types)
        .zip(existing)
        .map(|((item, item_type), existing)| {
            let mut row = row(plan, &sources, &columns, &unmapped, item, item_type);
            row.existing = existing;
            row
        })
        .collect();

    Model {
        issues,
        group_names: pattern
            .as_ref()
            .map(|p| p.group_names().to_vec())
            .unwrap_or_default(),
        files: grouping.files,
        sets: grouping.sets,
        file_items: grouping.items,
        sheet,
        join,
        columns,
        unmapped,
        rows,
        new_fields,
    }
}

/// What the run would refuse before its first write: a term no vocabulary
/// has, or a new type named like an existing template. The run writes the
/// identifier, every column, the media tag rules, and a new set's title.
/// `types` are those of the rows that do not exist yet: a type only rows
/// that exist would get is never made.
fn catalog_issues(
    plan: &ImportPlan,
    catalog: &Catalog,
    columns: &[Column],
    types: &[&ItemType],
    new_fields: &[(String, String)],
) -> Vec<Issue> {
    let mut issues = Vec::new();
    let written = [IDENTIFIER_TERM, "dcterms:title"]
        .into_iter()
        .chain(columns.iter().map(|column| column.term.as_str()))
        .chain(
            plan.meta
                .iter()
                .filter(|rule| rule.on.on_media())
                .map(|rule| rule.term.as_str()),
        );
    for term in written {
        let known =
            catalog.labels.contains_key(term) || new_fields.iter().any(|(new, _)| new == term);
        let issue = Issue::UnknownTerm(term.to_string());
        if !known && !issues.contains(&issue) {
            issues.push(issue);
        }
    }
    for item_type in types {
        if let TypeSource::New(new) = &item_type.source {
            let name = new.name.trim();
            let issue = Issue::TypeExists(name.to_string());
            if catalog
                .templates
                .values()
                .any(|template| template.label == name)
                && !issues.contains(&issue)
            {
                issues.push(issue);
            }
        }
    }
    issues
}

/// The kept file items, then one item per spreadsheet row no file matches.
/// The first row of an identifier joins; a later one, or a row with an empty
/// join cell, is not an item.
fn join_items(file_items: &[grouping::FileItem], sheet: Option<&Sheet>) -> (Vec<Item>, Join) {
    let mut items: Vec<Item> = Vec::new();
    let mut at: BTreeMap<&str, Option<usize>> = BTreeMap::new();
    for item in file_items {
        let index = item.kept.then(|| {
            items.push(Item {
                identifier: item.identifier.clone(),
                files: item.files.clone(),
                set_keys: item.set_keys.clone(),
                csv_row: None,
            });
            items.len() - 1
        });
        at.insert(&item.identifier, index);
    }
    let mut join = Join::default();
    let Some(sheet) = sheet else {
        return (items, join);
    };
    let mut seen: BTreeSet<&str> = BTreeSet::new();
    for (index, row) in sheet.rows.iter().enumerate() {
        let key = row.cell(sheet.join).trim();
        if key.is_empty() || !seen.insert(key) {
            continue;
        }
        match at.get(key) {
            Some(Some(item)) => {
                items[*item].csv_row = Some(index);
                join.joined += 1;
            }
            Some(None) => {}
            None => {
                join.rows_without_file += 1;
                items.push(Item {
                    identifier: key.to_string(),
                    files: Vec::new(),
                    set_keys: Vec::new(),
                    csv_row: Some(index),
                });
            }
        }
    }
    join.items_without_row = items
        .iter()
        .filter(|item| !item.files.is_empty() && item.csv_row.is_none())
        .count();
    (items, join)
}

/// The type of the item's first set, else the catalogue's default.
fn item_type(
    plan: &ImportPlan,
    catalog: &Catalog,
    item: &Item,
    issues: &mut Vec<Issue>,
) -> ItemType {
    let choice = item.set_keys.first().and_then(|key| plan.type_for.get(key));
    let id = match choice {
        Some(TypeChoice::New(new)) => {
            return ItemType {
                source: TypeSource::New(new.clone()),
                fields: starter_fields(new.starter),
            };
        }
        Some(TypeChoice::Template(id)) => Some(*id),
        None => catalog.default_template,
    };
    let Some(id) = id else {
        return ItemType {
            source: TypeSource::None,
            fields: Vec::new(),
        };
    };
    match catalog.templates.get(&id) {
        Some(template) => ItemType {
            source: TypeSource::Template {
                id,
                label: template.label.clone(),
            },
            fields: template.fields.clone(),
        },
        None => {
            let issue = Issue::MissingTemplate(id);
            if !issues.contains(&issue) {
                issues.push(issue);
            }
            ItemType {
                source: TypeSource::None,
                fields: Vec::new(),
            }
        }
    }
}

fn new_fields(plan: &ImportPlan, sheet: Option<&Sheet>) -> Vec<(String, String)> {
    let (Some(csv), Some(_)) = (&plan.csv, sheet) else {
        return Vec::new();
    };
    let mut fields: Vec<(String, String)> = Vec::new();
    for target in csv.columns.values() {
        if let ColumnTarget::NewField(field) = target {
            let term = target.term();
            if !fields.iter().any(|(known, _)| *known == term) {
                fields.push((term, field.new_field.trim().to_string()));
            }
        }
    }
    fields
}

/// Type fields first, then mapped spreadsheet columns, then item tag rules.
/// The identifier is the row's own, never a column.
fn columns(
    plan: &ImportPlan,
    catalog: &Catalog,
    sheet: Option<&Sheet>,
    types: &[ItemType],
    new_fields: &[(String, String)],
) -> Vec<Column> {
    let mut columns: Vec<Column> = Vec::new();
    let mut add = |term: String, source: String| {
        if term == IDENTIFIER_TERM || term.is_empty() {
            return;
        }
        let at = match columns.iter().position(|column| column.term == term) {
            Some(at) => at,
            None => {
                let label = catalog
                    .labels
                    .get(&term)
                    .cloned()
                    .or_else(|| {
                        new_fields
                            .iter()
                            .find(|(known, _)| *known == term)
                            .map(|(_, label)| label.clone())
                    })
                    .unwrap_or_else(|| term.clone());
                columns.push(Column {
                    term,
                    label,
                    sources: Vec::new(),
                });
                columns.len() - 1
            }
        };
        if !columns[at].sources.contains(&source) {
            columns[at].sources.push(source);
        }
    };
    for field in types.iter().flat_map(|item_type| &item_type.fields) {
        add(field.term.clone(), "type".to_string());
    }
    if let (Some(sheet), Some(csv)) = (sheet, &plan.csv) {
        for (index, header) in sheet.head.iter().enumerate() {
            if let Some(target) = csv.columns.get(header).filter(|_| index != sheet.join) {
                add(target.term(), format!("csv:{header}"));
            }
        }
    }
    for rule in plan.meta.iter().filter(|rule| rule.on.on_item()) {
        add(rule.term.clone(), format!("tag:{}", rule.tag));
    }
    columns
}

fn row(
    plan: &ImportPlan,
    sources: &Sources<'_>,
    columns: &[Column],
    unmapped: &[String],
    item: Item,
    item_type: ItemType,
) -> Row {
    let required: BTreeSet<&str> = item_type
        .fields
        .iter()
        .filter(|f| f.required)
        .map(|f| f.term.as_str())
        .collect();
    let cells = columns
        .iter()
        .map(|column| {
            let value = sources.resolve(&item, &column.term);
            let problems = cell_problems(
                &column.term,
                &value,
                required.contains(column.term.as_str()),
            );
            Cell {
                term: column.term.clone(),
                value,
                problems,
            }
        })
        .collect();

    let csv_row = sources
        .sheet
        .zip(item.csv_row)
        .map(|(sheet, index)| (sheet, &sheet.rows[index]));
    let mut row_problems = Vec::new();
    if let Some((sheet, csv_row)) = csv_row
        && csv_row.cells.len() != sheet.head.len()
        && !plan.csv_row.contains_key(&item.identifier)
    {
        row_problems.push(RowProblem::Cells {
            line: csv_row.line,
            cells: csv_row.cells.len(),
        });
    }
    if item.files.is_empty() && !plan.without_files.contains(&item.identifier) {
        row_problems.push(RowProblem::Nofile);
    }
    let ignored = plan.csv_row.get(&item.identifier) == Some(&CsvRowChoice::Ignore);
    let extra = unmapped
        .iter()
        .map(|header| match csv_row {
            Some((sheet, csv_row)) if !ignored => sheet
                .head
                .iter()
                .position(|known| known == header)
                .map_or_else(String::new, |index| csv_row.cell(index).to_string()),
            _ => String::new(),
        })
        .collect();
    Row {
        skipped: plan.skip.contains(&item.identifier),
        item,
        item_type,
        cells,
        extra,
        row_problems,
        existing: None,
    }
}
