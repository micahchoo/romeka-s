use super::*;
use crate::blocks::test_support::{TestResources, context, item, literal_value, site};
use crate::share::{ShareTarget, ShareView};
use crate::view::{
    DownloadLink, MediaIcon, MediaKindCue, MediaRef, ResourceRef, ResourceView, Thumbnail,
};
use omeka_core::ids::ResourceId;
use omeka_core::resource::ResourceKind;

/// The layout writes the document frame from `view/layout/layout.phtml`:
/// doctype, head, the installation title, the asset base, and the footer.
#[test]
fn layout_matches_upstream_layout() {
    let site = site();
    let resources = TestResources::default();
    let ctx = context(&site, &resources);
    let document = DefaultTheme.render(&View::Index, &ctx);
    assert!(
        document.starts_with(
            "<!DOCTYPE html>\n<html lang=\"en\" class=\"hero-split cards-row search-open \
             footer-inline item-side\">"
        ),
        "{document}"
    );
    assert!(document.contains("<title>Romeka S</title>"), "{document}");
    assert!(
        document.contains("href=\"/assets/css/style.css?v="),
        "{document}"
    );
    assert!(document.contains("<body class=\"home\">"), "{document}");
    assert!(
        document.contains("<p>This site has no pages.</p>"),
        "{document}"
    );
    assert!(
        document.contains("<p class=\"powered-by\">Powered by Romeka S</p>"),
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
    for asset in DefaultTheme
        .info()
        .assets
        .iter()
        .filter(|asset| asset.kind.is_head())
    {
        let element = asset.kind.element(&format!(
            "{}/{}?v={}",
            ctx.asset_base,
            asset.path,
            asset.version()
        ));
        assert!(document.contains(&element), "{document}");
    }
    // A view-referenced image is shipped but not linked from the head.
    assert!(
        !document.contains("/img/placeholder.svg"),
        "the placeholder is not a head asset: {document}"
    );
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
        document.contains("href=\"/theme/assets/css/style.css?v="),
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
        document.contains("<body class=\"item resource show\">"),
        "{document}"
    );
    assert!(
        document.contains(
            "<nav class=\"breadcrumbs\"><a href=\"/s/main/item\">Items</a> / <span>An item</span></nav>\
             <div class=\"resource-header\"><p class=\"resource-kind-label\">Item</p>\
             <h2 class=\"resource-title\">An item</h2></div>"
        ),
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
        media_kind: None,
        notes: 0,
    });
    let document = DefaultTheme.render(&View::Media(&media), &ctx);
    assert!(
        document.contains(
            "<p class=\"media-parent-item\">Part of <a href=\"/s/main/item/4\">An item</a></p>"
        ),
        "{document}"
    );
    assert!(
        document.contains("<div class=\"annotation-list\"></div>"),
        "the notes list has its place on the media page: {document}"
    );
}

