//! `PUT`, `PATCH` and `DELETE` for `assets`.
//!
//! `AssetAdapter::hydrate` writes `o:name` and `o:alt_text` on an update and
//! nothing else: the file identity columns (`o:media_type`, `o:storage_id`,
//! `o:extension`) are fixed when the upload stores the file, and the owner is
//! only hydrated on create.
//!
//! `POST /api/assets` is not built. Upstream's create reads the uploaded file
//! out of `$request->getFileData()` and refuses the body with
//! `{"errors":{"file":["No file was uploaded"]}}` when it is absent; the port's
//! write boundary parses JSON only, so there is no way to supply that file.
//! Reported as a gap.

use omeka_db::ids::AssetId;
use omeka_db::repos::AssetRepo;
use omeka_db::rows::AssetRow;
use omeka_media::store::{FileStore, StorePrefix};

use crate::errors::{ApiError, ValidationErrors};
use crate::read::{Api, ApiResponse, db_error, not_found, parse_id};
use crate::write::access;
use crate::write::body::Payload;
use crate::write::vocab_member;

const ASSET_CLASS: &str = "Omeka\\Entity\\Asset";

pub async fn update(
    api: &Api<'_>,
    raw_id: &str,
    payload: &Payload,
    partial: bool,
) -> Result<ApiResponse, ApiError> {
    let id = parse_id(ASSET_CLASS, raw_id)?;
    let repo = AssetRepo::new(api.pool());
    let Some(mut row) = repo.get(AssetId(id)).await.map_err(db_error)? else {
        return Err(not_found(ASSET_CLASS, raw_id));
    };
    access::resource_entity_update(api.actor(), ASSET_CLASS, row.owner_id)?;
    // `shouldHydrate`: a full write always writes the member, an absent one
    // hydrates to null, and `validateEntity` then refuses the null name.
    row.name = match (
        partial && !payload.has("o:name"),
        payload.str_member("o:name"),
    ) {
        (true, _) => row.name.clone(),
        (false, Some(value)) => value.to_string(),
        (false, None) => String::new(),
    };
    if !partial || payload.has("o:alt_text") {
        row.alt_text = payload.str_member("o:alt_text").map(str::to_string);
    }
    validate(&row)?;
    repo.update(&row).await.map_err(db_error)?;
    api.read_member("assets", raw_id).await
}

/// `AssetAdapter::validateEntity`.
fn validate(row: &AssetRow) -> Result<(), ApiError> {
    if !row.name.is_empty() {
        return Ok(());
    }
    let mut errors = ValidationErrors::new();
    vocab_member::push(&mut errors, "o:name", "An asset must have a name.");
    Err(ApiError::Validation(errors))
}

/// Delete an asset and the file it stored.
///
/// `Module::deleteAssetFile` listens on `Entity\Asset` `entity.remove.post` and
/// deletes `asset/<filename>` (`application/Module.php:347`). The port removes
/// the bytes first, so a store failure answers 500 with the row intact, the
/// order the media delete uses. A resource or site thumbnail that named the
/// asset loses the reference (`ON DELETE SET NULL`).
pub async fn delete(api: &Api<'_>, raw_id: &str) -> Result<ApiResponse, ApiError> {
    let id = parse_id(ASSET_CLASS, raw_id)?;
    let repo = AssetRepo::new(api.pool());
    let Some(row) = repo.get(AssetId(id)).await.map_err(db_error)? else {
        return Err(not_found(ASSET_CLASS, raw_id));
    };
    access::resource_entity(api.actor(), ASSET_CLASS, row.owner_id)?;
    let answer = api.read_member("assets", raw_id).await?;
    delete_asset_file(api.store(), &row)?;
    repo.delete(row.id).await.map_err(db_error)?;
    Ok(answer)
}

/// The stored bytes one asset owns (`Module::deleteAssetFile`).
fn delete_asset_file(store: Option<&FileStore>, row: &AssetRow) -> Result<(), ApiError> {
    let Some(store) = store else {
        return Ok(());
    };
    store
        .delete(&FileStore::storage_path(
            StorePrefix::Asset,
            &row.storage_id,
            row.extension.as_deref(),
        ))
        .map(|_| ())
        .map_err(ApiError::internal)
}
