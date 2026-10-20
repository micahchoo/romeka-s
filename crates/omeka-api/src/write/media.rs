//! `POST` and `PUT` for `media`.
//!
//! `MediaAdapter` sets the ingester and renderer only on create, links the item
//! (and refuses the row when none is given), stores the ingester's `data`, and
//! takes the owner from the item. `position`, `o:lang` and `o:alt_text` hydrate
//! on every write.
//!
//! A create runs the ingester itself, through `omeka-media`'s registry, so the
//! media row carries what the ingester produced: the stored file and its facts
//! (`upload`, `url`, `youtube`, `iiif`), the typed payload (`oembed`, `youtube`,
//! `iiif`, `iiif_presentation`) and the purified body (`html`). The ingester
//! reads the request content the way `Request::getContent()` does, so
//! `Ingester\Url`'s `ingest_url` and `Ingester\Html`'s `html` are top-level
//! members of the body, not of `data`. `data` stays the member
//! `MediaAdapter::hydrate` copies when the ingester writes no payload of its own.
//!
//! `Ingester\Fallback` backs every unregistered name and has no renderer, so an
//! unknown ingester is refused the way upstream refuses the fallback.

use std::sync::Arc;

use chrono::NaiveDateTime;
use omeka_core::ids::{ItemId as CoreItemId, MediaId as CoreMediaId};
use omeka_core::resource::{Media as CoreMedia, Timestamp};
use omeka_db::ids::{ItemId, MediaId, ResourceId};
use omeka_db::repos::{ItemRepo, MediaRepo};
use omeka_db::rows::{ItemRow, MediaRow, NewMedia, NewResource};
use omeka_db::with_db;
use omeka_media::data::MediaData;
use omeka_media::http_client::HttpClient;
use omeka_media::ingest::{
    ErrorStore, HtmlPurifier, IngestEnv, IngestRequest, IngesterRegistry, MediaDraft,
    UploadedFile as IngestUpload,
};
use omeka_media::oembed::{Oembed, OembedAllowList};
use omeka_media::store::Store;
use omeka_media::thumbnailer::ImageThumbnailer;
use omeka_media::validation::FileValidation;
use serde_json::Value;

use crate::errors::{ApiError, ValidationErrors};
use crate::messages;
use crate::read::{Api, ApiResponse, db_error, not_found, parse_id};
use crate::write::access;
use crate::write::body::{Payload, php_json, ref_row};
use crate::write::fulltext;
use crate::write::multipart::UploadedFile;
use crate::write::resource::{self, not_found as ref_not_found};
use crate::write::values;

const MEDIA_CLASS: &str = "Omeka\\Entity\\Media";
const ITEM_CLASS: &str = "Omeka\\Entity\\Item";

const CONTROL: &[&str] = &[
    "o:is_public",
    "o:owner",
    "o:resource_class",
    "o:resource_template",
    "o:thumbnail",
    "o:ingester",
    "o:renderer",
    "o:item",
    "o:source",
    "o:lang",
    "o:alt_text",
    "data",
    "position",
    // The parameters the `Media\Ingester\*` classes read out of the request
    // content. They name no property, so they are control like `data` is.
    "html",
    "ingest_url",
    "store_original",
    "file_index",
    "refresh_oembed",
    "start",
    "end",
];

/// The media columns one create's ingester produced, before a row exists.
pub(crate) struct Prepared {
    hydrated: resource::Hydrated,
    ingester: String,
    renderer: String,
    data: Option<Value>,
    source: Option<String>,
    media_type: Option<String>,
    storage_id: Option<String>,
    extension: Option<String>,
    sha256: Option<String>,
    size: Option<i64>,
    has_original: bool,
    has_thumbnails: bool,
    position: Option<i64>,
    lang: Option<String>,
    alt_text: Option<String>,
    names_owner_id: bool,
}

pub async fn create(
    api: &Api<'_>,
    payload: &Payload,
    file: Option<&UploadedFile>,
) -> Result<ApiResponse, ApiError> {
    let item = item(api, payload).await?;
    let prepared = prepare(api, payload, file, None).await?;
    let row = insert(api, &item, prepared).await?;
    // `Module::saveFulltext`'s media branch: the item's text includes its media.
    fulltext::reindex(api, "items", ResourceId(row.item_id.0)).await?;
    api.read_member("media", &row.resource.id.0.to_string())
        .await
}

