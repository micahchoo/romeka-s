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
//! [`PageLayoutData`] and [`BlockLayoutData`] parse the page's and the block's
//! `layout_data` JSON. Every member is optional and wrong kinds read as absent,
//! because `layoutDataValue` returns a default for a missing key.
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
    /// `site_page.layout`, the normal flow or a column grid.
    pub layout: PageLayout,
    /// `site_page.layout_data`, the grid's columns and gaps.
    pub layout_data: PageLayoutData,
    /// `site_page_block` rows, ordered by `position`.
    pub blocks: Vec<SiteBlock>,
}

impl SitePage {
    /// A public page with the normal flow layout and no blocks.
    #[must_use]
    pub fn new(id: PageId, site: SiteId, slug: Slug, title: impl Into<String>) -> Self {
        Self {
            id,
            site,
            slug,
            title: title.into(),
            is_public: true,
            layout: PageLayout::Normal,
            layout_data: PageLayoutData::default(),
            blocks: Vec::new(),
        }
    }
}

/// `site_page.layout`: the normal flow or a CSS grid
/// (`View/Helper/PageLayout.php::render` switches on the name).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PageLayout {
    /// The empty string or any unregistered name: normal flow.
    #[default]
    Normal,
    /// `grid`: a grid of `grid_columns` columns with the stored gaps.
    Grid,
}

impl PageLayout {
    /// Read the stored `site_page.layout` value; only `grid` selects the grid,
    /// every other value (including `NULL`) is the normal flow.
    #[must_use]
    pub fn from_stored(stored: Option<&str>) -> Self {
        match stored {
            Some("grid") => Self::Grid,
            _ => Self::Normal,
        }
    }

    /// The value `site_page.layout` stores.
    #[must_use]
    pub const fn wire_name(self) -> &'static str {
        match self {
            Self::Normal => "",
            Self::Grid => "grid",
        }
    }
}

impl Serialize for PageLayout {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.wire_name())
    }
}

/// `site_page.layout_data` (`Form/PageLayoutDataForm.php` and the grid branch
/// of `View/Helper/PageLayout.php::render`).
///
/// `PageLayout::render` reads only the grid members; `template_name` and
/// `class` round-trip from storage but the port does not act on them.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize)]
pub struct PageLayoutData {
    /// `template_name`, a theme page template (`PageLayout::render` ignores it).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub template_name: Option<String>,
    /// `class`, a styling class on the page (`PageLayout::render` ignores it).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub class: Option<String>,
    /// `grid_columns`, the column count (`(int)` cast).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub grid_columns: Option<i64>,
    /// `grid_column_gap`, pixels between columns, default 10.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub grid_column_gap: Option<i64>,
    /// `grid_row_gap`, pixels between rows, default 10.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub grid_row_gap: Option<i64>,
}

