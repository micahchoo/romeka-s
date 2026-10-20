//! The page and block layout: `View/Helper/PageLayout.php::render` and
//! `View/Helper/AbstractLayout.php`.
//!
//! `PageLayout` opens the `blocks-inner` element with the page's layout classes
//! and gaps, and wraps a `blockGroup`'s children. `BlockLayout::render` wraps
//! every other block in `block block-{layout}`; its classes and inline styles
//! come from `AbstractLayout::getBlockClasses` and `getBlockInlineStyles`, which
//! read the page's grid and the block's layout data.
//!
//! The port keeps the upstream validation: a CSS length must match
//! `LengthCssDataType::PATTERN`, a background colour must be a `<hex-color>`, and
//! a grid position or span outside the page's columns falls back to `auto` or to
//! the column count. A bare number is pixels (`prepareLength`).
//!
//! After upstream's classes and styles each wrapper carries the Romeka-only
//! styling of [`super::style`], and a group with a section layout carries the
//! section's classes. Upstream ignores both, so its wrapper is the same div.

use omeka_core::ids::BlockId;
use omeka_core::site::{BlockLayout, PageLayout, PageLayoutData, SiteBlock};

use super::style::{RomekaStyle, Section, is_hex_color};
use crate::html::escape_html;
use crate::view::RenderContext;

/// The opening `<div class="blocks-inner …">` for a page, with the page
/// layout's classes and, for a grid, its pixel gaps (`PageLayout::render`).
#[must_use]
pub(crate) fn page_open(layout: PageLayout, data: &PageLayoutData) -> String {
    let mut classes = Vec::new();
    let mut styles = Vec::new();
    match layout {
        PageLayout::Grid => {
            let columns = data.grid_columns.unwrap_or(0);
            let column_gap = data.grid_column_gap.unwrap_or(10);
            let row_gap = data.grid_row_gap.unwrap_or(10);
            classes.push("page-layout-grid".to_string());
            classes.push(format!("grid-template-columns-{columns}"));
            styles.push(format!("column-gap: {column_gap}px;"));
            styles.push(format!("row-gap: {row_gap}px;"));
        }
        PageLayout::Normal => classes.push("page-layout-normal".to_string()),
    }
    format!(
        "<div class=\"blocks-inner {}\" style=\"{}\">",
        escape_html(&classes.join(" ")),
        escape_html(&styles.join(" "))
    )
}

/// The classes `AbstractLayout::getBlockClasses` puts on one block's wrapper.
#[must_use]
pub(crate) fn block_classes(
    block: &SiteBlock,
    page_layout: PageLayout,
    columns: i64,
) -> Vec<String> {
    let mut classes = vec![
        "block".to_string(),
        format!("block-{}", layout_name(&block.layout)),
    ];
    // A blockGroup's grid placement is set by the page around its children.
    if page_layout == PageLayout::Grid && block.layout != BlockLayout::BlockGroup {
        classes.push(position_class(
            block.layout_data.grid_column_position.as_deref(),
            columns,
        ));
        classes.push(span_class(
            block.layout_data.grid_column_span.as_deref(),
            columns,
        ));
    }
    let data = &block.layout_data;
    let romeka = RomekaStyle::parse(&data.romeka_raw);
    if let Some(class) = data.class.as_deref()
        && !class.trim().is_empty()
    {
        classes.push(class.to_string());
    }
    push_known(
        &mut classes,
        data.alignment_block.as_deref(),
        &[
            ("left", "block-layout-alignment-block-left"),
            ("right", "block-layout-alignment-block-right"),
            ("center", "block-layout-alignment-block-center"),
        ],
    );
    push_known(
        &mut classes,
        data.alignment_text.as_deref(),
        &[
            ("left", "block-layout-alignment-text-left"),
            ("center", "block-layout-alignment-text-center"),
            ("right", "block-layout-alignment-text-right"),
            ("justify", "block-layout-alignment-text-justify"),
        ],
    );
    if data.background_image_asset.is_some()
        || romeka.background.is_some()
        || data
            .background_color
            .as_deref()
            .is_some_and(|color| !color.is_empty())
    {
        classes.push("has-background".to_string());
    }
    push_known(
        &mut classes,
        data.background_image_position_y.as_deref(),
        &[
            ("top", "block-layout-background-image-position-y-top"),
            ("center", "block-layout-background-image-position-y-center"),
            ("bottom", "block-layout-background-image-position-y-bottom"),
        ],
    );
    push_known(
        &mut classes,
        data.background_image_position_x.as_deref(),
        &[
            ("left", "block-layout-background-image-position-x-left"),
            ("center", "block-layout-background-image-position-x-center"),
            ("right", "block-layout-background-image-position-x-right"),
        ],
    );
    push_known(
        &mut classes,
        data.background_image_size.as_deref(),
        &[
            ("cover", "block-layout-background-image-size-cover"),
            ("contain", "block-layout-background-image-size-contain"),
        ],
    );
    if romeka.bleed {
        classes.push("romeka-bleed".to_string());
    }
    classes
}

