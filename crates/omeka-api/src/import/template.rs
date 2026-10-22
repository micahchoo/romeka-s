//! The list round trip (`docs/import-v2.md`, "The list round trip"): the
//! list a person downloads to fill in, and the plan change that reads it
//! back once they upload it.
//!
//! [`template_csv`] writes one row per item the run writes whose files the
//! pattern or a decision grouped, in record order. Its `Identifier` is the
//! group key, the text a sheet's identifier join compares, so a filled row
//! finds its item exactly. An item a list row made with no file has no
//! group key to join on: a second list naming it would make a second item,
//! so such items are not in the template.
//!
//! [`add_list`] reads the upload's headers: a header that ends in a
//! bracketed term or slot (`Title [dcterms:title]`) maps that column, with
//! `|` as its split, ahead of every other source of the slot. Any other
//! header stays unmapped. [`put_list`] puts a list the series already has
//! in its old place instead.

use serde::Serialize;

use super::catalog::Catalog;
use super::model::{Model, Origin, Record};
use super::plan::{
    Auto, ColumnOptions, ColumnPlan, ColumnRef, Combine, Delimiter, EachSet, EachValue,
    EncodingChoice, ImportPlan, Join, JoinNormalize, JoinOn, PlanVersion, Quote, ReadOptions,
    RecordKind, RelPath, RowKind, SeriesIx, SetNames, SetSource, SheetId, SheetPlan, SlotName,
    SlotRule, SourceId, SourcePlan, SourceRef, Term,
};
use super::sheet::Sheet;
use super::values::{ValueType, Visibility};

/// The column that names each row's item.
pub const IDENTIFIER: &str = "Identifier";
/// The column that lists each item's file names.
pub const FILES: &str = "Files";
/// What separates two values, or two file names, in one cell.
pub const SPLIT: &str = "|";
/// The largest list a person may upload.
pub const MAX_LIST_BYTES: usize = 50 * 1024 * 1024;
/// The id a plan's staging source gets when the upload adds it.
pub const STAGING: &str = "staging";

/// A column the person may choose for the list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ListColumn {
    Property(Term),
    Sets,
    Template,
    Visibility,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ListError {
    /// A column that is no property of the install and no list slot.
    #[error("the list cannot have the column {0:?}")]
    UnknownColumn(String),
    /// An upload with no `Identifier` header.
    #[error("the list has no Identifier column")]
    NoIdentifier,
    /// A plan of several series, and no `?series=` to say which.
    #[error("the plan has several series and the list names none")]
    SeriesNeeded,
    /// A `?series=` that names no series of the plan.
    #[error("the plan has no series {0:?}")]
    UnknownSeries(String),
}

/// The series a list is for: the one `named`, which `?series=` gives, or
/// the plan's only series when it gives none.
///
/// # Errors
/// [`ListError::UnknownSeries`] for a name no series has, and
/// [`ListError::SeriesNeeded`] for no name in a plan of several series.
pub fn list_series(plan: &ImportPlan, named: Option<&str>) -> Result<SeriesIx, ListError> {
    let mut all = plan.structure.all_series();
    match named {
        Some(named) => all
            .find(|(_, series)| series.id().0 == named)
            .map(|(at, _)| at)
            .ok_or_else(|| ListError::UnknownSeries(named.to_string())),
        None => match (all.next(), all.next()) {
            (Some((only, _)), None) => Ok(only),
            _ => Err(ListError::SeriesNeeded),
        },
    }
}

impl ListColumn {
    /// `dcterms:title`, `sets`, `template` or `visibility`.
    ///
    /// # Errors
    /// [`ListError::UnknownColumn`] for a term the install does not know or
    /// any other slot.
    pub fn parse(text: &str, catalog: &Catalog) -> Result<Self, ListError> {
        let unknown = || ListError::UnknownColumn(text.to_string());
        match SlotName::parse(text.trim()).map_err(|_| unknown())? {
            SlotName::Sets => Ok(Self::Sets),
            SlotName::Template => Ok(Self::Template),
            SlotName::Visibility => Ok(Self::Visibility),
            SlotName::Term(term) if catalog.properties.contains_key(&term) => {
                Ok(Self::Property(term))
            }
            _ => Err(unknown()),
        }
    }

