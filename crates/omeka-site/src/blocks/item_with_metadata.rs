//! `BlockLayout/ItemWithMetadata.php` and
//! `view/common/block-layout/item-with-metadata.phtml`.
//!
//! Every attachment's item, with its values and its item-set and media lists.
//! The class returns `No item selected` when it has no attachments, and the
//! template skips an attachment whose item is gone.
//!
//! `displayValues` walks the item's property values through the
//! `resource-values` view helper. The site layer has no value store, so the
//! provider hands the rendered pairs on [`ResourceView::values`] and the port
//! prints them as a plain definition list. The `resource show` wrapper, the
//! `Item sets` and `Media` lists and their `dt`/`dd` markup match upstream; the
//! media entries link pretty (`linkPretty('square', ...)`, a thumbnail and a
//! `resource-name` span), the item-set entries link plain (`link`).
//!
//! Two Romeka settings change that: a chosen, ordered list of properties
//! prints only those values, and the card display shows each item compactly,
//! with a citation line.

use omeka_core::site::{BlockData, SiteBlock};
use serde_json::Value as JsonValue;

use crate::blocks::BlockLayout;
use crate::blocks::schema::{LayoutSchema, attachments, choice, option, property_list};
use crate::html::escape_html;
use crate::resource_page::link_pretty;
use crate::theme::cards::{display_name, empty_thumb};
use crate::view::{RenderContext, ResourceRef, ResourceView, Thumbnail};

/// The `itemWithMetadata` block.
#[derive(Debug, Clone, Copy, Default)]
pub struct ItemWithMetadata;

/// `BlockLayout/ItemWithMetadata.php::form`: `blockAttachmentsForm($block,
/// true)`, the item-only attachments list. The `romeka.` fields are Romeka
/// settings: the properties to print and the display.
const SCHEMA: LayoutSchema = LayoutSchema {
    fields: &[
        attachments(
            "blockField.itemWithMetadata.attachments",
            "fieldHelp.itemWithMetadata.attachments",
            false,
        ),
        property_list(
            "romeka.fields",
            "blockField.itemWithMetadata.fields",
            "fieldHelp.itemWithMetadata.fields",
        ),
        choice(
            "romeka.display",
            "blockField.itemWithMetadata.display",
            "fieldHelp.itemWithMetadata.display",
            &[
                option("", "blockChoice.metadata_full"),
                option("card", "blockChoice.metadata_card"),
            ],
        )
        .default_text(""),
    ],
};

impl BlockLayout for ItemWithMetadata {
    fn name(&self) -> &'static str {
        "itemWithMetadata"
    }

    fn label(&self) -> &'static str {
        "Item with metadata"
    }

    fn render(&self, block: &SiteBlock, ctx: &RenderContext<'_>) -> String {
        if !matches!(block.data, BlockData::ItemWithMetadata) {
            return String::new();
        }
        let attachments = ctx.resources.attachments(block.id);
        if attachments.is_empty() {
            // `BlockLayout/ItemWithMetadata.php::render` returns this string
            // when the block has no attachments.
            return ctx.translator().messages().no_item_selected.to_string();
        }
        let items: Vec<ResourceView> = attachments
            .iter()
            .filter_map(|attachment| {
                attachment.item.and_then(|id| {
                    ctx.resources
                        .read(omeka_core::resource::ResourceKind::Item, id.get())
                })
            })
            .collect();
        let look = Look::of(block);
        let mut out = String::from(if look.card {
            "<div class=\"item-with-metadata item-cards\">"
        } else {
            "<div class=\"item-with-metadata\">"
        });
        for item in &items {
            let chosen = look.chosen_values(item);
            if look.card {
                let values = look.fields.as_ref().map(|_| &chosen);
                out.push_str(&render_card(item, values, ctx));
            } else {
                out.push_str(&render_item(&chosen, ctx));
            }
        }
        out.push_str("</div>");
        out
    }

    fn schema(&self) -> &'static LayoutSchema {
        &SCHEMA
    }
}

/// The block's Romeka settings. Unset, it prints the whole record, as
/// upstream does.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct Look {
    /// The property terms to print, in order; `None` prints every value.
    fields: Option<Vec<String>>,
    /// Whether each item shows as a compact card.
    card: bool,
}

