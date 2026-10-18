//! The resource page blocks: the fragments an item, item-set or media show page
//! renders below its heading.
//!
//! Upstream reference (`source/omeka-s/application`):
//! `src/Site/ResourcePageBlockLayout/*.php`, `view/common/resource-page-block-layout/*.phtml`
//! and `src/Site/ResourcePageBlockLayout/Manager.php`. The manager keeps a
//! per-theme configuration of block names per resource and region, stored in the
//! theme's settings map under `resource_page_blocks` and defaulted by the
//! theme's `theme.ini`. The port carries the default here and the site's stored
//! override on [`RenderContext`](crate::view::RenderContext).
//!
//! The theme configuration the bundled theme stands in for is the default
//! theme's (`themes/default/config/theme.ini:47-55` at the oracle): items render
//! `mediaEmbeds, values, itemSets, sitePages, mediaList, linkedResources`, item
//! sets render `values`, and media render `mediaRender, values`. The manager's
//! own `RESOURCE_PAGE_BLOCKS_DEFAULT` names `mediaLinks` instead of `mediaList`,
//! but no layout is registered under `mediaLinks`, so it renders through the
//! fallback and emits nothing. The oracle theme config is what a visitor sees,
//! and the port follows it.

use omeka_core::resource::ResourceKind;

use crate::html::{escape_html, hyperlink};
use crate::settings::SiteSettings;
use crate::view::{LinkedResource, RenderContext, ResourceRef, ResourceView, Thumbnail, ValueRef};

/// One registered resource page block layout, as the manager lists it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResourcePageBlockSpec {
    /// The registered name (`module.config.php` `resource_page_block_layouts`).
    pub name: &'static str,
    /// The label the admin shows (`getLabel`).
    pub label: &'static str,
    /// The resource names the layout accepts (`getCompatibleResourceNames`).
    pub resources: &'static [ResourceKind],
}

/// Every registered resource page block layout, in `module.config.php` order.
pub static RESOURCE_PAGE_BLOCKS: [ResourcePageBlockSpec; 9] = [
    ResourcePageBlockSpec {
        name: "itemSets",
        label: "Item sets",
        resources: &[ResourceKind::Item],
    },
    ResourcePageBlockSpec {
        name: "lightboxGallery",
        label: "Lightbox gallery",
        resources: &[
            ResourceKind::Item,
            ResourceKind::ItemSet,
            ResourceKind::Media,
        ],
    },
    ResourcePageBlockSpec {
        name: "linkedResources",
        label: "Linked resources",
        resources: &[ResourceKind::Item, ResourceKind::ItemSet],
    },
    ResourcePageBlockSpec {
        name: "mediaEmbeds",
        label: "Media embeds",
        resources: &[ResourceKind::Item],
    },
    ResourcePageBlockSpec {
        name: "mediaList",
        label: "Media list",
        resources: &[ResourceKind::Item],
    },
    ResourcePageBlockSpec {
        name: "mediaRender",
        label: "Media render",
        resources: &[ResourceKind::Media],
    },
    ResourcePageBlockSpec {
        name: "resourceClass",
        label: "Resource class",
        resources: &[
            ResourceKind::Item,
            ResourceKind::ItemSet,
            ResourceKind::Media,
        ],
    },
    ResourcePageBlockSpec {
        name: "sitePages",
        label: "Site pages",
        resources: &[ResourceKind::Item],
    },
    ResourcePageBlockSpec {
        name: "values",
        label: "Values",
        resources: &[
            ResourceKind::Item,
            ResourceKind::ItemSet,
            ResourceKind::Media,
        ],
    },
];

/// The blocks the bundled theme renders when a site stores no override, from
/// `themes/default/config/theme.ini`.
#[must_use]
pub fn default_blocks(kind: ResourceKind) -> &'static [&'static str] {
    match kind {
        ResourceKind::Item => &[
            "mediaEmbeds",
            "values",
            "itemSets",
            "sitePages",
            "mediaList",
            "linkedResources",
        ],
        ResourceKind::ItemSet => &["values"],
        ResourceKind::Media => &["mediaRender", "values"],
    }
}

