//! JSON-LD serializer helpers.
//!
//! The serializer is `serde_json` over the types in [`crate::wire`], whose
//! field order is upstream insertion order. This module owns the pieces the
//! structs cannot express: the canonical base URL, the `@context` document,
//! and the generated search links.

use std::fmt::Display;

use indexmap::IndexMap;
use serde::Serialize;
use thiserror::Error;

use crate::wire::{ApiUrl, ResourceRef};

/// `Module::OMEKA_VOCABULARY_TERM`.
pub const OMEKA_VOCABULARY_TERM: &str = "o";
/// `Module::OMEKA_VOCABULARY_IRI`.
pub const OMEKA_VOCABULARY_IRI: &str = "http://omeka.org/s/vocabs/o#";
/// `Module::addTermDefinitionsToContext`.
pub const O_CONTENT_TERM: &str = "o-cnt";
/// `Module::addTermDefinitionsToContext`.
pub const O_CONTENT_IRI: &str = "http://www.w3.org/2011/content#";
/// `Module::addTermDefinitionsToContext`.
pub const O_TIME_TERM: &str = "o-time";
/// `Module::addTermDefinitionsToContext`.
pub const O_TIME_IRI: &str = "http://www.w3.org/2006/time#";

/// `routes.config.php` route name `api-context`.
pub const API_CONTEXT_PATH: &str = "api-context";
/// Mount point of the REST layer.
pub const API_PATH: &str = "api";

/// Failure to parse a canonical base URL.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum BaseUrlError {
    #[error("base URL must start with http:// or https://")]
    Scheme,
    #[error("base URL needs a host after the scheme")]
    Host,
}

/// A canonical origin such as `http://localhost`, validated once.
///
/// Upstream builds every `@id`, `@context` and generated link from the request
/// URL with `force_canonical`. The port takes one base URL at startup and
/// stores it in this type, so link building cannot inject an unrelated string.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct BaseUrl(String);

