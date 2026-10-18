//! Sites, site pages, site blocks and their layouts, mirroring
//! `application/src/Entity/Site.php`, `Entity/SitePage.php`,
//! `Entity/SitePageBlock.php`, the slug rules in
//! `Api/Adapter/SiteSlugTrait.php` and `Api/Adapter/SiteAdapter.php`, and the
//! block layouts in `application/src/Site/BlockLayout/*.php`.
//!
//! [`Slug`] carries the upstream validity rule: non-empty, and only
//! `[A-Za-z0-9_-]` (`SiteAdapter.php:264`). [`slugify`] mirrors the automatic
//! slug (`SiteSlugTrait::slugify`) for ASCII input; transliteration of non-ASCII
//! needs PHP's `intl`/`iconv`, which core does not reimplement (follow-up).
//!
//! [`BlockData`] parses at the storage boundary from the block's `data` JSON and
//! serialises back to the same keys. The variant is chosen by the stored
//! `layout` name, not by the JSON shape, exactly as the upstream block layout
//! manager dispatches. Layouts whose content lives in `site_page_block_attachment`
//! rows (item-with-metadata, media, asset) carry only their scalar options here.
//!
//! DESIGN.md omits two upstream facts, and upstream wins:
//! - `site.owner_id` is nullable (`ON DELETE SET NULL`, `schema.sql:231`), so
//!   [`Site::owner`] is an `Option`.
//! - `site_page` has no `position` column (`schema.sql:280-300`); page order is
//!   navigation, and block order is `site_page_block.position`.

use std::fmt;

use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;

use crate::ids::{AssetId, BlockId, PageId, SiteId, UserId};

/// A site or page slug, validated against the upstream rule
/// (`SiteAdapter.php:260-266`, `SitePage` uniqueness is per site).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Slug(String);

/// Why a string is not a valid slug.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SlugError {
    /// `$slug === ''` after trim (`SiteAdapter.php:261`).
    #[error("the slug cannot be empty")]
    Empty,
    /// `preg_match('/[^a-zA-Z0-9_-]/u', $slug)`
    /// (`SiteAdapter.php:264`); only letters, digits, underscores and hyphens.
    #[error("a slug can only contain letters, numbers, underscores and hyphens: {0:?}")]
    InvalidCharacter(String),
}

impl Slug {
    /// Validate a raw slug string.
    ///
    /// # Errors
    /// [`SlugError::Empty`] or [`SlugError::InvalidCharacter`].
    pub fn parse(raw: &str) -> Result<Self, SlugError> {
        if raw.is_empty() {
            return Err(SlugError::Empty);
        }
        if !raw
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
        {
            return Err(SlugError::InvalidCharacter(raw.to_string()));
        }
        Ok(Self(raw.to_string()))
    }

    /// The slug as a string.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Slug {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for Slug {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for Slug {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(deserializer)?;
        Self::parse(&raw).map_err(serde::de::Error::custom)
    }
}

/// Whether a string is a valid slug (`SiteAdapter.php:260-266`).
#[must_use]
pub fn is_valid_slug(raw: &str) -> bool {
    Slug::parse(raw).is_ok()
}

/// `SiteSlugTrait::slugify`, ASCII-exact: lowercase, runs of characters outside
/// `[a-z0-9-]` become one `-`, trailing `-` stripped. Upstream transliterates
/// non-ASCII first when `intl` or `iconv` is loaded; core leaves that to the
/// caller and documents the difference.
#[must_use]
pub fn slugify(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut pending_dash = false;
    for ch in input.chars() {
        let lower = ch.to_ascii_lowercase();
        if lower.is_ascii_alphanumeric() {
            if pending_dash {
                out.push('-');
            }
            pending_dash = false;
            out.push(lower);
        } else {
            pending_dash = true;
        }
    }
    out
}