    fn slot(&self) -> SlotName {
        match self {
            Self::Property(term) => SlotName::Term(term.clone()),
            Self::Sets => SlotName::Sets,
            Self::Template => SlotName::Template,
            Self::Visibility => SlotName::Visibility,
        }
    }

    /// `Label [prefix:local]`, or the slot's name in plain words.
    fn header(&self, catalog: &Catalog) -> String {
        let label = match self {
            Self::Property(term) => catalog
                .properties
                .get(term)
                .map_or_else(|| term.to_string(), Clone::clone),
            Self::Sets => "Item sets".to_string(),
            Self::Template => "Item type".to_string(),
            Self::Visibility => "Visibility".to_string(),
        };
        format!("{label} [{}]", self.slot().as_str())
    }

    fn cell(&self, record: &Record) -> String {
        match self {
            Self::Visibility => match record.resolved.visibility {
                Visibility::Public => "public".to_string(),
                Visibility::Private => "private".to_string(),
            },
            other => record
                .slots
                .get(&other.slot())
                .map(|values| {
                    values
                        .iter()
                        .map(|value| value.value.text.as_str())
                        .collect::<Vec<_>>()
                        .join(SPLIT)
                })
                .unwrap_or_default(),
        }
    }
}

/// The list of one series' items as CSV: a UTF-8 byte order mark, so a
/// spreadsheet reads Devanagari right, then the header and one CRLF-ended
/// row per item.
#[must_use]
pub fn template_csv(
    model: &Model,
    catalog: &Catalog,
    columns: &[ListColumn],
    series: SeriesIx,
) -> Vec<u8> {
    let mut out = String::from('\u{feff}');
    let mut header = vec![IDENTIFIER.to_string(), FILES.to_string()];
    header.extend(columns.iter().map(|column| column.header(catalog)));
    push_line(&mut out, &header);
    for record in model.kept() {
        let Origin::Group(key) = &record.origin else {
            continue;
        };
        if record.kind != RecordKind::Item || record.series != series {
            continue;
        }
        let names: Vec<&str> = record
            .files
            .iter()
            .map(|file| {
                let path = file.path.as_str();
                path.rsplit_once('/').map_or(path, |(_, name)| name)
            })
            .collect();
        let mut line = vec![guard(key.text()), guard(&names.join(SPLIT))];
        line.extend(columns.iter().map(|column| guard(&column.cell(record))));
        push_line(&mut out, &line);
    }
    out.into_bytes()
}

/// Whether a spreadsheet reads a cell starting with `c` as a formula.
fn runs_as_formula(c: char) -> bool {
    matches!(c, '=' | '+' | '-' | '@' | '\t' | '\r')
}

/// The cell as the list writes it: a leading `'` keeps a spreadsheet from
/// running the cell as a formula. A cell that already starts with `'` and
/// then a formula character gets one more, so [`unescape_list`] gives back
/// exactly the value that went out.
pub(crate) fn guard(cell: &str) -> String {
    if hides_formula(cell) {
        format!("'{cell}")
    } else {
        cell.to_string()
    }
}

/// The cell as the person meant it: the one `'` [`guard`] put in front of a
/// formula character, taken off.
pub(crate) fn unguard(cell: &str) -> &str {
    if cell.starts_with('\'') && hides_formula(cell) {
        &cell[1..]
    } else {
        cell
    }
}

/// Whether the cell is a formula character behind zero or more quotes.
fn hides_formula(cell: &str) -> bool {
    cell.trim_start_matches('\'').starts_with(runs_as_formula)
}

/// Take the `'` that [`guard`] put in front of a formula character off the
/// cells of the `Identifier` column and every bracketed column of an
/// uploaded list, then key the rows by `key` again, so a row keyed by its
/// identifier is keyed by the value that went out. `Files` is never read
/// back, so it is left as it came.
pub fn unescape_list(sheet: &mut Sheet, key: Option<&ColumnRef>) {
    let columns: Vec<usize> = sheet
        .columns
        .iter()
        .filter(|column| {
            column.header.eq_ignore_ascii_case(IDENTIFIER) || bracketed(&column.header).is_some()
        })
        .map(|column| column.index as usize)
        .collect();
    for row in &mut sheet.rows {
        for &at in &columns {
            let Some(cell) = row.cells.get_mut(at) else {
                continue;
            };
            if unguard(cell).len() < cell.len() {
                cell.remove(0);
            }
        }
    }
    if let Some(key) = key {
        super::sheet::key_rows(&mut sheet.rows, key);
    }
}

