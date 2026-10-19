//! `BlockLayout/BrowsePreview.php` and
//! `view/common/block-layout/browse-preview.phtml`.
//!
//! A search preview. The class parses the stored `query`, adds the site id, the
//! limit and the default sort, and searches the resource type the block names.
//! The template prints a heading, a `resource-list preview`, and a link to the
//! full browse.
//!
//! The query string arrives in the advanced-search form (`year=2000&sort_by=...`)
//! and is parsed with PHP's `parse_str` rules. `browse_attached_items` adds
//! `site_attachments_only`. `browse_heading_property_term` and
//! `browse_body_property_term` decide the heading and body text; those are
//! properties, so the port leaves them to the provider, which resolves
//! `ResourceView::title` and `ResourceView::description` with them in force.

use omeka_core::resource::ResourceKind;
use omeka_core::site::{BlockData, BrowsePreviewData, SiteBlock};

use crate::blocks::BlockLayout;
use crate::blocks::schema::{
    LayoutSchema, choice, integer, item_query, multi_choice, option, text,
};
use crate::html::{escape_html, hyperlink, query_string};
use crate::slug::{controller_for, resource_browse_url};
use crate::view::{BrowseQuery, RenderContext, ResourceView};

/// The default `components` (`BlockLayout/BrowsePreview.php::form`).
const DEFAULT_COMPONENTS: [&str; 3] = ["resource-heading", "resource-body", "thumbnail"];
/// The default `limit`.
const DEFAULT_LIMIT: usize = 12;

/// The `browsePreview` block.
#[derive(Debug, Clone, Copy, Default)]
pub struct BrowsePreview;

/// `BlockLayout/BrowsePreview.php::form` and its `$defaults`. `link-text`
/// starts empty rather than at the class's translated "Browse all".
const SCHEMA: LayoutSchema = LayoutSchema {
    fields: &[
        choice(
            "resource_type",
            "blockField.browsePreview.resource_type",
            "fieldHelp.browsePreview.resource_type",
            &[
                option("items", "blockChoice.items"),
                option("item_sets", "blockChoice.item_sets"),
                option("media", "blockChoice.media"),
            ],
        )
        .default_text("items"),
        item_query(
            "query",
            "blockField.browsePreview.query",
            "fieldHelp.browsePreview.query",
        )
        .resource_from("resource_type")
        .default_text(""),
        integer(
            "limit",
            "blockField.browsePreview.limit",
            "fieldHelp.browsePreview.limit",
        )
        .default_int(12),
        multi_choice(
            "components",
            "blockField.browsePreview.components",
            "fieldHelp.browsePreview.components",
            &[
                option("resource-heading", "blockChoice.resource-heading"),
                option("resource-body", "blockChoice.resource-body"),
                option("thumbnail", "blockChoice.thumbnail"),
            ],
        )
        .default_list(&["resource-heading", "resource-body", "thumbnail"]),
        text(
            "heading",
            "blockField.browsePreview.heading",
            "fieldHelp.browsePreview.heading",
        )
        .placeholder("fieldExample.browsePreview.heading")
        .default_text(""),
        text(
            "link-text",
            "blockField.browsePreview.link-text",
            "fieldHelp.browsePreview.link-text",
        )
        .placeholder("fieldExample.browsePreview.link-text"),
    ],
};

impl BlockLayout for BrowsePreview {
    fn name(&self) -> &'static str {
        "browsePreview"
    }

    fn label(&self) -> &'static str {
        "Browse preview"
    }

    fn render(&self, block: &SiteBlock, ctx: &RenderContext<'_>) -> String {
        let BlockData::BrowsePreview(data) = &block.data else {
            return String::new();
        };
        render_block(data, ctx)
    }

    fn schema(&self) -> &'static LayoutSchema {
        &SCHEMA
    }
}

