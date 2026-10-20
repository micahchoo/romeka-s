//! What a page shows as a card in a list-of-pages block: an image and a
//! summary, both taken from the page's own blocks.
//!
//! The data model has no page thumbnail and no page summary, so the card reads
//! them from the content a curator already wrote: the image of the first block
//! that shows one (an asset, a media or a Romeka block with an asset setting,
//! such as a hero background) and the first paragraph of the first HTML block.
//! [`source`] finds both without I/O; the server resolves the image and hands
//! the result over [`crate::view::ResourceProvider::page_card`].

use omeka_core::ids::{AssetId, BlockId};
use omeka_core::site::{BlockData, SitePage};

use crate::blocks::romeka;

/// The longest summary a card prints, in characters.
const SUMMARY_MAX: usize = 180;

/// Where a page's card image comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CardImage {
    /// An asset, named by an asset block or a Romeka block's settings.
    Asset(AssetId),
    /// The first media attached to a media block.
    MediaBlock(BlockId),
}

/// A page's card, before its image is resolved.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CardSource {
    /// The first image block's image.
    pub image: Option<CardImage>,
    /// The first paragraph of the first HTML block, as plain text.
    pub summary: Option<String>,
}

/// The image and summary a page's blocks give its card, in block order.
#[must_use]
pub fn source(page: &SitePage) -> CardSource {
    let mut blocks: Vec<_> = page.blocks.iter().collect();
    blocks.sort_by_key(|block| block.position);
    let image = blocks.iter().find_map(|block| match &block.data {
        BlockData::Asset(slots) => slots.iter().find_map(|slot| slot.id).map(CardImage::Asset),
        BlockData::Media(_) => Some(CardImage::MediaBlock(block.id)),
        _ => block
            .layout_data
            .romeka
            .as_ref()
            .and_then(|carrier| romeka::asset_refs(carrier).first().copied())
            .map(CardImage::Asset),
    });
    let summary = blocks.iter().find_map(|block| match &block.data {
        BlockData::Html { html } => Some(summary(html)),
        _ => None,
    });
    CardSource {
        image,
        summary: summary.flatten(),
    }
}

/// The plain text of the first `<p>` of `html`, or of the whole fragment when
/// it has none, cut at a word near [`SUMMARY_MAX`] characters.
fn summary(html: &str) -> Option<String> {
    let lower = html.to_ascii_lowercase();
    let paragraph = lower
        .find("<p>")
        .or_else(|| lower.find("<p "))
        .map(|start| {
            let body = &html[start..];
            let end = body.to_ascii_lowercase().find("</p>").unwrap_or(body.len());
            &body[..end]
        })
        .unwrap_or(html);
    let text = collapse(&decode(&strip_tags(paragraph)));
    if text.is_empty() {
        return None;
    }
    if text.chars().count() <= SUMMARY_MAX {
        return Some(text);
    }
    let cut: String = text.chars().take(SUMMARY_MAX).collect();
    let cut = cut.rsplit_once(' ').map_or(cut.as_str(), |(head, _)| head);
    Some(format!(
        "{}…",
        cut.trim_end_matches([',', ';', ':', '.', ' '])
    ))
}

/// The text outside tags. A tag ends a word, so `a<br>b` reads as two.
fn strip_tags(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut in_tag = false;
    for ch in html.chars() {
        match ch {
            '<' => in_tag = true,
            '>' if in_tag => {
                in_tag = false;
                out.push(' ');
            }
            _ if !in_tag => out.push(ch),
            _ => {}
        }
    }
    out
}

/// The entities a purifier or this product's escaping writes.
fn decode(text: &str) -> String {
    text.replace("&nbsp;", " ")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#039;", "'")
        .replace("&#39;", "'")
        .replace("&amp;", "&")
}

fn collapse(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blocks::test_support::{block, block_with_layout, page};
    use omeka_core::site::PageLayout;
    use serde_json::json;

    /// The first image block gives the image and the first HTML block's
    /// first paragraph gives the summary, whatever the order between them.
    #[test]
    fn a_page_card_reads_the_first_image_and_paragraph() {
        let mut intro = block(
            "html",
            json!({"html": "<h2>Welcome</h2><p>Our <em>river</em> maps &amp; charts.</p><p>More.</p>"}),
        );
        intro.position = 1;
        let mut later = block("html", json!({"html": "<p>Not this one.</p>"}));
        later.position = 3;
        let mut image = block("asset", json!([{"id": ""}, {"id": 9}]));
        image.position = 2;
        let page = page(PageLayout::Normal, json!({}), vec![later, image, intro]);
        assert_eq!(
            source(&page),
            CardSource {
                image: Some(CardImage::Asset(AssetId::new(9))),
                summary: Some("Our river maps & charts.".to_string()),
            }
        );
    }

    /// A media block gives its block, whose first attachment the server reads;
    /// a page with no image or HTML block gives an empty card.
    #[test]
    fn a_page_card_names_a_media_block_or_nothing() {
        let mut media = block_with_layout("media", json!({}), json!({}));
        media.position = 1;
        let page_with_media = page(PageLayout::Normal, json!({}), vec![media]);
        assert_eq!(
            source(&page_with_media).image,
            Some(CardImage::MediaBlock(BlockId::new(1)))
        );
        let bare = page(
            PageLayout::Normal,
            json!({}),
            vec![block("lineBreak", json!({}))],
        );
        assert_eq!(source(&bare), CardSource::default());
    }

    /// A long paragraph is cut at a word and marked as cut.
    #[test]
    fn a_long_summary_is_cut_at_a_word() {
        let long = "word ".repeat(60);
        let text = summary(&format!("<p>{long}</p>")).unwrap();
        assert!(text.ends_with("word…"), "{text}");
        assert!(text.chars().count() <= SUMMARY_MAX + 1, "{text}");
        assert_eq!(
            summary("<div>No paragraph</div>"),
            Some("No paragraph".to_string())
        );
        assert_eq!(summary("<p> </p>"), None);
    }
}