/// Resolve everything one media create needs before a row exists, so an item
/// create can prepare every inline entry and only then write the item.
///
/// `position` is the ordering the caller wants when the body names no
/// `position` member: an inline `o:media` entry takes its index in the array,
/// a standalone create takes none.
pub(crate) async fn prepare(
    api: &Api<'_>,
    payload: &Payload,
    file: Option<&UploadedFile>,
    position: Option<i64>,
) -> Result<Prepared, ApiError> {
    // `hydrateEntity` validates the request before it hydrates, so a body with
    // no usable `o:ingester` is refused before a value is read.
    let ingester = ingester_name(payload)?;
    // A value that names no resource is a 404 `findEntity` throws during
    // hydration; the ingester is the only step with a side effect, so the
    // shared hydrate runs first and nothing is stored for a body whose values
    // are already refused.
    let hydrated = resource::hydrate(api, payload, CONTROL, None, false).await?;
    let Some(store) = api.store().cloned() else {
        return Err(ApiError::internal("the media write reached no file store"));
    };
    let request = ingest_request(payload, file);
    // `MediaAdapter::hydrate` sets `source` from `o:source` before the ingester
    // runs, and an ingester overwrites it only when the body named none.
    let source = payload.str_member("o:source").map(str::to_string);
    let validation = crate::settings::file_validation(api.pool())
        .await
        .map_err(db_error)?;
    let policy = crate::settings::address_policy(api.pool())
        .await
        .map_err(db_error)?;
    let http = HttpClient::new().with_address_policy(policy);
    let (draft, errors) =
        ingest(store, ingester.clone(), request, source, validation, http).await?;
    if !errors.is_empty() {
        return Err(ingest_errors(&errors));
    }
    Ok(Prepared {
        hydrated,
        data: media_data(payload, &ingester, &draft.data),
        renderer: draft.row.renderer.clone(),
        ingester,
        source: draft.row.source,
        media_type: draft.row.media_type,
        storage_id: draft.row.storage_id,
        extension: draft.row.extension,
        sha256: draft.row.sha256,
        size: draft.row.size,
        has_original: draft.row.has_original,
        has_thumbnails: draft.row.has_thumbnails,
        position: payload.int_member("position").or(position),
        lang: payload.str_member("o:lang").map(str::to_string),
        alt_text: payload.str_member("o:alt_text").map(str::to_string),
        names_owner_id: payload
            .get("o:owner")
            .and_then(Value::as_object)
            .is_some_and(|owner| owner.contains_key("o:id")),
    })
}

/// Drive one ingester off the async worker.
///
/// Every ingester is synchronous, because upstream's are and the port's HTTP
/// seam is: a `url` or `iiif` create would otherwise hold its tokio worker for
/// the length of a fetch and stall every other request the worker carries.
async fn ingest(
    store: Arc<dyn Store>,
    ingester: String,
    request: IngestRequest,
    source: Option<String>,
    validation: FileValidation,
    http: HttpClient,
) -> Result<(MediaDraft, ErrorStore), ApiError> {
    tokio::task::spawn_blocking(move || {
        run_ingester(
            store.as_ref(),
            &ingester,
            request,
            source,
            &validation,
            &http,
        )
    })
    .await
    .map_err(|error| ApiError::internal(format!("the media ingester did not finish: {error}")))
}

/// The ingester a create names.
///
/// `MediaAdapter::validateRequest` refuses a body with no truthy `o:ingester`;
/// `Manager::get` then answers `Fallback` for a name no ingester registered, and
/// the create branch refuses the fallback with the same 422.
fn ingester_name(payload: &Payload) -> Result<String, ApiError> {
    if !payload.truthy("o:ingester") {
        return Err(validation("o:ingester", messages::MEDIA_INGESTER_REQUIRED));
    }
    let Some(ingester) = payload.str_member("o:ingester").map(str::to_string) else {
        return Err(validation("o:ingester", messages::MEDIA_INGESTER_REQUIRED));
    };
    if IngesterRegistry::with_defaults().find(&ingester).is_none() {
        return Err(validation("o:ingester", messages::MEDIA_INGESTER_INVALID));
    }
    Ok(ingester)
}

