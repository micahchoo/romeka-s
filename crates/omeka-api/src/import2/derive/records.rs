//! Step 4 of the derive: records from file groups and rows, after the file
//! flags that can leave a file out (`name-encoding`, `not-media`,
//! `format-mismatch`).

use super::super::catalog::{Catalog, extension};
use super::super::decisions::EntryRef;
use super::super::flags::{FlagBook, expected_types};
use super::super::grouping::Grouping;
use super::super::join::{Joined, RowAt, SheetIn};
use super::super::model::{LeftBy, Origin, RecordId, Subject};
use super::super::plan::{FlagKind, FlagPolicy, RecordKind};

/// A record before its slots are resolved.
pub(crate) struct Draft {
    pub kind: RecordKind,
    pub origin: Origin,
    /// An item's kept files, a media record's file: indices into the files.
    pub files: Vec<usize>,
    pub rows: Vec<RowAt>,
    pub parent: Option<RecordId>,
    pub media: Vec<RecordId>,
    pub left: Option<LeftBy>,
    /// The `dcterms:identifier` an item has when no rule gives one.
    pub identifier: Option<String>,
}

impl Draft {
    fn new(kind: RecordKind, origin: Origin) -> Self {
        Self {
            kind,
            origin,
            files: Vec::new(),
            rows: Vec::new(),
            parent: None,
            media: Vec::new(),
            left: None,
            identifier: None,
        }
    }

    /// The entries the record holds: what its flag decisions are keyed by.
    pub(crate) fn entries(&self, grouping: &Grouping<'_>, sheets: &[SheetIn<'_>]) -> Vec<EntryRef> {
        self.files
            .iter()
            .map(|file| EntryRef::File(grouping.files[*file].file.clone()))
            .chain(self.rows.iter().map(|row| EntryRef::Row(row.entry(sheets))))
            .collect()
    }
}

pub(crate) struct Made {
    pub drafts: Vec<Draft>,
    /// Per file: why it is out of every record.
    pub file_left: Vec<Option<LeftBy>>,
    /// Per file: the media record it is in.
    pub file_record: Vec<Option<RecordId>>,
}

/// Raise each file's flags; a `leave` policy takes the file out.
fn file_flags(
    grouping: &Grouping<'_>,
    catalog: &Catalog,
    book: &mut FlagBook<'_>,
) -> Vec<Option<LeftBy>> {
    let mut left: Vec<Option<LeftBy>> = grouping
        .files
        .iter()
        .enumerate()
        .map(|(at, _)| {
            grouping
                .moved_out
                .get(&at)
                .map(|decision| LeftBy::Decision(*decision))
        })
        .collect();
    for (at, file) in grouping.files.iter().enumerate() {
        let entry = [EntryRef::File(file.file.clone())];
        let mut found = Vec::new();
        if let Some(decoded) = &file.entry.decoded {
            found.push((FlagKind::NameEncoding, format!("{:?}", decoded.encoding)));
        }
        let name = file.entry.name();
        if let Some(refused) = catalog.files.refusal(name, &file.entry.sniffed) {
            found.push((FlagKind::NotMedia, refused));
        }
        let mismatch = extension(name).and_then(|extension| {
            let expected = expected_types(&extension)?;
            let sniffed = file.entry.sniffed.as_str();
            (sniffed != "application/octet-stream" && !expected.contains(&sniffed))
                .then(|| format!(".{extension} {sniffed}"))
        });
        if let Some(detail) = mismatch {
            found.push((FlagKind::FormatMismatch, detail));
        }
        for (kind, detail) in found {
            let policy = book.raise(
                kind,
                Subject::Entry(entry[0].clone()),
                None,
                Some(detail),
                &entry,
            );
            if policy == Some(FlagPolicy::Leave) && left[at].is_none() {
                left[at] = Some(LeftBy::Flag(kind));
            }
        }
    }
    left
}

pub(crate) fn make(
    grouping: &Grouping<'_>,
    joined: &Joined,
    sheets: &[SheetIn<'_>],
    catalog: &Catalog,
    book: &mut FlagBook<'_>,
) -> Made {
    let file_left = file_flags(grouping, catalog, book);
    let mut file_record = vec![None; grouping.files.len()];
    let mut drafts: Vec<Draft> = Vec::new();
    let next = |drafts: &Vec<Draft>| RecordId(u32::try_from(drafts.len()).unwrap_or(u32::MAX));

    for at in &joined.set_rows {
        let mut draft = Draft::new(RecordKind::ItemSet, Origin::Row(at.entry(sheets)));
        draft.rows.push(*at);
        drafts.push(draft);
    }

    for (key, files) in &grouping.groups {
        let rows = joined.group_rows.get(key).cloned().unwrap_or_default();
        let kept: Vec<usize> = files
            .iter()
            .copied()
            .filter(|file| file_left[*file].is_none())
            .collect();
        if kept.is_empty() && rows.is_empty() {
            continue;
        }
        let item = next(&drafts);
        let mut draft = Draft::new(RecordKind::Item, Origin::Group(key.clone()));
        draft.identifier = Some(key.text().to_string());
        draft.rows = rows;
        draft.files.clone_from(&kept);
        drafts.push(draft);
        if kept.is_empty() {
            no_file(&mut drafts[item.index()], grouping, sheets, item, book);
        }
        for file in kept {
            let media = next(&drafts);
            let mut draft = Draft::new(
                RecordKind::Media,
                Origin::File(grouping.files[file].file.clone()),
            );
            draft.files.push(file);
            draft.parent = Some(item);
            drafts.push(draft);
            drafts[item.index()].media.push(media);
            file_record[file] = Some(media);
        }
    }

    for at in &joined.lone_items {
        let item = next(&drafts);
        let mut draft = Draft::new(RecordKind::Item, Origin::Row(at.entry(sheets)));
        draft.rows.push(*at);
        let plan = sheets[at.sheet].plan;
        let key_column = plan
            .join
            .as_ref()
            .map(|join| &join.column)
            .or(plan.item_key.as_ref());
        draft.identifier = key_column
            .map(|column| {
                super::super::join::cell(at.row(sheets), column.index)
                    .trim()
                    .to_string()
            })
            .filter(|text| !text.is_empty());
        drafts.push(draft);
        no_file(&mut drafts[item.index()], grouping, sheets, item, book);
    }

    for media_row in &joined.media_rows {
        if let Some(file) = media_row.file
            && let Some(media) = file_record[file]
        {
            drafts[media.index()].rows.push(media_row.at);
            continue;
        }
        let id = next(&drafts);
        let mut draft = Draft::new(RecordKind::Media, Origin::Row(media_row.at.entry(sheets)));
        draft.rows.push(media_row.at);
        if let Some(file) = media_row.file {
            draft.files.push(file);
            draft.left = file_left[file];
            file_record[file] = Some(id);
        }
        drafts.push(draft);
    }
    Made {
        drafts,
        file_left,
        file_record,
    }
}

fn no_file(
    draft: &mut Draft,
    grouping: &Grouping<'_>,
    sheets: &[SheetIn<'_>],
    id: RecordId,
    book: &mut FlagBook<'_>,
) {
    let entries = draft.entries(grouping, sheets);
    if book.raise(FlagKind::NoFile, Subject::Record(id), None, None, &entries)
        == Some(FlagPolicy::Leave)
    {
        draft.left = Some(LeftBy::Flag(FlagKind::NoFile));
    }
}