/// The inline styles `AbstractLayout::getBlockInlineStyles` puts on one block's
/// wrapper, in upstream's order.
#[must_use]
pub(crate) fn block_inline_styles(block: &SiteBlock, ctx: &RenderContext<'_>) -> Vec<String> {
    let data = &block.layout_data;
    let mut styles = Vec::new();
    if let Some(color) = data.background_color.as_deref()
        && is_hex_color(color)
    {
        styles.push(format!("background-color: {color}"));
    }
    if let Some(id) = data.background_image_asset
        && let Some(asset) = ctx.resources.read_asset(id)
        && let Some(url) = asset.url.as_deref()
    {
        styles.push(format!("background-image: url(\"{}\")", escape_css(url)));
    }
    push_length(&mut styles, "max-width", data.max_width.as_deref());
    push_length(&mut styles, "min-height", data.min_height.as_deref());
    push_length(&mut styles, "padding-top", data.padding_top.as_deref());
    push_length(&mut styles, "padding-right", data.padding_right.as_deref());
    push_length(
        &mut styles,
        "padding-bottom",
        data.padding_bottom.as_deref(),
    );
    push_length(&mut styles, "padding-left", data.padding_left.as_deref());
    styles.extend(RomekaStyle::parse(&data.romeka_raw).styles());
    styles
}

/// ` data-romeka-block="n"`, or nothing when the render carries no markers.
fn marker_attribute(marker: Option<BlockId>) -> String {
    marker.map_or_else(String::new, |id| format!(" data-romeka-block=\"{id}\""))
}

/// ` id="anchor"`, or nothing for a block without one. The anchor is a
/// validated slug, so it needs no escaping, but it is escaped all the same.
fn id_attribute(anchor: Option<&str>) -> String {
    anchor.map_or_else(String::new, |id| format!(" id=\"{}\"", escape_html(id)))
}

/// The `<div>` `BlockLayout::render` wraps a block in.
#[must_use]
pub(crate) fn block_wrapper(
    classes: &[String],
    styles: &[String],
    anchor: Option<&str>,
    marker: Option<BlockId>,
    body: &str,
) -> String {
    format!(
        "<div{} class=\"{}\" style=\"{}\"{}>{}</div>",
        id_attribute(anchor),
        escape_html(&classes.join(" ")),
        escape_html(&styles.join("; ")),
        marker_attribute(marker),
        body
    )
}

/// The opening `<div>` for a `blockGroup`: `PageLayout::render`'s own branch,
/// which appends the grid classes to the block's own, then the section's
/// classes when the group has a section layout.
#[must_use]
pub(crate) fn group_open(
    block: &SiteBlock,
    page_layout: PageLayout,
    columns: i64,
    anchor: Option<&str>,
    ctx: &RenderContext<'_>,
) -> String {
    let mut classes = block_classes(block, page_layout, columns);
    if page_layout == PageLayout::Grid {
        classes.push("block-group-grid".to_string());
        classes.push("grid-position-1".to_string());
        classes.push(format!("grid-span-{columns}"));
    }
    if let Some(section) = Section::parse(&block.layout_data.romeka_raw) {
        classes.extend(section.classes());
    }
    let styles = block_inline_styles(block, ctx);
    format!(
        "<div{} class=\"{}\" style=\"{}\"{}>",
        id_attribute(anchor),
        escape_html(&classes.join(" ")),
        escape_html(&styles.join("; ")),
        marker_attribute(ctx.block_marker(block.id))
    )
}

/// The stored name of a block's layout: the registered name, or the name a
/// fallback carries, as `$block->layout()` returns it.
fn layout_name(layout: &BlockLayout) -> &str {
    match layout {
        BlockLayout::Fallback(name) => name,
        other => other.wire_name().unwrap_or_default(),
    }
}

/// Push the class a value's case names, and nothing for another value.
fn push_known(classes: &mut Vec<String>, value: Option<&str>, cases: &[(&str, &str)]) {
    if let Some(value) = value
        && let Some((_, class)) = cases.iter().find(|(name, _)| *name == value)
    {
        classes.push((*class).to_string());
    }
}

/// `getValidPositionClass`: a column in range keeps its name, everything else
/// (including the absent default `grid-position-auto`) is `grid-position-auto`.
fn position_class(value: Option<&str>, columns: i64) -> String {
    match value {
        Some("auto") => "grid-position-auto".to_string(),
        Some(value) => match value.trim().parse::<i64>() {
            Ok(number) if number >= 1 && number <= columns => format!("grid-position-{value}"),
            _ => "grid-position-auto".to_string(),
        },
        None => "grid-position-auto".to_string(),
    }
}

/// `getValidSpanClass`: a span in range keeps its name, everything else is the
/// page's column count, which is also the absent default.
fn span_class(value: Option<&str>, columns: i64) -> String {
    match value
        .map(str::trim)
        .and_then(|value| value.parse::<i64>().ok())
    {
        Some(number) if number >= 1 && number <= columns => format!("grid-span-{number}"),
        _ => format!("grid-span-{columns}"),
    }
}

