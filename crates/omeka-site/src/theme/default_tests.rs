use super::*;
use crate::blocks::test_support::{TestResources, context, item, literal_value, site};
use crate::view::ResourceRef;

/// The layout writes the document frame from `view/layout/layout.phtml`:
/// doctype, head, the installation title, the asset base, and the footer.
#[test]
fn layout_matches_upstream_layout() {
    let site = site();
    let resources = TestResources::default();
    let ctx = context(&site, &resources);
    let document = DefaultTheme.render(&View::Index, &ctx);
    assert!(
        document.starts_with("<!DOCTYPE html>\n<html lang=\"en\">"),
        "{document}"
    );
    assert!(document.contains("<title>Romeka S</title>"), "{document}");
    assert!(
        document.contains("href=\"/assets/css/style.css\""),
        "{document}"
    );
    assert!(
        document.contains("<body class=\"minimal home\">"),
        "{document}"
    );
    assert!(
        document.contains("<p>This site has no pages.</p>"),
        "{document}"
    );
    assert!(
        document.contains("<span class=\"powered-by\">Powered by Romeka S</span>"),
        "{document}"
    );
}

/// The theme's frame gives every document a level-one heading, the site title.
/// Upstream's bare layout prints no heading, so the item browse and the empty
/// index had none; axe reports that as `page-has-heading-one`.
#[test]
fn layout_heading_is_the_site_title_for_every_view() {
    let site = site();
    let resources = TestResources::default();
    let ctx = context(&site, &resources);
    let documents = [
        DefaultTheme.render(&View::Index, &ctx),
        DefaultTheme.render(
            &View::Browse(&BrowseView::new(ResourceKind::Item, "Items", Vec::new())),
            &ctx,
        ),
    ];
    for document in documents {
        assert!(
            document.contains("<h1 class=\"site-title\"><a href=\"/s/main\">Main Site</a></h1>"),
            "{document}"
        );
    }
}

/// The site title is escaped, so a stored title cannot inject markup.
#[test]
fn layout_escapes_the_site_title() {
    let mut site = site();
    site.title = "Main & <b>Site</b>".to_string();
    let resources = TestResources::default();
    let ctx = context(&site, &resources);
    let document = DefaultTheme.render(&View::Index, &ctx);
    assert!(
        document.contains(
            "<h1 class=\"site-title\"><a href=\"/s/main\">Main &amp; &lt;b&gt;Site&lt;/b&gt;</a></h1>"
        ),
        "{document}"
    );
}

/// The head links the theme's own manifest assets, and the core assets the
/// port's theme does not need (`fonts.css`, `iconfonts.css`, jQuery,
/// `global.js`) are not linked.
#[test]
fn layout_links_the_theme_manifest_assets() {
    let site = site();
    let resources = TestResources::default();
    let ctx = context(&site, &resources);
    let document = DefaultTheme.render(&View::Index, &ctx);
    for asset in DefaultTheme.info().assets {
        let element = asset
            .kind
            .element(&format!("{}/{}", ctx.asset_base, asset.path));
        assert!(document.contains(&element), "{document}");
    }
    for missing in [
        "/css/fonts.css",
        "/css/iconfonts.css",
        "/vendor/jquery/jquery.min.js",
        "/js/global.js",
    ] {
        assert!(!document.contains(missing), "{missing} in {document}");
    }
}

/// The asset base is the caller's, trimmed of a trailing slash.
#[test]
fn layout_uses_the_context_asset_base() {
    let site = site();
    let resources = TestResources::default();
    let mut ctx = context(&site, &resources);
    ctx.asset_base = "/theme/assets/".to_string();
    let document = DefaultTheme.render(&View::Index, &ctx);
    assert!(
        document.contains("href=\"/theme/assets/css/style.css\""),
        "{document}"
    );
    assert!(!document.contains("/theme/assets//"), "{document}");
}

/// The item view prints the title, the `Item` label and the values.
#[test]
fn item_view_matches_upstream_item_show() {
    let site = site();
    let resources = TestResources::default();
    let ctx = context(&site, &resources);
    let mut resource = item(4, "An item", "/s/main/item/4");
    resource.values.push(literal_value(1, "Title", "An item"));
    let document = DefaultTheme.render(&View::Item(&resource), &ctx);
    assert!(
        document.contains("<title>An item · Romeka S</title>"),
        "{document}"
    );
    assert!(
        document.contains("<body class=\"minimal item resource show\">"),
        "{document}"
    );
    assert!(
        document.contains("<h2>An item</h2><h3>Item</h3>"),
        "{document}"
    );
    assert!(
        document.contains(
            "<dd class=\"value literal\" lang=\"\"><span class=\"value-content\">An item</span></dd>"
        ),
        "{document}"
    );
}