/// A `site` row (`application/src/Entity/Site.php`, `schema.sql:227-250`).
///
/// DESIGN.md's `owner: UserId` is upstream-nullable, so it is an `Option` here.
/// Upstream's `navigation`, `item_pool`, `thumbnail`, `created` and `modified`
/// columns are left to wave 2 (follow-up), since their typed shapes are not
/// fixed yet.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Site {
    /// `site.id`.
    pub id: SiteId,
    /// `site.slug`, unique across sites.
    pub slug: Slug,
    /// `site.theme`.
    pub theme: String,
    /// `site.title`.
    pub title: String,
    /// `site.summary`.
    pub summary: Option<String>,
    /// `site.owner_id`, null after the owner is deleted.
    pub owner: Option<UserId>,
    /// `site.is_public`, true by default (`Site::$isPublic = true`).
    pub is_public: bool,
    /// `site.homepage_id`.
    pub homepage: Option<PageId>,
    /// `site.assign_new_items`, false by default (`Site::$assignNewItems = false`).
    pub assign_new_items: bool,
}

impl Site {
    /// A site with upstream's defaults: public today, no homepage, not
    /// assigning new items.
    #[must_use]
    pub fn new(id: SiteId, slug: Slug, theme: impl Into<String>, title: impl Into<String>) -> Self {
        Self {
            id,
            slug,
            theme: theme.into(),
            title: title.into(),
            summary: None,
            owner: None,
            is_public: true,
            homepage: None,
            assign_new_items: false,
        }
    }
}

/// A `site_page` row (`application/src/Entity/SitePage.php`,
/// `schema.sql:280-300`). There is no `position` column; upstream orders pages
/// through the site's `navigation` data.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SitePage {
    /// `site_page.id`.
    pub id: PageId,
    /// `site_page.site_id`.
    pub site: SiteId,
    /// `site_page.slug`, unique within the site.
    pub slug: Slug,
    /// `site_page.title`.
    pub title: String,
    /// `site_page.is_public`, true by default (`SitePage::$isPublic = true`).
    pub is_public: bool,
    /// `site_page_block` rows, ordered by `position`.
    pub blocks: Vec<SiteBlock>,
}

impl SitePage {
    /// A public page with no blocks.
    #[must_use]
    pub fn new(id: PageId, site: SiteId, slug: Slug, title: impl Into<String>) -> Self {
        Self {
            id,
            site,
            slug,
            title: title.into(),
            is_public: true,
            blocks: Vec::new(),
        }
    }
}

/// A `site_page_block` row (`application/src/Entity/SitePageBlock.php`,
/// `schema.sql:293-310`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SiteBlock {
    /// `site_page_block.id`.
    pub id: BlockId,
    /// `site_page_block.layout`, the registered layout name.
    pub layout: BlockLayout,
    /// `site_page_block.position`, one-based within the page.
    pub position: i64,
    /// `site_page_block.data`, parsed by the block's layout.
    pub data: BlockData,
}

/// A registered block layout (`application/config/module.config.php:805-825`).
///
/// [`BlockLayout::Fallback`] stands for a name with no registered layout. It
/// carries the stored name so a round trip keeps it, exactly as upstream's
/// `Fallback` class preserves the original block data.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum BlockLayout {
    /// `pageTitle`
    PageTitle,
    /// `media`
    Media,
    /// `browsePreview`
    BrowsePreview,
    /// `listOfSites`
    ListOfSites,
    /// `tableOfContents`
    TableOfContents,
    /// `lineBreak`
    LineBreak,
    /// `itemWithMetadata`
    ItemWithMetadata,
    /// `pageDateTime`
    PageDateTime,
    /// `blockGroup`
    BlockGroup,
    /// `iiifPresentation`
    IiifPresentation,
    /// `iiifImage`
    IiifImage,
    /// `asset`
    Asset,
    /// `html`
    Html,
    /// `listOfPages`
    ListOfPages,
    /// `oembed`
    Oembed,
    /// An unregistered layout name, preserving it (`Fallback`).
    Fallback(String),
}