/// A CSS `<length>: `LengthCssDataType::PATTERN`, `^(\d*\.?\d+)(unit)?$`.
///
/// The unit set is the pattern's, and the number needs at least one digit with
/// at most one dot before it. An empty unit is valid, and `prepare_length`
/// turns such a bare number into pixels.
fn is_valid_length(value: &str) -> bool {
    const UNITS: &[&str] = &[
        "%", "cap", "ch", "em", "ex", "ic", "lh", "rem", "rlh", "vh", "svh", "lvh", "dvh", "vw",
        "svw", "lvw", "dvw", "vmax", "svmax", "lvmax", "dvmax", "vmin", "svmin", "lvmin", "dvmin",
        "vb", "svb", "lvb", "dvb", "vi", "svi", "lvi", "dvi", "cqw", "cqh", "cqi", "cqb", "cqmin",
        "cqmax", "px", "cm", "mm", "Q", "in", "pc", "pt",
    ];
    let number_len = value
        .bytes()
        .take_while(|b| b.is_ascii_digit() || *b == b'.')
        .count();
    let (number, unit) = value.split_at(number_len);
    let mut dots = 0_u8;
    let mut digits = 0_u8;
    for byte in number.bytes() {
        match byte {
            b'.' if dots == 0 => dots += 1,
            b'.' => return false,
            _ if byte.is_ascii_digit() => digits += 1,
            _ => return false,
        }
    }
    if digits == 0 || !number.bytes().last().is_some_and(|b| b.is_ascii_digit()) {
        return false;
    }
    unit.is_empty() || UNITS.contains(&unit)
}

/// `$prepareLength`: a bare number becomes pixels, a length with a unit passes.
fn prepare_length(value: &str) -> String {
    if value.bytes().all(|b| b.is_ascii_digit() || b == b'.') {
        format!("{value}px")
    } else {
        value.to_string()
    }
}

/// Push `name: <length>` when the value is a valid length.
fn push_length(styles: &mut Vec<String>, name: &str, value: Option<&str>) {
    if let Some(value) = value
        && is_valid_length(value)
    {
        styles.push(format!("{name}: {}", prepare_length(value)));
    }
}

/// `Laminas\Escaper\Escaper::escapeCss` as the background-image URL uses it:
/// every byte outside `[A-Za-z0-9]` becomes `\` and its uppercase hex, then a
/// space. Measured against the oracle, which renders `\3A`, `\2F` and `\2E` for
/// `:`, `/` and `.`; the vendor tree is not in this checkout.
fn escape_css(value: &str) -> String {
    let mut out = String::with_capacity(value.len() * 2);
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() {
            out.push(char::from(byte));
        } else {
            out.push('\\');
            out.push_str(&format!("{byte:X}"));
            out.push(' ');
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blocks::test_support::{TestResources, block_with_layout, context, site};
    use crate::view::{AssetView, Thumbnail};
    use omeka_core::ids::AssetId;
    use serde_json::json;

    /// `LengthCssDataType::PATTERN` and the hex-colour rule.
    #[test]
    fn length_and_colour_validation_match_upstream() {
        for valid in [
            "0", "12", "1.5", ".5", "40rem", "10px", "100%", "5Q", "1.5em",
        ] {
            assert!(is_valid_length(valid), "{valid} should be a length");
        }
        for invalid in ["", "abc", "5.", "5px5", "-5", "5 px", "5e3", "5o"] {
            assert!(
                !is_valid_length(invalid),
                "{invalid} should not be a length"
            );
        }
        assert!(is_hex_color("#fff"));
        assert!(is_hex_color("#FFEEEE"));
        assert!(!is_hex_color("red"));
        assert!(!is_hex_color("#ff"));
        assert!(!is_hex_color("#ggg"));
    }

    /// A bare number is pixels, a unit passes through (`prepareLength`), and a
    /// background image's URL is escaped the way `escapeCss` escapes it.
    #[test]
    fn prepare_length_and_escape_css() {
        assert_eq!(prepare_length("200"), "200px");
        assert_eq!(prepare_length(".5"), ".5px");
        assert_eq!(prepare_length("40rem"), "40rem");
        assert_eq!(
            escape_css("/files/original/4.jpg"),
            "\\2F files\\2F original\\2F 4\\2E jpg"
        );
    }

    /// The background image's URL comes from the asset the provider resolves,
    /// and an unresolved asset yields no style.
    #[test]
    fn background_image_uses_the_resolved_asset_url() {
        let site = site();
        let mut resources = TestResources::default();
        resources.assets.insert(
            4,
            AssetView {
                id: AssetId::new(4),
                title: "A background".to_string(),
                url: Some("/files/original/4.jpg".to_string()),
                thumbnail: Some(Thumbnail::new("/files/square/4.jpg", "A background")),
            },
        );
        let ctx = context(&site, &resources);
        let block = block_with_layout(
            "html",
            json!({"html": "<p>x</p>"}),
            json!({"background_image_asset": "4"}),
        );
        assert_eq!(
            block_inline_styles(&block, &ctx),
            ["background-image: url(\"\\2F files\\2F original\\2F 4\\2E jpg\")"]
        );

        let ctx = context(&site, &crate::view::NoResources);
        assert!(block_inline_styles(&block, &ctx).is_empty());
    }
}