/// The media view names the parent item (`view/omeka/site/media/show.phtml`).
#[test]
fn media_view_matches_upstream_media_show() {
    let site = site();
    let resources = TestResources::default();
    let ctx = context(&site, &resources);
    let mut media = item(9, "A file", "/s/main/media/9");
    media.parent_item = Some(ResourceRef {
        url: "/s/main/item/4".to_string(),
        title: "An item".to_string(),
        thumbnail: None,
    });
    let document = DefaultTheme.render(&View::Media(&media), &ctx);
    assert!(
        document.contains(
            "<p class=\"media-parent-item\">Part of <a href=\"/s/main/item/4\">An item</a></p>"
        ),
        "{document}"
    );
}

/// The IIIF viewer page reuses the media renderer's OpenSeadragon container
/// (`Media/Renderer/IIIF.php` via `omeka-media/src/render/iiif.rs`) inside the
/// theme document, and loads the library from the asset base.
#[test]
fn iiif_viewer_matches_the_media_renderer_container() {
    let site = site();
    let resources = TestResources::default();
    let ctx = context(&site, &resources);
    let view = IiifViewerView::new(
        omeka_core::ids::MediaId::new(7),
        "A map",
        r#"{"@id":"https://example.org/iiif/7","width":1000,"height":800}"#,
    );
    let document = DefaultTheme.render(&View::IiifViewer(&view), &ctx);
    assert!(
        document.contains("<title>A map · Romeka S</title>"),
        "{document}"
    );
    assert!(
        document.contains("<body class=\"minimal iiif-viewer\">"),
        "{document}"
    );
    assert!(
        document.contains(
            "<script src=\"/assets/vendor/openseadragon/openseadragon.min.js\"></script>\n"
        ),
        "{document}"
    );
    assert!(
        document.contains(
            "<div class=\"openseadragon\" id=\"iiif-7\" data-prefix-url=\"/assets/vendor/openseadragon/images/\" \
             data-tile-sources=\"{&quot;@id&quot;:&quot;https://example.org/iiif/7&quot;,&quot;width&quot;:1000,&quot;height&quot;:800}\" \
             style=\"height: 400px;\"></div>"
        ),
        "{document}"
    );
    assert!(
        document.contains("const viewer = OpenSeadragon({"),
        "{document}"
    );
    assert!(
        document.contains("tileSources: [JSON.parse(openSeadragonDiv.dataset.tileSources)]"),
        "{document}"
    );
}

/// The viewer page uses the caller's asset base and height.
#[test]
fn iiif_viewer_uses_the_context_asset_base_and_height() {
    let site = site();
    let resources = TestResources::default();
    let mut ctx = context(&site, &resources);
    ctx.asset_base = "/theme/assets/".to_string();
    let mut view = IiifViewerView::new(omeka_core::ids::MediaId::new(3), "Map", "{}");
    view.height = "700px".to_string();
    let document = DefaultTheme.render(&View::IiifViewer(&view), &ctx);
    assert!(
        document.contains(
            "<script src=\"/theme/assets/vendor/openseadragon/openseadragon.min.js\"></script>"
        ),
        "{document}"
    );
    assert!(
        document.contains("data-prefix-url=\"/theme/assets/vendor/openseadragon/images/\""),
        "{document}"
    );
    assert!(document.contains("style=\"height: 700px;\""), "{document}");
    assert!(!document.contains("/theme/assets//"), "{document}");
}

/// The viewer page escapes the title and the tile source, so a stored document
/// carrying markup cannot inject it.
#[test]
fn iiif_viewer_escapes_the_title_and_tile_source() {
    let site = site();
    let resources = TestResources::default();
    let ctx = context(&site, &resources);
    let view = IiifViewerView::new(
        omeka_core::ids::MediaId::new(7),
        "<script>alert('x')</script>",
        "{\"@id\":\"x\" onmouseover=\"steal()\"}",
    );
    let document = DefaultTheme.render(&View::IiifViewer(&view), &ctx);
    assert!(
        document.contains(
            "<title>&lt;script&gt;alert(&#039;x&#039;)&lt;/script&gt; · Romeka S</title>"
        ),
        "{document}"
    );
    assert!(!document.contains("<script>alert"), "{document}");
    assert!(!document.contains("\" onmouseover=\"steal()"), "{document}");
    assert!(
        document.contains("&quot; onmouseover=&quot;steal()"),
        "{document}"
    );
}

