//! `BlockLayout/Media.php` and `view/common/block-layout/file.phtml`.
//!
//! The block reads its attachments, drops the ones whose item is gone, and
//! collects CSS classes from the stored options:
//!
//! - `layout`: `horizontal` or, by default, `vertical`
//! - `media_display`: `thumbnail` or, by default, `embed`
//! - the attachment count: `attachment-count-N` up to three, then
//!   `multiple-attachments`
//!
//! Each attachment shows the item's media, or the media the attachment names.
//! A thumbnail display links through `attachment_link_type` (`item`, `media` or
//! `original`). An embed asks the media renderer for its markup
//! ([`MediaRef::embed`]), which is the `omeka-media` seam. The title above each
//! entry follows `show_title_option`: `item_title` links the item, `file_name`
//! prints the media title, `no_title` prints no heading.
//!
//! Two departures. An unknown `attachment_link_type` falls back to the item URL
//! where upstream throws. And the stored `thumbnail_type` (`square` by default)
//! picks the thumbnail size upstream; the port's provider supplies one
//! thumbnail per media, so the port places that one. A media the provider has
//! no thumbnail for keeps its slot as the kind placeholder, so a thumbnail
//! display never prints an empty link.

use omeka_core::site::{BlockData, MediaData, SiteBlock};

use crate::blocks::BlockLayout;
use crate::blocks::schema::{LayoutSchema, When, attachments, choice, option};
use crate::html::escape_html;
use crate::view::{AttachmentRef, MediaRef, RenderContext, ResourceView, Thumbnail};

/// The `media` block.
#[derive(Debug, Clone, Copy, Default)]
pub struct Media;

/// `BlockLayout/Media.php::form`, `blockThumbnailTypeSelect` and
/// `blockShowTitleSelect`. The empty value is a real choice: upstream's
/// "Vertical" and "Embed media".
const SCHEMA: LayoutSchema = LayoutSchema {
    fields: &[
        attachments(
            "blockField.media.attachments",
            "fieldHelp.media.attachments",
            true,
        ),
        choice(
            "layout",
            "blockField.media.layout",
            "fieldHelp.media.layout",
            &[
                option("", "blockChoice.vertical"),
                option("horizontal", "blockChoice.horizontal"),
            ],
        )
        .default_text(""),
        choice(
            "media_display",
            "blockField.media.media_display",
            "fieldHelp.media.media_display",
            &[
                option("", "blockChoice.embed"),
                option("thumbnail", "blockChoice.thumbnail"),
            ],
        )
        .default_text(""),
        choice(
            "thumbnail_type",
            "blockField.media.thumbnail_type",
            "fieldHelp.media.thumbnail_type",
            &[
                option("large", "blockChoice.size_large"),
                option("medium", "blockChoice.size_medium"),
                option("square", "blockChoice.size_square"),
            ],
        )
        .default_text("square")
        .when(When::equals("media_display", "thumbnail")),
        choice(
            "show_title_option",
            "blockField.media.show_title_option",
            "fieldHelp.media.show_title_option",
            &[
                option("item_title", "blockChoice.item_title"),
                option("file_name", "blockChoice.file_name"),
                option("no_title", "blockChoice.no_title"),
            ],
        )
        .default_text("item_title"),
    ],
};

impl BlockLayout for Media {
    fn name(&self) -> &'static str {
        "media"
    }

    fn label(&self) -> &'static str {
        "Media embed"
    }

    fn render(&self, block: &SiteBlock, ctx: &RenderContext<'_>) -> String {
        let BlockData::Media(data) = &block.data else {
            return String::new();
        };
        let attachments = resolve(block, ctx);
        if attachments.is_empty() {
            return String::new();
        }
        let classes = classes(&attachments, data);
        let mut out = format!("<div class=\"{}\">", escape_html(&classes));
        for attachment in &attachments {
            out.push_str(&render_attachment(attachment, data, ctx));
        }
        out.push_str("</div>");
        out
    }

    fn schema(&self) -> &'static LayoutSchema {
        &SCHEMA
    }
}