impl BlockLayout {
    /// The string stored in `site_page_block.layout`, or `None` for
    /// [`BlockLayout::Fallback`] whose name is carried by the variant.
    #[must_use]
    pub const fn wire_name(&self) -> Option<&'static str> {
        match self {
            Self::PageTitle => Some("pageTitle"),
            Self::Media => Some("media"),
            Self::BrowsePreview => Some("browsePreview"),
            Self::ListOfSites => Some("listOfSites"),
            Self::TableOfContents => Some("tableOfContents"),
            Self::LineBreak => Some("lineBreak"),
            Self::ItemWithMetadata => Some("itemWithMetadata"),
            Self::PageDateTime => Some("pageDateTime"),
            Self::BlockGroup => Some("blockGroup"),
            Self::IiifPresentation => Some("iiifPresentation"),
            Self::IiifImage => Some("iiifImage"),
            Self::Asset => Some("asset"),
            Self::Html => Some("html"),
            Self::ListOfPages => Some("listOfPages"),
            Self::Oembed => Some("oembed"),
            Self::Fallback(_) => None,
        }
    }

    /// Parse a stored layout name; unknown names become
    /// [`BlockLayout::Fallback`] rather than an error, matching the layout
    /// manager's fallback behaviour.
    #[must_use]
    pub fn from_wire_name(name: &str) -> Self {
        match name {
            "pageTitle" => Self::PageTitle,
            "media" => Self::Media,
            "browsePreview" => Self::BrowsePreview,
            "listOfSites" => Self::ListOfSites,
            "tableOfContents" => Self::TableOfContents,
            "lineBreak" => Self::LineBreak,
            "itemWithMetadata" => Self::ItemWithMetadata,
            "pageDateTime" => Self::PageDateTime,
            "blockGroup" => Self::BlockGroup,
            "iiifPresentation" => Self::IiifPresentation,
            "iiifImage" => Self::IiifImage,
            "asset" => Self::Asset,
            "html" => Self::Html,
            "listOfPages" => Self::ListOfPages,
            "oembed" => Self::Oembed,
            other => Self::Fallback(other.to_string()),
        }
    }

    /// The layout label each class returns from `getLabel`.
    #[must_use]
    pub fn label(&self) -> String {
        match self {
            Self::PageTitle => "Page title".to_string(),
            Self::Media => "Media embed".to_string(),
            Self::BrowsePreview => "Browse preview".to_string(),
            Self::ListOfSites => "List of sites".to_string(),
            Self::TableOfContents => "Table of contents".to_string(),
            Self::LineBreak => "Line break".to_string(),
            Self::ItemWithMetadata => "Item with metadata".to_string(),
            Self::PageDateTime => "Page date/time".to_string(),
            Self::BlockGroup => "Block group".to_string(),
            Self::IiifPresentation => "IIIF presentation".to_string(),
            Self::IiifImage => "IIIF image".to_string(),
            Self::Asset => "Asset".to_string(),
            Self::Html => "HTML".to_string(),
            Self::ListOfPages => "List of pages".to_string(),
            Self::Oembed => "oEmbed".to_string(),
            Self::Fallback(name) => format!("Unknown [{name}]"),
        }
    }
}

impl fmt::Display for BlockLayout {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.wire_name() {
            Some(name) => f.write_str(name),
            None => write!(f, "{self:?}"),
        }
    }
}

/// Why a block's `data` JSON did not parse for its layout.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum BlockDataError {
    /// The JSON shape does not match the layout's keys.
    #[error("invalid block data for layout {layout:?}: {reason}")]
    Invalid {
        /// The layout whose parse failed.
        layout: String,
        /// The underlying serde error.
        reason: String,
    },
}

/// A JSON scalar that PHP may have stored as a number or as a numeric string
/// (Laminas `Number` form elements submit strings). Read either into an `i64`.
fn de_opt_i64<'de, D>(deserializer: D) -> Result<Option<i64>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = Option::<JsonValue>::deserialize(deserializer)?;
    Ok(match value {
        None | Some(JsonValue::Null) => None,
        Some(JsonValue::Number(n)) => n.as_i64(),
        Some(JsonValue::String(s)) => s.trim().parse::<i64>().ok(),
        Some(_) => None,
    })
}

/// One asset attachment inside an `asset` block (`BlockLayout/Asset.php`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssetAttachment {
    /// The asset id (`id`); upstream keeps an empty string for an unset slot.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<AssetId>,
    /// The page id the asset links to (`page`), empty when it links nowhere.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub page: Option<PageId>,
    /// Alternative link text (`alt_link_title`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub alt_link_title: Option<String>,
    /// The caption (`caption`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub caption: Option<String>,
}

