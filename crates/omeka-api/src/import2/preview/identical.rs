//! The `identical` section: the sets of files the last scan found with equal
//! size and equal SHA-256 (`evidence::identical`), paged.

use serde::Serialize;

use super::query::{Page, PageQuery, page};
use crate::import2::plan::SourceId;
use crate::import2::scan::StoredIndex;
use crate::import2::values::fold_title;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IdenticalView {
    pub source: SourceId,
    pub size: u64,
    pub sha256: String,
    pub paths: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IdenticalFiles {
    /// Files that share their bytes with another file, over every set.
    pub files: usize,
    pub sets: Page<IdenticalView>,
}

pub(super) fn identical(stored: &StoredIndex, query: &PageQuery) -> IdenticalFiles {
    let text = query.text.as_deref().map(fold_title);
    let all = stored.index.sources.iter().flat_map(|source| {
        source
            .walk
            .anomalies
            .identical
            .iter()
            .map(move |set| (&source.id, set))
    });
    let files = all.clone().map(|(_, set)| set.paths.len()).sum();
    let matching = all.filter(|(_, set)| {
        text.as_ref().is_none_or(|text| {
            set.paths
                .iter()
                .any(|path| fold_title(path).contains(text.as_str()))
        })
    });
    IdenticalFiles {
        files,
        sets: page(matching, query.page, query.per_page, |(source, set)| {
            IdenticalView {
                source: source.clone(),
                size: set.size,
                sha256: set.sha256.clone(),
                paths: set.paths.clone(),
            }
        }),
    }
}