/// The stored `resource_page_blocks` configuration, standardized the way
/// `Manager::standardizeResourcePageBlocks` does.
///
/// A stored map is the site administrator's whole answer: a resource name the
/// map omits renders no blocks, as `standardizeResourcePageBlocks` fills it with
/// an empty `main` region.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ResourcePageBlocks {
    items: Vec<String>,
    item_sets: Vec<String>,
    media: Vec<String>,
}

impl ResourcePageBlocks {
    /// Read a theme settings map (or the `resource_page_blocks` member itself)
    /// at the storage boundary. A member of the wrong kind reads as absent.
    #[must_use]
    pub fn parse(value: &serde_json::Value) -> Self {
        let blocks = value.get("resource_page_blocks").unwrap_or(value);
        Self {
            items: names(blocks, "items"),
            item_sets: names(blocks, "item_sets"),
            media: names(blocks, "media"),
        }
    }

    /// The configured block names for one resource kind.
    #[must_use]
    pub fn names(&self, kind: ResourceKind) -> &[String] {
        match kind {
            ResourceKind::Item => &self.items,
            ResourceKind::ItemSet => &self.item_sets,
            ResourceKind::Media => &self.media,
        }
    }
}

fn names(blocks: &serde_json::Value, resource: &str) -> Vec<String> {
    blocks
        .get(resource)
        .and_then(|region| region.get("main"))
        .and_then(serde_json::Value::as_array)
        .map(|names| {
            names
                .iter()
                .filter_map(serde_json::Value::as_str)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

/// Render the configured blocks for a resource, in order. An unknown name
/// renders nothing, as the manager's fallback does.
#[must_use]
pub fn render_blocks(resource: &ResourceView, ctx: &RenderContext<'_>) -> String {
    let configured;
    let names: Vec<&str> = match &ctx.resource_page_blocks {
        Some(blocks) => {
            configured = blocks.names(resource.kind);
            configured.iter().map(String::as_str).collect()
        }
        None => default_blocks(resource.kind).to_vec(),
    };
    let mut out = String::new();
    for name in names {
        out.push_str(&render_block(name, resource, ctx));
    }
    out
}

fn render_block(name: &str, resource: &ResourceView, ctx: &RenderContext<'_>) -> String {
    match name {
        "mediaEmbeds" => media_embeds(resource),
        "values" => values(resource, &ctx.settings),
        "itemSets" => item_sets(resource, ctx),
        "sitePages" => site_pages(resource, ctx),
        "mediaList" => media_list(resource, ctx),
        "linkedResources" => linked_resources(resource),
        "mediaRender" => media_render(resource),
        _ => String::new(),
    }
}

/// `media-embeds.phtml`: each media's renderer markup inside `media-embeds`.
fn media_embeds(resource: &ResourceView) -> String {
    if resource.media_embeds.is_empty() {
        return String::new();
    }
    let mut out = String::from("<div class=\"media-embeds\">");
    for media in &resource.media_embeds {
        if let Some(embed) = &media.embed {
            out.push_str(embed);
        }
    }
    out.push_str("</div>");
    out
}

/// `media-render.phtml`: the media's own renderer markup.
fn media_render(resource: &ResourceView) -> String {
    resource
        .media_embeds
        .first()
        .and_then(|media| media.embed.clone())
        .unwrap_or_default()
}

/// `common/resource-values.phtml`: values grouped by property, each value with
/// its `asHtml()` markup and the labels the site settings ask for.
#[must_use]
pub fn values(resource: &ResourceView, settings: &SiteSettings) -> String {
    if resource.values.is_empty() {
        return String::new();
    }
    let mut out = String::from("<dl class=\"values\">");
    let mut open: Option<u64> = None;
    for value in &resource.values {
        if open != Some(value.property_id) {
            if open.is_some() {
                out.push_str("</div>");
            }
            out.push_str("<div class=\"property\"><dt>");
            out.push_str(&escape_html(&value.property));
            out.push_str(&property_term(value, settings));
            out.push_str("</dt>");
            open = Some(value.property_id);
        }
        out.push_str(&value_item(value, settings));
    }
    if open.is_some() {
        out.push_str("</div>");
    }
    out.push_str("</dl>");
    out
}

/// The optional `(term)` after a property label.
fn property_term(value: &ValueRef, settings: &SiteSettings) -> String {
    let text = match settings.property_label_information() {
        "term" if !value.property_term.is_empty() => value.property_term.as_str(),
        "vocab" if !value.vocabulary.is_empty() => value.vocabulary.as_str(),
        _ => return String::new(),
    };
    format!("<span class=\"field-term\">({})</span>", escape_html(text))
}

/// One `<dd class="value ...">`.
fn value_item(value: &ValueRef, settings: &SiteSettings) -> String {
    let lang = value.lang.as_deref().unwrap_or("");
    let mut out = format!(
        "<dd class=\"value {}\" lang=\"{}\">",
        escape_html(&value.class),
        escape_html(lang)
    );
    if settings.show_locale_label() && !lang.is_empty() {
        out.push_str(&format!(
            "<span class=\"language\">{}</span>",
            escape_html(lang)
        ));
    }
    out.push_str("<span class=\"value-content\">");
    out.push_str(&value.html);
    out.push_str("</span></dd>");
    out
}

/// `item-sets.phtml`: the item's item sets as links.
fn item_sets(resource: &ResourceView, ctx: &RenderContext<'_>) -> String {
    if resource.item_sets.is_empty() {
        return String::new();
    }
    let mut out = format!(
        "<dl class=\"item-sets\"><div class=\"property\"><dt>{}</dt>",
        escape_html(ctx.translator().messages().item_sets)
    );
    for entry in &resource.item_sets {
        out.push_str("<dd class=\"value\">");
        out.push_str(&hyperlink(&entry.title, Some(&entry.url), &[]));
        out.push_str("</dd>");
    }
    out.push_str("</div></dl>");
    out
}

/// `site-pages.phtml`: the pages of this site that attach the item, unless
/// `show_attached_pages` is false.
fn site_pages(resource: &ResourceView, ctx: &RenderContext<'_>) -> String {
    if !ctx.settings.show_attached_pages() || resource.site_pages.is_empty() {
        return String::new();
    }
    let mut out =
        String::from("<dl class=\"site-pages\"><div class=\"property\"><dt>Site pages</dt>");
    for page in &resource.site_pages {
        out.push_str("<dd class=\"value\">");
        out.push_str(&hyperlink(&page.title, Some(&page.url), &[]));
        out.push_str("</dd>");
    }
    out.push_str("</div></dl>");
    out
}

/// `media-list.phtml`: each media as a pretty link with its square thumbnail.
fn media_list(resource: &ResourceView, ctx: &RenderContext<'_>) -> String {
    if resource.media.is_empty() {
        return String::new();
    }
    let mut out = format!(
        "<dl class=\"media-list\"><div class=\"property\"><dt>{}</dt>",
        escape_html(ctx.translator().messages().media)
    );
    for entry in &resource.media {
        out.push_str("<dd class=\"value\">");
        out.push_str(&link_pretty(entry));
        out.push_str("</dd>");
    }
    out.push_str("</div></dl>");
    out
}

/// `linked-resources.phtml`: one table per property, one row per linking
/// resource. The filter and pagination controls are not reproduced.
fn linked_resources(resource: &ResourceView) -> String {
    if resource.linked_resources.is_empty() {
        return String::new();
    }
    let mut out = String::from("<div id=\"linked-resources\">");
    let mut open: Option<(&str, ResourceKind)> = None;
    for link in &resource.linked_resources {
        let key = (link.property.as_str(), link.kind);
        if open != Some(key) {
            if open.is_some() {
                out.push_str("</tbody></table>");
            }
            out.push_str("<table><caption class=\"linked-resource-property\">");
            out.push_str(&escape_html(&caption(link, resource)));
            out.push_str("</caption><thead><tr><th>Title</th><th>Class</th></tr></thead><tbody>");
            open = Some(key);
        }
        out.push_str("<tr class=\"linked-resource\"><td>");
        out.push_str(&link_pretty_target(&link.resource, true));
        out.push_str("</td><td></td></tr>");
    }
    if open.is_some() {
        out.push_str("</tbody></table>");
    }
    out.push_str("</div>");
    out
}

/// `%s with "%s: %s"`, the table caption.
fn caption(link: &LinkedResource, resource: &ResourceView) -> String {
    let kind = match link.kind {
        ResourceKind::Item => "Items",
        ResourceKind::ItemSet => "Item sets",
        ResourceKind::Media => "Media",
    };
    format!("{kind} with \"{}: {}\"", link.property, resource.title)
}

/// `linkPretty('square')`: the thumbnail when there is one, then the display
/// title, as one `resource-link`.
#[must_use]
pub fn link_pretty(entry: &ResourceRef) -> String {
    link_pretty_target(entry, false)
}

/// `linkPretty` with the `target="_blank"` the linked-resources table passes.
fn link_pretty_target(entry: &ResourceRef, target_blank: bool) -> String {
    let mut content = String::new();
    if let Some(Thumbnail { url, alt }) = &entry.thumbnail {
        content.push_str(&format!(
            "<img src=\"{}\" alt=\"{}\">",
            escape_html(url),
            escape_html(alt)
        ));
    }
    content.push_str(&format!(
        "<span class=\"resource-name\">{}</span>",
        escape_html(&entry.title)
    ));
    let mut attrs = String::from(" class=\"resource-link\"");
    if target_blank {
        attrs.push_str(" target=\"_blank\"");
    }
    format!(
        "<a href=\"{}\"{}>{}</a>",
        escape_html(&entry.url),
        attrs,
        content
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::view::{NoResources, RenderContext, ResourceView};
    use omeka_core::ids::ResourceId;
    use omeka_core::resource::ResourceKind;
    use omeka_core::site::Site;

    fn site() -> Site {
        Site::new(
            omeka_core::ids::SiteId::new(1),
            omeka_core::site::Slug::parse("main").expect("a valid slug"),
            "default".to_string(),
            "Main",
        )
    }

    fn context<'a>(site: &'a Site, resources: &'a NoResources) -> RenderContext<'a> {
        RenderContext::new(site, resources)
    }

    fn resource(kind: ResourceKind) -> ResourceView {
        ResourceView::new(ResourceId::new(1), kind, "/s/main/item/1", "An item")
    }

    fn value() -> ValueRef {
        ValueRef {
            property_id: 1,
            property: "Title".to_string(),
            property_term: "dcterms:title".to_string(),
            vocabulary: "Dublin Core".to_string(),
            value: "An item".to_string(),
            html: "An item".to_string(),
            lang: None,
            class: "literal".to_string(),
        }
    }

    /// The default item blocks render media, values, item sets, site pages, the
    /// media list and linked resources, in that order, and the empty lists
    /// leave their blocks out.
    #[test]
    fn item_default_blocks_render_in_theme_order() {
        let site = site();
        let resources = NoResources;
        let ctx = context(&site, &resources);
        let mut item = resource(ResourceKind::Item);
        item.values.push(value());
        item.media.push(ResourceRef {
            url: "/s/main/media/9".to_string(),
            title: "A file".to_string(),
            thumbnail: Some(Thumbnail::new("/files/square/9.jpg", "A file")),
        });
        item.item_sets.push(ResourceRef {
            url: "/s/main/item-set/2".to_string(),
            title: "A set".to_string(),
            thumbnail: None,
        });
        let out = render_blocks(&item, &ctx);
        assert!(
            out.contains("<dl class=\"values\"><div class=\"property\"><dt>Title</dt>"),
            "{out}"
        );
        assert!(out.contains("<dl class=\"item-sets\">"), "{out}");
        assert!(out.contains("<dl class=\"media-list\">"), "{out}");
        assert!(!out.contains("<div class=\"media-embeds\">"), "{out}");
        let values_at = out.find("<dl class=\"values\">").expect("values");
        let sets_at = out.find("<dl class=\"item-sets\">").expect("item sets");
        let media_at = out.find("<dl class=\"media-list\">").expect("media list");
        assert!(values_at < sets_at && sets_at < media_at, "{out}");
    }

    /// The media page renders its own markup, then its values.
    #[test]
    fn media_default_blocks_render_the_file_then_values() {
        let site = site();
        let resources = NoResources;
        let ctx = context(&site, &resources);
        let mut media = resource(ResourceKind::Media);
        media.media_embeds.push(crate::view::MediaRef {
            url: "/s/main/media/1".to_string(),
            original_url: None,
            title: "A file".to_string(),
            thumbnail: None,
            embed: Some("<div class=\"media-render html\">A file</div>".to_string()),
        });
        media.values.push(value());
        assert_eq!(
            render_blocks(&media, &ctx),
            "<div class=\"media-render html\">A file</div>\
             <dl class=\"values\"><div class=\"property\"><dt>Title</dt>\
             <dd class=\"value literal\" lang=\"\"><span class=\"value-content\">An item</span></dd>\
             </div></dl>"
        );
    }

    /// The stored configuration wins, and a resource it omits renders nothing.
    #[test]
    fn stored_configuration_replaces_the_default() {
        let site = site();
        let resources = NoResources;
        let mut ctx = context(&site, &resources);
        ctx.resource_page_blocks = Some(ResourcePageBlocks::parse(&serde_json::json!({
            "items": {"main": ["values"]}
        })));
        let mut item = resource(ResourceKind::Item);
        item.values.push(value());
        assert_eq!(
            render_blocks(&item, &ctx),
            "<dl class=\"values\"><div class=\"property\"><dt>Title</dt>\
             <dd class=\"value literal\" lang=\"\"><span class=\"value-content\">An item</span></dd>\
             </div></dl>"
        );
        let other = resource(ResourceKind::Media);
        assert_eq!(render_blocks(&other, &ctx), "");
    }

    /// `property_label_information` adds the term or the vocabulary label, and
    /// `show_locale_label` adds the language span.
    #[test]
    fn value_labels_follow_the_site_settings() {
        let site = site();
        let resources = NoResources;
        let mut ctx = context(&site, &resources);
        ctx.settings.property_label_information = Some("term".to_string());
        ctx.settings.show_locale_label = Some(true);
        let mut item = resource(ResourceKind::Item);
        let mut labelled = value();
        labelled.lang = Some("fr".to_string());
        item.values.push(labelled);
        let out = render_blocks(&item, &ctx);
        assert!(
            out.contains("<dt>Title<span class=\"field-term\">(dcterms:title)</span></dt>"),
            "{out}"
        );
        assert!(out.contains("lang=\"fr\""), "{out}");
        assert!(out.contains("<span class=\"language\">fr</span>"), "{out}");
    }

    /// A resource value is a pretty link and its class names the resource kind.
    #[test]
    fn resource_value_renders_a_pretty_link() {
        let site = site();
        let resources = NoResources;
        let ctx = context(&site, &resources);
        let mut item = resource(ResourceKind::Item);
        item.values.push(ValueRef {
            property_id: 2,
            property: "Subject".to_string(),
            property_term: "dcterms:subject".to_string(),
            vocabulary: "Dublin Core".to_string(),
            value: "Another item".to_string(),
            html: "<a href=\"/s/main/item/2\" class=\"resource-link\"><img src=\"/files/square/2.jpg\" alt=\"X\"><span class=\"resource-name\">Another item</span></a>".to_string(),
            lang: None,
            class: "resource items".to_string(),
        });
        let out = render_blocks(&item, &ctx);
        assert!(
            out.contains("<dd class=\"value resource items\" lang=\"\">"),
            "{out}"
        );
        assert!(out.contains("class=\"resource-link\""), "{out}");
    }
}