/// The media view says which file of the item this is, and links the file
/// before and the one after it. A file alone in its item says neither, because
/// "File 1 of 1" tells a reader nothing.
#[test]
fn media_view_places_the_file_among_its_items_files() {
    let site = site();
    let resources = TestResources::default();
    let ctx = context(&site, &resources);
    let mut media = item(9, "A file", "/s/main/media/9");
    media.media_position = Some(MediaPosition {
        number: 3,
        total: 5,
        previous: Some(ResourceRef {
            url: "/s/main/media/2".to_string(),
            title: "Before".to_string(),
            thumbnail: None,
            media_kind: None,
            notes: 0,
        }),
        next: Some(ResourceRef {
            url: "/s/main/media/4".to_string(),
            title: "After".to_string(),
            thumbnail: None,
            media_kind: None,
            notes: 0,
        }),
    });
    let document = DefaultTheme.render(&View::Media(&media), &ctx);
    assert!(
        document.contains(
            "<span class=\"media-position-count\">File 3 of 5</span>\
             <a class=\"media-previous\" rel=\"prev\" href=\"/s/main/media/2\">Previous</a>\
             <a class=\"media-next\" rel=\"next\" href=\"/s/main/media/4\">Next</a>"
        ),
        "{document}"
    );

    media.media_position = Some(MediaPosition {
        number: 1,
        total: 1,
        previous: None,
        next: None,
    });
    let document = DefaultTheme.render(&View::Media(&media), &ctx);
    assert!(!document.contains("media-position"), "{document}");
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
        document.contains("<li class=\"item resource no-thumb\">"),
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

    let not_found = ErrorView::not_found(ctx.translator().messages());
    let document = DefaultTheme.render(&View::Error(&not_found), &ctx);
    assert!(
        document.contains("<title>Page not found · Romeka S</title>"),
        "{document}"
    );
    assert!(document.contains("<body class=\"error\">"), "{document}");
    assert!(document.contains("<h1>Page not found</h1>"), "{document}");
    assert!(
        document.contains(
            "<p>The page you are trying to view does not exist. \
                 Use the back button to return to the last page.</p>"
        ),
        "{document}"
    );

    let forbidden = ErrorView::forbidden(ctx.translator().messages());
    let document = DefaultTheme.render(&View::Error(&forbidden), &ctx);
    assert!(document.contains("<h1>Forbidden</h1>"), "{document}");
    assert!(!document.contains("<p>"), "{document}");
}