impl PageLayoutData {
    /// Read `site_page.layout_data`. A value that is not an object, or a member
    /// of the wrong kind, reads as absent, the way `layoutDataValue` reads a
    /// missing key. PHP forms submit numbers as strings, so a grid member
    /// accepts either.
    #[must_use]
    pub fn parse(data: &JsonValue) -> Self {
        Self {
            template_name: opt_text(data, "template_name"),
            class: opt_text(data, "class"),
            grid_columns: data.get("grid_columns").and_then(de_json_i64),
            grid_column_gap: data.get("grid_column_gap").and_then(de_json_i64),
            grid_row_gap: data.get("grid_row_gap").and_then(de_json_i64),
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
    /// `site_page_block.layout_data`, the block's placement and styling.
    pub layout_data: BlockLayoutData,
}

/// `site_page_block.layout_data` (`View/Helper/AbstractLayout.php` and
/// `Form/BlockLayoutDataForm.php`).
///
/// Every member is optional and read with the shape upstream reads it: text
/// members take a JSON string only, because `getBlockInlineStyles` guards the
/// CSS lengths with `is_string`, and the grid placement members take a string
/// or a number because `getBlockClasses` compares them loosely. `parse` never
/// fails: a member of the wrong kind reads as absent, which is what
/// `layoutDataValue` does for a missing key.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize)]
pub struct BlockLayoutData {
    /// `template_name`, a theme block template (`BlockLayout::render` ignores
    /// it in this port).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub template_name: Option<String>,
    /// `class`, an extra class on the block's wrapper.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub class: Option<String>,
    /// `grid_column_position`, `auto` or a one-based column.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub grid_column_position: Option<String>,
    /// `grid_column_span`, the number of columns the block spans.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub grid_column_span: Option<String>,
    /// `alignment_block`, `left`, `right` or `center`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub alignment_block: Option<String>,
    /// `alignment_text`, `left`, `center`, `right` or `justify`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub alignment_text: Option<String>,
    /// `max_width`, a CSS length.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_width: Option<String>,
    /// `min_height`, a CSS length.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_height: Option<String>,
    /// `padding_top`, a CSS length.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub padding_top: Option<String>,
    /// `padding_right`, a CSS length.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub padding_right: Option<String>,
    /// `padding_bottom`, a CSS length.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub padding_bottom: Option<String>,
    /// `padding_left`, a CSS length.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub padding_left: Option<String>,
    /// `background_color`, a CSS `<hex-color>`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub background_color: Option<String>,
    /// `background_image_asset`, the asset shown behind the block.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub background_image_asset: Option<AssetId>,
    /// `background_image_position_y`, `top`, `center` or `bottom`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub background_image_position_y: Option<String>,
    /// `background_image_position_x`, `left`, `center` or `right`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub background_image_position_x: Option<String>,
    /// `background_image_size`, `cover` or `contain`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub background_image_size: Option<String>,
    /// `romeka`, the carrier of the block's Romeka settings: a Romeka block
    /// stored under an upstream layout, or the Romeka settings of an upstream
    /// block (a section, shared styling, a viewer height), which name the
    /// block's own layout. Upstream ignores it and its admin keeps it (see
    /// `docs/compat.md`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub romeka: Option<RomkaCarrier>,
}

/// The `o:layout_data.romeka` member of a Romeka block's carrier.
///
/// A Romeka block is stored under an upstream layout, usually `html`, with its
/// own settings here. `plain_hash` is the hash of the block's stored plain
/// rendering (`o:data.html`); a mismatch means the plain rendering was edited
/// outside Romeka S, and the block renders that edit rather than the rich form.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RomkaCarrier {
    /// `block`, the registered Romeka block name.
    pub block: String,
    /// `v`, the carrier version.
    pub v: i64,
    /// `settings`, the block's own settings, as the block reads them.
    #[serde(default)]
    pub settings: JsonValue,
    /// `plain_hash`, the hash of the plain rendering this product wrote.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plain_hash: Option<String>,
}

impl RomkaCarrier {
    /// Read `o:layout_data.romeka`. A value that is not an object, or that
    /// names no block, reads as absent: upstream's own layout data is the only
    /// data such a block then has.
    #[must_use]
    pub fn parse(data: &JsonValue) -> Option<Self> {
        let object = data.as_object()?;
        let block = object.get("block").and_then(JsonValue::as_str)?;
        if block.is_empty() {
            return None;
        }
        let settings = object
            .get("settings")
            .cloned()
            .unwrap_or_else(|| JsonValue::Object(serde_json::Map::new()));
        Some(Self {
            block: block.to_string(),
            v: object.get("v").and_then(de_json_i64).unwrap_or(1),
            settings,
            plain_hash: object
                .get("plain_hash")
                .and_then(JsonValue::as_str)
                .map(str::to_string),
        })
    }
}

impl BlockLayoutData {
    /// Read `site_page_block.layout_data`.
    #[must_use]
    pub fn parse(data: &JsonValue) -> Self {
        Self {
            template_name: opt_text(data, "template_name"),
            class: opt_text(data, "class"),
            grid_column_position: opt_scalar_text(data, "grid_column_position"),
            grid_column_span: opt_scalar_text(data, "grid_column_span"),
            alignment_block: opt_text(data, "alignment_block"),
            alignment_text: opt_text(data, "alignment_text"),
            max_width: opt_text(data, "max_width"),
            min_height: opt_text(data, "min_height"),
            padding_top: opt_text(data, "padding_top"),
            padding_right: opt_text(data, "padding_right"),
            padding_bottom: opt_text(data, "padding_bottom"),
            padding_left: opt_text(data, "padding_left"),
            background_color: opt_text(data, "background_color"),
            background_image_asset: opt_asset_id(data, "background_image_asset"),
            background_image_position_y: opt_text(data, "background_image_position_y"),
            background_image_position_x: opt_text(data, "background_image_position_x"),
            background_image_size: opt_text(data, "background_image_size"),
            romeka: data.get("romeka").and_then(RomkaCarrier::parse),
        }
    }