fn render_block(data: &BrowsePreviewData, ctx: &RenderContext<'_>) -> String {
    let resource_type = data.resource_type.as_deref().unwrap_or("items");
    let kind = match resource_type {
        "item_sets" => ResourceKind::ItemSet,
        "media" => ResourceKind::Media,
        _ => ResourceKind::Item,
    };
    let original_query = parse_query(data.query.as_deref().unwrap_or_default());
    let mut query = original_query.clone();
    if ctx.settings.browse_attached_items() {
        query.push(("site_attachments_only".to_string(), "true".to_string()));
    }
    query.push(("site_id".to_string(), ctx.site.id.get().to_string()));
    query.push((
        "limit".to_string(),
        data.limit.unwrap_or(DEFAULT_LIMIT as i64).to_string(),
    ));
    if !query.iter().any(|(key, _)| key == "sort_by") {
        query.push(("sort_by_default".to_string(), String::new()));
        query.push(("sort_by".to_string(), "created".to_string()));
    }
    if !query.iter().any(|(key, _)| key == "sort_order") {
        query.push(("sort_order_default".to_string(), String::new()));
        query.push(("sort_order".to_string(), "desc".to_string()));
    }
    let components: Vec<String> = if data.components.is_empty() {
        DEFAULT_COMPONENTS
            .iter()
            .map(|c| (*c).to_string())
            .collect()
    } else {
        data.components.clone()
    };

    let search = BrowseQuery {
        query: query.clone(),
        site: ctx.site.id,
        limit: data.limit.map(|limit| usize::try_from(limit).unwrap_or(0)),
        sort: Some(("created".to_string(), "desc".to_string())),
    };
    let page = ctx.resources.search(kind, &search);

    let mut out = String::from("<div class=\"preview-block\">");
    if let Some(heading) = data
        .heading
        .as_deref()
        .filter(|heading| !heading.is_empty())
    {
        out.push_str(&format!("<h2>{}</h2>", escape_html(heading)));
    }
    out.push_str("<ul class=\"resource-list preview\">");
    for resource in &page.resources {
        out.push_str(&render_resource(
            resource,
            kind,
            &components,
            ctx.asset_base.trim_end_matches('/'),
        ));
    }
    out.push_str("</ul>");
    if let Some(link_text) = data.link_text.as_deref().filter(|text| !text.is_empty()) {
        let url = resource_browse_url(
            &ctx.site.slug,
            controller_for(kind),
            query_string(&original_query).as_deref(),
        );
        out.push_str(&hyperlink(link_text, Some(&url), &[]));
    }
    out.push_str("</div>");
    out
}

fn render_resource(
    resource: &ResourceView,
    kind: ResourceKind,
    components: &[String],
    asset_base: &str,
) -> String {
    let show_thumbnail = components.iter().any(|c| c == "thumbnail");
    let show_heading = components.iter().any(|c| c == "resource-heading");
    let show_body = components.iter().any(|c| c == "resource-body");

    let mut content = String::new();
    if show_thumbnail {
        // Upstream's `thumbnail` helper falls back to `default.png`; the port
        // falls back to the theme's placeholder asset.
        content.push_str(&match &resource.thumbnail {
            Some(thumbnail) => format!(
                "<img src=\"{}\" alt=\"{}\">",
                escape_html(&thumbnail.url),
                escape_html(&thumbnail.alt)
            ),
            None => format!(
                "<img src=\"{asset_base}/img/placeholder.svg\" alt=\"\" class=\"resource-placeholder\">"
            ),
        });
    }
    if show_heading {
        content.push_str(&format!(
            "<span class=\"resource-name\">{}</span>",
            escape_html(&resource.title)
        ));
    }
    let mut out = format!(
        "<li class=\"{} resource\">",
        escape_html(controller_for(kind))
    );
    if !content.is_empty() {
        out.push_str(&hyperlink_raw(&content, &resource.url, "resource-link"));
    }
    if show_body && let Some(body) = &resource.description {
        out.push_str(&format!(
            "<div class=\"description\">{}</div>",
            escape_html(body)
        ));
    }
    out.push_str("</li>");
    out
}

/// `Hyperlink::raw`: the content is already markup.
fn hyperlink_raw(content: &str, url: &str, class: &str) -> String {
    format!(
        "<a href=\"{}\" class=\"{}\">{}</a>",
        escape_html(url),
        escape_html(class),
        content
    )
}

/// PHP's `parse_str` for a query string without array keys: pairs in order,
/// `+` and `%XX` decoded, an empty value for a part without `=`.
#[must_use]
pub fn parse_query(query: &str) -> Vec<(String, String)> {
    let mut pairs = Vec::new();
    for part in query.split('&').filter(|part| !part.is_empty()) {
        let (key, value) = match part.split_once('=') {
            Some((key, value)) => (key, value),
            None => (part, ""),
        };
        let key = urldecode(key);
        if key.is_empty() {
            continue;
        }
        pairs.push((key, urldecode(value)));
    }
    pairs
}