/// One attachment with its item and media resolved, the ones the provider
/// cannot find dropped as the class drops a missing item.
struct Resolved {
    item: ResourceView,
    media: Option<MediaRef>,
    caption: String,
}

fn resolve(block: &SiteBlock, ctx: &RenderContext<'_>) -> Vec<Resolved> {
    ctx.resources
        .attachments(block.id)
        .into_iter()
        .filter_map(|attachment| resolve_one(&attachment, ctx))
        .collect()
}

fn resolve_one(attachment: &AttachmentRef, ctx: &RenderContext<'_>) -> Option<Resolved> {
    let item = attachment.item.and_then(|id| {
        ctx.resources
            .read(omeka_core::resource::ResourceKind::Item, id.get())
    })?;
    let media = attachment
        .media
        .and_then(|id| ctx.resources.read_media(id))
        .or_else(|| item.primary_media.clone());
    Some(Resolved {
        item,
        media,
        caption: attachment.caption.clone(),
    })
}

fn classes(attachments: &[Resolved], data: &MediaData) -> String {
    let mut classes = vec!["media-embed".to_string()];
    classes.push(match data.layout.as_deref() {
        Some("horizontal") => "layout-horizontal".to_string(),
        _ => "layout-vertical".to_string(),
    });
    classes.push(match data.media_display.as_deref() {
        Some("thumbnail") => "media-display-thumbnail".to_string(),
        _ => "media-display-embed".to_string(),
    });
    if attachments.len() > 3 {
        classes.push("multiple-attachments".to_string());
    } else {
        classes.push(format!("attachment-count-{}", attachments.len()));
    }
    classes.join(" ")
}

fn render_attachment(attachment: &Resolved, data: &MediaData, ctx: &RenderContext<'_>) -> String {
    let mut out = String::from("<div class=\"item resource\">");
    if let Some(media) = &attachment.media {
        if data.media_display.as_deref() == Some("thumbnail") {
            let url = thumbnail_link(media, &attachment.item, ctx);
            let thumbnail = match &media.thumbnail {
                Some(Thumbnail { url, alt }) => format!(
                    "<img src=\"{}\" alt=\"{}\">",
                    escape_html(url),
                    escape_html(alt)
                ),
                // A media with no thumbnail keeps the slot rather than leaving
                // the link empty: the kind placeholder names what the link
                // opens.
                None => crate::theme::cards::empty_thumb(
                    omeka_core::resource::ResourceKind::Media,
                    if media.title.is_empty() {
                        crate::theme::cards::kind_label(
                            omeka_core::resource::ResourceKind::Media,
                            ctx,
                        )
                    } else {
                        &media.title
                    },
                ),
            };
            out.push_str(&format!(
                "<a href=\"{}\">{}</a>",
                escape_html(&url),
                thumbnail
            ));
        } else if let Some(embed) = &media.embed {
            out.push_str(embed);
        }
    }
    match data.show_title_option.as_deref() {
        Some("file_name") => {
            let title = attachment
                .media
                .as_ref()
                .map(|media| media.title.as_str())
                .unwrap_or_default();
            out.push_str(&format!("<h3>{}</h3>", escape_html(title)));
        }
        // `item_title` is the default when the key is absent; any other stored
        // value, including `no_title` and the empty string, prints no heading.
        None | Some("item_title") => out.push_str(&format!(
            "<h3>{}</h3>",
            crate::html::hyperlink(&attachment.item.title, Some(&attachment.item.url), &[])
        )),
        _ => {}
    }
    if !attachment.caption.is_empty() {
        // `file.phtml` echoes the caption raw; upstream purified it on write
        // (`SitePageAdapter::hydrateAttachments`). The render purifies again so
        // a row stored before that write path did still cannot execute.
        out.push_str(&format!(
            "<div class=\"caption\">{}</div>",
            omeka_core::purify::purify(&attachment.caption)
        ));
    }
    out.push_str("</div>");
    out
}

