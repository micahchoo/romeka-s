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

use omeka_core::ids::SiteId;
use omeka_core::resource::ResourceKind;
use omeka_core::site::{BlockData, BrowsePreviewData, SiteBlock};
use serde_json::Value as JsonValue;

use crate::blocks::schema::{
    LayoutSchema, choice, integer, item_query, multi_choice, option, property, text,
};
use crate::blocks::{BlockLayout, columns};
use crate::html::{escape_html, hyperlink, query_string};
use crate::settings::SiteSettings;
use crate::slug::{controller_for, resource_browse_url};
use crate::theme::cards::{CardParts, Detail, list_class, resource_card};
use crate::theme::choices::CardStyle;
use crate::view::{BrowseQuery, RenderContext};

/// The default `components` (`BlockLayout/BrowsePreview.php::form`).
const DEFAULT_COMPONENTS: [&str; 3] = ["resource-heading", "resource-body", "thumbnail"];
/// The default `limit`.
const DEFAULT_LIMIT: usize = 12;

/// The `browsePreview` block.
#[derive(Debug, Clone, Copy, Default)]
pub struct BrowsePreview;

/// `BlockLayout/BrowsePreview.php::form` and its `$defaults`. `link-text`
/// starts empty rather than at the class's translated "Browse all". The
/// `romeka.` fields are Romeka settings: the card style, the columns and the
/// details under each card's name.
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
        choice(
            "romeka.cards",
            "blockField.browsePreview.cards",
            "fieldHelp.browsePreview.cards",
            &[
                option("", "blockChoice.cards_theme"),
                option("caption", "blockChoice.cards_caption"),
                option("overlay", "blockChoice.cards_overlay"),
                option("row", "blockChoice.cards_row"),
                option("carousel", "blockChoice.cards_carousel"),
            ],
        )
        .default_text(""),
        choice(
            "romeka.columns",
            "blockField.browsePreview.columns",
            "fieldHelp.browsePreview.columns",
            columns::OPTIONS,
        )
        .default_text(""),
        multi_choice(
            "romeka.details",
            "blockField.browsePreview.details",
            "fieldHelp.browsePreview.details",
            &[
                option("type", "blockChoice.detail_type"),
                option("creator", "blockChoice.detail_creator"),
                option("date", "blockChoice.detail_date"),
                option("item_set", "blockChoice.detail_item_set"),
            ],
        )
        .default_list(&["type", "creator", "date"]),
        property(
            "romeka.detail_property",
            "blockField.browsePreview.detail_property",
            "fieldHelp.browsePreview.detail_property",
        ),
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
        render_block(data, &Look::of(block), block, ctx)
    }

    fn schema(&self) -> &'static LayoutSchema {
        &SCHEMA
    }
}

/// How the block draws its cards: in the theme's card style, in a style of
/// its own, or in the theme's style along a carousel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Arrangement {
    Theme,
    Style(CardStyle),
    Carousel,
}

impl Arrangement {
    /// The card style the list draws, given the theme's token. A carousel of
    /// rows is not a carousel, so a carousel on a row theme draws captions.
    fn style(self, theme: CardStyle) -> CardStyle {
        match self {
            Self::Theme => theme,
            Self::Style(style) => style,
            Self::Carousel if theme == CardStyle::Row => CardStyle::Caption,
            Self::Carousel => theme,
        }
    }
}

/// The block's Romeka settings, read once from `o:layout_data.romeka`.
/// Unset, each one keeps the upstream rendering.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Look {
    arrangement: Arrangement,
    /// 2 to 6, or `None` to fit as many cards as the width holds.
    columns: Option<u8>,
    /// The chosen details, `None` for the theme's.
    details: Option<Vec<Detail<'static>>>,
    /// A property whose first value shows after the details.
    property: Option<String>,
}

impl Look {
    fn of(block: &SiteBlock) -> Self {
        let setting = |key| block.layout_data.romeka_setting(key);
        let text = |key| setting(key).and_then(JsonValue::as_str).unwrap_or_default();
        let arrangement = match text("cards") {
            "carousel" => Arrangement::Carousel,
            key => CardStyle::parse(key).map_or(Arrangement::Theme, Arrangement::Style),
        };
        let columns = columns::of(block);
        let details = setting("details")
            .and_then(JsonValue::as_array)
            .map(|names| {
                names
                    .iter()
                    .filter_map(JsonValue::as_str)
                    .filter_map(Detail::parse)
                    .collect()
            });
        let property = Some(text("detail_property").trim())
            .filter(|term| !term.is_empty())
            .map(str::to_string);
        Self {
            arrangement,
            columns,
            details,
            property,
        }
    }

    /// The details each card in `style` shows: the chosen ones or the
    /// style's own, then the property's value.
    fn details(&self, style: CardStyle) -> Vec<Detail<'_>> {
        let mut details: Vec<Detail<'_>> = self
            .details
            .as_deref()
            .unwrap_or(Detail::theme(style))
            .to_vec();
        if let Some(term) = &self.property {
            details.push(Detail::Value(term));
        }
        details
    }
}

