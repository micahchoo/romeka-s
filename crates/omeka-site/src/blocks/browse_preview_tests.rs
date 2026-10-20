//! The browse preview tests: upstream's rendering, and each Romeka setting.

use super::*;
use crate::blocks::test_support::{TestResources, block, context, item, site};
use crate::view::Thumbnail;
use serde_json::json;

/// `block-layout/browse-preview.phtml`: heading, resource list with a
/// thumbnail and a name, body, and the browse link with the stored query.
#[test]
fn browse_preview_matches_upstream_block_layout() {
    let site = site();
    let mut resources = TestResources::default();
    let mut resource = item(4, "An item", "/s/main/item/4");
    resource.description = Some("A description".to_string());
    resource.thumbnail = Some(Thumbnail::new("/files/medium/4.jpg", "An item"));
    resources.browse.push(resource);
    let ctx = context(&site, &resources);
    let stored = block(
        "browsePreview",
        json!({
            "resource_type": "items",
            "query": "year=2000",
            "heading": "Latest",
            "limit": 12,
            "components": ["resource-heading", "resource-body", "thumbnail"],
            "link-text": "Browse all"
        }),
    );
    assert_eq!(
        BrowsePreview.render(&stored, &ctx),
        "<div class=\"preview-block\"><h2>Latest</h2><ul class=\"resource-list preview resource-cards-row\"><li class=\"item resource has-thumb\"><a href=\"/s/main/item/4\" class=\"resource-link\"><span class=\"resource-thumb\"><img src=\"/files/medium/4.jpg\" alt=\"An item\" loading=\"lazy\"></span><span class=\"resource-text\"><span class=\"resource-name\">An item</span></span></a><div class=\"description\">A description</div></li></ul><a href=\"/s/main/item?year=2000\">Browse all</a></div>"
    );
}

/// The class adds the site id, the limit and the default sort to the search,
/// and `browse_attached_items` adds `site_attachments_only`.
#[test]
fn browse_preview_search_query_matches_upstream() {
    let site = site();
    let mut resources = TestResources::default();
    resources.browse.push(item(4, "An item", "/s/main/item/4"));
    let mut ctx = context(&site, &resources);
    ctx.settings.browse_attached_items = Some(true);
    let stored = block(
        "browsePreview",
        json!({"resource_type": "items", "query": "year=2000", "limit": 5}),
    );
    // The provider sees the query the class builds; the first resource is
    // rendered without components set to the defaults.
    let out = BrowsePreview.render(&stored, &ctx);
    assert!(
        out.contains("<span class=\"resource-name\">An item</span>"),
        "{out}"
    );
    assert!(!out.contains("<div class=\"description\">"), "{out}");
}

/// A resource type other than items picks the item-set controller; the link
/// and class names follow `BlockLayout/BrowsePreview.php::render`.
#[test]
fn browse_preview_item_sets_use_the_item_set_controller() {
    let site = site();
    let mut resources = TestResources::default();
    resources
        .browse
        .push(item(2, "A set", "/s/main/item-set/2"));
    let ctx = context(&site, &resources);
    let stored = block(
        "browsePreview",
        json!({"resource_type": "item_sets", "link-text": "All sets", "components": ["resource-heading"]}),
    );
    let out = BrowsePreview.render(&stored, &ctx);
    assert!(
        out.contains("<li class=\"item-set resource no-thumb\"><a href=\"/s/main/item-set/2\" class=\"resource-link\"><span class=\"resource-text\">"),
        "{out}"
    );
    assert!(
        out.contains("<a href=\"/s/main/item-set\">All sets</a>"),
        "{out}"
    );
}

/// `parse_str`: pairs keep their order, `+` and `%XX` decode, a part with no
/// `=` has an empty value.
#[test]
fn parse_query_matches_php_parse_str() {
    assert_eq!(
        parse_query("year=2000&search=a+b&flag"),
        vec![
            ("year".to_string(), "2000".to_string()),
            ("search".to_string(), "a b".to_string()),
            ("flag".to_string(), String::new()),
        ]
    );
    assert_eq!(
        parse_query("q=caf%C3%A9"),
        vec![("q".to_string(), "café".to_string())]
    );
}

/// A site that holds items, whose block query matches none of them.
struct NoMatch(TestResources);