impl Look {
    fn of(block: &SiteBlock) -> Self {
        let setting = |key| block.layout_data.romeka_setting(key);
        let fields = setting("fields")
            .and_then(JsonValue::as_array)
            .map(|terms| {
                terms
                    .iter()
                    .filter_map(JsonValue::as_str)
                    .map(str::to_string)
                    .collect::<Vec<_>>()
            })
            .filter(|terms| !terms.is_empty());
        let card = setting("display").and_then(JsonValue::as_str) == Some("card");
        Self { fields, card }
    }

    /// The item with only the chosen properties' values, in the chosen
    /// order; each property keeps its values' stored order.
    fn chosen_values(&self, item: &ResourceView) -> ResourceView {
        let Some(terms) = &self.fields else {
            return item.clone();
        };
        let mut chosen = item.clone();
        chosen.values = terms
            .iter()
            .flat_map(|term| {
                item.values
                    .iter()
                    .filter(move |value| &value.property_term == term)
                    .cloned()
            })
            .collect();
        chosen
    }
}

fn render_item(item: &ResourceView, ctx: &RenderContext<'_>) -> String {
    let mut out = String::from("<div class=\"resource show\">");
    out.push_str(&crate::resource_page::values(item, &ctx.settings));
    let messages = ctx.translator().messages();
    if !item.item_sets.is_empty() {
        out.push_str(&render_list(
            "item-sets",
            messages.item_sets,
            &item.item_sets,
            false,
        ));
    }
    if !item.media.is_empty() {
        out.push_str(&render_list(
            "media-list",
            messages.media,
            &item.media,
            true,
        ));
    }
    out.push_str("</div>");
    out
}

/// One item as a compact card: its image, its name as a link, the chosen
/// values when the curator chose any, and a citation line from the whole
/// record.
fn render_card(
    item: &ResourceView,
    chosen: Option<&ResourceView>,
    ctx: &RenderContext<'_>,
) -> String {
    let url = escape_html(&item.url);
    let (name, _) = display_name(item, ctx);
    let thumb = match &item.thumbnail {
        Some(Thumbnail { url: src, .. }) => format!(
            "<span class=\"resource-thumb\"><img src=\"{}\" alt=\"\" loading=\"lazy\"></span>",
            escape_html(src)
        ),
        None => empty_thumb(
            item.kind,
            crate::theme::cards::type_label(item)
                .unwrap_or_else(|| crate::theme::cards::kind_label(item.kind, ctx)),
        ),
    };
    let values = chosen.map_or_else(String::new, |chosen| {
        crate::resource_page::values(chosen, &ctx.settings)
    });
    format!(
        "<div class=\"item-card\"><a href=\"{url}\" class=\"item-card-thumb\" tabindex=\"-1\" \
         aria-hidden=\"true\">{thumb}</a><div class=\"item-card-body\">\
         <h3 class=\"item-card-title\"><a href=\"{url}\">{}</a></h3>{values}{}</div></div>",
        escape_html(&name),
        citation(item, &name, ctx),
    )
}

/// The citation line: creator, title, date and identifier, each when the
/// item has it, then the site's name linked to the item's page.
fn citation(item: &ResourceView, name: &str, ctx: &RenderContext<'_>) -> String {
    let first = |term: &str| {
        item.values
            .iter()
            .find(|value| value.property_term == term)
            .map(|value| value.value.trim())
            .filter(|value| !value.is_empty())
    };
    let mut parts: Vec<String> = Vec::new();
    if let Some(creator) = first("dcterms:creator") {
        parts.push(escape_html(creator));
    }
    parts.push(format!("<cite>{}</cite>", escape_html(name)));
    for term in ["dcterms:date", "dcterms:identifier"] {
        if let Some(value) = first(term) {
            parts.push(escape_html(value));
        }
    }
    parts.push(format!(
        "<a href=\"{}\">{}</a>",
        escape_html(&item.url),
        escape_html(&ctx.site.title)
    ));
    format!("<p class=\"item-card-citation\">{}.</p>", parts.join(". "))
}