/// The browse view prints the pagination and the advanced-search link only
/// when the caller supplies a page size and a total.
#[test]
fn browse_view_matches_upstream_browse() {
    let site = site();
    let resources = TestResources::default();
    let mut ctx = context(&site, &resources);
    ctx.current_path = "/s/main/item".to_string();
    let mut resource = item(4, "An item", "/s/main/item/4");
    resource.description = Some("A description".to_string());
    let browse = BrowseView {
        kind: ResourceKind::Item,
        title: "Items".to_string(),
        resources: vec![resource],
        total: 3,
        query: Vec::new(),
        page: 1,
        per_page: 1,
    };
    let document = DefaultTheme.render(&View::Browse(&browse), &ctx);
    assert!(document.contains("<h2>Items</h2>"), "{document}");
    assert!(
        document.contains(
            "<a href=\"/s/main/item/search\" class=\"advanced-search\">Advanced search</a>"
        ),
        "{document}"
    );
    assert!(
        document.contains("<li class=\"item resource\">"),
        "{document}"
    );
    assert!(
        document.contains("<div class=\"description\">A description</div>"),
        "{document}"
    );
    assert!(
        document.contains("href=\"/s/main/item?page=3\""),
        "{document}"
    );
}

/// `view/error/404.phtml` and `view/error/403.phtml`: the heading and the
/// 404 message, inside the site layout.
#[test]
fn error_views_match_upstream_error_templates() {
    let site = site();
    let resources = TestResources::default();
    let ctx = context(&site, &resources);

    let not_found = ErrorView::not_found();
    let document = DefaultTheme.render(&View::Error(&not_found), &ctx);
    assert!(
        document.contains("<title>Page not found · Romeka S</title>"),
        "{document}"
    );
    assert!(
        document.contains("<body class=\"minimal error\">"),
        "{document}"
    );
    assert!(document.contains("<h1>Page not found</h1>"), "{document}");
    assert!(
        document.contains(
            "<p>The page you are trying to view does not exist. \
                 Use the back button to return to the last page.</p>"
        ),
        "{document}"
    );

    let forbidden = ErrorView::forbidden();
    let document = DefaultTheme.render(&View::Error(&forbidden), &ctx);
    assert!(document.contains("<h1>Forbidden</h1>"), "{document}");
    assert!(!document.contains("<p>"), "{document}");
}

/// A site locale of `es` renders the frame and its labels in Spanish, from the
/// upstream `es.po` terms. The `lang` attribute follows the locale, and the
/// installation title stays the stored value.
#[test]
fn layout_and_labels_render_in_the_site_locale() {
    let site = site();
    let resources = TestResources::default();
    let mut ctx = context(&site, &resources);
    ctx.settings.locale = Some("es_ES".to_string());

    let document = DefaultTheme.render(&View::Index, &ctx);
    assert!(
        document.starts_with("<!DOCTYPE html>\n<html lang=\"es\">"),
        "{document}"
    );
    assert!(document.contains("<title>Romeka S</title>"), "{document}");
    assert!(
        document.contains("<p>Este sitio no tiene páginas.</p>"),
        "{document}"
    );
    assert!(
        document.contains("<span class=\"powered-by\">Funciona con Romeka S</span>"),
        "{document}"
    );

    let item = item(4, "An item", "/s/main/item/4");
    let document = DefaultTheme.render(&View::Item(&item), &ctx);
    assert!(
        document.contains("<h2>An item</h2><h3>Ficha</h3>"),
        "{document}"
    );
}

/// The media view's parent line and the browse search link carry the Spanish
/// catalogue terms too.
#[test]
fn media_and_browse_links_render_in_the_site_locale() {
    let site = site();
    let resources = TestResources::default();
    let mut ctx = context(&site, &resources);
    ctx.settings.locale = Some("es".to_string());

    let mut media = item(9, "A file", "/s/main/media/9");
    media.parent_item = Some(ResourceRef {
        url: "/s/main/item/4".to_string(),
        title: "An item".to_string(),
        thumbnail: None,
    });
    let document = DefaultTheme.render(&View::Media(&media), &ctx);
    assert!(
        document.contains(
            "<p class=\"media-parent-item\">Parte de <a href=\"/s/main/item/4\">An item</a></p>"
        ),
        "{document}"
    );

    let browse = BrowseView {
        kind: ResourceKind::Item,
        title: "Fichas".to_string(),
        resources: Vec::new(),
        total: 0,
        query: Vec::new(),
        page: 1,
        per_page: 0,
    };
    let document = DefaultTheme.render(&View::Browse(&browse), &ctx);
    assert!(
        document.contains("class=\"advanced-search\">Búsqueda avanzada</a>"),
        "{document}"
    );
}