/// The URL a thumbnail links through, from `attachment_link_type`.
fn thumbnail_link(media: &MediaRef, item: &ResourceView, ctx: &RenderContext<'_>) -> String {
    match ctx.settings.attachment_link_type() {
        "original" => media
            .original_url
            .clone()
            .unwrap_or_else(|| media.url.clone()),
        "media" => media.url.clone(),
        _ => item.url.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blocks::test_support::{TestResources, block, context, item, media_ref, site};
    use crate::view::{AttachmentRef, Thumbnail};
    use omeka_core::ids::{BlockId, ItemId, MediaId};
    use serde_json::json;

    fn resources() -> TestResources {
        let mut resources = TestResources::default();
        let mut item_view = item(4, "An item", "/s/main/item/4");
        item_view.primary_media = Some(media_ref("A file", "/s/main/media/9"));
        resources.items.insert(4, item_view);
        resources
    }

    /// `block-layout/file.phtml` with the class list
    /// `BlockLayout/Media.php::render` builds: a thumbnail display links the
    /// item and puts the item title in an `<h3>`.
    #[test]
    fn media_matches_upstream_block_layout() {
        let site = site();
        let mut resources = resources();
        resources.media.insert(
            9,
            MediaRef {
                url: "/s/main/media/9".to_string(),
                original_url: Some("/files/original/9.jpg".to_string()),
                title: "A file".to_string(),
                media_type: None,
                thumbnail: Some(Thumbnail::new("/files/square/9.jpg", "A file")),
                embed: None,
            },
        );
        let mut block = block(
            "media",
            json!({"layout": "horizontal", "media_display": "thumbnail", "show_title_option": "item_title"}),
        );
        block.id = BlockId::new(1);
        resources.attachments.insert(
            1,
            vec![AttachmentRef {
                item: Some(ItemId::new(4)),
                media: Some(MediaId::new(9)),
                caption: "A caption".to_string(),
            }],
        );
        let ctx = context(&site, &resources);
        let out = Media.render(&block, &ctx);
        assert!(
            out.starts_with("<div class=\"media-embed layout-horizontal media-display-thumbnail attachment-count-1\">"),
            "{out}"
        );
        assert!(
            out.contains(
                "<a href=\"/s/main/item/4\"><img src=\"/files/square/9.jpg\" alt=\"A file\"></a>"
            ),
            "{out}"
        );
        assert!(
            out.contains("<h3><a href=\"/s/main/item/4\">An item</a></h3>"),
            "{out}"
        );
        assert!(
            out.contains("<div class=\"caption\">A caption</div>"),
            "{out}"
        );
    }

    /// `attachment_link_type` picks the link target, and an embed display
    /// places the renderer's markup.
    #[test]
    fn media_link_type_and_embed_match_upstream() {
        let site = site();
        let mut resources = resources();
        resources.media.insert(
            9,
            MediaRef {
                url: "/s/main/media/9".to_string(),
                original_url: Some("/files/original/9.jpg".to_string()),
                title: "A file".to_string(),
                media_type: None,
                thumbnail: Some(Thumbnail::new("/files/square/9.jpg", "A file")),
                embed: Some("<iframe src=\"/embed/9\"></iframe>".to_string()),
            },
        );
        resources.attachments.insert(
            1,
            vec![AttachmentRef {
                item: Some(ItemId::new(4)),
                media: Some(MediaId::new(9)),
                caption: String::new(),
            }],
        );
        let mut ctx = context(&site, &resources);
        ctx.settings.attachment_link_type = Some("original".to_string());
        let stored = block(
            "media",
            json!({"media_display": "thumbnail", "show_title_option": "file_name"}),
        );
        let out = Media.render(&stored, &ctx);
        assert!(out.contains("<a href=\"/files/original/9.jpg\">"), "{out}");
        assert!(out.contains("<h3>A file</h3>"), "{out}");

        ctx.settings.attachment_link_type = None;
        let embedded = block("media", json!({"media_display": "embed"}));
        let out = Media.render(&embedded, &ctx);
        assert!(out.contains("<iframe src=\"/embed/9\"></iframe>"), "{out}");
        assert!(out.contains("layout-vertical media-display-embed"), "{out}");
    }

    /// More than three attachments switch the class, and a missing item drops
    /// the attachment (`BlockLayout/Media.php::render`).
    #[test]
    fn media_attachment_count_and_missing_item() {
        let site = site();
        let mut resources = resources();
        resources.attachments.insert(
            1,
            vec![
                AttachmentRef {
                    item: Some(ItemId::new(4)),
                    media: None,
                    caption: String::new(),
                },
                AttachmentRef {
                    item: Some(ItemId::new(4)),
                    media: None,
                    caption: String::new(),
                },
                AttachmentRef {
                    item: Some(ItemId::new(4)),
                    media: None,
                    caption: String::new(),
                },
                AttachmentRef {
                    item: Some(ItemId::new(4)),
                    media: None,
                    caption: String::new(),
                },
                AttachmentRef {
                    item: Some(ItemId::new(99)),
                    media: None,
                    caption: String::new(),
                },
            ],
        );
        let ctx = context(&site, &resources);
        let out = Media.render(&block("media", json!({})), &ctx);
        assert!(out.contains("multiple-attachments"), "{out}");
        assert_eq!(
            out.matches("<div class=\"item resource\">").count(),
            4,
            "{out}"
        );
    }

    /// No attachments render nothing, as the class returns early.
    #[test]
    fn media_without_attachments_renders_nothing() {
        let site = site();
        let resources = TestResources::default();
        let ctx = context(&site, &resources);
        assert_eq!(Media.render(&block("media", json!({})), &ctx), "");
    }

    /// A media with no thumbnail still gets a thumbnail slot: the kind
    /// placeholder stands in, so the link is never empty. The block's `file.phtml`
    /// prints the link around whatever the slot holds.
    #[test]
    fn media_thumbnail_mode_without_a_thumbnail_shows_the_kind_placeholder() {
        let site = site();
        let mut resources = resources();
        resources.media.insert(
            9,
            MediaRef {
                url: "/s/main/media/9".to_string(),
                original_url: None,
                title: "A file".to_string(),
                media_type: None,
                thumbnail: None,
                embed: None,
            },
        );
        resources.attachments.insert(
            1,
            vec![AttachmentRef {
                item: Some(ItemId::new(4)),
                media: Some(MediaId::new(9)),
                caption: String::new(),
            }],
        );
        let ctx = context(&site, &resources);
        let stored = block(
            "media",
            json!({"media_display": "thumbnail", "show_title_option": "no_title"}),
        );
        let out = Media.render(&stored, &ctx);
        assert!(
            out.contains(
                "<a href=\"/s/main/item/4\"><span class=\"resource-thumb resource-thumb-empty\" data-kind=\"media\">"
            ),
            "{out}"
        );
        assert!(
            out.contains("<span class=\"resource-kind\">A file</span>"),
            "{out}"
        );
        assert!(
            !out.contains("<a href=\"/s/main/item/4\"></a>"),
            "an empty thumbnail link: {out}"
        );
    }

    /// `block-layout/file.phtml`: `show_title_option` `no_title` is the third
    /// branch and prints no heading, so the `<h3>` appears only for
    /// `item_title` and `file_name`.
    #[test]
    fn media_no_title_option_matches_upstream_file_phtml() {
        let site = site();
        let mut resources = resources();
        resources.media.insert(
            9,
            MediaRef {
                url: "/s/main/media/9".to_string(),
                original_url: None,
                title: "A file".to_string(),
                media_type: None,
                thumbnail: Some(Thumbnail::new("/files/square/9.jpg", "A file")),
                embed: None,
            },
        );
        resources.attachments.insert(
            1,
            vec![AttachmentRef {
                item: Some(ItemId::new(4)),
                media: Some(MediaId::new(9)),
                caption: String::new(),
            }],
        );
        let ctx = context(&site, &resources);
        let stored = block(
            "media",
            json!({"media_display": "thumbnail", "show_title_option": "no_title"}),
        );
        let out = Media.render(&stored, &ctx);
        assert!(out.contains("<img src=\"/files/square/9.jpg\""), "{out}");
        assert!(!out.contains("<h3>"), "{out}");

        // A stored empty value is not `item_title`, so it prints no heading
        // either.
        let stored = block(
            "media",
            json!({"media_display": "thumbnail", "show_title_option": ""}),
        );
        assert!(!Media.render(&stored, &ctx).contains("<h3>"));
    }
}
