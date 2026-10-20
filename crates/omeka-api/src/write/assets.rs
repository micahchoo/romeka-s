//! `POST`, `PUT`, `PATCH` and `DELETE` for `assets`.
//!
//! `AssetAdapter::hydrate` on create reads the uploaded file out of
//! `$request->getFileData()`, validates it against `api_assets.allowed_media_types`
//! and `api_assets.allowed_extensions`, and stores it under the `asset` prefix
//! (`AssetAdapter.php:64-105`, `File/TempFile.php:246`); on an update it writes
//! `o:name` and `o:alt_text` and nothing else. The file identity columns
//! (`o:media_type`, `o:storage_id`, `o:extension`) are fixed when the upload
//! stores the file, and the owner is hydrated on create only.
//!
//! The port receives the upload through the multipart boundary as an
//! [`UploadedFile`]; a create with no `file` field answers the upstream
//! `{"errors":{"file":["No file was uploaded"]}}`. The media type comes from
//! the bytes (`omeka_media::media_type::detected_media_type`, the port's
//! `finfo`) and the extension from the client file name, then the media-type
//! map. The check is `omeka_media::validation`, the same validator media use,
//! built with the asset lists instead of the settings.

use omeka_db::ids::AssetId;
use omeka_db::repos::AssetRepo;
use omeka_db::rows::{AssetRow, NewAsset};
use omeka_media::media_type::{detected_media_type, extension_for_media_type};
use omeka_media::store::{FileStore, StorePrefix, extension_from_source_name, new_storage_id};
use omeka_media::validation::{FileRefusal, FileValidation};
use serde_json::Value;

use crate::errors::{ApiError, ValidationErrors};
use crate::messages;
use crate::read::{Api, ApiResponse, db_error, not_found, parse_id};
use crate::write::access;
use crate::write::body::Payload;
use crate::write::multipart::UploadedFile;
use crate::write::resource;
use crate::write::vocab_member;

const ASSET_CLASS: &str = "Omeka\\Entity\\Asset";

/// `api_assets.allowed_media_types` (`application/config/module.config.php:45`).
const ALLOWED_MEDIA_TYPES: [&str; 4] = ["image/jpeg", "image/png", "image/gif", "image/webp"];
/// `api_assets.allowed_extensions` (`application/config/module.config.php:51`).
const ALLOWED_EXTENSIONS: [&str; 5] = ["jpeg", "jpg", "png", "gif", "webp"];

/// Create an asset from an uploaded file (`AssetAdapter::hydrate` on create).
pub async fn create(
    api: &Api<'_>,
    payload: &Payload,
    file: Option<UploadedFile>,
) -> Result<ApiResponse, ApiError> {
    let Some(file) = file else {
        return Err(errors("file", vec![messages::NO_FILE_UPLOADED.to_string()]));
    };
    let source_name = file.filename.clone().unwrap_or_default();
    let media_type = detected_media_type(&file.bytes);
    let extension = extension_from_source_name(&source_name)
        .or_else(|| extension_for_media_type(&media_type).map(str::to_string));
    let validation = FileValidation::Allow {
        media_types: ALLOWED_MEDIA_TYPES.map(str::to_string).to_vec(),
        extensions: ALLOWED_EXTENSIONS.map(str::to_string).to_vec(),
    };
    let invalid: Vec<String> = validation
        .refusals(&source_name, &media_type, extension.as_deref())
        .iter()
        .map(refusal_message)
        .collect();
    if !invalid.is_empty() {
        return Err(errors("file", invalid));
    }

    // `hydrateOwner`, then the name: `$request->getValue('o:name', $fileData['file']['name'])`.
    let owner_id = resource::owner_member(api, payload, true, None).await?;
    let name = match payload.get("o:name") {
        Some(Value::String(name)) => name.clone(),
        // A non-string name is not a string to `validateEntity` either.
        Some(_) => String::new(),
        None => source_name,
    };
    if name.is_empty() {
        let mut invalid = ValidationErrors::new();
        vocab_member::push(&mut invalid, "o:name", messages::ASSET_NAME_REQUIRED);
        return Err(ApiError::Validation(invalid));
    }
    let alt_text = payload
        .get("o:alt_text")
        .and_then(Value::as_str)
        .map(str::to_string);

    let Some(store) = api.store() else {
        return Err(ApiError::internal("the process has no file store"));
    };
    let storage_id = new_storage_id().map_err(ApiError::internal)?;
    let path = FileStore::storage_path(StorePrefix::Asset, &storage_id, extension.as_deref());
    store.put(&file.bytes, &path).map_err(ApiError::internal)?;
    let row = AssetRepo::new(api.pool())
        .create(&NewAsset {
            owner_id,
            name,
            media_type,
            storage_id,
            extension,
            alt_text,
        })
        .await
        .map_err(db_error)?;
    api.read_member("assets", &row.id.0.to_string()).await
}

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
    vocab_member::push(&mut errors, "o:name", messages::ASSET_NAME_REQUIRED);
    Err(ApiError::Validation(errors))
}

/// A validation failure under one key.
fn errors(key: &str, messages: Vec<String>) -> ApiError {
    let mut errors = ValidationErrors::new();
    errors.insert(key.to_string(), messages);
    ApiError::Validation(errors)
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

/// A refusal as the registry words it, under the `file` key.
fn refusal_message(refusal: &FileRefusal) -> String {
    match refusal {
        FileRefusal::MediaType { file, media_type } => messages::asset_media_type(file, media_type),
        FileRefusal::Extension { file, extension } => messages::asset_extension(file, extension),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A media ingest records a refusal by its display text, and the admin
    /// translates it by the registered pattern, so the two must agree.
    #[test]
    fn a_refusal_displays_as_the_registered_message() {
        let refusals = [
            FileRefusal::MediaType {
                file: "Thumbs.db".to_string(),
                media_type: "application/CDFV2".to_string(),
            },
            FileRefusal::Extension {
                file: "Thumbs.db".to_string(),
                extension: "db".to_string(),
            },
        ];
        for refusal in &refusals {
            assert_eq!(refusal.to_string(), refusal_message(refusal));
        }
    }
}