impl BaseUrl {
    /// Parse an origin. A trailing slash is dropped; a path is kept.
    pub fn parse(raw: &str) -> Result<Self, BaseUrlError> {
        let trimmed = raw.trim().trim_end_matches('/');
        let rest = trimmed
            .strip_prefix("http://")
            .or_else(|| trimmed.strip_prefix("https://"))
            .ok_or(BaseUrlError::Scheme)?;
        if rest.is_empty() || rest.starts_with('/') {
            return Err(BaseUrlError::Host);
        }
        Ok(Self(trimmed.to_string()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// `$url('api-context', [], ['force_canonical' => true])`.
    pub fn api_context(&self) -> String {
        format!("{}/{API_CONTEXT_PATH}", self.0)
    }

    /// `$url('api/default', ['resource' => $name, 'id' => $id])`.
    pub fn api_resource(&self, resource: &str, id: impl Display) -> String {
        format!("{}/{API_PATH}/{resource}/{id}", self.0)
    }

    /// The `o:items` link `ItemSetRepresentation::getResourceJsonLd` builds.
    pub fn items_for_item_set(&self, item_set_id: u64) -> ApiUrl {
        ApiUrl::new(self.api_search("items", &format!("item_set_id={item_set_id}")))
    }

    /// The `o:site_item` link `SiteRepresentation::getJsonLd` builds.
    pub fn items_for_site(&self, site_id: u64) -> ApiUrl {
        ApiUrl::new(self.api_search("items", &format!("site_id={site_id}")))
    }

    /// An `/api/{resource}?{query}` URL.
    pub fn api_search(&self, resource: &str, query: &str) -> String {
        format!("{}?{query}", self.api_collection(resource))
    }

    /// The collection URL without a query: `<base>/api/{resource}`.
    pub fn api_collection(&self, resource: &str) -> String {
        format!("{}/{API_PATH}/{resource}", self.0)
    }

    /// The fallback thumbnail asset for a media type, as the `assetUrl` view
    /// helper resolves `thumbnails.fallbacks` from
    /// `application/config/module.config.php:241-248`. Upstream matches the
    /// full media type first, then its top-level type, then the default.
    ///
    /// Only the top-level types the config lists are in the map; every other
    /// value, including an empty one, gets `thumbnails/default.png`.
    pub fn fallback_thumbnail_url(&self, media_type: Option<&str>) -> String {
        let asset = match media_type.and_then(|value| value.split_once('/')) {
            Some(("image", _)) => "thumbnails/image.png",
            Some(("video", _)) => "thumbnails/video.png",
            Some(("audio", _)) => "thumbnails/audio.png",
            _ => "thumbnails/default.png",
        };
        format!("{}/application/asset/{asset}", self.0)
    }

    /// `ResourceReference::jsonSerialize` for a resource on this base URL.
    pub fn resource_ref(&self, resource: &str, id: u64) -> ResourceRef {
        ResourceRef::new(self.api_resource(resource, id), id)
    }
}

/// The `/api-context` document: `{"@context": {...}}`.
///
/// `Module::addTermDefinitionsToContext` fills it: `o` first, then one entry
/// per vocabulary row, then `o-cnt` and `o-time`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ContextDocument {
    #[serde(rename = "@context")]
    pub context: IndexMap<String, String>,
}

impl ContextDocument {
    /// Compose the document from loaded vocabularies, in load order.
    pub fn new(vocabularies: impl IntoIterator<Item = (String, String)>) -> Self {
        let mut context = IndexMap::new();
        context.insert(
            OMEKA_VOCABULARY_TERM.to_string(),
            OMEKA_VOCABULARY_IRI.to_string(),
        );
        for (prefix, namespace_uri) in vocabularies {
            context.insert(prefix, namespace_uri);
        }
        context.insert(O_CONTENT_TERM.to_string(), O_CONTENT_IRI.to_string());
        context.insert(O_TIME_TERM.to_string(), O_TIME_IRI.to_string());
        Self { context }
    }
}

/// Failure to serialize a wire document.
#[derive(Debug, Error)]
pub enum JsonLdError {
    #[error("serializing JSON-LD: {0}")]
    Serialize(#[from] serde_json::Error),
}

/// Serialize one wire document with upstream key order.
///
/// The differential normalizer sorts keys; keeping upstream insertion order
/// here means a human can diff a response against the PHP server byte for byte.
pub fn to_string<T: Serialize>(value: &T) -> Result<String, JsonLdError> {
    Ok(serde_json::to_string(value)?)
}

/// Pretty form of [`to_string`], matching the golden fixture files.
pub fn to_string_pretty<T: Serialize>(value: &T) -> Result<String, JsonLdError> {
    Ok(serde_json::to_string_pretty(value)?)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;

    #[test]
    fn base_url_rejects_a_bare_host() {
        assert_eq!(BaseUrl::parse("localhost:8080"), Err(BaseUrlError::Scheme));
    }

    #[test]
    fn base_url_drops_a_trailing_slash() {
        let base = BaseUrl::parse("http://localhost/").unwrap();
        assert_eq!(base.api_context(), "http://localhost/api-context");
        assert_eq!(
            base.api_resource("items", 1),
            "http://localhost/api/items/1"
        );
    }

    #[test]
    fn api_context_terms_match_module_php_order() {
        let doc = ContextDocument::new([(
            "dcterms".to_string(),
            "http://purl.org/dc/terms/".to_string(),
        )]);
        let keys: Vec<&String> = doc.context.keys().collect();
        assert_eq!(keys, ["o", "dcterms", "o-cnt", "o-time"]);
        assert_eq!(
            serde_json::to_string(&doc).unwrap(),
            "{\"@context\":{\"o\":\"http://omeka.org/s/vocabs/o#\",\
             \"dcterms\":\"http://purl.org/dc/terms/\",\
             \"o-cnt\":\"http://www.w3.org/2011/content#\",\
             \"o-time\":\"http://www.w3.org/2006/time#\"}}"
        );
    }

    #[test]
    fn generated_search_links_match_the_representation() {
        let base = BaseUrl::parse("http://localhost").unwrap();
        assert_eq!(
            base.items_for_item_set(1),
            ApiUrl::new("http://localhost/api/items?item_set_id=1")
        );
        assert_eq!(
            base.items_for_site(2),
            ApiUrl::new("http://localhost/api/items?site_id=2")
        );
    }
}