impl crate::view::ResourceProvider for NoMatch {
    fn search(&self, kind: ResourceKind, query: &BrowseQuery) -> crate::view::ResourcePage {
        let membership_only = query.query.len() == 1;
        if membership_only {
            self.0.search(kind, query)
        } else {
            crate::view::ResourcePage {
                resources: Vec::new(),
                total: 0,
            }
        }
    }
    fn read(&self, kind: ResourceKind, id: u64) -> Option<crate::view::ResourceView> {
        self.0.read(kind, id)
    }
    fn read_media(&self, id: omeka_core::ids::MediaId) -> Option<crate::view::MediaRef> {
        self.0.read_media(id)
    }
    fn read_asset(&self, id: omeka_core::ids::AssetId) -> Option<crate::view::AssetView> {
        self.0.read_asset(id)
    }
    fn attachments(&self, block: omeka_core::ids::BlockId) -> Vec<crate::view::AttachmentRef> {
        self.0.attachments(block)
    }
    fn search_sites(&self, query: &crate::view::SiteQuery) -> crate::view::SiteSearch {
        self.0.search_sites(query)
    }
}

/// A block whose search matches nothing, on a site that holds items, says
/// so in place of upstream's empty list and keeps its browse link; the
/// admin preview names the fix.
#[test]
fn browse_preview_without_results_says_nothing_matches() {
    let site = site();
    let resources = NoMatch(TestResources {
        browse: vec![item(3, "Kept", "/s/main/item/3")],
        ..TestResources::default()
    });
    let ctx = context(&site, &resources);
    let stored = block("browsePreview", json!({"link-text": "Browse all"}));
    assert_eq!(
        BrowsePreview.render(&stored, &ctx),
        "<div class=\"preview-block\"><p class=\"preview-empty\">Nothing here matches yet.</p><a href=\"/s/main/item\">Browse all</a></div>"
    );
    let mut preview = context(&site, &resources);
    preview.block_markers = crate::view::BlockMarkers::Preview;
    assert!(
        BrowsePreview
            .render(&stored, &preview)
            .contains("Nothing matches this block&#039;s search."),
    );
}

/// On a site with no items the public block says so, with no admin hint,
/// and the admin preview points the curator at the Items tab.
#[test]
fn browse_preview_on_a_site_with_no_items_says_so() {
    let site = site();
    let resources = TestResources::default();
    let stored = block("browsePreview", json!({"link-text": "Browse all"}));
    assert_eq!(
        BrowsePreview.render(&stored, &context(&site, &resources)),
        "<div class=\"preview-block\"><p class=\"preview-empty\">There are no items to show yet.</p></div>"
    );
    let mut preview = context(&site, &resources);
    preview.block_markers = crate::view::BlockMarkers::Preview;
    assert!(
        BrowsePreview
            .render(&stored, &preview)
            .contains("No items on this site yet. Add items on the Items tab."),
    );
}

/// A block with Romeka settings: `romeka.settings` under its layout data.
fn styled(data: serde_json::Value, settings: serde_json::Value) -> SiteBlock {
    crate::blocks::test_support::block_with_layout(
        "browsePreview",
        data,
        json!({"romeka": {"block": "browsePreview", "v": 2, "settings": settings}}),
    )
}

fn three_items() -> TestResources {
    let mut resources = TestResources::default();
    for id in 1..=3 {
        resources.browse.push(item(
            id,
            &format!("Item {id}"),
            &format!("/s/main/item/{id}"),
        ));
    }
    resources
}

/// The card style overrides the theme's token for this block alone, and a
/// column count fixes the grid; a list of rows has no columns.
#[test]
fn browse_preview_card_style_and_columns_override_the_theme() {
    let site = site();
    let resources = three_items();
    let ctx = context(&site, &resources);
    let tiles = styled(json!({}), json!({"cards": "overlay", "columns": "3"}));
    assert!(
        BrowsePreview.render(&tiles, &ctx).contains(
            "<ul class=\"resource-list preview resource-cards-overlay resource-columns-3\">"
        ),
        "{}",
        BrowsePreview.render(&tiles, &ctx)
    );
    let rows = styled(json!({}), json!({"cards": "row", "columns": "3"}));
    assert!(
        BrowsePreview
            .render(&rows, &ctx)
            .contains("<ul class=\"resource-list preview resource-cards-row\">"),
        "{}",
        BrowsePreview.render(&rows, &ctx)
    );
    // A column count outside 2 to 6 is no column count.
    let wide = styled(json!({}), json!({"cards": "caption", "columns": "9"}));
    assert!(
        BrowsePreview
            .render(&wide, &ctx)
            .contains("<ul class=\"resource-list preview resource-cards-caption\">")
    );
}