/// Write a prepared media under `item`.
pub(crate) async fn insert(
    api: &Api<'_>,
    item: &ItemRow,
    prepared: Prepared,
) -> Result<MediaRow, ApiError> {
    // `MediaAdapter::hydrateOwner`: an `o:owner` that names an `o:id` wins,
    // otherwise the media is owned by the item's owner.
    let owner_id = if prepared.names_owner_id {
        prepared.hydrated.resource.owner_id
    } else {
        item.resource.owner_id
    };
    let row = with_db!(api.pool(), |db| MediaRepo::new(db)
        .create(&NewMedia {
            resource: NewResource {
                owner_id,
                ..prepared.hydrated.resource
            },
            item_id: item.id(),
            ingester: prepared.ingester,
            renderer: prepared.renderer,
            data: prepared.data,
            source: prepared.source,
            media_type: prepared.media_type,
            storage_id: prepared.storage_id,
            extension: prepared.extension,
            sha256: prepared.sha256,
            size: prepared.size,
            has_original: prepared.has_original,
            has_thumbnails: prepared.has_thumbnails,
            position: prepared.position,
            lang: prepared.lang,
            alt_text: prepared.alt_text,
        })
        .await)
    .map_err(db_error)?;
    resource::store_values(
        api,
        row.resource.id,
        &prepared.hydrated.values,
        prepared.hydrated.keep_stored_values,
    )
    .await?;
    fulltext::reindex(api, "media", row.resource.id).await?;
    Ok(row)
}

/// The request the ingester reads: the body as it arrived, plus the multipart
/// upload when the caller sent one.
fn ingest_request(payload: &Payload, file: Option<&UploadedFile>) -> IngestRequest {
    let request = IngestRequest::new(payload.object().clone());
    match file {
        Some(file) => request.with_files(vec![IngestUpload::new(
            file.filename.clone().unwrap_or_default(),
            file.bytes.clone(),
        )]),
        None => request,
    }
}

/// Drive one ingester over one request. Its caller is [`ingest`], which puts it
/// on the blocking pool because every ingester is synchronous.
fn run_ingester(
    store: &dyn Store,
    ingester: &str,
    request: IngestRequest,
    source: Option<String>,
    validation: &FileValidation,
    http: &HttpClient,
) -> (MediaDraft, ErrorStore) {
    let registry = IngesterRegistry::with_defaults();
    let implementation = registry.get(ingester);
    let mut draft = MediaDraft::new(CoreMedia::new(
        CoreMediaId::new(0),
        CoreItemId::new(0),
        ingester.to_string(),
        implementation.renderer().to_string(),
        Timestamp::from_naive(NaiveDateTime::default()),
    ));
    draft.row.source = source;
    let purifier = HtmlPurifier;
    let thumbnailer = ImageThumbnailer;
    let oembed = Oembed::new(OembedAllowList::shipped());
    let env = IngestEnv {
        store,
        http,
        purifier: &purifier,
        thumbnailer: &thumbnailer,
        oembed: &oembed,
        validation,
    };
    let mut errors = ErrorStore::default();
    implementation.ingest(&mut draft, &request, &env, &mut errors);
    (draft, errors)
}

/// The `data` a create stores.
///
/// `Ingester\Html::ingest` keeps the whole request content and purifies the
/// `html` member, so a body with a top-level `html` stores that. Every other
/// ingester writes its own typed payload or none, and `MediaAdapter::hydrate`
/// falls back to the `data` member the body carried.
fn media_data(payload: &Payload, ingester: &str, draft: &MediaData) -> Option<Value> {
    if ingester == "html"
        && let Some(html) = payload.get("html").and_then(Value::as_str)
    {
        let mut content = payload.php_object();
        content.insert(
            "html".to_string(),
            Value::String(omeka_core::purify::purify(html)),
        );
        return Some(Value::Object(content));
    }
    if !matches!(draft, MediaData::None) {
        return Some(draft.to_value());
    }
    data_member(payload, ingester)
}

/// The stored `data` member. The `html` ingester purifies its body the way
/// `Media\Ingester\Html::ingest` does (`Media/Ingester/Html.php:49`); every
/// other ingester stores the payload as received.
fn data_member(payload: &Payload, ingester: &str) -> Option<Value> {
    let data = payload.get("data").map(php_json);
    if ingester != "html" {
        return data;
    }
    let Some(Value::Object(mut map)) = data else {
        return data;
    };
    if let Some(Value::String(html)) = map.get("html") {
        let purified = omeka_core::purify::purify(html);
        map.insert("html".to_string(), Value::String(purified));
    }
    Some(Value::Object(map))
}

/// The ingester errors as the 422 document, keyed the way the ingester added
/// them (`error`, `ingest_url`, `o:source`).
fn ingest_errors(errors: &ErrorStore) -> ApiError {
    let mut map = ValidationErrors::new();
    for error in errors.errors() {
        map.entry(error.key.clone())
            .or_default()
            .push(error.message.clone());
    }
    ApiError::Validation(map)
}