/// `BlockLayout/BrowsePreview.php` `data` keys and defaults.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BrowsePreviewData {
    /// `resource_type`, default `items`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resource_type: Option<String>,
    /// `query`, the raw search string.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub query: Option<String>,
    /// `heading`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub heading: Option<String>,
    /// `limit`, a number.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "de_opt_i64"
    )]
    pub limit: Option<i64>,
    /// `components`, the displayed parts of each resource.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub components: Vec<String>,
    /// `link-text`, the label of the browse-all link.
    #[serde(rename = "link-text", default, skip_serializing_if = "Option::is_none")]
    pub link_text: Option<String>,
}

/// `BlockLayout/ListOfSites.php` `data` keys and defaults.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ListOfSitesData {
    /// `sort`, default `alpha`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sort: Option<String>,
    /// `limit`, null for no limit.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "de_opt_i64"
    )]
    pub limit: Option<i64>,
    /// `pagination`, default false.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pagination: Option<bool>,
    /// `summaries`, default true.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub summaries: Option<bool>,
    /// `thumbnails`, default true.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thumbnails: Option<bool>,
    /// `exclude_current`, default true.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exclude_current: Option<bool>,
}

/// `BlockLayout/PageDateTime.php` `data` keys and defaults.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PageDateTimeData {
    /// `display`, default `created_modified`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display: Option<String>,
    /// `date_format`, default `medium`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub date_format: Option<String>,
    /// `time_format`, default `none`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub time_format: Option<String>,
}

/// `BlockLayout/Media.php` `data` keys (attachments carry the media).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MediaData {
    /// `layout`, `horizontal` or absent for vertical.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub layout: Option<String>,
    /// `media_display`, `thumbnail` or absent for embed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub media_display: Option<String>,
    /// `thumbnail_type`, default `square`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thumbnail_type: Option<String>,
    /// `show_title_option`, default `item_title`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub show_title_option: Option<String>,
}

/// `BlockLayout/IiifImage.php` and `IiifPresentation.php` `data` keys.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IiifData {
    /// `image_url` for an image block, `manifest_url` for a presentation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    /// `title`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// `show_title`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub show_title: Option<bool>,
}

/// `BlockLayout/Oembed.php` `data` keys and defaults.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OembedData {
    /// `url`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    /// `url_old`, the previous URL the cached response was fetched for.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url_old: Option<String>,
    /// `refresh`, whether the response must be fetched again.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refresh: Option<bool>,
    /// `oembed`, the cached response object.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub oembed: Option<JsonValue>,
}

/// The typed `site_page_block.data` object, chosen by the block's layout.
///
/// Serialises back to the upstream keys through [`BlockData::to_json`]; it does
/// not derive `Deserialize` because the layout, not the JSON shape, selects the
/// variant. Use [`BlockData::parse`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BlockData {
    /// `html`.
    Html {
        /// The purified HTML (`html`).
        html: String,
    },
    /// `media`.
    Media(MediaData),
    /// `browsePreview`.
    BrowsePreview(BrowsePreviewData),
    /// `listOfSites`.
    ListOfSites(ListOfSitesData),
    /// `tableOfContents`.
    TableOfContents {
        /// `depth`, the number of child page levels.
        depth: i64,
    },
    /// `lineBreak`.
    LineBreak {
        /// `break_type`, `transparent` or `opaque`.
        break_type: String,
    },
    /// `itemWithMetadata`, whose content is its attachments.
    ItemWithMetadata,
    /// `pageDateTime`.
    PageDateTime(PageDateTimeData),
    /// `pageTitle`, which renders the page title and stores nothing.
    PageTitle,
    /// `blockGroup`.
    BlockGroup {
        /// `span`, the group's width hint (a string upstream).
        span: Option<String>,
    },
    /// `iiifPresentation`.
    IiifPresentation(IiifData),
    /// `iiifImage`.
    IiifImage(IiifData),
    /// `asset`, a keyed list of asset attachments.
    Asset(Vec<AssetAttachment>),
    /// `listOfPages`, whose `pagelist` is a JSON string of the page tree.
    ListOfPages {
        /// The raw `pagelist` JSON string upstream stores.
        pagelist: Option<String>,
    },
    /// `oembed`.
    Oembed(OembedData),
    /// Any unregistered layout, its `data` preserved as-is (`Fallback`).
    Fallback(JsonValue),
}