pub(crate) fn push_line(out: &mut String, cells: &[String]) {
    for (at, cell) in cells.iter().enumerate() {
        if at > 0 {
            out.push(',');
        }
        if cell.contains([',', '"', '\r', '\n']) {
            out.push('"');
            out.push_str(&cell.replace('"', "\"\""));
            out.push('"');
        } else {
            out.push_str(cell);
        }
    }
    out.push_str("\r\n");
}

/// How an uploaded list is read: the encoding guessed (a spreadsheet may
/// save it as Windows-1252), a comma or a tab, the first line the header.
#[must_use]
pub fn list_read(tab: bool) -> ReadOptions {
    ReadOptions {
        encoding: EncodingChoice::Auto(Auto::Auto),
        delimiter: if tab {
            Delimiter::Tab
        } else {
            Delimiter::Comma
        },
        quote: Quote::Double,
        header_line: std::num::NonZeroU32::MIN,
        trim: true,
        sheet_name: None,
    }
}

/// The term or slot a header names in brackets at its end.
#[must_use]
pub fn bracketed(header: &str) -> Option<&str> {
    let inner = header.trim_end().strip_suffix(']')?;
    let open = inner.rfind('[')?;
    Some(inner[open + 1..].trim())
}

/// What [`add_list`] added.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ListAdded {
    pub sheet: SheetId,
    /// The columns mapped, by index.
    pub mapped: Vec<u32>,
}

/// Add the list stored at `file` in the plan's staging source to the
/// series `series` of `plan`: a sheet joined on `Identifier`, each
/// bracketed column mapped first in its slot. A staging source is added
/// when the plan has none.
///
/// The `sets` columns of a v2 plan join the free set read as v2 read sets,
/// first, so the plan keeps one sets rule. In a v3 plan they make one more
/// free set whose values are titles; a column of this list gives nothing
/// to an item of another series.
///
/// # Errors
/// [`ListError::NoIdentifier`] when no header is `Identifier`.
pub fn add_list(
    plan: &mut ImportPlan,
    series: SeriesIx,
    file: RelPath,
    read: ReadOptions,
    sheet: &Sheet,
    catalog: &Catalog,
) -> Result<ListAdded, ListError> {
    let identifier = sheet
        .columns
        .iter()
        .find(|column| column.header.eq_ignore_ascii_case(IDENTIFIER))
        .ok_or(ListError::NoIdentifier)?;
    let source = staging_source(plan);
    let id = (1_u32..)
        .map(|n| SheetId(format!("list-{n}")))
        .find(|id| plan.structure.lists().all(|(_, sheet)| sheet.id != *id))
        .unwrap_or_else(|| unreachable!());
    let mut columns = Vec::new();
    let mut mapped = Vec::new();
    let mut set_columns = Vec::new();
    for column in sheet.columns.iter().filter(|column| column.mappable) {
        let Some(slot) = bracketed(&column.header)
            .and_then(|text| ListColumn::parse(text, catalog).ok())
            .map(|column| column.slot())
        else {
            continue;
        };
        let column_ref = ColumnRef {
            index: column.index,
            header: column.header.clone(),
        };
        columns.push(ColumnPlan {
            column: column_ref.clone(),
            options: ColumnOptions {
                split: Some(SPLIT.to_string()),
                value_type: ValueType::Literal,
                lang: None,
                private: false,
            },
        });
        let from = SourceRef::Column {
            sheet: id.clone(),
            column: column_ref,
        };
        if slot == SlotName::Sets {
            match plan.version {
                PlanVersion::V2 => plan.structure.text_sets_mut().sources.insert(0, from),
                PlanVersion::V3 => set_columns.push(from),
            }
            mapped.push(column.index);
            continue;
        }
        let rules = plan.structure.rules_mut(series, RecordKind::Item);
        match rules.get_mut(&slot) {
            Some(rule) => rule.sources.insert(0, from),
            None => {
                rules.insert(
                    slot,
                    SlotRule {
                        sources: vec![from],
                        combine: Combine::First,
                        map: indexmap::IndexMap::new(),
                    },
                );
            }
        }
        mapped.push(column.index);
    }
    if !set_columns.is_empty() {
        plan.structure.push_free_set(SetSource::Each(EachValue {
            each: EachSet {
                sources: set_columns,
                combine: Combine::First,
                map: indexmap::IndexMap::new(),
                names: SetNames::Title,
            },
        }));
    }
    plan.structure.push_list(
        series,
        SheetPlan {
            id: id.clone(),
            source,
            file,
            hash: Some(sheet.hash.clone()),
            read,
            row_kind: RowKind::All(RecordKind::Item),
            join: Some(Join {
                on: JoinOn::Identifier,
                column: ColumnRef {
                    index: identifier.index,
                    header: identifier.header.clone(),
                },
                normalize: JoinNormalize {
                    case_fold: false,
                    leading_zeros: false,
                    separators: false,
                    last_places: None,
                },
            }),
            item_key: None,
            columns,
        },
    );
    Ok(ListAdded { sheet: id, mapped })
}

