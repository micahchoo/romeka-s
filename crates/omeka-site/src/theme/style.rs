//! The bundled theme's own stylesheet, emitted as data so the caller can serve
//! it. `theme::default::DEFAULT_INFO` lists it as the theme's asset at
//! `css/style.css`, and the document frame links it under the render context's
//! asset base.
//!
//! The sheet is the port's own. Its block and page-layout rules are the subset
//! of `application/asset/css/page-blocks.css` and `page-grid.css` that applies
//! to the markup this port emits, plus a rule for every block wrapper class no
//! upstream rule names, and its shell, list and navigation rules stand in for
//! `application/asset/css/style.css`. Both are core assets upstream.

/// The bundled theme's stylesheet.
pub const STYLE_CSS: &str = include_str!("style.css");

/// The placeholder a resource without a thumbnail shows
/// (`application/asset/img/default.png` upstream).
pub const PLACEHOLDER_SVG: &str = include_str!("placeholder.svg");

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;
    use crate::blocks::test_support::{
        TestResources, block, block_with_layout, context, item, literal_value, resource_ref, site,
    };
    use crate::nav::{NavItem, PageRef};
    use crate::search::{
        AdvancedSearchForm, CrossSiteBody, CrossSitePage, FormOption, SearchHit, SearchList,
        SearchResource, SearchResults, SearchSection,
    };
    use crate::theme::{DefaultTheme, Theme};
    use crate::view::{
        AssetView, AttachmentRef, BrowseView, ErrorView, MediaRef, ResourceRef, ResourceView,
        SiteView, Thumbnail, View,
    };
    use omeka_core::ids::{AssetId, ItemId, PageId, SiteId};
    use omeka_core::resource::ResourceKind;
    use omeka_core::site::{PageLayout, PageLayoutData, SitePage, Slug};
    use serde_json::json;

    /// The class tokens a rendered document carries inside its content region.
    /// The body element's own classes are theme hooks, not fragment classes.
    fn emitted_classes(document: &str) -> Vec<String> {
        let content = document
            .split_once("id=\"content\"")
            .map_or(document, |(_, rest)| rest);
        let mut out = Vec::new();
        for (needle, quote) in [("class=\"", '"'), ("class='", '\'')] {
            let mut rest = content;
            while let Some(index) = rest.find(needle) {
                rest = &rest[index + needle.len()..];
                let end = rest.find(quote).unwrap_or(rest.len());
                out.extend(rest[..end].split_whitespace().map(str::to_string));
                rest = &rest[end..];
            }
        }
        out
    }

    /// The class selectors a stylesheet defines.
    fn selector_tokens(css: &str) -> BTreeSet<String> {
        let bytes = css.as_bytes();
        let mut out = BTreeSet::new();
        let mut index = 0;
        while index < bytes.len() {
            if bytes[index] != b'.' {
                index += 1;
                continue;
            }
            let start = index + 1;
            let mut end = start;
            while end < bytes.len()
                && (bytes[end].is_ascii_alphanumeric() || bytes[end] == b'-' || bytes[end] == b'_')
            {
                end += 1;
            }
            if end > start {
                out.insert(css[start..end].to_string());
            }
            index = end.max(index + 1);
        }
        out
    }

    fn rich_provider() -> TestResources {
        let mut resources = TestResources::default();
        let mut browsed = item(4, "An item", "/s/main/item/4");
        browsed.description = Some("A description".to_string());
        browsed.thumbnail = Some(Thumbnail::new("/files/medium/4.jpg", "An item"));
        resources.browse.push(browsed);

        let mut attached = item(7, "An attached item", "/s/main/item/7");
        attached.values = vec![literal_value(1, "Title", "An attached item")];
        attached.item_sets = vec![resource_ref("A set", "/s/main/item-set/2")];
        attached.media = vec![ResourceRef {
            url: "/s/main/media/9".to_string(),
            title: "A file".to_string(),
            thumbnail: Some(Thumbnail::new("/files/square/9.jpg", "A file")),
            media_kind: None,
            notes: 0,
        }];
        attached.primary_media = Some(media());
        attached.parent_item = Some(resource_ref("An attached item", "/s/main/item/7"));
        resources.items.insert(7, attached);

        resources.media.insert(9, media());
        resources.assets.insert(
            3,
            AssetView {
                id: AssetId::new(3),
                title: "A logo".to_string(),
                url: Some("/files/original/3.png".to_string()),
                thumbnail: Some(Thumbnail::new("/files/square/3.jpg", "A logo")),
            },
        );
        resources.sites = vec![SiteView {
            id: SiteId::new(2),
            title: "Another site".to_string(),
            url: "/s/another".to_string(),
            summary: Some("The summary".to_string()),
            thumbnail: Some(Thumbnail::new("/files/square/2.jpg", "Another site")),
        }];
        resources
    }

    fn media() -> MediaRef {
        MediaRef {
            url: "/s/main/media/9".to_string(),
            original_url: Some("/files/original/9.jpg".to_string()),
            title: "A file".to_string(),
            media_type: None,
            thumbnail: Some(Thumbnail::new("/files/square/9.jpg", "A file")),
            embed: Some("<iframe src=\"/embed/9\"></iframe>".to_string()),
        }
    }

    /// The blocks a page needs to print every block class the crate emits, one
    /// per layout, plus a group and an unregistered name.
    fn every_layout_blocks() -> Vec<omeka_core::site::SiteBlock> {
        let mut blocks = vec![
            block("pageTitle", json!({})),
            block("html", json!({"html": "<p>Body copy</p>"})),
            block("lineBreak", json!({"break_type": "transparent"})),
            block("lineBreak", json!({"break_type": "opaque"})),
            block("pageDateTime", json!({"display": "created_modified"})),
            block("tableOfContents", json!({"depth": 1})),
            block(
                "listOfPages",
                json!({"pagelist": "[{\"text\": \"About\", \"data\": {\"id\": 3}}]"}),
            ),
            block(
                "listOfSites",
                json!({"limit": 1, "summaries": true, "thumbnails": true}),
            ),
            block(
                "browsePreview",
                json!({"resource_type": "items", "heading": "Latest", "link-text": "Browse all"}),
            ),
            block("itemWithMetadata", json!({})),
            block(
                "media",
                json!({"layout": "horizontal", "media_display": "thumbnail", "show_title_option": "item_title"}),
            ),
            block(
                "media",
                json!({"layout": "vertical", "media_display": "embed"}),
            ),
            block(
                "asset",
                json!([{"id": 3, "page": 3, "alt_link_title": "Read on", "caption": "A caption"}]),
            ),
            block(
                "iiifImage",
                json!({"image_url": "https://example.org/i", "title": "A map", "show_title": true}),
            ),
            block(
                "iiifPresentation",
                json!({"manifest_url": "https://example.org/m", "title": "An album", "show_title": true}),
            ),
            block(
                "oembed",
                json!({"oembed": {"type": "photo", "url": "https://example.org/p.jpg", "width": 640, "height": 480}}),
            ),
            block("blockGroup", json!({"span": "1"})),
            block("html", json!({"html": "<p>Grouped copy</p>"})),
            block("myModuleLayout", json!({"x": 1})),
            romeka_hero(),
            romeka_block(
                "gallery",
                json!({"media": [{"item": 7, "caption": "<p>A caption</p>"}]}),
            ),
            romeka_block(
                "gallery",
                json!({"media": [{"item": 7}], "layout": "carousel", "columns": 4}),
            ),
            romeka_block(
                "button",
                json!({"button_label": "Go", "button_url": "/go", "button2_label": "Back",
                       "button2_url": "/back", "size": "small"}),
            ),
            romeka_block(
                "quote",
                json!({"text": "Words", "attribution": "Someone", "item": 7, "style": "band"}),
            ),
            romeka_block(
                "accordion",
                json!({"entries": [{"heading": "A question", "answer": "<p>An answer</p>"}]}),
            ),
            romeka_block("timeline", json!({"items": [7, 4]})),
            romeka_block("searchBox", json!({"resource": "items"})),
            romeka_block(
                "featuredItem",
                json!({"item": 7, "fields": [{"property": "dcterms:title"}]}),
            ),
        ];
        for (index, block) in blocks.iter_mut().enumerate() {
            block.position = index as i64 + 1;
        }
        blocks
    }

    /// A Romeka hero carrier whose plain hash matches, so the rich banner and
    /// every class it emits is part of the coverage check.
    fn romeka_hero() -> omeka_core::site::SiteBlock {
        let plain = "<h2>Hero</h2>";
        block_with_layout(
            "html",
            json!({"html": plain}),
            json!({"romeka": {
                "block": "hero",
                "v": 1,
                "settings": {
                    "heading": "Hero",
                    "subheading": "A subheading",
                    "body": "<p>Body copy</p>",
                    "background": 3,
                    "overlay": "medium",
                    "alignment": "center",
                    "cta_label": "Browse",
                    "cta_page": 3,
                    "cta2_label": "About",
                    "cta2_url": "/s/main/page/about",
                    "height": "tall"
                },
                "plain_hash": crate::blocks::romeka::plain_hash(plain)
            }}),
        )
    }

    /// A Romeka carrier of `name` whose plain hash matches, so its rich render
    /// and every class it emits is part of the coverage check.
    fn romeka_block(name: &str, settings: serde_json::Value) -> omeka_core::site::SiteBlock {
        let plain = "<p>Plain</p>";
        block_with_layout(
            "html",
            json!({"html": plain}),
            json!({"romeka": {
                "block": name,
                "v": 2,
                "settings": settings,
                "plain_hash": crate::blocks::romeka::plain_hash(plain)
            }}),
        )
    }

    fn nav() -> Vec<NavItem> {
        fn item(label: &str, slug: &str, id: u64, children: Vec<NavItem>) -> NavItem {
            NavItem {
                label: label.to_string(),
                url: format!("/s/main/page/{slug}"),
                target_blank: false,
                page: Some(PageId::new(id)),
                children,
            }
        }
        vec![item(
            "Welcome",
            "welcome",
            2,
            vec![item(
                "About",
                "about",
                3,
                vec![item("Team", "team", 4, Vec::new())],
            )],
        )]
    }

    /// A page whose active page is `About`, so the render carries breadcrumbs, a
    /// sub-menu and a table of contents.
    fn about_page() -> SitePage {
        let mut page = SitePage::new(
            PageId::new(3),
            SiteId::new(1),
            Slug::parse("about").unwrap(),
            "About",
        );
        page.blocks = every_layout_blocks();
        page
    }

    /// A grid page whose blocks carry layout data, so the sheet's page-grid and
    /// layout-data rules are checked against the classes the grid markup emits.
    /// No block sets a stored `class`, because a stored class is arbitrary and
    /// the sheet cannot name it.
    fn grid_page() -> SitePage {
        let mut page = SitePage::new(
            PageId::new(5),
            SiteId::new(1),
            Slug::parse("grid").unwrap(),
            "Grid",
        );
        page.layout = PageLayout::Grid;
        page.layout_data = PageLayoutData::parse(&json!({"grid_columns": 12}));
        page.blocks = vec![
            block_with_layout(
                "html",
                json!({"html": "<p>Hero</p>"}),
                json!({
                    "grid_column_span": "6",
                    "alignment_text": "justify",
                    "background_color": "#eeeeee",
                    "padding_top": "1em"
                }),
            ),
            block_with_layout(
                "blockGroup",
                json!({"span": "1"}),
                json!({
                    "alignment_block": "center",
                    "background_image_asset": "3",
                    "background_image_position_y": "top",
                    "background_image_position_x": "left",
                    "background_image_size": "cover"
                }),
            ),
            block("html", json!({"html": "<p>Body</p>"})),
        ];
        for (index, block) in page.blocks.iter_mut().enumerate() {
            block.position = index as i64 + 1;
        }
        page
    }

    /// A page with one section per layout, each with an alignment, a gap and
    /// an overlay, and a block with every shared styling member.
    fn sections_page() -> SitePage {
        let mut page = SitePage::new(
            PageId::new(6),
            SiteId::new(1),
            Slug::parse("sections").unwrap(),
            "Sections",
        );
        let aligns = ["top", "center", "bottom"];
        let gaps = ["none", "small", "medium", "large"];
        let overlays = ["none", "light", "dark"];
        for (index, (layout, _)) in crate::blocks::style::SectionLayout::ALL.iter().enumerate() {
            page.blocks.push(block_with_layout(
                "blockGroup",
                json!({"span": "1"}),
                json!({"romeka": {"block": "blockGroup", "v": 2, "settings": {"section": {
                    "layout": layout,
                    "align": aligns[index % aligns.len()],
                    "gap": gaps[index % gaps.len()],
                    "overlay": overlays[index % overlays.len()],
                }}}}),
            ));
            page.blocks
                .push(block("html", json!({"html": "<p>Slot</p>"})));
        }
        page.blocks.push(block_with_layout(
            "html",
            json!({"html": "<p>Styled</p>"}),
            json!({"romeka": {"block": "html", "v": 2, "settings": {
                "anchor": "styled",
                "space_top": "small",
                "space_bottom": "large",
                "bleed": true,
                "text_color": "muted",
                "background_role": "surface",
            }}}),
        ));
        for (index, block) in page.blocks.iter_mut().enumerate() {
            block.position = index as i64 + 1;
        }
        page
    }

    /// The documents whose fragments the stylesheet must style: a page with
    /// every layout, a grid page, a browse per resource kind, and the remaining
    /// views.
    fn documents() -> Vec<String> {
        let site = site();
        let mut resources = rich_provider();
        resources.attachments.insert(
            1,
            (0..2)
                .map(|_| AttachmentRef {
                    item: Some(ItemId::new(7)),
                    media: None,
                    caption: String::new(),
                })
                .collect(),
        );
        let mut ctx = context(&site, &resources);
        ctx.pages = vec![
            PageRef {
                id: PageId::new(2),
                slug: Slug::parse("welcome").unwrap(),
                title: "Welcome".to_string(),
                is_public: true,
            },
            PageRef {
                id: PageId::new(3),
                slug: Slug::parse("about").unwrap(),
                title: "About".to_string(),
                is_public: true,
            },
            PageRef {
                id: PageId::new(4),
                slug: Slug::parse("team").unwrap(),
                title: "Team".to_string(),
                is_public: true,
            },
        ];
        ctx.current_path = "/s/main/page/about".to_string();
        ctx.page_created = Some(crate::blocks::test_support::timestamp());
        ctx.page_modified = Some(crate::blocks::test_support::timestamp());
        ctx.nav = nav();

        let page = about_page();
        let mut documents = vec![crate::render::render_page(
            &crate::theme::DefaultTheme,
            &page,
            &mut ctx,
        )];
        let grid = grid_page();
        documents.push(crate::render::render_page(
            &crate::theme::DefaultTheme,
            &grid,
            &mut ctx,
        ));
        let sections = sections_page();
        documents.push(crate::render::render_page(
            &crate::theme::DefaultTheme,
            &sections,
            &mut ctx,
        ));

        let mut browse = BrowseView {
            kind: ResourceKind::Item,
            title: "Items".to_string(),
            resources: resources.browse.clone(),
            total: 3,
            query: Vec::new(),
            page: 1,
            per_page: 1,
        };
        for (kind, title) in [
            (ResourceKind::Item, "Items"),
            (ResourceKind::ItemSet, "Item sets"),
            (ResourceKind::Media, "Media"),
        ] {
            browse.kind = kind;
            browse.title = title.to_string();
            documents.push(DefaultTheme.render(&View::Browse(&browse), &ctx));
        }

        let attached = resources.items.get(&7).cloned().unwrap_or_else(|| {
            ResourceView::new(
                omeka_core::ids::ResourceId::new(7),
                ResourceKind::Item,
                "/s/main/item/7",
                "An attached item",
            )
        });
        documents.push(DefaultTheme.render(&View::Item(&attached), &ctx));
        let mut with_manifest = attached.clone();
        with_manifest.iiif_manifest =
            Some("https://example.org/iiif/3/item/7/manifest".to_string());
        documents.push(DefaultTheme.render(&View::Item(&with_manifest), &ctx));
        documents.push(DefaultTheme.render(&View::ItemSet(&attached), &ctx));
        documents.push(DefaultTheme.render(&View::Media(&attached), &ctx));
        documents.push(DefaultTheme.render(&View::Index, &ctx));
        documents.push(DefaultTheme.render(
            &View::Error(&ErrorView::not_found(ctx.translator().messages())),
            &ctx,
        ));

        // The search pages: the results sections, one cross-site list, the page
        // browse and the advanced form.
        let hits = vec![SearchHit::new("An item", "/s/main/item/4")];
        let sections = vec![SearchSection::new(SearchResource::Items, hits.clone(), 1)];
        documents.push(DefaultTheme.render(
            &View::Search(&SearchResults {
                needle: "cat".to_string(),
                sections: sections.clone(),
            }),
            &ctx,
        ));
        let cross = CrossSitePage {
            needle: "cat".to_string(),
            form_action: "/s/main/cross-site-search/results".to_string(),
            body: CrossSiteBody::Results(sections),
        };
        documents.push(DefaultTheme.render(&View::CrossSiteSearch(&cross), &ctx));
        let list = CrossSitePage {
            body: CrossSiteBody::List(Box::new(SearchList {
                resource: SearchResource::SitePages,
                hits: vec![SearchHit::new("Welcome", "/s/main/page/welcome")],
                total: 1,
                page: 1,
                per_page: 10,
            })),
            ..cross
        };
        documents.push(DefaultTheme.render(&View::CrossSiteSearch(&list), &ctx));
        documents.push(DefaultTheme.render(
            &View::SearchList(&SearchList {
                resource: SearchResource::SitePages,
                hits: vec![SearchHit::new("Welcome", "/s/main/page/welcome")],
                total: 1,
                page: 1,
                per_page: 10,
            }),
            &ctx,
        ));
        documents.push(DefaultTheme.render(
            &View::AdvancedSearch(&AdvancedSearchForm {
                item_sets: vec![FormOption::new("5", "Featured photographs")],
                ..AdvancedSearchForm::new(
                    SearchResource::Items,
                    "/s/main/item",
                    vec![
                        ("property[0][property]".to_string(), "1".to_string()),
                        ("property[0][type]".to_string(), "in".to_string()),
                        ("property[0][text]".to_string(), "cat".to_string()),
                    ],
                )
            }),
            &ctx,
        ));
        documents
    }

    /// Every class token a rendered fragment carries has a rule in the sheet, so
    /// a view that starts emitting a new class fails here until the sheet styles
    /// it. One exception: an unregistered layout's wrapper is `block-{name}` for
    /// a stored name the sheet cannot know, so `every_layout_blocks`' fallback
    /// block is skipped.
    #[test]
    fn style_css_styles_every_class_the_views_emit() {
        let selectors = selector_tokens(STYLE_CSS);
        let dynamic: BTreeSet<String> = ["block-myModuleLayout".to_string()].into();
        let mut missing = BTreeSet::new();
        for document in documents() {
            for class in emitted_classes(&document) {
                if !selectors.contains(&class) && !dynamic.contains(&class) {
                    missing.insert(class);
                }
            }
        }
        assert!(missing.is_empty(), "classes with no rule: {missing:?}");
    }

    /// A structural class sits on the root element, so no view may print the
    /// same class on an element of its own: the sheet's rule for the one
    /// would land on the other. A magnifier once shared `search-icon` with
    /// the search token and shrank the whole page to its 20px.
    #[test]
    fn root_classes_never_name_an_element_class() {
        use crate::theme::choices::{
            CardStyle, FooterLayout, HeroStyle, ItemLayout, SearchDisplay,
        };
        let mut roots: Vec<&str> = Vec::new();
        roots.extend(HeroStyle::ALL.iter().map(|v| v.class()));
        roots.extend(CardStyle::ALL.iter().map(|v| v.class()));
        roots.extend(SearchDisplay::ALL.iter().map(|v| v.class()));
        roots.extend(FooterLayout::ALL.iter().map(|v| v.class()));
        roots.extend(ItemLayout::ALL.iter().map(|v| v.class()));
        for document in documents() {
            let body = document.split_once("<body").map_or("", |(_, rest)| rest);
            for attribute in body.split("class=\"").skip(1) {
                let value = attribute.split('"').next().unwrap_or_default();
                for class in value.split_whitespace() {
                    assert!(!roots.contains(&class), "{class} is a root class");
                }
            }
        }
    }

    /// The sheet names no colour of its own. Colours arrive only as the custom
    /// properties the frame emits from the site's tokens, so a hard-coded value
    /// here would ignore the Theme page.
    #[test]
    fn style_css_has_no_hard_coded_colour() {
        let bytes = STYLE_CSS.as_bytes();
        for (index, byte) in bytes.iter().enumerate() {
            if *byte != b'#' {
                continue;
            }
            let hex: String = STYLE_CSS[index + 1..]
                .chars()
                .take_while(char::is_ascii_hexdigit)
                .take(6)
                .collect();
            assert!(
                hex.len() < 3,
                "a hex colour `#{hex}` at byte {index}; colours live in the tokens"
            );
        }
        let lower = STYLE_CSS.to_ascii_lowercase();
        for functional in ["rgb(", "rgba(", "hsl(", "hsla("] {
            assert!(!lower.contains(functional), "{functional} in the sheet");
        }
        // The design values do come from properties.
        for property in [
            "var(--color-text)",
            "var(--font-body)",
            "var(--font-size-base)",
            "var(--space-3)",
            "var(--radius)",
        ] {
            assert!(STYLE_CSS.contains(property), "{property} missing");
        }
    }

    /// Every custom property the sheet reads is one the tokens emit, for every
    /// preset, so a token renamed on one side cannot leave the sheet reading
    /// nothing. And every header and button property the tokens emit has a
    /// reader in the sheet.
    #[test]
    fn style_css_reads_only_properties_the_tokens_emit() {
        let read: std::collections::BTreeSet<&str> = STYLE_CSS
            .split("var(--")
            .skip(1)
            .filter_map(|tail| tail.split([')', ',']).next())
            .collect();
        for preset in crate::theme::presets::PresetId::KEYS {
            let css = crate::theme::ThemeSettings::from_value(&serde_json::json!({
                "preset": preset
            }))
            .css();
            for name in &read {
                assert!(
                    css.contains(&format!("--{name}:")),
                    "the sheet reads --{name}, which the {preset} tokens do not emit"
                );
            }
        }
        for name in [
            "header-background",
            "header-text",
            "header-link",
            "header-border",
            "header-stripe",
            "header-mark",
            "button-background",
            "button-text",
            "button-border",
            "content-width",
        ] {
            assert!(read.contains(name), "--{name} has no reader in the sheet");
        }
    }

    /// The sheet's rules are balanced, no rule is empty, and every rule
    /// declares something.
    #[test]
    fn style_css_is_well_formed() {
        assert!(STYLE_CSS.contains(".blocks-inner:after"), "not the sheet");
        let mut depth = 0_i32;
        for ch in STYLE_CSS.chars() {
            match ch {
                '{' => depth += 1,
                '}' => depth -= 1,
                _ => {}
            }
            assert!(depth >= 0, "a closing brace before its rule");
        }
        assert_eq!(depth, 0, "an unclosed rule");
        assert!(!STYLE_CSS.contains("{}"), "an empty rule");
        for rule in STYLE_CSS.split('}') {
            let Some((_, body)) = rule.split_once('{') else {
                continue;
            };
            assert!(body.contains(':'), "a rule with no declaration: {rule}");
        }
    }

    /// Every registered layout that emits a class has a rule naming it, so a
    /// reader can find a block's styling by its upstream layout name. The
    /// wrapper class `block-{layout}` is named for each layout, and the grid
    /// placement and layout-data classes the page-grid and page-blocks rules
    /// carry.
    #[test]
    fn style_css_names_the_block_layouts_it_styles() {
        let selectors = selector_tokens(STYLE_CSS);
        for layout in [
            "block-blockGroup",
            "block-html",
            "block-pageTitle",
            "block-media",
            "block-browsePreview",
            "block-listOfSites",
            "block-tableOfContents",
            "block-lineBreak",
            "block-itemWithMetadata",
            "block-pageDateTime",
            "block-iiifPresentation",
            "block-iiifImage",
            "block-asset",
            "block-listOfPages",
            "block-oembed",
            "media-embed",
            "preview-block",
            "toc-block",
            "item-with-metadata",
            "assets",
            "page-date-time",
            "list-of-sites",
            "break",
            "list-of-pages-link",
            "romeka-hero",
            "romeka-hero-cta",
            "romeka-hero-cta-outline",
            "romeka-hero-actions",
            "romeka-hero-height-tall",
            "romeka-gallery",
            "romeka-gallery-grid",
            "romeka-gallery-masonry",
            "romeka-gallery-carousel",
            "romeka-gallery-columns-2",
            "romeka-gallery-columns-6",
            "romeka-gallery-empty",
            "romeka-lightbox",
            "romeka-lightbox-image",
            "romeka-lightbox-close",
            "romeka-lightbox-previous",
            "romeka-lightbox-next",
            "romeka-featured",
            "romeka-featured-card",
            "romeka-featured-image-left",
            "romeka-featured-image-right",
            "romeka-featured-image-top",
            "romeka-featured-no-image",
            "romeka-featured-empty",
            "romeka-buttons",
            "romeka-buttons-align-left",
            "romeka-buttons-align-center",
            "romeka-buttons-align-right",
            "romeka-button-solid",
            "romeka-button-outline",
            "romeka-button-small",
            "romeka-button-medium",
            "romeka-button-large",
            "romeka-buttons-empty",
            "romeka-quote",
            "romeka-quote-plain",
            "romeka-quote-large",
            "romeka-quote-band",
            "romeka-quote-text",
            "romeka-quote-attribution",
            "romeka-quote-empty",
            "romeka-accordion",
            "romeka-accordion-item",
            "romeka-accordion-heading",
            "romeka-accordion-answer",
            "romeka-accordion-empty",
            "romeka-timeline",
            "romeka-timeline-list",
            "romeka-timeline-entry",
            "romeka-timeline-date",
            "romeka-timeline-thumbnail",
            "romeka-timeline-undated",
            "romeka-timeline-empty",
            "romeka-search",
            "romeka-search-field",
            "romeka-search-input",
            "romeka-search-submit",
            "romeka-map",
            "romeka-map-canvas",
            "romeka-map-poster",
            "romeka-map-section",
            "romeka-section",
            "romeka-section-split-40-60",
            "romeka-section-cards-3",
            "romeka-section-band",
            "romeka-bleed",
            "iiif-poster",
            "iiif-links",
            "page-layout-grid",
            "grid-template-columns-12",
            "grid-position-auto",
            "grid-span-12",
            "block-group-grid",
            "block-layout-alignment-text-justify",
            "block-layout-alignment-block-center",
            "block-layout-background-image-position-y-top",
            "block-layout-background-image-position-x-left",
            "block-layout-background-image-size-cover",
            "has-background",
        ] {
            assert!(selectors.contains(layout), "no rule for {layout}");
        }
    }
}