/// The resource kind and the search a block runs:
/// `BlockLayout/BrowsePreview.php::render` adds the site, the limit and the
/// default sort to the stored query, and the stored sort decides the order. The page snapshot builds its search with
/// this same function, so the two cannot drift.
#[must_use]
pub fn search(
    data: &BrowsePreviewData,
    site: SiteId,
    settings: &SiteSettings,
) -> (ResourceKind, BrowseQuery) {
    let kind = match data.resource_type.as_deref() {
        Some("item_sets") => ResourceKind::ItemSet,
        Some("media") => ResourceKind::Media,
        _ => ResourceKind::Item,
    };
    let mut query = parse_query(data.query.as_deref().unwrap_or_default());
    if settings.browse_attached_items() {
        query.push(("site_attachments_only".to_string(), "true".to_string()));
    }
    query.push(("site_id".to_string(), site.get().to_string()));
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
    // The API reads the last of a repeated key, so the sort does too.
    let last = |name: &str| {
        query
            .iter()
            .rev()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.clone())
            .unwrap_or_default()
    };
    let sort = (last("sort_by"), last("sort_order"));
    let search = BrowseQuery {
        limit: data.limit.map(|limit| usize::try_from(limit).unwrap_or(0)),
        query,
        site,
        sort: Some(sort),
    };
    (kind, search)
}

fn render_block(
    data: &BrowsePreviewData,
    look: &Look,
    block: &SiteBlock,
    ctx: &RenderContext<'_>,
) -> String {
    let (kind, search) = search(data, ctx.site.id, &ctx.settings);
    let original_query = parse_query(data.query.as_deref().unwrap_or_default());
    let components: Vec<String> = if data.components.is_empty() {
        DEFAULT_COMPONENTS
            .iter()
            .map(|c| (*c).to_string())
            .collect()
    } else {
        data.components.clone()
    };
    let page = ctx.resources.search(kind, &search);

    let heading = data
        .heading
        .as_deref()
        .filter(|heading| !heading.is_empty());
    let mut out = String::from("<div class=\"preview-block\">");
    if let Some(heading) = heading {
        out.push_str(&format!("<h2>{}</h2>", escape_html(heading)));
    }
    if page.resources.is_empty() {
        let messages = ctx.translator().messages();
        let site_is_empty = kind == ResourceKind::Item && !site_has_items(ctx);
        let text = match (site_is_empty, ctx.is_preview()) {
            (true, true) => messages.no_site_items_preview,
            (true, false) => messages.no_site_items,
            (false, true) => messages.no_matches_preview,
            (false, false) => messages.no_matches,
        };
        out.push_str(&format!(
            "<p class=\"preview-empty\">{}</p>",
            escape_html(text)
        ));
        if site_is_empty {
            out.push_str("</div>");
            return out;
        }
    } else {
        let style = look
            .arrangement
            .style(ctx.theme_settings.tokens.structure.cards);
        let details = look.details(style);
        let parts = CardParts {
            thumbnail: components.iter().any(|c| c == "thumbnail"),
            heading: components.iter().any(|c| c == "resource-heading"),
            body: components.iter().any(|c| c == "resource-body"),
            details: &details,
        };
        let cards: Vec<String> = page
            .resources
            .iter()
            .map(|resource| resource_card(kind, resource, ctx, parts))
            .collect();
        out.push_str(&card_list(look, style, &cards, heading, block, ctx));
    }
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

/// The list of cards in the block's look. A block with no Romeka settings
/// prints upstream's `resource-list preview` in the theme's card style; a
/// carousel is the same list, scrolled sideways, with links to step it.
fn card_list(
    look: &Look,
    style: CardStyle,
    cards: &[String],
    heading: Option<&str>,
    block: &SiteBlock,
    ctx: &RenderContext<'_>,
) -> String {
    let mut classes = vec!["resource-list", "preview", list_class(style)];
    let columns = look
        .columns
        .filter(|_| style != CardStyle::Row)
        .map(|count| columns::class("resource", count));
    if let Some(class) = &columns {
        classes.push(class);
    }
    if look.arrangement != Arrangement::Carousel {
        return format!(
            "<ul class=\"{}\">{}</ul>",
            classes.join(" "),
            cards.concat()
        );
    }
    classes.push("resource-carousel");
    let messages = ctx.translator().messages();
    crate::blocks::carousel::render(&crate::blocks::carousel::Carousel {
        id: &format!("carousel-{}", block.id.get()),
        classes: &classes.join(" "),
        cards,
        step: usize::from(look.columns.unwrap_or(4)),
        label: heading.unwrap_or(messages.items),
        prev: messages.prev,
        next: messages.next,
    })
}

/// The one-item search that tells whether the site holds any item, so an
/// empty preview can say why it is empty. The page snapshot runs it too.
#[must_use]
pub fn site_items_probe(site: SiteId) -> BrowseQuery {
    BrowseQuery {
        query: vec![("site_id".to_string(), site.get().to_string())],
        site,
        limit: Some(1),
        sort: None,
    }
}

fn site_has_items(ctx: &RenderContext<'_>) -> bool {
    !ctx.resources
        .search(ResourceKind::Item, &site_items_probe(ctx.site.id))
        .resources
        .is_empty()
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
#[path = "browse_preview_tests.rs"]
mod tests;
