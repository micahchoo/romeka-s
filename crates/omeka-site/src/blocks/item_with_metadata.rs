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

use omeka_core::site::{BlockData, SiteBlock};

use crate::blocks::BlockLayout;
use crate::blocks::schema::{LayoutSchema, attachments};
use crate::html::escape_html;
use crate::i18n::Messages;
use crate::view::{RenderContext, ResourceRef, ResourceView};

/// The `itemWithMetadata` block.
#[derive(Debug, Clone, Copy, Default)]
pub struct ItemWithMetadata;

/// `BlockLayout/ItemWithMetadata.php::form`: `blockAttachmentsForm($block,
/// true)`, the item-only attachments list.
const SCHEMA: LayoutSchema = LayoutSchema {
    fields: &[attachments(
        "blockField.itemWithMetadata.attachments",
        false,
    )],
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
        let messages = ctx.translator().messages();
        let mut out = String::from("<div class=\"item-with-metadata\">");
        for item in &items {
            out.push_str(&render_item(item, messages));
        }
        out.push_str("</div>");
        out
    }

    fn schema(&self) -> &'static LayoutSchema {
        &SCHEMA
    }
}

fn render_item(item: &ResourceView, messages: &Messages) -> String {
    let mut out = String::from("<div class=\"resource show\">");
    out.push_str(&render_values(item));
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

fn render_values(item: &ResourceView) -> String {
    if item.values.is_empty() {
        return String::new();
    }
    let mut out = String::from("<dl class=\"values\">");
    for value in &item.values {
        out.push_str(&format!(
            "<div class=\"property\"><dt>{}</dt><dd class=\"value\">{}</dd></div>",
            escape_html(&value.property),
            escape_html(&value.value)
        ));
    }
    out.push_str("</dl>");
    out
}

fn render_list(class: &str, label: &str, entries: &[ResourceRef], with_thumbnails: bool) -> String {
    let mut out = format!(
        "<dl class=\"{class}\"><div class=\"property\"><dt>{}</dt>",
        escape_html(label)
    );
    for entry in entries {
        let link = if with_thumbnails {
            pretty_link(entry)
        } else {
            crate::html::hyperlink(&entry.title, Some(&entry.url), &[])
        };
        out.push_str(&format!("<dd class=\"value\">{link}</dd>"));
    }
    out.push_str("</div></dl>");
    out
}

/// `AbstractResourceEntityRepresentation::linkPretty`: a thumbnail and a
/// `resource-name` span inside the link.
fn pretty_link(entry: &ResourceRef) -> String {
    let thumbnail = entry
        .thumbnail
        .as_ref()
        .map(|thumbnail| {
            format!(
                "<img src=\"{}\" alt=\"{}\">",
                escape_html(&thumbnail.url),
                escape_html(&thumbnail.alt)
            )
        })
        .unwrap_or_default();
    format!(
        "<a href=\"{}\" class=\"resource-link\">{}<span class=\"resource-name\">{}</span></a>",
        escape_html(&entry.url),
        thumbnail,
        escape_html(&entry.title)
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blocks::test_support::{TestResources, block, context, item, resource_ref, site};
    use crate::view::{AttachmentRef, ResourceRef, ValueRef};
    use omeka_core::ids::{BlockId, ItemId};
    use serde_json::json;

    /// `block-layout/item-with-metadata.phtml`: values, the item-set list and
    /// the media list for each attached item.
    #[test]
    fn item_with_metadata_matches_upstream_block_layout() {
        let site = site();
        let mut resources = TestResources::default();
        let mut item_view = item(4, "An item", "/s/main/item/4");
        item_view.values = vec![ValueRef {
            property: "Title".to_string(),
            value: "An item".to_string(),
        }];
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
            "<div class=\"item-with-metadata\"><div class=\"resource show\"><dl class=\"values\"><div class=\"property\"><dt>Title</dt><dd class=\"value\">An item</dd></div></dl><dl class=\"item-sets\"><div class=\"property\"><dt>Item sets</dt><dd class=\"value\"><a href=\"/s/main/item-set/2\">A set</a></dd></div></dl><dl class=\"media-list\"><div class=\"property\"><dt>Media</dt><dd class=\"value\"><a href=\"/s/main/media/9\" class=\"resource-link\"><span class=\"resource-name\">A file</span></a></dd></div></dl></div></div>"
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
}
