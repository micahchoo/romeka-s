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
//! `mediaEmbeds, values, itemSets, sitePages, linkedResources`, item sets render
//! `values`, and media render `mediaRender, values`. The item page leaves out
//! upstream's `mediaList`, because its `mediaEmbeds` block carries every file as
//! the first file whole and the rest as a strip. The manager's
//! own `RESOURCE_PAGE_BLOCKS_DEFAULT` names `mediaLinks` instead of `mediaList`,
//! but no layout is registered under `mediaLinks`, so it renders through the
//! fallback and emits nothing. The oracle theme config is what a visitor sees,
//! and the port follows it except for that one omission (`docs/compat.md`).

use omeka_core::resource::ResourceKind;

use crate::html::{escape_html, hyperlink};
use crate::i18n::{Messages, sprintf, sprintf1};
use crate::settings::SiteSettings;
use crate::view::{
    LinkedResource, MediaRef, RenderContext, ResourceRef, ResourceView, Thumbnail, ValueRef,
};

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
pub static RESOURCE_PAGE_BLOCKS: [ResourcePageBlockSpec; 10] = [
    ResourcePageBlockSpec {
        name: "itemSets",
        label: "Item sets",
        resources: &[ResourceKind::Item],
    },
    // `LightboxGallery::getCompatibleResourceNames`: items and media only.
    ResourcePageBlockSpec {
        name: "lightboxGallery",
        label: "Lightbox gallery",
        resources: &[ResourceKind::Item, ResourceKind::Media],
    },
    ResourcePageBlockSpec {
        name: "linkedResources",
        label: "Linked resources",
        resources: &[ResourceKind::Item, ResourceKind::ItemSet],
    },
    // A Romeka addition: the item's own places, from the site's location
    // field. Upstream has no such layout, so its manager renders nothing for
    // the name.
    ResourcePageBlockSpec {
        name: "map",
        label: "Map of the item's place",
        resources: &[ResourceKind::Item],
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
/// `themes/default/config/theme.ini`, except that the item page leaves out
/// upstream's separate `mediaList`: the `mediaEmbeds` block now carries every
/// file, the first whole and the rest as a strip, so a second list of the same
/// files would say nothing new and would load every thumbnail eagerly. A site
/// that wants the list configures it.
#[must_use]
pub fn default_blocks(kind: ResourceKind) -> &'static [&'static str] {
    match kind {
        ResourceKind::Item => &[
            "mediaEmbeds",
            "values",
            "itemSets",
            "sitePages",
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
        "mediaEmbeds" => media_embeds(resource, ctx),
        "values" => values(resource, &ctx.settings),
        "itemSets" => item_sets(resource, ctx),
        "sitePages" => site_pages(resource, ctx),
        "mediaList" => media_list(resource, ctx),
        "linkedResources" => linked_resources(resource, ctx.translator().messages()),
        "mediaRender" => media_render(resource),
        "resourceClass" => resource_class(resource, ctx.translator().messages()),
        "lightboxGallery" => lightbox_gallery(resource),
        "map" => crate::map::item_section(resource, ctx),
        _ => String::new(),
    }
}

/// `media-embeds.phtml`, laid out the way this fork's reader moves through an
/// item: the first file whole, then every other file as a strip of thumbnails
/// that opens its media page. Upstream renders every file large, so a scan of
/// 300 pages draws 300 large images; this is a deliberate difference
/// (`docs/compat.md`).
///
/// The empty element after the first file is the place the notes list takes
/// (contract unit 9). It carries no text of its own.
fn media_embeds(resource: &ResourceView, ctx: &RenderContext<'_>) -> String {
    if resource.media_embeds.is_empty() {
        return format!(
            "<div class=\"media-embeds media-empty\">{}</div>",
            crate::theme::cards::empty_thumb(
                resource.kind,
                ctx.translator().messages().no_media_attached
            )
        );
    }
    let mut out = String::from("<div class=\"media-embeds\">");
    if let Some(embed) = resource.media_embeds[0].embed.as_deref() {
        out.push_str(embed);
    }
    out.push_str("<div class=\"annotation-list\"></div>");
    out.push_str(&media_strip(resource, ctx));
    out.push_str("</div>");
    out
}

/// The files after the first, as square thumbnails or kind placeholders, each
/// one a link to its media page with its display name below.
///
/// A strip of 299 files stays light: each image is `loading="lazy"` and carries
/// its 200 pixel size, and the row scrolls sideways instead of growing the
/// page.
fn media_strip(resource: &ResourceView, ctx: &RenderContext<'_>) -> String {
    let Some(rest) = resource.media.get(1..).filter(|rest| !rest.is_empty()) else {
        return String::new();
    };
    let count = sprintf1(
        ctx.translator().messages().files_count,
        &resource.media.len().to_string(),
    );
    let mut out = format!(
        "<div class=\"media-files\"><p class=\"media-files-title\">{}</p><ul class=\"media-files-list\">",
        escape_html(&count)
    );
    for entry in rest {
        out.push_str(&format!(
            "<li class=\"media-file\"><a class=\"media-file-link\" href=\"{}\">",
            escape_html(&entry.url)
        ));
        match &entry.thumbnail {
            Some(Thumbnail { url, .. }) => out.push_str(&format!(
                "<span class=\"media-file-thumb\"><img src=\"{}\" alt=\"\" loading=\"lazy\" \
                 width=\"200\" height=\"200\"></span>",
                escape_html(url)
            )),
            None => {
                let placeholder = entry.media_kind.as_ref().map_or_else(
                    || crate::theme::cards::kind_icon(ResourceKind::Media).to_string(),
                    crate::theme::cards::media_placeholder,
                );
                out.push_str(&format!(
                    "<span class=\"media-file-thumb media-file-placeholder\">{placeholder}</span>"
                ));
            }
        }
        out.push_str(&format!(
            "<span class=\"media-file-name\">{}</span></a>",
            escape_html(&entry.title)
        ));
        // A cropped tile never carries the overlay: it names the note count and
        // opens the media page, where the notes are drawn on the whole file.
        if entry.notes > 0 {
            out.push_str(&format!(
                "<a class=\"media-file-notes\" href=\"{}\">{}</a>",
                escape_html(&entry.url),
                escape_html(&sprintf1(
                    ctx.translator().messages().notes_count,
                    &entry.notes.to_string()
                ))
            ));
        }
        out.push_str("</li>");
    }
    out.push_str("</ul></div>");
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

/// `resource-class.phtml`: the class label, nothing when there is no class.
fn resource_class(resource: &ResourceView, messages: &Messages) -> String {
    let Some(label) = &resource.class_label else {
        return String::new();
    };
    format!(
        "<dl class=\"resource-class\"><div class=\"property\"><dt>{}</dt>\
         <dd class=\"value\">{}</dd></div></dl>",
        escape_html(messages.resource_class),
        escape_html(label)
    )
}

/// `lightbox-gallery-item.phtml` and `lightbox-gallery-media.phtml`, without
/// the script: each image file as its thumbnail, linked to the full image.
/// A media page whose file is not an image falls back to `media-render.phtml`,
/// as upstream does. Upstream's "Other Media" list under an item's gallery is
/// not reproduced; the media list block shows every media.
fn lightbox_gallery(resource: &ResourceView) -> String {
    let images: Vec<String> = resource
        .media_embeds
        .iter()
        .filter_map(image_link)
        .collect();
    if images.is_empty() {
        return match resource.kind {
            ResourceKind::Media => media_render(resource),
            ResourceKind::Item | ResourceKind::ItemSet => String::new(),
        };
    }
    format!("<div class=\"lightbox-gallery\">{}</div>", images.concat())
}

/// One gallery entry: an image file with a stored original, as a link to it.
fn image_link(media: &MediaRef) -> Option<String> {
    let is_image = media
        .media_type
        .as_deref()
        .is_some_and(|kind| kind.starts_with("image/"));
    let original = media.original_url.as_deref().filter(|_| is_image)?;
    let (src, alt) = match &media.thumbnail {
        Some(Thumbnail { url, alt }) if !alt.is_empty() => (url.as_str(), alt.as_str()),
        Some(Thumbnail { url, .. }) => (url.as_str(), media.title.as_str()),
        None => (original, media.title.as_str()),
    };
    Some(format!(
        "<a href=\"{}\" class=\"resource-link\"><img src=\"{}\" alt=\"{}\"></a>",
        escape_html(original),
        escape_html(src),
        escape_html(alt)
    ))
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
    let mut out = format!(
        "<dl class=\"site-pages\"><div class=\"property\"><dt>{}</dt>",
        escape_html(ctx.translator().messages().site_pages)
    );
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
fn linked_resources(resource: &ResourceView, messages: &Messages) -> String {
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
            out.push_str(&escape_html(&caption(link, resource, messages)));
            out.push_str(&format!(
                "</caption><thead><tr><th>{}</th><th>{}</th></tr></thead><tbody>",
                escape_html(messages.title),
                escape_html(messages.class)
            ));
            open = Some(key);
        }
        out.push_str("<tr class=\"linked-resource\"><td>");
        out.push_str(&link_pretty_target(&link.resource, true));
        out.push_str("</td><td>");
        if let Some(class) = &link.class_label {
            out.push_str(&format!(
                "<span class=\"resource-class\">{}</span>",
                escape_html(class)
            ));
        }
        out.push_str("</td></tr>");
    }
    if open.is_some() {
        out.push_str("</tbody></table>");
    }
    out.push_str("</div>");
    out
}

/// `%s with "%s: %s"`, the table caption: the translated resource label, then
/// the property label and the title as stored.
fn caption(link: &LinkedResource, resource: &ResourceView, messages: &Messages) -> String {
    let kind = match link.kind {
        ResourceKind::Item => messages.items,
        ResourceKind::ItemSet => messages.item_sets,
        ResourceKind::Media => messages.media,
    };
    sprintf(
        messages.linked_caption,
        &[kind, &link.property, &resource.title],
    )
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
#[path = "resource_page_tests.rs"]
mod tests;
