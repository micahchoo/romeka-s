//! The representation a `DELETE` answers.
//!
//! `Api\Manager::delete` renders the entity the adapter returns *after*
//! `EntityManager::remove` and the flush, so the answer is the detached entity
//! (Doctrine clears the identifier and detaches it):
//!
//! - `o:id` is null and `@id` is the collection URL, because
//!   `AbstractResourceRepresentation::apiUrl` builds `api/default` with an id of
//!   null.
//! - `@reverse` is absent, because `AbstractResourceEntityRepresentation`
//!   builds it only `if ($this->id())`.
//! - Every association the delete cascaded away reads as a detached reference:
//!   an item's media, a site's pages. A lazy collection that was never loaded
//!   reads empty, so an item's `o:item_set` and `o:site` are `[]`.
//! - A reference to a row that survives keeps its id (`o:owner`, a
//!   resource-valued value, a site page's `o:site`).
//!
//! The port builds the live document from the row before it removes it, because
//! the values, pages and blocks are gone afterwards. This module rewrites that
//! document into the shape the flush leaves behind.

use serde_json::{Map, Value, json};

use crate::errors::ApiError;
use crate::jsonld::BaseUrl;
use crate::read::ApiResponse;

/// Rewrite one delete answer into the detached entity upstream renders.
pub fn detach(
    mut answer: ApiResponse,
    resource: &str,
    base: &BaseUrl,
) -> Result<ApiResponse, ApiError> {
    let mut document: Value = serde_json::from_str(&answer.body).map_err(ApiError::internal)?;
    let Some(object) = document.as_object_mut() else {
        return Ok(answer);
    };
    object.insert("o:id".to_string(), Value::Null);
    object.insert(
        "@id".to_string(),
        Value::String(base.api_collection(resource)),
    );
    object.remove("@reverse");
    match resource {
        "items" => {
            object.insert("o:item_set".to_string(), Value::Array(Vec::new()));
            object.insert("o:site".to_string(), Value::Array(Vec::new()));
            detach_list(object, "o:media", base, "media");
            detach_ref(object, "o:primary_media", base, "media");
        }
        "item_sets" => {
            object.insert(
                "o:items".to_string(),
                json!({ "@id": base.api_collection("items") }),
            );
        }
        "sites" => {
            detach_list(object, "o:page", base, "site_pages");
            detach_ref(object, "o:homepage", base, "site_pages");
            object.insert(
                "o:site_item".to_string(),
                json!({ "@id": base.api_collection("items") }),
            );
        }
        _ => {}
    }
    answer.body = serde_json::to_string(&document).map_err(ApiError::internal)?;
    Ok(answer)
}

/// `ResourceReference::jsonSerialize` for a row the delete removed: the
/// collection URL and a null id.
fn detached_ref(base: &BaseUrl, resource: &str) -> Value {
    json!({ "@id": base.api_collection(resource), "o:id": null })
}

/// Replace every element of a reference list with the detached reference, so the
/// answer keeps the count upstream's loaded collection had.
fn detach_list(object: &mut Map<String, Value>, key: &str, base: &BaseUrl, resource: &str) {
    let Some(Value::Array(items)) = object.get_mut(key) else {
        return;
    };
    let detached = detached_ref(base, resource);
    for item in items.iter_mut() {
        *item = detached.clone();
    }
}

/// Replace one nullable reference with the detached reference; a null stays
/// null.
fn detach_ref(object: &mut Map<String, Value>, key: &str, base: &BaseUrl, resource: &str) {
    let Some(value) = object.get_mut(key) else {
        return;
    };
    if !value.is_null() {
        *value = detached_ref(base, resource);
    }
}