    /// `romeka.settings.<key>`, the one read of a block's Romeka settings;
    /// `None` when the block keeps no carrier or the carrier no such setting.
    #[must_use]
    pub fn romeka_setting(&self, key: &str) -> Option<&JsonValue> {
        self.romeka.as_ref()?.settings.get(key)
    }
}

/// A text member: a JSON string only, as `is_string` guards it upstream.
fn opt_text(data: &JsonValue, key: &str) -> Option<String> {
    data.get(key)
        .and_then(JsonValue::as_str)
        .map(str::to_string)
}

/// A member that may arrive as text or as a number (PHP forms and JSON
/// numbers): the grid placement members.
fn opt_scalar_text(data: &JsonValue, key: &str) -> Option<String> {
    match data.get(key)? {
        JsonValue::String(text) => Some(text.clone()),
        JsonValue::Number(number) => Some(number.to_string()),
        _ => None,
    }
}

/// An id member: a JSON number, or a numeric string as the asset form submits.
fn opt_asset_id(data: &JsonValue, key: &str) -> Option<AssetId> {
    match data.get(key)? {
        JsonValue::Number(number) => number.as_u64().map(AssetId::new),
        JsonValue::String(text) => text.trim().parse::<u64>().ok().map(AssetId::new),
        _ => None,
    }
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
        /// `span`, the number of blocks after the group that it holds. Upstream
        /// reads it as `(int) $block->dataValue('span')`, so a JSON number and
        /// a numeric string both arrive; the number is kept as its text.
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
                span: data.get("span").and_then(|span| match span {
                    JsonValue::String(text) => Some(text.clone()),
                    JsonValue::Number(number) => Some(number.to_string()),
                    _ => None,
                }),
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

    /// `PageLayout::render` switches on `site_page.layout`; only `grid` is the
    /// grid and everything else, `NULL` included, is the normal flow.
    #[test]
    fn page_layout_reads_only_grid_as_the_grid() {
        assert_eq!(PageLayout::from_stored(Some("grid")), PageLayout::Grid);
        assert_eq!(PageLayout::from_stored(None), PageLayout::Normal);
        assert_eq!(PageLayout::from_stored(Some("")), PageLayout::Normal);
        assert_eq!(PageLayout::from_stored(Some("other")), PageLayout::Normal);
        assert_eq!(PageLayout::Grid.wire_name(), "grid");
        assert_eq!(PageLayout::Normal.wire_name(), "");
    }

    /// `PageLayoutDataForm.php` keys: the grid members accept a number or a
    /// numeric string, `template_name` and `class` are strings, and a member of
    /// another kind reads as absent.
    #[test]
    fn page_layout_data_parses_form_and_stored_shapes() {
        let data = serde_json::json!({
            "template_name": "full",
            "class": "home",
            "grid_columns": "6",
            "grid_column_gap": 20,
            "grid_row_gap": null
        });
        let parsed = PageLayoutData::parse(&data);
        assert_eq!(parsed.template_name.as_deref(), Some("full"));
        assert_eq!(parsed.class.as_deref(), Some("home"));
        assert_eq!(parsed.grid_columns, Some(6));
        assert_eq!(parsed.grid_column_gap, Some(20));
        assert_eq!(parsed.grid_row_gap, None);

        let parsed = PageLayoutData::parse(&serde_json::json!({"grid_columns": false}));
        assert_eq!(parsed.grid_columns, None);
        assert_eq!(
            PageLayoutData::parse(&serde_json::json!("nope")),
            PageLayoutData::default()
        );

        let serialised = serde_json::to_value(&parsed).unwrap();
        assert_eq!(serialised, serde_json::json!({}));
    }

    /// `AbstractLayout::getBlockClasses` / `getBlockInlineStyles` read these
    /// members: text members are strings, the grid placement takes a string or a
    /// number, and `background_image_asset` takes an id or a numeric string.
    #[test]
    fn block_layout_data_parses_form_and_stored_shapes() {
        let data = serde_json::json!({
            "class": "r-hero",
            "grid_column_position": 3,
            "grid_column_span": "6",
            "alignment_block": "center",
            "alignment_text": "justify",
            "max_width": "40rem",
            "min_height": 200,
            "padding_top": "1em",
            "background_color": "#ffeeee",
            "background_image_asset": "4",
            "background_image_position_y": "top",
            "background_image_position_x": "right",
            "background_image_size": "cover"
        });
        let parsed = BlockLayoutData::parse(&data);
        assert_eq!(parsed.class.as_deref(), Some("r-hero"));
        assert_eq!(parsed.grid_column_position.as_deref(), Some("3"));
        assert_eq!(parsed.grid_column_span.as_deref(), Some("6"));
        assert_eq!(parsed.alignment_block.as_deref(), Some("center"));
        assert_eq!(parsed.alignment_text.as_deref(), Some("justify"));
        assert_eq!(parsed.max_width.as_deref(), Some("40rem"));
        // A number on a text member reads as absent, as `is_string` ignores it.
        assert_eq!(parsed.min_height, None);
        assert_eq!(parsed.padding_top.as_deref(), Some("1em"));
        assert_eq!(parsed.background_color.as_deref(), Some("#ffeeee"));
        assert_eq!(parsed.background_image_asset, Some(AssetId::new(4)));
        assert_eq!(parsed.background_image_position_y.as_deref(), Some("top"));
        assert_eq!(parsed.background_image_position_x.as_deref(), Some("right"));
        assert_eq!(parsed.background_image_size.as_deref(), Some("cover"));

        let empty = BlockLayoutData::parse(&serde_json::json!("nope"));
        assert_eq!(empty, BlockLayoutData::default());
        assert_eq!(
            serde_json::to_value(BlockLayoutData::default()).unwrap(),
            serde_json::json!({})
        );
    }
}
