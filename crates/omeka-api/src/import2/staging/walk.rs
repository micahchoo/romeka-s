//! The walk of a staging area, through the store (`docs/import-v2.md`,
//! "Evidence"): the same [`Walk`] a folder gives, read from the upload rows
//! and the bytes they name.
//!
//! An upload still in progress is not an entry yet. A dot name counts as a
//! dot entry, as in a folder. A file a ZIP named in cp437 carries its decoded
//! name, so it flags `name-encoding`; one renamed for a capitals clash is
//! listed among the collisions; what a ZIP left out is counted from its
//! [`super::Refused`] record.

use std::collections::{BTreeMap, BTreeSet};

use omeka_db::ids::ImportId;
use omeka_db::rows::UploadRow;
use omeka_media::store::Store;

use super::{Meta, Refused, add_refused, entry, file_path, identical, meta_folder, meta_path};
use crate::import2::evidence::{SNIFF_BYTES, Walk};
use crate::import2::plan::RelPath;

/// Every complete upload of `import`, as entries and sheets.
#[must_use]
pub fn walk(store: &dyn Store, import: ImportId, uploads: &[UploadRow]) -> Walk {
    let mut walk = Walk::default();
    let records: BTreeSet<String> = store
        .list(&meta_folder(import))
        .map(|files| files.into_iter().map(|file| file.storage_path).collect())
        .unwrap_or_default();
    for record in &records {
        if record
            .rsplit('/')
            .next()
            .is_some_and(|name| name.starts_with("zip-"))
        {
            let refused: Refused = read_json(store, record).unwrap_or_default();
            add_refused(&mut walk.anomalies, refused);
        }
    }
    let mut complete: Vec<&UploadRow> = uploads.iter().filter(|row| row.is_complete()).collect();
    complete.sort_by(|a, b| a.path.cmp(&b.path).then(a.created.cmp(&b.created)));
    let mut stored = BTreeMap::new();
    let mut last: Option<&str> = None;
    for upload in complete {
        if last == Some(upload.path.as_str()) {
            walk.anomalies.collisions.push(upload.path.clone());
            continue;
        }
        last = Some(upload.path.as_str());
        let Ok(path) = RelPath::parse(&upload.path) else {
            walk.anomalies.unusable_names.push(upload.path.clone());
            continue;
        };
        if path
            .as_str()
            .split('/')
            .any(|segment| segment.starts_with('.'))
        {
            walk.anomalies.dot += 1;
            continue;
        }
        let bytes = file_path(import, &upload.id);
        let Ok(head) = store.read_range(&bytes, 0..SNIFF_BYTES as u64) else {
            walk.anomalies.unreadable.push(upload.path.clone());
            continue;
        };
        let record = meta_path(import, &upload.id);
        let meta: Option<Meta> = records
            .contains(&record)
            .then(|| read_json(store, &record))
            .flatten();
        if let Some(from) = meta.as_ref().and_then(|meta| meta.renamed_from.as_ref()) {
            walk.anomalies
                .collisions
                .push(format!("{from} -> {}", upload.path));
        }
        let Some((found, is_sheet)) =
            entry(path, upload.length, upload.created, &head, meta.as_ref())
        else {
            walk.anomalies.unreadable.push(upload.path.clone());
            continue;
        };
        stored.insert(upload.path.clone(), bytes);
        if is_sheet {
            walk.sheets.push(found);
        } else {
            walk.entries.push(found);
        }
    }
    walk.anomalies.identical = identical(store, &stored, &walk.entries);
    walk
}

fn read_json<T: serde::de::DeserializeOwned>(store: &dyn Store, path: &str) -> Option<T> {
    store
        .read(path)
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
}
