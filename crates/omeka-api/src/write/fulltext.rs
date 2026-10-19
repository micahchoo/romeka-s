//! `Module::saveFulltext` and `::deleteFulltext`: keep the `fulltext_search`
//! index in step with a write.
//!
//! `IndexFulltextSearch` rebuilds the whole index in one pass; upstream also
//! updates one resource on every `api.create.post`, `api.update.post` and
//! `api.delete.post`, which is what makes a freshly written resource findable.
//! This module is that second half.
//!
//! The indexed text mirrors the adapters:
//! `AbstractResourceEntityAdapter::getFulltextText` joins the text of every
//! public value, plus the public values of each value's annotation;
//! `ItemAdapter::getFulltextText` appends each media's text;
//! `MediaAdapter::getFulltextText` appends the html renderer's text;
//! `SitePageAdapter::getFulltextText` joins each block's text and its
//! attachments. A media delete re-saves the item, because the item's text
//! included that media (`Module::deleteFulltextMedia`).

use omeka_db::ids::{AnnotationId, PageId, ResourceId};
use omeka_db::repos::{
    FulltextRepo, MediaFilter, MediaRepo, Paging, ResourceRepo, SiteRepo, ValueRepo,
};
use omeka_db::rows::{FulltextSearchRow, MediaRow, ValueRow};
use serde_json::Value;

use crate::errors::ApiError;
use crate::read::{Api, db_error};

/// The resource names `FulltextSearchableInterface` covers at this SHA:
/// `ItemAdapter`, `ItemSetAdapter`, `MediaAdapter` and `SitePageAdapter`. The
/// generic resource and value-annotation adapters are excluded upstream.
pub const SEARCHABLE: [&str; 4] = ["items", "item_sets", "media", "site_pages"];

/// Write or refresh the index row for one resource.
pub async fn reindex(api: &Api<'_>, resource: &str, id: ResourceId) -> Result<(), ApiError> {
    let Some(row) = index_row(api, resource, id).await? else {
        return Ok(());
    };
    FulltextRepo::new(api.pool())
        .save(&row)
        .await
        .map_err(db_error)
}

/// Drop the index row for one resource.
pub async fn remove(api: &Api<'_>, resource: &str, id: ResourceId) -> Result<(), ApiError> {
    FulltextRepo::new(api.pool())
        .delete(id, resource)
        .await
        .map_err(db_error)?;
    Ok(())
}

/// The index row for one resource, or `None` when the resource name is not
/// searchable or the row is gone.
async fn index_row(
    api: &Api<'_>,
    resource: &str,
    id: ResourceId,
) -> Result<Option<FulltextSearchRow>, ApiError> {
    if !SEARCHABLE.contains(&resource) {
        return Ok(None);
    }
    if resource == "site_pages" {
        return site_page_row(api, PageId(id.0)).await;
    }
    let Some(row) = ResourceRepo::new(api.pool())
        .get(id)
        .await
        .map_err(db_error)?
    else {
        return Ok(None);
    };
    let mut texts = value_texts(api, id).await?;
    if resource == "items" {
        for media in media_of(api, id).await? {
            let text = media_text(api, &media).await?;
            if !text.is_empty() {
                texts.push(text);
            }
        }
    } else if resource == "media" {
        let text = renderer_text(api, id).await?;
        if !text.is_empty() {
            texts.push(text);
        }
    }
    let text = texts.join("\n");
    Ok(Some(FulltextSearchRow {
        id,
        resource: resource.to_string(),
        owner_id: row.owner_id,
        is_public: row.is_public,
        title: row.title,
        text: Some(text),
    }))
}