fn render_list(class: &str, label: &str, entries: &[ResourceRef], with_thumbnails: bool) -> String {
    let mut out = format!(
        "<dl class=\"{class}\"><div class=\"property\"><dt>{}</dt>",
        escape_html(label)
    );
    for entry in entries {
        let link = if with_thumbnails {
            link_pretty(entry)
        } else {
            crate::html::hyperlink(&entry.title, Some(&entry.url), &[])
        };
        out.push_str(&format!("<dd class=\"value\">{link}</dd>"));
    }
    out.push_str("</div></dl>");
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blocks::test_support::{
        TestResources, block, context, item, literal_value, resource_ref, site,
    };
    use crate::view::{AttachmentRef, ResourceRef};
    use omeka_core::ids::{BlockId, ItemId};
    use serde_json::json;

    /// `block-layout/item-with-metadata.phtml`: values, the item-set list and
    /// the media list for each attached item.
    #[test]
    fn item_with_metadata_matches_upstream_block_layout() {
        let site = site();
        let mut resources = TestResources::default();
        let mut item_view = item(4, "An item", "/s/main/item/4");
        item_view.values = vec![literal_value(1, "Title", "An item")];
        item_view
            .item_sets
            .push(resource_ref("A set", "/s/main/item-set/2"));
        item_view
            .media
            .push(resource_ref("A file", "/s/main/media/9"));
        resources.items.insert(4, item_view);
        resources.attachments.insert(
            1,
            vec![AttachmentRef {
                item: Some(ItemId::new(4)),
                media: None,
                caption: String::new(),
            }],
        );
        let mut block = block("itemWithMetadata", json!({}));
        block.id = BlockId::new(1);
        let ctx = context(&site, &resources);
        assert_eq!(
            ItemWithMetadata.render(&block, &ctx),
            "<div class=\"item-with-metadata\"><div class=\"resource show\"><dl class=\"values\"><div class=\"property\"><dt>Title</dt><dd class=\"value literal\" lang=\"\"><span class=\"value-content\">An item</span></dd></div></dl><dl class=\"item-sets\"><div class=\"property\"><dt>Item sets</dt><dd class=\"value\"><a href=\"/s/main/item-set/2\">A set</a></dd></div></dl><dl class=\"media-list\"><div class=\"property\"><dt>Media</dt><dd class=\"value\"><a href=\"/s/main/media/9\" class=\"resource-link\"><span class=\"resource-name\">A file</span></a></dd></div></dl></div></div>"
        );
    }

    /// The media list links pretty: the media's square thumbnail inside the
    /// link, ahead of the `resource-name` span
    /// (`AbstractResourceEntityRepresentation::linkPretty`).
    #[test]
    fn item_with_metadata_media_list_links_pretty() {
        let site = site();
        let mut resources = TestResources::default();
        let mut item_view = item(4, "An item", "/s/main/item/4");
        item_view.media = vec![ResourceRef {
            url: "/s/main/media/9".to_string(),
            title: "A file".to_string(),
            thumbnail: Some(crate::view::Thumbnail::new("/files/square/9.jpg", "A file")),
        }];
        resources.items.insert(4, item_view);
        resources.attachments.insert(
            1,
            vec![AttachmentRef {
                item: Some(ItemId::new(4)),
                media: None,
                caption: String::new(),
            }],
        );
        let mut block = block("itemWithMetadata", json!({}));
        block.id = BlockId::new(1);
        let ctx = context(&site, &resources);
        let out = ItemWithMetadata.render(&block, &ctx);
        assert!(
            out.contains(
                "<dd class=\"value\"><a href=\"/s/main/media/9\" class=\"resource-link\"><img src=\"/files/square/9.jpg\" alt=\"A file\"><span class=\"resource-name\">A file</span></a></dd>"
            ),
            "{out}"
        );
    }

    /// An attachment whose item is gone is skipped; a block with no attachments
    /// at all renders the class's own message.
    #[test]
    fn item_with_metadata_skips_missing_items() {
        let site = site();
        let mut resources = TestResources::default();
        resources.attachments.insert(
            1,
            vec![
                AttachmentRef {
                    item: Some(ItemId::new(99)),
                    media: None,
                    caption: String::new(),
                },
                AttachmentRef {
                    item: None,
                    media: None,
                    caption: String::new(),
                },
            ],
        );
        let mut block = block("itemWithMetadata", json!({}));
        block.id = BlockId::new(1);
        let ctx = context(&site, &resources);
        assert_eq!(
            ItemWithMetadata.render(&block, &ctx),
            "<div class=\"item-with-metadata\"></div>"
        );

        let empty_resources = TestResources::default();
        let empty = context(&site, &empty_resources);
        assert_eq!(ItemWithMetadata.render(&block, &empty), "No item selected");
    }

    /// An item with a title, a creator, two subjects, a date and an
    /// identifier, attached to block 1.
    fn described() -> TestResources {
        let mut resources = TestResources::default();
        let mut item_view = item(4, "River chart", "/s/main/item/4");
        item_view.values = vec![
            literal_value(1, "Title", "River chart"),
            literal_value(2, "Creator", "Ada Byron"),
            literal_value(3, "Subject", "Rivers"),
            literal_value(3, "Subject", "Maps"),
            literal_value(4, "Date", "1901"),
            literal_value(5, "Identifier", "MS-12"),
        ];
        item_view.thumbnail = Some(crate::view::Thumbnail::new(
            "/files/medium/4.jpg",
            "River chart",
        ));
        resources.items.insert(4, item_view);
        resources.attachments.insert(
            1,
            vec![AttachmentRef {
                item: Some(ItemId::new(4)),
                media: None,
                caption: String::new(),
            }],
        );
        resources
    }

    fn styled(settings: serde_json::Value) -> SiteBlock {
        crate::blocks::test_support::block_with_layout(
            "itemWithMetadata",
            json!({}),
            json!({"romeka": {"block": "itemWithMetadata", "v": 2, "settings": settings}}),
        )
    }

    /// The chosen properties print in the chosen order, each with its values
    /// in their stored order; every other value is left out.
    #[test]
    fn item_with_metadata_prints_only_the_chosen_fields() {
        let site = site();
        let resources = described();
        let ctx = context(&site, &resources);
        let out = ItemWithMetadata.render(
            &styled(json!({"fields": ["dcterms:date", "dcterms:subject"]})),
            &ctx,
        );
        assert!(
            out.starts_with(
                "<div class=\"item-with-metadata\"><div class=\"resource show\"><dl class=\"values\">\
                 <div class=\"property\"><dt>Date</dt><dd class=\"value literal\" lang=\"\"><span class=\"value-content\">1901</span></dd></div>\
                 <div class=\"property\"><dt>Subject</dt><dd class=\"value literal\" lang=\"\"><span class=\"value-content\">Rivers</span></dd>\
                 <dd class=\"value literal\" lang=\"\"><span class=\"value-content\">Maps</span></dd></div></dl>"
            ),
            "{out}"
        );
        assert!(!out.contains("Ada Byron"), "{out}");
    }

    /// The card shows the image, the name as a link and a citation line from
    /// the whole record; with no chosen fields it prints no values.
    #[test]
    fn item_with_metadata_card_cites_the_item() {
        let site = site();
        let resources = described();
        let ctx = context(&site, &resources);
        assert_eq!(
            ItemWithMetadata.render(&styled(json!({"display": "card"})), &ctx),
            "<div class=\"item-with-metadata item-cards\"><div class=\"item-card\">\
             <a href=\"/s/main/item/4\" class=\"item-card-thumb\" tabindex=\"-1\" aria-hidden=\"true\">\
             <span class=\"resource-thumb\"><img src=\"/files/medium/4.jpg\" alt=\"\" loading=\"lazy\"></span></a>\
             <div class=\"item-card-body\"><h3 class=\"item-card-title\"><a href=\"/s/main/item/4\">River chart</a></h3>\
             <p class=\"item-card-citation\">Ada Byron. <cite>River chart</cite>. 1901. MS-12. \
             <a href=\"/s/main/item/4\">Main Site</a>.</p></div></div></div>"
        );
        let with_fields = ItemWithMetadata.render(
            &styled(json!({"display": "card", "fields": ["dcterms:subject"]})),
            &ctx,
        );
        assert!(
            with_fields
                .contains("</h3><dl class=\"values\"><div class=\"property\"><dt>Subject</dt>"),
            "{with_fields}"
        );
        assert!(
            with_fields.contains("<p class=\"item-card-citation\">Ada Byron. "),
            "{with_fields}"
        );
    }
}