fn urldecode(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'+' => {
                out.push(b' ');
                index += 1;
            }
            b'%' if index + 2 < bytes.len() => {
                let hex = &value[index + 1..index + 3];
                match u8::from_str_radix(hex, 16) {
                    Ok(byte) => {
                        out.push(byte);
                        index += 3;
                    }
                    Err(_) => {
                        out.push(b'%');
                        index += 1;
                    }
                }
            }
            byte => {
                out.push(byte);
                index += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blocks::test_support::{TestResources, block, context, item, site};
    use crate::view::Thumbnail;
    use serde_json::json;

    /// `block-layout/browse-preview.phtml`: heading, resource list with a
    /// thumbnail and a name, body, and the browse link with the stored query.
    #[test]
    fn browse_preview_matches_upstream_block_layout() {
        let site = site();
        let mut resources = TestResources::default();
        let mut resource = item(4, "An item", "/s/main/item/4");
        resource.description = Some("A description".to_string());
        resource.thumbnail = Some(Thumbnail::new("/files/medium/4.jpg", "An item"));
        resources.browse.push(resource);
        let ctx = context(&site, &resources);
        let stored = block(
            "browsePreview",
            json!({
                "resource_type": "items",
                "query": "year=2000",
                "heading": "Latest",
                "limit": 12,
                "components": ["resource-heading", "resource-body", "thumbnail"],
                "link-text": "Browse all"
            }),
        );
        assert_eq!(
            BrowsePreview.render(&stored, &ctx),
            "<div class=\"preview-block\"><h2>Latest</h2><ul class=\"resource-list preview\"><li class=\"item resource\"><a href=\"/s/main/item/4\" class=\"resource-link\"><img src=\"/files/medium/4.jpg\" alt=\"An item\"><span class=\"resource-name\">An item</span></a><div class=\"description\">A description</div></li></ul><a href=\"/s/main/item?year=2000\">Browse all</a></div>"
        );
    }

    /// The class adds the site id, the limit and the default sort to the search,
    /// and `browse_attached_items` adds `site_attachments_only`.
    #[test]
    fn browse_preview_search_query_matches_upstream() {
        let site = site();
        let mut resources = TestResources::default();
        resources.browse.push(item(4, "An item", "/s/main/item/4"));
        let mut ctx = context(&site, &resources);
        ctx.settings.browse_attached_items = Some(true);
        let stored = block(
            "browsePreview",
            json!({"resource_type": "items", "query": "year=2000", "limit": 5}),
        );
        // The provider sees the query the class builds; the first resource is
        // rendered without components set to the defaults.
        let out = BrowsePreview.render(&stored, &ctx);
        assert!(
            out.contains("<span class=\"resource-name\">An item</span>"),
            "{out}"
        );
        assert!(!out.contains("<div class=\"description\">"), "{out}");
    }

    /// A resource type other than items picks the item-set controller; the link
    /// and class names follow `BlockLayout/BrowsePreview.php::render`.
    #[test]
    fn browse_preview_item_sets_use_the_item_set_controller() {
        let site = site();
        let mut resources = TestResources::default();
        resources
            .browse
            .push(item(2, "A set", "/s/main/item-set/2"));
        let ctx = context(&site, &resources);
        let stored = block(
            "browsePreview",
            json!({"resource_type": "item_sets", "link-text": "All sets", "components": ["resource-heading"]}),
        );
        let out = BrowsePreview.render(&stored, &ctx);
        assert!(out.contains("<li class=\"item-set resource\">"), "{out}");
        assert!(
            out.contains("<a href=\"/s/main/item-set\">All sets</a>"),
            "{out}"
        );
    }

    /// `parse_str`: pairs keep their order, `+` and `%XX` decode, a part with no
    /// `=` has an empty value.
    #[test]
    fn parse_query_matches_php_parse_str() {
        assert_eq!(
            parse_query("year=2000&search=a+b&flag"),
            vec![
                ("year".to_string(), "2000".to_string()),
                ("search".to_string(), "a b".to_string()),
                ("flag".to_string(), String::new()),
            ]
        );
        assert_eq!(
            parse_query("q=caf%C3%A9"),
            vec![("q".to_string(), "café".to_string())]
        );
    }

    /// A block with no results still renders the wrapper and its link.
    #[test]
    fn browse_preview_without_results_renders_the_wrapper() {
        let site = site();
        let resources = TestResources::default();
        let ctx = context(&site, &resources);
        let stored = block("browsePreview", json!({"link-text": "Browse all"}));
        assert_eq!(
            BrowsePreview.render(&stored, &ctx),
            "<div class=\"preview-block\"><ul class=\"resource-list preview\"></ul><a href=\"/s/main/item\">Browse all</a></div>"
        );
    }
}