impl BlockData {
    /// Parse a block's `data` for its layout. Unknown layouts keep their data as
    /// [`BlockData::Fallback`]; layouts with no data keys accept any object.
    ///
    /// # Errors
    /// [`BlockDataError::Invalid`] when the JSON does not match the layout's keys.
    pub fn parse(layout: &BlockLayout, data: &JsonValue) -> Result<Self, BlockDataError> {
        fn parse_as<T: serde::de::DeserializeOwned>(
            layout: &BlockLayout,
            data: &JsonValue,
        ) -> Result<T, BlockDataError> {
            serde_json::from_value(data.clone()).map_err(|source| BlockDataError::Invalid {
                layout: layout.to_string(),
                reason: source.to_string(),
            })
        }

        Ok(match layout {
            BlockLayout::Html => Self::Html {
                html: data
                    .get("html")
                    .and_then(JsonValue::as_str)
                    .unwrap_or_default()
                    .to_string(),
            },
            BlockLayout::Media => Self::Media(parse_as(layout, data)?),
            BlockLayout::BrowsePreview => Self::BrowsePreview(parse_as(layout, data)?),
            BlockLayout::ListOfSites => Self::ListOfSites(parse_as(layout, data)?),
            BlockLayout::TableOfContents => Self::TableOfContents {
                depth: data.get("depth").and_then(de_json_i64).unwrap_or(1),
            },
            BlockLayout::LineBreak => Self::LineBreak {
                break_type: data
                    .get("break_type")
                    .and_then(JsonValue::as_str)
                    .unwrap_or("transparent")
                    .to_string(),
            },
            BlockLayout::ItemWithMetadata => Self::ItemWithMetadata,
            BlockLayout::PageDateTime => Self::PageDateTime(parse_as(layout, data)?),
            BlockLayout::PageTitle => Self::PageTitle,
            BlockLayout::BlockGroup => Self::BlockGroup {
                span: data
                    .get("span")
                    .and_then(JsonValue::as_str)
                    .map(str::to_string),
            },
            BlockLayout::IiifPresentation => Self::IiifPresentation(IiifData {
                url: data
                    .get("manifest_url")
                    .and_then(JsonValue::as_str)
                    .map(str::to_string),
                title: data
                    .get("title")
                    .and_then(JsonValue::as_str)
                    .map(str::to_string),
                show_title: data.get("show_title").and_then(JsonValue::as_bool),
            }),
            BlockLayout::IiifImage => Self::IiifImage(IiifData {
                url: data
                    .get("image_url")
                    .and_then(JsonValue::as_str)
                    .map(str::to_string),
                title: data
                    .get("title")
                    .and_then(JsonValue::as_str)
                    .map(str::to_string),
                show_title: data.get("show_title").and_then(JsonValue::as_bool),
            }),
            BlockLayout::Asset => Self::Asset(parse_as(layout, data)?),
            BlockLayout::ListOfPages => Self::ListOfPages {
                pagelist: data
                    .get("pagelist")
                    .and_then(JsonValue::as_str)
                    .map(str::to_string),
            },
            BlockLayout::Oembed => Self::Oembed(parse_as(layout, data)?),
            BlockLayout::Fallback(_) => Self::Fallback(data.clone()),
        })
    }

    /// Serialise back to the block's `data` object, with the upstream keys.
    #[must_use]
    pub fn to_json(&self) -> JsonValue {
        match self {
            Self::Html { html } => serde_json::json!({ "html": html }),
            Self::Media(data) => to_object(data),
            Self::BrowsePreview(data) => to_object(data),
            Self::ListOfSites(data) => to_object(data),
            Self::TableOfContents { depth } => serde_json::json!({ "depth": depth }),
            Self::LineBreak { break_type } => serde_json::json!({ "break_type": break_type }),
            Self::ItemWithMetadata | Self::PageTitle => serde_json::json!({}),
            Self::PageDateTime(data) => to_object(data),
            Self::BlockGroup { span } => match span {
                Some(span) => serde_json::json!({ "span": span }),
                None => serde_json::json!({}),
            },
            Self::IiifPresentation(data) => {
                let mut object = serde_json::json!({});
                if let Some(url) = &data.url {
                    object["manifest_url"] = JsonValue::String(url.clone());
                }
                write_iiif_common(&mut object, data);
                object
            }
            Self::IiifImage(data) => {
                let mut object = serde_json::json!({});
                if let Some(url) = &data.url {
                    object["image_url"] = JsonValue::String(url.clone());
                }
                write_iiif_common(&mut object, data);
                object
            }
            Self::Asset(attachments) => {
                serde_json::to_value(attachments).unwrap_or(JsonValue::Null)
            }
            Self::ListOfPages { pagelist } => match pagelist {
                Some(pagelist) => serde_json::json!({ "pagelist": pagelist }),
                None => serde_json::json!({}),
            },
            Self::Oembed(data) => to_object(data),
            Self::Fallback(data) => data.clone(),
        }
    }
}