pub async fn update(
    api: &Api<'_>,
    raw_id: &str,
    payload: &Payload,
    partial: bool,
) -> Result<ApiResponse, ApiError> {
    let id = parse_id(MEDIA_CLASS, raw_id)?;
    let Some(mut row) =
        with_db!(api.pool(), |db| MediaRepo::new(db).get(MediaId(id)).await).map_err(db_error)?
    else {
        return Err(not_found(MEDIA_CLASS, raw_id));
    };
    api.authorize_resource(MEDIA_CLASS, row.resource.owner_id)?;
    let hydrated = resource::hydrate(api, payload, CONTROL, Some(&row.resource), partial).await?;
    resource::apply(&mut row.resource, &hydrated.resource);
    let placed = row.position;
    if let Some(position) = payload.int_member("position") {
        row.position = Some(position);
    }
    if !partial || payload.has("o:lang") {
        row.lang = payload.str_member("o:lang").map(str::to_string);
    }
    if !partial || payload.has("o:alt_text") {
        row.alt_text = payload.str_member("o:alt_text").map(str::to_string);
    }
    with_db!(api.pool(), |db| MediaRepo::new(db).update(&row).await).map_err(db_error)?;
    if row.position != placed {
        // A media moved in its item's order is a change to the item too:
        // import undo keeps an item whose `modified` moved after the import.
        with_db!(api.pool(), |db| omeka_db::repos::ResourceRepo::new(db)
            .touch(&[ResourceId(row.item_id.0)])
            .await)
        .map_err(db_error)?;
    }
    resource::store_values(
        api,
        row.resource.id,
        &hydrated.values,
        hydrated.keep_stored_values,
    )
    .await?;
    fulltext::reindex(api, "media", row.resource.id).await?;
    fulltext::reindex(api, "items", ResourceId(row.item_id.0)).await?;
    api.read_member("media", raw_id).await
}

/// Delete a media: its stored bytes, then its row, its values and its resource
/// row.
pub async fn delete(api: &Api<'_>, raw_id: &str) -> Result<ApiResponse, ApiError> {
    let id = parse_id(MEDIA_CLASS, raw_id)?;
    let Some(row) =
        with_db!(api.pool(), |db| MediaRepo::new(db).get(MediaId(id)).await).map_err(db_error)?
    else {
        return Err(not_found(MEDIA_CLASS, raw_id));
    };
    access::resource_entity(api.actor(), MEDIA_CLASS, row.resource.owner_id)?;
    let answer = crate::read::media_document(api, &row).await?;
    remove(api, &row).await?;
    Ok(answer)
}

/// Remove one media row and the bytes it owns, in the order a failure should
/// leave things: the files first, so a store failure keeps a readable row.
///
/// `Item::$media` cascades remove, so an item delete reaches this for each of
/// its media; `item.primary_media_id` and any site block attachment lose the
/// reference (`ON DELETE SET NULL`).
pub(crate) async fn remove(api: &Api<'_>, row: &MediaRow) -> Result<(), ApiError> {
    crate::write::files::delete_media_files(api.store().map(|store| store.as_ref()), row)?;
    values::clear(api, row.resource.id).await?;
    with_db!(api.pool(), |db| MediaRepo::new(db).delete(row.id()).await).map_err(db_error)?;
    fulltext::remove(api, "media", row.resource.id).await?;
    // `Module::deleteFulltextMedia`: the item's index text included this media,
    // so the item is re-indexed after the media row is gone.
    fulltext::reindex(api, "items", ResourceId(row.item_id.0)).await?;
    Ok(())
}

/// The item a create names. `MediaAdapter::validateEntity` rejects a media with
/// no item; a named item that does not exist is a 404. The id is read with
/// `isset`, so a present non-numeric value reaches the lookup and is a 404.
async fn item(api: &Api<'_>, payload: &Payload) -> Result<ItemRow, ApiError> {
    let Some(raw) = payload.present_ref("o:item") else {
        return Err(validation("o:item", messages::MEDIA_ITEM_REQUIRED));
    };
    let Some(id) = ref_row(raw) else {
        return Err(ref_not_found(ITEM_CLASS, raw));
    };
    let Some(item) =
        with_db!(api.pool(), |db| ItemRepo::new(db).get(ItemId(id)).await).map_err(db_error)?
    else {
        return Err(ref_not_found(ITEM_CLASS, raw));
    };
    api.authorize_resource(ITEM_CLASS, item.resource.owner_id)?;
    Ok(item)
}

fn validation(key: &str, message: &str) -> ApiError {
    let mut errors = ValidationErrors::new();
    errors.insert(key.to_string(), vec![message.to_string()]);
    ApiError::Validation(errors)
}