/// What [`put_list`] did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ListPut {
    /// A new list of the series.
    Added(ListAdded),
    /// The new bytes of a list the series already had. The list keeps its
    /// id, its columns, and the rules and free set that read it; `old` is
    /// the staged file it no longer reads.
    Replaced { sheet: SheetId, old: RelPath },
}

/// The lists of `series` that the plan's staging source holds: the lists
/// a person uploaded.
#[must_use]
pub fn staged_lists(plan: &ImportPlan, series: SeriesIx) -> Vec<&SheetPlan> {
    let staging = plan.sources.iter().find_map(|source| match source {
        SourcePlan::Staging { id } => Some(id),
        SourcePlan::Folder { .. } => None,
    });
    plan.structure
        .series(series)
        .lists()
        .iter()
        .filter(|list| Some(&list.source) == staging)
        .collect()
}

/// Put the list stored at `file` into series `series`. `staged` holds the
/// series' [`staged_lists`] as they read now. One whose header row is the
/// upload's is the same list sent again: both would join the same items on
/// the same columns, and the older one could only give back values the
/// person has since removed. So the upload takes that list's place, and
/// keeps every choice made on it. Any other upload is added by
/// [`add_list`].
///
/// # Errors
/// As [`add_list`].
pub fn put_list(
    plan: &mut ImportPlan,
    series: SeriesIx,
    file: RelPath,
    read: ReadOptions,
    sheet: &Sheet,
    catalog: &Catalog,
    staged: &[(SheetId, Sheet)],
) -> Result<ListPut, ListError> {
    let headers = |sheet: &Sheet| {
        sheet
            .columns
            .iter()
            .map(|column| (column.index, column.header.clone()))
            .collect::<Vec<_>>()
    };
    let same = staged
        .iter()
        .find(|(_, old)| headers(old) == headers(sheet))
        .and_then(|(id, _)| plan.structure.list_mut(id));
    let Some(list) = same else {
        return add_list(plan, series, file, read, sheet, catalog).map(ListPut::Added);
    };
    let old = std::mem::replace(&mut list.file, file);
    list.hash = Some(sheet.hash.clone());
    list.read = read;
    Ok(ListPut::Replaced {
        sheet: list.id.clone(),
        old,
    })
}

fn staging_source(plan: &mut ImportPlan) -> SourceId {
    if let Some(found) = plan
        .sources
        .iter()
        .find(|source| matches!(source, SourcePlan::Staging { .. }))
    {
        return found.id().clone();
    }
    let id = (1_u32..)
        .map(|n| match n {
            1 => SourceId(STAGING.to_string()),
            n => SourceId(format!("{STAGING}-{n}")),
        })
        .find(|id| plan.sources.iter().all(|source| source.id() != id))
        .unwrap_or_else(|| unreachable!());
    plan.sources.push(SourcePlan::Staging { id: id.clone() });
    id
}

#[cfg(test)]
mod tests;