/// `SitePageAdapter::getFulltextOwner`, `::getFulltextIsPublic`,
/// `::getFulltextTitle` and `::getFulltextText`.
///
/// The page is public only when the page and its site are both public, and the
/// owner is the site's owner.
async fn site_page_row(api: &Api<'_>, id: PageId) -> Result<Option<FulltextSearchRow>, ApiError> {
    let repo = SiteRepo::new(api.pool());
    let Some(page) = repo.get_page(id).await.map_err(db_error)? else {
        return Ok(None);
    };
    let Some(site) = repo.get(page.site_id).await.map_err(db_error)? else {
        return Ok(None);
    };
    let mut texts = Vec::new();
    for block in repo.list_blocks(id).await.map_err(db_error)? {
        // `SitePageAdapter::getFulltextText` asks each layout for its text. The
        // layouts live in `omeka-site`, so the port reads every string in the
        // stored `data` and strips markup, which reaches the same words for the
        // text-bearing layouts (html, media caption, table of contents).
        collect_strings(&block.data, &mut texts);
        if let Some(layout_data) = &block.layout_data {
            collect_strings(layout_data, &mut texts);
        }
    }
    Ok(Some(FulltextSearchRow {
        id: ResourceId(id.0),
        resource: "site_pages".to_string(),
        owner_id: site.owner_id,
        is_public: page.is_public && site.is_public,
        title: Some(page.title),
        text: Some(texts.join("\n")),
    }))
}

/// `AbstractResourceEntityAdapter::getFulltextText`: the text of every public
/// value, then the public values of each value's annotation.
async fn value_texts(api: &Api<'_>, resource: ResourceId) -> Result<Vec<String>, ApiError> {
    let values = ValueRepo::new(api.pool())
        .list_for_resource(resource)
        .await
        .map_err(db_error)?;
    let mut texts = Vec::new();
    for value in values.iter().filter(|value| value.is_public) {
        texts.push(value_text(api, value).await?);
        if let Some(annotation) = value.value_annotation_id {
            texts.extend(annotation_texts(api, annotation).await?);
        }
    }
    Ok(texts)
}

/// The public values of one value annotation.
async fn annotation_texts(
    api: &Api<'_>,
    annotation: AnnotationId,
) -> Result<Vec<String>, ApiError> {
    let values = ValueRepo::new(api.pool())
        .list_for_resource(ResourceId(annotation.0))
        .await
        .map_err(db_error)?;
    let mut texts = Vec::new();
    for value in values.iter().filter(|value| value.is_public) {
        texts.push(value_text(api, value).await?);
    }
    Ok(texts)
}

/// One value's fulltext text, as the data type registry gives it.
///
/// `Literal::getFulltextText` is the stored value, `Uri::getFulltextText` is
/// the URI and the label space separated, and `AbstractResource::getFulltextText`
/// is the referenced resource's title. `CustomVocab::getFulltextText` is the
/// stored value whatever the shape: a term, a URI's label, nothing for an item.
async fn value_text(api: &Api<'_>, value: &ValueRow) -> Result<String, ApiError> {
    if omeka_core::term_list::list_id(&value.r#type).is_some() {
        return Ok(value.value.clone().unwrap_or_default());
    }
    if value.r#type == "uri" {
        return Ok(format!(
            "{} {}",
            value.uri.as_deref().unwrap_or_default(),
            value.value.as_deref().unwrap_or_default()
        )
        .trim()
        .to_string());
    }
    if value.r#type.starts_with("resource") {
        return referenced_title(api, value.value_resource_id).await;
    }
    Ok(value.value.clone().unwrap_or_default())
}

async fn referenced_title(api: &Api<'_>, resource: Option<ResourceId>) -> Result<String, ApiError> {
    let Some(id) = resource else {
        return Ok(String::new());
    };
    Ok(ResourceRepo::new(api.pool())
        .get(id)
        .await
        .map_err(db_error)?
        .and_then(|row| row.title)
        .unwrap_or_default())
}

/// Every media of an item, in `position` order.
async fn media_of(api: &Api<'_>, item: ResourceId) -> Result<Vec<MediaRow>, ApiError> {
    MediaRepo::new(api.pool())
        .list(
            &MediaFilter {
                item_id: Some(omeka_db::ids::ItemId(item.0)),
                ..MediaFilter::default()
            },
            &Paging::default(),
        )
        .await
        .map_err(db_error)
}