impl Serialize for BlockData {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.to_json().serialize(serializer)
    }
}

fn de_json_i64(value: &JsonValue) -> Option<i64> {
    match value {
        JsonValue::Number(n) => n.as_i64(),
        JsonValue::String(s) => s.trim().parse::<i64>().ok(),
        _ => None,
    }
}

fn to_object<T: Serialize>(value: &T) -> JsonValue {
    serde_json::to_value(value).unwrap_or(JsonValue::Null)
}

fn write_iiif_common(object: &mut JsonValue, data: &IiifData) {
    if let Some(title) = &data.title {
        object["title"] = JsonValue::String(title.clone());
    }
    if let Some(show_title) = data.show_title {
        object["show_title"] = JsonValue::Bool(show_title);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `SiteAdapter.php:260-266`: a slug is non-empty and holds only letters,
    /// numbers, underscores and hyphens.
    #[test]
    fn slug_validation_matches_upstream_site_adapter() {
        for valid in ["welcome", "my-page_1", "A", "123", "a_b-c"] {
            assert!(is_valid_slug(valid), "{valid} should be valid");
            assert_eq!(Slug::parse(valid).unwrap().as_str(), valid);
        }
        for invalid in ["", "My Page", "caf\u{e9}", "a/b", "a.b"] {
            assert!(!is_valid_slug(invalid), "{invalid} should be invalid");
        }
        assert_eq!(Slug::parse("").unwrap_err(), SlugError::Empty);
        assert_eq!(
            Slug::parse("a b").unwrap_err(),
            SlugError::InvalidCharacter("a b".to_string())
        );
    }

    /// `SiteSlugTrait::slugify` for ASCII input: lowercase, runs of other
    /// characters collapse to one hyphen, a leading hyphen survives, trailing
    /// hyphen stripped.
    #[test]
    fn slugify_matches_upstream_site_slug_trait() {
        assert_eq!(slugify("Welcome to the Site"), "welcome-to-the-site");
        assert_eq!(slugify("  Trim  Me!!  "), "-trim-me");
        assert_eq!(slugify("a--b"), "a-b");
        assert_eq!(slugify("Trailing ---"), "trailing");
        assert_eq!(slugify("A_B"), "a-b");
        assert_eq!(slugify("!!!"), "");
    }

    /// `Site` defaults match the entity: public, not assigning new items, owner
    /// nullable (`Site.php`, `schema.sql:227-250`).
    #[test]
    fn site_defaults_match_upstream_entity() {
        let slug = Slug::parse("main").unwrap();
        let site = Site::new(SiteId::new(1), slug.clone(), "default", "Main Site");
        assert!(site.is_public);
        assert!(!site.assign_new_items);
        assert_eq!(site.owner, None);
        assert_eq!(site.homepage, None);
        assert_eq!(site.slug, slug);
    }

    /// `site_page` defaults and block ordering (`SitePage.php`,
    /// `schema.sql:280-300`): public by default, blocks by position.
    #[test]
    fn site_page_defaults_match_upstream_entity() {
        let page = SitePage::new(
            PageId::new(5),
            SiteId::new(1),
            Slug::parse("welcome").unwrap(),
            "Welcome",
        );
        assert!(page.is_public);
        assert!(page.blocks.is_empty());
        assert_eq!(page.site, SiteId::new(1));
    }

    /// `module.config.php:805-825`: the registered layout names round-trip, and
    /// an unregistered name becomes the fallback carrying its name.
    #[test]
    fn block_layout_names_match_upstream_registration() {
        let names = [
            "pageTitle",
            "media",
            "browsePreview",
            "listOfSites",
            "tableOfContents",
            "lineBreak",
            "itemWithMetadata",
            "pageDateTime",
            "blockGroup",
            "iiifPresentation",
            "iiifImage",
            "asset",
            "html",
            "listOfPages",
            "oembed",
        ];
        for name in names {
            let layout = BlockLayout::from_wire_name(name);
            assert_eq!(layout.wire_name(), Some(name));
        }
        assert_eq!(BlockLayout::Html.label(), "HTML");
        assert_eq!(BlockLayout::Media.label(), "Media embed");
        let unknown = BlockLayout::from_wire_name("myModuleLayout");
        assert_eq!(unknown, BlockLayout::Fallback("myModuleLayout".to_string()));
        assert_eq!(unknown.wire_name(), None);
        assert_eq!(unknown.label(), "Unknown [myModuleLayout]");
    }

    /// Each layout's `data` parses and serialises back to the same keys
    /// (`Site/BlockLayout/*.php`).
    #[test]
    fn block_data_round_trip_matches_upstream_keys() {
        let cases: &[(&str, JsonValue)] = &[
            ("html", serde_json::json!({"html": "<p>Hi</p>"})),
            (
                "media",
                serde_json::json!({"layout": "horizontal", "media_display": "thumbnail"}),
            ),
            (
                "browsePreview",
                serde_json::json!({
                    "resource_type": "items",
                    "query": "year:2000",
                    "heading": "Latest",
                    "limit": 12,
                    "components": ["resource-heading", "thumbnail"],
                    "link-text": "Browse all"
                }),
            ),
            (
                "listOfSites",
                serde_json::json!({"sort": "alpha", "pagination": false, "exclude_current": true}),
            ),
            ("tableOfContents", serde_json::json!({"depth": 3})),
            ("lineBreak", serde_json::json!({"break_type": "opaque"})),
            ("itemWithMetadata", serde_json::json!({})),
            (
                "pageDateTime",
                serde_json::json!({"display": "created", "date_format": "short", "time_format": "none"}),
            ),
            ("pageTitle", serde_json::json!({})),
            ("blockGroup", serde_json::json!({"span": "6"})),
            (
                "iiifPresentation",
                serde_json::json!({"manifest_url": "https://example.org/m", "title": "M", "show_title": true}),
            ),
            (
                "iiifImage",
                serde_json::json!({"image_url": "https://example.org/i", "show_title": false}),
            ),
            ("asset", serde_json::json!([{"id": 4, "caption": "cap"}])),
            ("listOfPages", serde_json::json!({"pagelist": "[]"})),
            (
                "oembed",
                serde_json::json!({"url": "https://example.org/x", "refresh": false}),
            ),
        ];
        for (name, data) in cases {
            let layout = BlockLayout::from_wire_name(name);
            let parsed = BlockData::parse(&layout, data)
                .unwrap_or_else(|e| panic!("{name} failed to parse: {e}"));
            assert_eq!(&parsed.to_json(), data, "{name} did not round-trip");
        }
    }

    /// PHP form numbers arrive as strings; the parser reads them into ints.
    #[test]
    fn block_data_reads_numeric_strings_like_php_forms() {
        let layout = BlockLayout::from_wire_name("browsePreview");
        let parsed = BlockData::parse(&layout, &serde_json::json!({"limit": "8"})).unwrap();
        let BlockData::BrowsePreview(data) = parsed else {
            panic!("expected browse preview");
        };
        assert_eq!(data.limit, Some(8));
    }

    /// An unregistered layout keeps its data verbatim, as `Fallback` does.
    #[test]
    fn block_data_fallback_preserves_unknown_layout_data() {
        let layout = BlockLayout::from_wire_name("myModuleLayout");
        let data = serde_json::json!({"anything": [1, 2, 3]});
        let parsed = BlockData::parse(&layout, &data).unwrap();
        assert_eq!(parsed, BlockData::Fallback(data.clone()));
        assert_eq!(parsed.to_json(), data);
    }
}
