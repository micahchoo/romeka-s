//! The `unpaired` section: list rows whose item has no file, and files whose
//! item no list row joined, each sorted by name and paged, so the person
//! pairs them by hand (`docs/import-v2.md`, "Unpaired lists"). The server
//! never proposes a pair: names that look alike are not a fact.

use serde::{Deserialize, Serialize};

use super::query::{Page, PageQuery, page};
use crate::import::decisions::{EntryRef, FileRef, RowRef};
use crate::import::model::{Model, Parent, Record, RecordId};
use crate::import::plan::{RecordKind, SlotName, Term};
use crate::import::values::fold_title;

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UnpairedQuery {
    #[serde(default)]
    pub rows: Option<PageQuery>,
    #[serde(default)]
    pub files: Option<PageQuery>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Unpaired {
    /// By row key.
    pub rows: Page<UnpairedRow>,
    /// By file name, then path.
    pub files: Page<UnpairedFile>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UnpairedRow {
    pub row: RowRef,
    /// The item the row made.
    pub record: RecordId,
    /// The item's first title, as the row gave it.
    pub title: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UnpairedFile {
    pub file: FileRef,
    /// The item the file is in.
    pub record: RecordId,
}

/// The list rows whose item has no file, with that item, in entry order.
pub(super) fn unpaired_rows(model: &Model) -> impl Iterator<Item = (&RowRef, &Record)> {
    model.entries.iter().filter_map(|state| {
        let EntryRef::Row(row) = &state.entry else {
            return None;
        };
        let record = model.record(state.record?);
        (record.kind == RecordKind::Item && record.files.is_empty()).then_some((row, record))
    })
}

pub(super) fn unpaired(model: &Model, query: &UnpairedQuery) -> Unpaired {
    let title = SlotName::Term(Term::parse("dcterms:title").unwrap_or_else(|_| unreachable!()));
    let rows_query = query.rows.clone().unwrap_or_default();
    let rows_text = rows_query.text.as_deref().map(fold_title);
    let mut rows: Vec<(String, UnpairedRow)> = unpaired_rows(model)
        .map(|(row, record)| {
            (
                fold_title(row.key.as_str()),
                UnpairedRow {
                    row: row.clone(),
                    record: record.id,
                    title: record
                        .slots
                        .get(&title)
                        .and_then(|values| values.first())
                        .map(|value| value.value.text.clone()),
                },
            )
        })
        .filter(|(key, row)| {
            rows_text.as_ref().is_none_or(|text| {
                key.contains(text.as_str())
                    || row
                        .title
                        .as_deref()
                        .is_some_and(|title| fold_title(title).contains(text.as_str()))
            })
        })
        .collect();
    rows.sort_by(|(a_key, a), (b_key, b)| a_key.cmp(b_key).then_with(|| a.row.cmp(&b.row)));

    let files_query = query.files.clone().unwrap_or_default();
    let files_text = files_query.text.as_deref().map(fold_title);
    let mut files: Vec<(String, UnpairedFile)> = model
        .entries
        .iter()
        .filter_map(|state| {
            let EntryRef::File(file) = &state.entry else {
                return None;
            };
            let media = model.record(state.record?);
            let Some(Parent::Record(item)) = media.parent else {
                return None;
            };
            let name = file
                .path
                .as_str()
                .rsplit_once('/')
                .map_or(file.path.as_str(), |(_, name)| name);
            model.record(item).rows.is_empty().then(|| {
                (
                    fold_title(name),
                    UnpairedFile {
                        file: file.clone(),
                        record: item,
                    },
                )
            })
        })
        .filter(|(_, file)| {
            files_text
                .as_ref()
                .is_none_or(|text| fold_title(file.file.path.as_str()).contains(text.as_str()))
        })
        .collect();
    files.sort_by(|(a_name, a), (b_name, b)| a_name.cmp(b_name).then_with(|| a.file.cmp(&b.file)));

    Unpaired {
        rows: page(
            rows.into_iter(),
            rows_query.page,
            rows_query.per_page,
            |(_, row)| row,
        ),
        files: page(
            files.into_iter(),
            files_query.page,
            files_query.per_page,
            |(_, file)| file,
        ),
    }
}