/// A carousel is the same list scrolled sideways inside a labelled region,
/// each card an anchor target, and the previous and next links jump to the
/// first card and to the first card past one screen.
#[test]
fn browse_preview_carousel_steps_with_links() {
    let site = site();
    let resources = three_items();
    let ctx = context(&site, &resources);
    let stored = styled(
        json!({"heading": "Recent"}),
        json!({"cards": "carousel", "columns": "2"}),
    );
    let out = BrowsePreview.render(&stored, &ctx);
    assert!(
        out.contains(
            "<div class=\"preview-carousel\" role=\"region\" aria-roledescription=\"carousel\" aria-label=\"Recent\">\
             <ul class=\"resource-list preview resource-cards-caption resource-columns-2 resource-carousel\" id=\"carousel-1\" tabindex=\"0\">\
             <li id=\"carousel-1-1\" class=\"item resource no-thumb\">"
        ),
        "{out}"
    );
    assert!(out.contains("<li id=\"carousel-1-3\" "), "{out}");
    assert!(
        out.contains(
            "<a class=\"preview-carousel-prev\" href=\"#carousel-1-1\" aria-controls=\"carousel-1\">Prev</a>\
             <a class=\"preview-carousel-next\" href=\"#carousel-1-3\" aria-controls=\"carousel-1\">Next</a>"
        ),
        "{out}"
    );
}

/// A carousel on a theme whose cards are rows draws captions, since rows
/// do not scroll sideways.
#[test]
fn browse_preview_carousel_on_a_row_theme_draws_captions() {
    let site = site();
    let resources = three_items();
    let mut ctx = context(&site, &resources);
    ctx.theme_settings =
        crate::theme::tokens::ThemeSettings::from_value(&json!({"card_style": "row"}));
    let theme = block("browsePreview", json!({}));
    assert!(
        BrowsePreview
            .render(&theme, &ctx)
            .contains("<ul class=\"resource-list preview resource-cards-row\">")
    );
    let carousel = styled(json!({}), json!({"cards": "carousel"}));
    assert!(
        BrowsePreview
            .render(&carousel, &ctx)
            .contains("resource-cards-caption resource-carousel")
    );
}

/// The chosen details replace the theme's type, creator and date, in the
/// order the card prints them, and a chosen property's first value
/// follows them.
#[test]
fn browse_preview_shows_the_chosen_details() {
    let site = site();
    let mut resources = TestResources::default();
    let mut resource = item(4, "A map", "/s/main/item/4");
    resource.values = vec![
        crate::blocks::test_support::literal_value(1, "Creator", "Ada"),
        crate::blocks::test_support::literal_value(2, "Date", "1901"),
        crate::blocks::test_support::literal_value(3, "Subject", "Rivers"),
    ];
    resource
        .item_sets
        .push(crate::blocks::test_support::resource_ref(
            "Maps",
            "/s/main/item-set/2",
        ));
    resources.browse.push(resource);
    let ctx = context(&site, &resources);

    let theme = block("browsePreview", json!({}));
    assert!(
        BrowsePreview.render(&theme, &ctx).contains(
            "<span class=\"resource-meta\"><span class=\"resource-meta-creator\">Ada</span>\
             <span class=\"resource-meta-date\">1901</span></span>"
        ),
        "{}",
        BrowsePreview.render(&theme, &ctx)
    );
    // A tile has no room for the creator, so a caption card leaves it out.
    let tiles = styled(json!({}), json!({"cards": "caption"}));
    assert!(
        BrowsePreview.render(&tiles, &ctx).contains(
            "<span class=\"resource-meta\"><span class=\"resource-meta-date\">1901</span></span>"
        ),
        "{}",
        BrowsePreview.render(&tiles, &ctx)
    );
    let chosen = styled(
        json!({}),
        json!({"details": ["date", "item_set"], "detail_property": "dcterms:subject"}),
    );
    let out = BrowsePreview.render(&chosen, &ctx);
    assert!(
        out.contains(
            "<span class=\"resource-meta\"><span class=\"resource-meta-date\">1901</span>\
             <span class=\"resource-meta-item-set\">Maps</span>\
             <span class=\"resource-meta-value\">Rivers</span></span>"
        ),
        "{out}"
    );

    let none = styled(json!({}), json!({"details": []}));
    assert!(
        !BrowsePreview.render(&none, &ctx).contains("resource-meta"),
        "{}",
        BrowsePreview.render(&none, &ctx)
    );
}

/// The stored query's sort reaches the search, as
/// `BlockLayout/BrowsePreview.php::render` passes it to the API; with none
/// the block sorts newest first.
#[test]
fn browse_preview_search_carries_the_stored_sort() {
    let settings = SiteSettings::default();
    let site = SiteId::new(1);
    let data = |query: &str| BrowsePreviewData {
        resource_type: None,
        query: Some(query.to_string()),
        heading: None,
        limit: None,
        components: Vec::new(),
        link_text: None,
    };
    let (_, sorted) = search(
        &data("sort_by=dcterms%3Adate&sort_order=asc"),
        site,
        &settings,
    );
    assert_eq!(
        sorted.sort,
        Some(("dcterms:date".to_string(), "asc".to_string()))
    );
    let (_, default) = search(&data(""), site, &settings);
    assert_eq!(
        default.sort,
        Some(("created".to_string(), "desc".to_string()))
    );
}