/// `MediaAdapter::getFulltextText`: the media's own values plus its renderer's
/// text.
async fn media_text(api: &Api<'_>, media: &MediaRow) -> Result<String, ApiError> {
    let mut texts = value_texts(api, media.resource.id).await?;
    texts.push(renderer_text(api, media.resource.id).await?);
    Ok(texts
        .into_iter()
        .filter(|text| !text.is_empty())
        .collect::<Vec<_>>()
        .join("\n"))
}

/// `Renderer\Html::getFulltextText`: the `html` member of `data` with its tags
/// stripped. No other renderer implements the interface at this SHA.
async fn renderer_text(api: &Api<'_>, id: ResourceId) -> Result<String, ApiError> {
    let Some(row) = MediaRepo::new(api.pool())
        .get(omeka_db::ids::MediaId(id.0))
        .await
        .map_err(db_error)?
    else {
        return Ok(String::new());
    };
    if row.renderer != "html" {
        return Ok(String::new());
    }
    let html = row
        .data
        .as_ref()
        .and_then(|data| data.get("html"))
        .and_then(Value::as_str)
        .unwrap_or_default();
    Ok(strip_tags(html))
}

/// Every string inside a block's stored `data`, markup removed.
fn collect_strings(value: &Value, out: &mut Vec<String>) {
    match value {
        Value::String(text) => {
            let text = strip_tags(text);
            if !text.trim().is_empty() {
                out.push(text);
            }
        }
        Value::Array(items) => {
            for item in items {
                collect_strings(item, out);
            }
        }
        Value::Object(members) => {
            for member in members.values() {
                collect_strings(member, out);
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) => {}
    }
}

/// `strip_tags` plus `html_entity_decode` for the entities a block body
/// carries. The full HTML5 named-entity table is not ported; the numeric forms
/// and the five XML entities are.
fn strip_tags(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut in_tag = false;
    for ch in html.chars() {
        match ch {
            '<' => in_tag = true,
            '>' if in_tag => in_tag = false,
            _ if !in_tag => out.push(ch),
            _ => {}
        }
    }
    decode_entities(&out)
}

fn decode_entities(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find('&') {
        out.push_str(&rest[..start]);
        let Some(end) = rest[start..].find(';').map(|end| start + end) else {
            out.push_str(&rest[start..]);
            return out;
        };
        let entity = &rest[start + 1..end];
        match decode_entity(entity) {
            Some(decoded) => out.push_str(&decoded),
            None => out.push_str(&rest[start..=end]),
        }
        rest = &rest[end + 1..];
    }
    out.push_str(rest);
    out
}

fn decode_entity(entity: &str) -> Option<String> {
    let named = match entity {
        "amp" => '&',
        "lt" => '<',
        "gt" => '>',
        "quot" => '"',
        "apos" | "#39" => '\'',
        "nbsp" => '\u{a0}',
        _ => {
            let digits = entity.strip_prefix('#')?;
            let code = match digits.strip_prefix(['x', 'X']) {
                Some(hex) => u32::from_str_radix(hex, 16).ok()?,
                None => digits.parse().ok()?,
            };
            return char::from_u32(code).map(String::from);
        }
    };
    Some(named.to_string())
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;

    #[test]
    fn strip_tags_drops_markup_and_decodes_entities() {
        assert_eq!(
            strip_tags("<p>Bronze &amp; iron</p>"),
            "Bronze & iron".to_string()
        );
        // PHP `strip_tags` also treats a bare `<` as a tag start.
        assert_eq!(strip_tags("a < b"), "a ".to_string());
        assert_eq!(strip_tags("caf&#233;"), "café".to_string());
        assert_eq!(strip_tags("&#x27;quoted&#x27;"), "'quoted'".to_string());
    }

    #[test]
    fn collect_strings_walks_arrays_and_objects() {
        let data = serde_json::json!({
            "html": "<p>One</p>",
            "caption": "Two",
            "nested": ["Three", 4, null, true],
        });
        let mut out = Vec::new();
        collect_strings(&data, &mut out);
        assert!(out.contains(&"One".to_string()), "out was {out:?}");
        assert!(out.contains(&"Two".to_string()), "out was {out:?}");
        assert!(out.contains(&"Three".to_string()), "out was {out:?}");
        assert_eq!(out.len(), 3);
    }
}
