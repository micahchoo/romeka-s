//! `GET /api/romeka/archive?root=&path=`: one folder of a named archive root,
//! for the admin's folder picker.
//!
//! The listing shows what a walk of the folder would read: dot files and
//! symlinks are left out, because the walk does not follow them.

use std::fs;

use axum::extract::State;
use axum::http::{HeaderMap, StatusCode, Uri};
use axum::response::Response;
use omeka_api::import::archive::ArchiveRoots;
use omeka_api::import::plan::RelPath;
use omeka_api::messages;
use serde::Serialize;

use super::wire::{ImportError, admin, blocking, json};
use crate::AppState;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct Listing {
    /// Every configured root, by name, in configured order.
    roots: Vec<String>,
    root: Option<String>,
    path: String,
    entries: Vec<Entry>,
}

#[derive(Debug, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "camelCase")]
struct Entry {
    kind: Kind,
    name: String,
    /// Relative to the root: what a folder source's `path` takes.
    path: String,
    size: Option<u64>,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "camelCase")]
enum Kind {
    Folder,
    File,
}

pub(super) async fn archive(
    State(state): State<AppState>,
    uri: Uri,
    headers: HeaderMap,
) -> Response {
    super::answer(
        async {
            admin(&state, &uri, &headers).await?;
            let params = omeka_api::read::Params::parse(uri.query().unwrap_or(""));
            let roots = state.archive_roots.clone();
            let names: Vec<String> = roots.iter().map(|(name, _)| name.to_string()).collect();
            let Some(root) = params
                .get("root")
                .map(str::to_string)
                .or_else(|| names.first().cloned())
            else {
                return Ok(json(
                    StatusCode::OK,
                    &Listing {
                        roots: names,
                        root: None,
                        path: String::new(),
                        entries: Vec::new(),
                    },
                ));
            };
            let raw = params.get("path").unwrap_or("").to_string();
            let path = RelPath::parse(&raw)
                .map_err(|_| ImportError::BadRequest(messages::import_outside_root(&raw)))?;
            let listed_root = root.clone();
            let entries = blocking(move || list(&roots, &listed_root, &path)).await?;
            Ok(json(
                StatusCode::OK,
                &Listing {
                    roots: names,
                    root: Some(root),
                    path: raw,
                    entries,
                },
            ))
        }
        .await,
    )
}

fn list(roots: &ArchiveRoots, root: &str, path: &RelPath) -> Result<Vec<Entry>, ImportError> {
    let dir = roots.source(root, path)?;
    let listing =
        fs::read_dir(dir.dir()).map_err(|error| ImportError::Internal(error.to_string()))?;
    let mut entries = Vec::new();
    for child in listing.flatten() {
        let name = child.file_name().to_string_lossy().into_owned();
        if name.starts_with('.') {
            continue;
        }
        let Ok(kind) = child.file_type() else {
            continue;
        };
        let (kind, size) = if kind.is_dir() {
            (Kind::Folder, None)
        } else if kind.is_file() {
            (
                Kind::File,
                child.metadata().ok().map(|metadata| metadata.len()),
            )
        } else {
            continue;
        };
        let child_path = if path.as_str().is_empty() {
            name.clone()
        } else {
            format!("{}/{name}", path.as_str())
        };
        entries.push(Entry {
            kind,
            name,
            path: child_path,
            size,
        });
    }
    entries.sort();
    Ok(entries)
}
