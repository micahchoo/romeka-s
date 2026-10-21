//! `BlockLayout/Asset.php` and `view/common/block-layout/asset.phtml`.
//!
//! Each entry in the stored `data` list is one asset slot with a link target
//! page, an alternative link title and a caption. The class resolves the asset
//! id to an asset and the page id to a page of the current site
//! (`prepareAssetAttachments`); the template wraps the thumbnail in a link when
//! the page resolves, with the page title or the alternative as the link text.
//!
//! Core keeps the stored slots as `AssetAttachment`, which holds ids. The site
//! layer resolves them through [`ResourceProvider::read_asset`] and
//! [`RenderContext::pages`].

use omeka_core::site::{AssetAttachment, BlockData, SiteBlock};
use serde_json::Value as JsonValue;

use crate::blocks::schema::{
    Accepts, LayoutSchema, When, asset_ref, choice, link, option, repeat, text,
};
use crate::blocks::{BlockLayout, columns};
use crate::html::escape_html;
use crate::slug::page_url;
use crate::view::{AssetView, RenderContext, Thumbnail};

/// The `asset` block.
#[derive(Debug, Clone, Copy, Default)]
pub struct Asset;

/// `BlockLayout/Asset.php::form` and `common/asset-block-form.phtml`: a repeat
/// group keyed by slot index, one `id`, `page`, `alt_link_title` and `caption`
/// per attachment. A slot's link is a page or, as a Romeka setting, a URL; the
/// `romeka.` fields after the group are Romeka settings of the whole block.
const SCHEMA: LayoutSchema = LayoutSchema {
    fields: &[
        repeat(
            "blockField.asset.attachments",
            "fieldHelp.asset.attachments",
            &[
                asset_ref(
                    "id",
                    "blockField.asset.id",
                    "fieldHelp.asset.id",
                    Accepts::Any,
                ),
                link(
                    "page",
                    "romeka.url",
                    "blockField.asset.link",
                    "fieldHelp.asset.link",
                ),
                text(
                    "alt_link_title",
                    "blockField.asset.alt_link_title",
                    "fieldHelp.asset.alt_link_title",
                ),
                text(
                    "caption",
                    "blockField.asset.caption",
                    "fieldHelp.asset.caption",
                ),
            ],
        ),
        choice(
            "romeka.fit",
            "blockField.asset.fit",
            "fieldHelp.asset.fit",
            &[
                option("", "blockChoice.fit_natural"),
                option("cover", "blockChoice.fit_cover"),
                option("contain", "blockChoice.fit_contain"),
            ],
        )
        .default_text(""),
        choice(
            "romeka.arrangement",
            "blockField.asset.arrangement",
            "fieldHelp.asset.arrangement",
            &[
                option("", "blockChoice.arrangement_stack"),
                option("row", "blockChoice.arrangement_row"),
                option("grid", "blockChoice.arrangement_grid"),
            ],
        )
        .default_text(""),
        choice(
            "romeka.columns",
            "blockField.asset.columns",
            "fieldHelp.asset.columns",
            columns::OPTIONS,
        )
        .default_text("")
        .when(When::equals("romeka.arrangement", "grid")),
        choice(
            "romeka.caption_align",
            "blockField.asset.caption_align",
            "fieldHelp.asset.caption_align",
            &[
                option("", "blockChoice.align_start"),
                option("center", "blockChoice.align_center"),
                option("end", "blockChoice.align_end"),
            ],
        )
        .default_text(""),
    ],
};

impl BlockLayout for Asset {
    fn name(&self) -> &'static str {
        "asset"
    }

    fn label(&self) -> &'static str {
        "Asset"
    }

    fn render(&self, block: &SiteBlock, ctx: &RenderContext<'_>) -> String {
        let BlockData::Asset(attachments) = &block.data else {
            return String::new();
        };
        render_attachments(attachments, &Look::of(block), ctx)
    }

    fn schema(&self) -> &'static LayoutSchema {
        &SCHEMA
    }
}

/// The block's Romeka settings. Unset, the block keeps upstream's markup:
/// the images stacked at their own shape, the captions where the theme puts
/// them, and links to pages only.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct Look {
    /// The classes the `assets` element adds for the fit, the arrangement,
    /// the columns and the caption alignment.
    classes: Vec<String>,
    /// Each slot's URL, by slot index, used when the slot names no page.
    urls: Vec<Option<String>>,
}