/// A Spanish site prints the error pages, the menu toggle and the empty
/// navigation's home link with `es.po`'s terms.
#[test]
fn error_pages_and_menu_render_in_the_site_locale() {
    let site = site();
    let resources = TestResources::default();
    let mut ctx = context(&site, &resources);
    ctx.settings.locale = Some("es".to_string());
    ctx.nav = crate::nav::resolve(&[], &site.slug, &[], ctx.translator().messages());

    let not_found = ErrorView::not_found(ctx.translator().messages());
    let document = DefaultTheme.render(&View::Error(&not_found), &ctx);
    assert!(
        document.contains("<h1>Página no encontrada</h1>"),
        "{document}"
    );
    assert!(
        document.contains(
            "<p>La página solicitada no existe. El botón de volver atrás regresará a la última página.</p>"
        ),
        "{document}"
    );
    assert!(document.contains("</span>Menú</summary>"), "{document}");
    assert!(document.contains("aria-label=\"Menú\""), "{document}");
    assert!(document.contains(">Inicio</a>"), "{document}");

    let forbidden = ErrorView::forbidden(ctx.translator().messages());
    let document = DefaultTheme.render(&View::Error(&forbidden), &ctx);
    assert!(document.contains("<h1>Prohibido</h1>"), "{document}");
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
        document.starts_with("<!DOCTYPE html>\n<html lang=\"es\" class=\""),
        "{document}"
    );
    assert!(document.contains("<title>Romeka S</title>"), "{document}");
    assert!(
        document.contains("<p>Este sitio no tiene páginas.</p>"),
        "{document}"
    );
    assert!(
        document.contains("<p class=\"powered-by\">Funciona con Romeka S</p>"),
        "{document}"
    );

    let item = item(4, "An item", "/s/main/item/4");
    let document = DefaultTheme.render(&View::Item(&item), &ctx);
    assert!(
        document.contains(
            "<p class=\"resource-kind-label\">Ficha</p><h2 class=\"resource-title\">An item</h2>"
        ),
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
        media_kind: None,
        notes: 0,
    });
    media.media_position = Some(MediaPosition {
        number: 3,
        total: 5,
        previous: Some(ResourceRef {
            url: "/s/main/media/2".to_string(),
            title: "Before".to_string(),
            thumbnail: None,
            media_kind: None,
            notes: 0,
        }),
        next: Some(ResourceRef {
            url: "/s/main/media/4".to_string(),
            title: "After".to_string(),
            thumbnail: None,
            media_kind: None,
            notes: 0,
        }),
    });
    let document = DefaultTheme.render(&View::Media(&media), &ctx);
    assert!(
        document.contains(
            "<p class=\"media-parent-item\">Parte de <a href=\"/s/main/item/4\">An item</a></p>"
        ),
        "{document}"
    );
    assert!(
        document.contains(
            "<span class=\"media-position-count\">Archivo 3 de 5</span>\
             <a class=\"media-previous\" rel=\"prev\" href=\"/s/main/media/2\">Anterior</a>\
             <a class=\"media-next\" rel=\"next\" href=\"/s/main/media/4\">Siguiente</a>"
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

/// The frame emits the design tokens as custom properties in a `<style>` in
/// the head, and a curated token reaches CSS under its own name.
#[test]
fn layout_emits_the_theme_tokens_as_custom_properties() {
    let site = site();
    let resources = TestResources::default();
    let mut ctx = context(&site, &resources);
    ctx.theme_settings = crate::theme::ThemeSettings::from_value(&serde_json::json!({
        "brand_color": "#cc3300",
        "heading_font": "geometric",
        "density": "compact"
    }));
    let document = DefaultTheme.render(&View::Index, &ctx);

    let style = &document[document.find("<style>").expect("the token style")..];
    assert!(style.contains("\n:root {"), "{document}");
    assert!(document.contains("--color-brand: #cc3300;"), "{document}");
    assert!(
        document
            .contains("--font-heading: Futura, \"Century Gothic\", \"Avenir Next\", sans-serif;"),
        "{document}"
    );
    assert!(document.contains("--space-1: 0.125rem;"), "{document}");
    // The sheet link still follows the style block, so the tokens win.
    let style_at = document.find("<style>").expect("style block");
    let sheet_at = document.find("css/style.css").expect("stylesheet link");
    assert!(style_at < sheet_at, "{document}");
}

/// The header layout token sets the header's class, so the sheet can centre or
/// left-align the logo and navigation.
#[test]
fn layout_honours_the_header_layout_token() {
    let site = site();
    let resources = TestResources::default();
    let mut ctx = context(&site, &resources);
    ctx.theme_settings = crate::theme::ThemeSettings::from_value(&serde_json::json!({
        "header_layout": "centred"
    }));
    let document = DefaultTheme.render(&View::Index, &ctx);
    assert!(
        document.contains("<header class=\"header-centred\">"),
        "{document}"
    );
    assert!(
        !document.contains("<header class=\"header-logo-left\">"),
        "{document}"
    );
}

/// The logo and banner tokens resolve to their assets, and a missing asset
/// renders nothing rather than a broken image.
#[test]
fn layout_renders_the_logo_and_banner_assets() {
    let site = site();
    let mut resources = TestResources::default();
    resources.assets.insert(
        3,
        crate::view::AssetView {
            id: omeka_core::ids::AssetId::new(3),
            title: "A logo".to_string(),
            url: Some("/files/original/3.png".to_string()),
            thumbnail: None,
            kind: crate::view::AssetKind::Image,
            size: None,
        },
    );
    let mut ctx = context(&site, &resources);
    ctx.theme_settings = crate::theme::ThemeSettings::from_value(&serde_json::json!({
        "logo": 3,
        "banner": 4,
        "banner_text": "Open daily <10am>"
    }));
    let document = DefaultTheme.render(&View::Index, &ctx);
    assert!(
        document.contains(
            "<a class=\"site-logo\" href=\"/s/main\"><img src=\"/files/original/3.png\" alt=\"Main Site\"></a>"
        ),
        "{document}"
    );
    // Asset 4 is not in the provider, so only the banner text renders.
    assert!(
        document.contains("<div class=\"site-banner\">"),
        "{document}"
    );
    assert!(!document.contains("site-banner-image"), "{document}");
    assert!(
        document.contains("<p class=\"site-banner-text\">Open daily &lt;10am&gt;</p>"),
        "{document}"
    );
}

/// A logo and a banner take images, so an asset of another kind shows
/// neither: the header keeps no logo and the banner keeps its text, never a
/// broken `<img>`.
#[test]
fn a_non_image_logo_and_banner_render_nothing() {
    let site = site();
    let mut resources = TestResources::default();
    for id in [3, 4] {
        resources.assets.insert(
            id,
            crate::view::AssetView {
                id: omeka_core::ids::AssetId::new(id),
                title: "report.pdf".to_string(),
                url: Some("/files/asset/report.pdf".to_string()),
                thumbnail: None,
                kind: crate::view::AssetKind::Pdf,
                size: Some(4096),
            },
        );
    }
    let mut ctx = context(&site, &resources);
    ctx.theme_settings = crate::theme::ThemeSettings::from_value(&serde_json::json!({
        "logo": 3,
        "banner": 4,
        "banner_text": "Open daily"
    }));
    let document = DefaultTheme.render(&View::Index, &ctx);
    assert!(!document.contains("site-logo"), "{document}");
    assert!(!document.contains("site-banner-image"), "{document}");
    assert!(
        document.contains("<p class=\"site-banner-text\">Open daily</p>"),
        "{document}"
    );
}

/// The footer text token renders purified HTML, and a hostile value cannot
/// execute.
#[test]
fn layout_purifies_the_footer_text() {
    let site = site();
    let resources = TestResources::default();
    let mut ctx = context(&site, &resources);
    ctx.theme_settings = crate::theme::ThemeSettings::from_value(&serde_json::json!({
        "footer_text": "<p>Friends</p><script>alert(1)</script>"
    }));
    let document = DefaultTheme.render(&View::Index, &ctx);
    assert!(
        document.contains("<div class=\"footer-text\"><p>Friends</p></div>"),
        "{document}"
    );
    assert!(
        !document.contains("<script>alert(1)</script>"),
        "{document}"
    );
}

/// The poor-data card: an untitled item with no image shows its identifier
/// without the file extension, a tinted surface with the kind icon and its
/// type, and its type, creator and date as details. An untitled item's page
/// says no title was recorded and shows a panel where its media would be.
#[test]
fn an_untitled_imageless_item_still_reads_as_a_record() {
    let site = site();
    let resources = TestResources::default();
    let ctx = context(&site, &resources);
    let mut bare = item(4, "[Untitled]", "/s/main/item/4");
    bare.thumbnail = None;
    for (term, value) in [
        ("dcterms:identifier", "BHC006_GMT_PN_215.png"),
        ("dcterms:type", "Photo Narrative"),
        ("dcterms:creator", "Citizen Curators"),
        ("dcterms:date", "2018"),
    ] {
        let mut entry = literal_value(1, "Label", value);
        entry.property_term = term.to_string();
        bare.values.push(entry);
    }
    let browse = BrowseView {
        kind: ResourceKind::Item,
        title: "Items".to_string(),
        resources: vec![bare.clone()],
        total: 1,
        query: Vec::new(),
        page: 1,
        per_page: 1,
    };
    let document = DefaultTheme.render(&View::Browse(&browse), &ctx);
    assert!(
        document.contains(
            "<li class=\"item resource no-thumb\"><a href=\"/s/main/item/4\" class=\"resource-link\">\
             <span class=\"resource-thumb resource-thumb-empty\" data-kind=\"item\"><svg class=\"kind-icon\""
        ),
        "{document}"
    );
    assert!(
        document.contains(
            "<span class=\"resource-kind\">No file</span></span>\
             <span class=\"resource-text\"><span class=\"resource-name resource-name-fallback\">BHC006_<wbr>GMT_<wbr>PN_<wbr>215</span>\
             <span class=\"resource-meta\"><span class=\"resource-meta-type\">Photo Narrative</span>\
             <span class=\"resource-meta-creator\">Citizen Curators</span>\
             <span class=\"resource-meta-date\">2018</span></span></span></a></li>"
        ),
        "{document}"
    );
    assert!(!document.contains("[Untitled]</span>"), "{document}");

    let page = DefaultTheme.render(&View::Item(&bare), &ctx);
    assert!(
        page.contains(
            "<div class=\"resource-header\"><p class=\"resource-kind-label\">Item · Photo Narrative</p>\
             <h2 class=\"resource-title resource-name-fallback\">BHC006_<wbr>GMT_<wbr>PN_<wbr>215</h2>\
             <p class=\"resource-untitled-note\">No title recorded</p></div>"
        ),
        "{page}"
    );
    assert!(
        page.contains(
            "<div class=\"media-embeds media-empty\"><span class=\"resource-thumb resource-thumb-empty\" data-kind=\"item\">"
        ),
        "{page}"
    );
    assert!(
        page.contains("<span class=\"resource-kind\">No media attached</span>"),
        "{page}"
    );
}

/// A preset's webfonts load from the theme's own assets: the frame preloads
/// the Latin files and declares each face with `font-display: swap`. A site
/// on system faces loads no font.
#[test]
fn the_frame_loads_only_the_fonts_the_site_uses() {
    let site = site();
    let resources = TestResources::default();
    let mut ctx = context(&site, &resources);
    ctx.theme_settings =
        crate::theme::ThemeSettings::from_value(&serde_json::json!({"preset": "exhibit"}));
    let document = DefaultTheme.render(&View::Index, &ctx);
    assert!(
        document
            .contains("<link rel=\"preload\" href=\"/assets/fonts/fraunces-latin-opsz.woff2?v="),
        "{document}"
    );
    assert_eq!(document.matches("rel=\"preload\"").count(), 2, "{document}");
    assert!(
        document.contains("@font-face { font-family: \"Noto Sans Devanagari\""),
        "{document}"
    );
    assert!(!document.contains("\"Inter\""), "{document}");

    ctx.theme_settings = crate::theme::ThemeSettings::from_value(&serde_json::json!({
        "preset": "exhibit",
        "heading_font": "system-serif",
        "body_font": "system-sans",
    }));
    let document = DefaultTheme.render(&View::Index, &ctx);
    assert!(!document.contains("@font-face"), "{document}");
    assert!(!document.contains("rel=\"preload\""), "{document}");
}

/// The footer: structured columns, links, contact, licence and the credit
/// switch. A map from before the footer had structure shows its footer text
/// as the first column, and with no links set the footer repeats the menu.
#[test]
fn the_footer_renders_its_structured_settings() {
    let site = site();
    let resources = TestResources::default();
    let mut ctx = context(&site, &resources);
    ctx.nav = crate::nav::resolve(&[], &site.slug, &[], ctx.translator().messages());
    ctx.theme_settings = crate::theme::ThemeSettings::from_value(&serde_json::json!({
        "footer_text": "<p>Old footer</p><script>x()</script>",
    }));
    let document = DefaultTheme.render(&View::Index, &ctx);
    assert!(
        document.contains(
            "<footer class=\"site-footer\">\n<div class=\"footer-inner\">\n<div class=\"footer-site\">\
             <p class=\"footer-title\">Main Site</p><div class=\"footer-texts\">\
             <div class=\"footer-text\"><p>Old footer</p></div></div></div>"
        ),
        "{document}"
    );
    assert!(
        document.contains("<nav class=\"footer-links\" aria-label=\"Footer links\"><ul><li><a href=\"/s/main\">Home</a></li></ul></nav>"),
        "{document}"
    );
    assert!(document.contains("Powered by Romeka S"), "{document}");

    ctx.theme_settings = crate::theme::ThemeSettings::from_value(&serde_json::json!({
        "footer_text": "<p>Old footer</p>",
        "footer_columns": ["<p>One</p>", " ", "<p>Two</p>", "<p>Three</p>", "<p>Four</p>"],
        "footer_links": [{"type": "url", "data": {"label": "Contact us", "url": "https://example.org/contact"}}],
        "footer_contact": "<p>1 Museum Road</p>",
        "footer_licence": "cc-by-sa",
        "show_powered_by": false,
    }));
    let document = DefaultTheme.render(&View::Index, &ctx);
    assert!(!document.contains("Old footer"), "{document}");
    assert!(document.contains("<p>Three</p>"), "{document}");
    assert!(!document.contains("<p>Four</p>"), "{document}");
    assert!(
        document.contains("<li><a href=\"https://example.org/contact\">Contact us</a></li>"),
        "{document}"
    );
    assert!(
        !document.contains(">Home</a></li></ul></nav>\n<address"),
        "{document}"
    );
    assert!(
        document.contains("<address class=\"footer-contact\"><p>1 Museum Road</p></address>"),
        "{document}"
    );
    assert!(
        document.contains(
            "<a rel=\"license\" class=\"licence-badge\" href=\"https://creativecommons.org/licenses/by-sa/4.0/\">\
             <span class=\"licence-mark\" aria-hidden=\"true\">cc</span>CC BY-SA 4.0</a>"
        ),
        "{document}"
    );
    assert!(!document.contains("Powered by Romeka S"), "{document}");
}

/// A site's own CSS renders in its own `<style>` after the theme sheet, so its
/// rules win; CSS the sanitiser refuses renders nothing.
#[test]
fn layout_renders_custom_css_after_the_theme_sheet() {
    let site = site();
    let resources = TestResources::default();
    let mut ctx = context(&site, &resources);
    ctx.theme_settings = crate::theme::ThemeSettings::from_value(&serde_json::json!({
        "custom_css": ".site-title a { color: #cc0000 }"
    }));
    let document = DefaultTheme.render(&View::Index, &ctx);
    let sheet_at = document.find("css/style.css").expect("stylesheet link");
    let custom_at = document
        .find("<style id=\"custom-css\">\n.site-title a { color: #cc0000 }\n</style>")
        .expect("the custom CSS block");
    assert!(sheet_at < custom_at, "{document}");

    ctx.theme_settings = crate::theme::ThemeSettings::from_value(&serde_json::json!({
        "custom_css": "p {} </style><script>alert(1)</script>"
    }));
    let hostile = DefaultTheme.render(&View::Index, &ctx);
    assert!(!hostile.contains("custom-css"), "{hostile}");
    assert!(!hostile.contains("<script>alert(1)"), "{hostile}");
}

/// The media page's head names the media manifest and the picture a link
/// preview shows, and its "Use elsewhere" block carries the drag-and-drop
/// convention (`{m}?manifest={m}`) beside a copy button. The manifest URL is
/// also in a read-only input, so a reader without a script can select it.
#[test]
fn the_media_page_shares_its_manifest_and_picture() {
    let site = site();
    let resources = TestResources::default();
    let ctx = context(&site, &resources);
    let manifest = "https://example.org/iiif/3/media/9/manifest";
    let mut media = ResourceView::new(
        ResourceId::new(9),
        ResourceKind::Media,
        "/s/main/media/9",
        "A file",
    );
    media.thumbnail = Some(Thumbnail::new("https://example.org/files/square/9.jpg", ""));
    media.share = Some(ShareView::page(
        ShareTarget::Media,
        "A file",
        "https://example.org/s/main/media/9",
        Some("https://example.org/files/square/9.jpg".to_string()),
        Some(manifest.to_string()),
        Some("https://example.org/iiif-viewer?url=manifest".to_string()),
    ));

    let document = DefaultTheme.render(&View::Media(&media), &ctx);
    assert!(
        document.contains(&format!(
            "<link rel=\"alternate\" type=\"application/ld+json;profile=\
             &quot;http://iiif.io/api/presentation/3/context.json&quot;\" href=\"{manifest}\">"
        )),
        "{document}"
    );
    assert!(
        document.contains("<meta property=\"og:title\" content=\"A file\">"),
        "{document}"
    );
    assert!(
        document.contains(
            "<meta property=\"og:image\" content=\"https://example.org/files/square/9.jpg\">"
        ),
        "{document}"
    );
    assert!(
        document.contains(&format!(
            "<a class=\"iiif-manifest\" href=\"{manifest}?manifest={manifest}\" \
             title=\"Drag into a IIIF viewer\">"
        )),
        "{document}"
    );
    assert!(
        document.contains(&format!(
            "<input class=\"manifest-url\" type=\"text\" readonly value=\"{manifest}\" \
             aria-label=\"Manifest URL\">"
        )),
        "{document}"
    );
    assert!(document.contains(">Copy</button>"), "{document}");
    assert!(
        document.contains(
            "<a href=\"https://example.org/iiif-viewer?url=manifest\">\
                           View in the IIIF viewer</a>"
        ),
        "{document}"
    );
    assert!(
        document.contains(
            "Cite this file <a href=\"https://example.org/s/main/media/9\">\
             https://example.org/s/main/media/9</a>"
        ),
        "{document}"
    );
}

/// A kind with no canvas offers no viewer and no manifest link. The page still
/// cites the file and saves its original.
#[test]
fn a_kind_with_no_canvas_shows_no_viewer_link() {
    let site = site();
    let resources = TestResources::default();
    let ctx = context(&site, &resources);
    let mut media = ResourceView::new(
        ResourceId::new(9),
        ResourceKind::Media,
        "/s/main/media/9",
        "notes.txt",
    );
    media.media_kind = Some(MediaKindCue {
        kind: "other-file",
        label: "File",
        icon: MediaIcon::File,
    });
    media.download = Some(DownloadLink {
        url: "/s/main/media/9/download".to_string(),
        size: Some(2_411_724),
    });
    media.share = Some(ShareView::page(
        ShareTarget::Media,
        "notes.txt",
        "https://example.org/s/main/media/9",
        None,
        None,
        None,
    ));

    let document = DefaultTheme.render(&View::Media(&media), &ctx);
    assert!(
        document.contains(
            "<p class=\"media-download\"><a href=\"/s/main/media/9/download\">\
             Download the original · File · 2.3 MB</a></p>"
        ),
        "{document}"
    );
    assert!(!document.contains("use-elsewhere-viewer"), "{document}");
    assert!(!document.contains("iiif-manifest"), "{document}");
    assert!(!document.contains("rel=\"alternate\""), "{document}");
    assert!(
        document.contains(
            "Cite this file <a href=\"https://example.org/s/main/media/9\">\
             https://example.org/s/main/media/9</a>"
        ),
        "{document}"
    );
}

/// The item page shares the item manifest under "Use elsewhere", after the
/// file, and its head names the item's picture.
#[test]
fn the_item_page_shares_its_manifest() {
    let site = site();
    let resources = TestResources::default();
    let ctx = context(&site, &resources);
    let manifest = "https://example.org/iiif/3/item/7/manifest";
    let mut item = item(7, "An item", "/s/main/item/7");
    item.thumbnail = Some(Thumbnail::new("https://example.org/files/square/7.jpg", ""));
    item.iiif_manifest = Some(manifest.to_string());
    item.media_embeds.push(MediaRef {
        url: "/s/main/media/9".to_string(),
        original_url: None,
        title: "A file".to_string(),
        media_type: Some("image/jpeg".to_string()),
        thumbnail: None,
        embed: Some("<img src=\"/files/large/9.jpg\" alt=\"\">".to_string()),
    });
    item.share = Some(ShareView::page(
        ShareTarget::Item,
        "An item",
        "https://example.org/s/main/item/7",
        Some("https://example.org/files/square/7.jpg".to_string()),
        Some(manifest.to_string()),
        Some("https://example.org/iiif-viewer?url=manifest".to_string()),
    ));

    let document = DefaultTheme.render(&View::Item(&item), &ctx);
    assert!(
        document.contains(&format!("href=\"{manifest}\">")),
        "{document}"
    );
    assert!(document.contains(">Use elsewhere</h2>"), "{document}");
    assert!(
        document.contains("Cite this item <a href=\"https://example.org/s/main/item/7\">"),
        "{document}"
    );
}