impl Look {
    fn of(block: &SiteBlock) -> Self {
        let text = |key| {
            block
                .layout_data
                .romeka_setting(key)
                .and_then(JsonValue::as_str)
                .unwrap_or_default()
        };
        let mut classes = Vec::new();
        if let fit @ ("cover" | "contain") = text("fit") {
            classes.push(format!("assets-fit-{fit}"));
        }
        match text("arrangement") {
            "row" => classes.push("assets-row".to_string()),
            "grid" => {
                classes.push("assets-grid".to_string());
                if let Some(count) = columns::of(block) {
                    classes.push(columns::class("assets", count));
                }
            }
            _ => {}
        }
        if let align @ ("center" | "end") = text("caption_align") {
            classes.push(format!("assets-caption-{align}"));
        }
        let urls = block
            .layout_data
            .romeka_setting("attachments")
            .and_then(JsonValue::as_array)
            .map(|slots| {
                slots
                    .iter()
                    .map(|slot| {
                        slot.get("url")
                            .and_then(JsonValue::as_str)
                            .and_then(crate::blocks::setting::safe_url)
                    })
                    .collect()
            })
            .unwrap_or_default();
        Self { classes, urls }
    }
}

fn render_attachments(
    attachments: &[AssetAttachment],
    look: &Look,
    ctx: &RenderContext<'_>,
) -> String {
    let mut out = String::from("<div class=\"assets");
    for class in &look.classes {
        out.push(' ');
        out.push_str(class);
    }
    out.push_str("\">");
    for (index, attachment) in attachments.iter().enumerate() {
        out.push_str("<div class=\"asset\">");
        let asset = attachment.id.and_then(|id| ctx.resources.read_asset(id));
        let image = asset
            .as_ref()
            .filter(|asset| asset.is_image())
            .and_then(thumbnail_markup)
            .unwrap_or_default();
        let alt_title = attachment
            .alt_link_title
            .as_deref()
            .filter(|title| !title.is_empty());
        let page = attachment.page.and_then(|id| {
            ctx.pages
                .iter()
                .find(|page| page.id == id)
                .map(|page| (page.title.as_str(), page_url(&ctx.site.slug, &page.slug)))
        });
        // The page wins when a slot names both, as upstream knows only pages.
        let target = match page {
            Some((page_title, url)) => Some((Some(alt_title.unwrap_or(page_title)), url)),
            None => look
                .urls
                .get(index)
                .cloned()
                .flatten()
                .map(|url| (alt_title, url)),
        };
        // A file that is not a picture is a link to the file itself, so a slot
        // link around it would nest two anchors. The slot's page and URL are
        // left out for that slot.
        if let Some(file) = asset.as_ref().filter(|asset| !asset.is_image()) {
            out.push_str(&file_markup(file, ctx));
        } else {
            match target {
                Some((title, url)) => {
                    let title = title.map_or_else(String::new, |title| {
                        format!("<span class=\"link-title\">{}</span>", escape_html(title))
                    });
                    out.push_str(&format!(
                        "<a href=\"{}\">{image}{title}</a>",
                        escape_html(&url),
                    ));
                }
                None => out.push_str(&image),
            }
        }
        if let Some(caption) = attachment.caption.as_deref().filter(|c| !c.is_empty()) {
            out.push_str(&format!(
                "<div class=\"caption\">{}</div>",
                escape_html(caption)
            ));
        }
        out.push_str("</div>");
    }
    out.push_str("</div>");
    out
}

fn thumbnail_markup(asset: &AssetView) -> Option<String> {
    let Thumbnail { url, alt } = asset.thumbnail.clone()?;
    Some(format!(
        "<img src=\"{}\" alt=\"{}\">",
        escape_html(&url),
        escape_html(&alt)
    ))
}

/// A file that is not a picture: a link to it named by its alt text or the
/// asset's name, with the kind in words and the size beside that.
fn file_markup(asset: &AssetView, ctx: &RenderContext<'_>) -> String {
    let Some(url) = asset.file_url() else {
        return String::new();
    };
    let mut facts = asset.kind.label(ctx.translator().messages()).to_string();
    if let Some(size) = asset.size {
        facts.push_str(", ");
        facts.push_str(&file_size(size));
    }
    format!(
        "<a class=\"asset-file\" href=\"{}\">{}<span class=\"asset-facts\">{}</span></a>",
        escape_html(url),
        escape_html(asset.name()),
        escape_html(&facts)
    )
}

/// A file's size as the block prints it: whole bytes below a kilobyte, then
/// the decimal unit a file manager shows, one decimal below ten units.
fn file_size(bytes: u64) -> String {
    const UNITS: [&str; 4] = ["kB", "MB", "GB", "TB"];
    if bytes < 1000 {
        return format!("{bytes} B");
    }
    let mut value = bytes as f64 / 1000.0;
    let mut unit = 0;
    while value >= 999.95 && unit + 1 < UNITS.len() {
        value /= 1000.0;
        unit += 1;
    }
    if value < 9.95 {
        format!("{value:.1} {}", UNITS[unit])
    } else {
        format!("{value:.0} {}", UNITS[unit])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blocks::test_support::{TestResources, block, context, site};
    use crate::nav::PageRef;
    use crate::view::{AssetKind, NoResources};
    use omeka_core::ids::{AssetId, PageId};
    use omeka_core::site::Slug;
    use serde_json::json;

    fn pages() -> Vec<PageRef> {
        vec![PageRef {
            id: PageId::new(7),
            slug: Slug::parse("about").unwrap(),
            title: "About".to_string(),
            is_public: true,
        }]
    }

    /// `block-layout/asset.phtml`: a linked thumbnail with a caption, the page
    /// title inside `link-title`.
    #[test]
    fn asset_matches_upstream_block_layout() {
        let site = site();
        let mut resources = TestResources::default();
        resources.assets.insert(
            4,
            AssetView {
                id: AssetId::new(4),
                title: "Logo".to_string(),
                url: None,
                thumbnail: Some(Thumbnail::new("/files/square/4.jpg", "Logo")),
                kind: AssetKind::Image,
                size: None,
            },
        );
        let mut ctx = context(&site, &resources);
        ctx.pages = pages();
        let stored = block(
            "asset",
            json!([{"id": 4, "page": 7, "alt_link_title": "", "caption": "A caption"}]),
        );
        assert_eq!(
            Asset.render(&stored, &ctx),
            "<div class=\"assets\"><div class=\"asset\"><a href=\"/s/main/page/about\"><img src=\"/files/square/4.jpg\" alt=\"Logo\"><span class=\"link-title\">About</span></a><div class=\"caption\">A caption</div></div></div>"
        );
    }

    /// An alternative link title replaces the page title, and a slot whose page
    /// is gone renders the image alone. An empty asset id means no image.
    #[test]
    fn asset_uses_alt_link_title_and_skips_missing_pages() {
        let site = site();
        let mut resources = TestResources::default();
        resources.assets.insert(
            4,
            AssetView {
                id: AssetId::new(4),
                title: "Logo".to_string(),
                url: None,
                thumbnail: Some(Thumbnail::new("/files/square/4.jpg", "Logo")),
                kind: AssetKind::Image,
                size: None,
            },
        );
        let mut ctx = context(&site, &resources);
        ctx.pages = pages();
        let stored = block(
            "asset",
            json!([
                {"id": 4, "page": 7, "alt_link_title": "Read on", "caption": ""},
                {"id": 4, "page": 99, "alt_link_title": "", "caption": ""}
            ]),
        );
        let out = Asset.render(&stored, &ctx);
        assert!(
            out.contains("<a href=\"/s/main/page/about\"><img src=\"/files/square/4.jpg\" alt=\"Logo\"><span class=\"link-title\">Read on</span></a>"),
            "{out}"
        );
        assert_eq!(
            out,
            "<div class=\"assets\"><div class=\"asset\"><a href=\"/s/main/page/about\"><img src=\"/files/square/4.jpg\" alt=\"Logo\"><span class=\"link-title\">Read on</span></a></div><div class=\"asset\"><img src=\"/files/square/4.jpg\" alt=\"Logo\"></div></div>"
        );
    }

    /// A slot with no asset and no page renders an empty entry, as the template
    /// does when `$assetImage` is empty.
    #[test]
    fn asset_without_data_renders_empty_entries() {
        let site = site();
        let ctx = context(&site, &NoResources);
        let stored = block("asset", json!([{"id": "", "page": 99, "caption": ""}]));
        assert_eq!(
            Asset.render(&stored, &ctx),
            "<div class=\"assets\"><div class=\"asset\"></div></div>"
        );
    }

    fn logo() -> TestResources {
        let mut resources = TestResources::default();
        resources.assets.insert(
            4,
            AssetView {
                id: AssetId::new(4),
                title: "Logo".to_string(),
                url: None,
                thumbnail: Some(Thumbnail::new("/files/asset/4.png", "Logo")),
                kind: AssetKind::Image,
                size: None,
            },
        );
        resources
    }

    /// An asset that is not a picture: the slot links the file by its name,
    /// the kind in words and the size, with no `<img>` and no slot page link
    /// nested inside the file link. A file with alt text is named by it.
    #[test]
    fn a_non_image_asset_renders_as_a_link() {
        let site = site();
        let mut resources = TestResources::default();
        resources.assets.insert(
            4,
            AssetView {
                id: AssetId::new(4),
                title: "report.pdf".to_string(),
                url: Some("/files/asset/4.pdf".to_string()),
                thumbnail: Some(Thumbnail::new("/files/asset/4.pdf", "")),
                kind: AssetKind::Pdf,
                size: Some(2_400_000),
            },
        );
        resources.assets.insert(
            5,
            AssetView {
                id: AssetId::new(5),
                title: "minutes.docx".to_string(),
                url: Some("/files/asset/5.docx".to_string()),
                thumbnail: Some(Thumbnail::new("/files/asset/5.docx", "The minutes")),
                kind: AssetKind::File,
                size: None,
            },
        );
        let mut ctx = context(&site, &resources);
        ctx.pages = pages();
        let stored = block(
            "asset",
            json!([
                {"id": 4, "page": 7, "caption": "The annual report"},
                {"id": 5}
            ]),
        );
        assert_eq!(
            Asset.render(&stored, &ctx),
            "<div class=\"assets\">\
             <div class=\"asset\"><a class=\"asset-file\" href=\"/files/asset/4.pdf\">report.pdf<span class=\"asset-facts\">PDF, 2.4 MB</span></a><div class=\"caption\">The annual report</div></div>\
             <div class=\"asset\"><a class=\"asset-file\" href=\"/files/asset/5.docx\">The minutes<span class=\"asset-facts\">File</span></a></div>\
             </div>"
        );
    }

    /// A size reads in the unit a reader expects, with one decimal below ten.
    #[test]
    fn a_size_reads_in_its_largest_whole_unit() {
        assert_eq!(file_size(812), "812 B");
        assert_eq!(file_size(4_200), "4.2 kB");
        assert_eq!(file_size(12_000), "12 kB");
        assert_eq!(file_size(1_300_000), "1.3 MB");
        assert_eq!(file_size(999_999), "1.0 MB");
        assert_eq!(file_size(2_400_000_000), "2.4 GB");
    }

    /// A block with Romeka settings under its layout data.
    fn styled(data: serde_json::Value, settings: serde_json::Value) -> SiteBlock {
        crate::blocks::test_support::block_with_layout(
            "asset",
            data,
            json!({"romeka": {"block": "asset", "v": 2, "settings": settings}}),
        )
    }

    /// The fit, the arrangement, its columns and the caption alignment become
    /// classes on the `assets` element; a column count needs the grid.
    #[test]
    fn asset_settings_become_classes() {
        let site = site();
        let resources = logo();
        let ctx = context(&site, &resources);
        let slots = json!([{"id": 4}, {"id": 4}]);
        let grid = styled(
            slots.clone(),
            json!({"fit": "contain", "arrangement": "grid", "columns": "3", "caption_align": "center"}),
        );
        assert!(
            Asset.render(&grid, &ctx).starts_with(
                "<div class=\"assets assets-fit-contain assets-grid assets-columns-3 assets-caption-center\">"
            ),
            "{}",
            Asset.render(&grid, &ctx)
        );
        let row = styled(
            slots,
            json!({"arrangement": "row", "columns": "3", "fit": "natural"}),
        );
        assert!(
            Asset
                .render(&row, &ctx)
                .starts_with("<div class=\"assets assets-row\">"),
            "{}",
            Asset.render(&row, &ctx)
        );
    }

    /// A slot links to its URL when it names no page, with the alternative
    /// title as the link text when there is one; a page wins over a URL, and a
    /// `javascript:` URL links nowhere.
    #[test]
    fn asset_links_to_a_url_when_no_page_is_named() {
        let site = site();
        let resources = logo();
        let mut ctx = context(&site, &resources);
        ctx.pages = pages();
        let stored = styled(
            json!([
                {"id": 4, "alt_link_title": "Our shop"},
                {"id": 4},
                {"id": 4, "page": 7},
                {"id": 4}
            ]),
            json!({"attachments": [
                {"url": "https://example.org/shop"},
                {"url": "/s/main/item/3"},
                {"url": "https://example.org/ignored"},
                {"url": "javascript:alert(1)"}
            ]}),
        );
        assert_eq!(
            Asset.render(&stored, &ctx),
            "<div class=\"assets\">\
             <div class=\"asset\"><a href=\"https://example.org/shop\"><img src=\"/files/asset/4.png\" alt=\"Logo\"><span class=\"link-title\">Our shop</span></a></div>\
             <div class=\"asset\"><a href=\"/s/main/item/3\"><img src=\"/files/asset/4.png\" alt=\"Logo\"></a></div>\
             <div class=\"asset\"><a href=\"/s/main/page/about\"><img src=\"/files/asset/4.png\" alt=\"Logo\"><span class=\"link-title\">About</span></a></div>\
             <div class=\"asset\"><img src=\"/files/asset/4.png\" alt=\"Logo\"></div>\
             </div>"
        );
    }
}
