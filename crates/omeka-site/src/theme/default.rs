//! The bundled theme's views.
//!
//! Ported from `view/layout/layout.phtml`, `view/omeka/site/index/index.phtml`,
//! `view/omeka/site/page/show.phtml`, `view/omeka/site/page/content.phtml`,
//! `view/omeka/site/item/browse.phtml`, `view/omeka/site/item/show.phtml`,
//! `view/omeka/site/item-set/browse.phtml`,
//! `view/omeka/site/media/show.phtml`, `common/site-page-pagination.phtml`,
//! `view/error/403.phtml` and
//! `view/error/404.phtml`.
//!
//! What the templates get from view helpers the port cannot call, and how the
//! port stands in:
//!
//! - `headTitle`, `headMeta`, `headLink`, `headScript`: the layout writes a
//!   fixed head with the theme's asset base. The document title is
//!   `{view title} · {installation_title}`; the separator is upstream's `· `.
//!   Upstream's head appends a page title after the installation title set in
//!   the layout, so the joined order is inferred.
//! - `trigger('view.show.before')` and module view listeners: not ported,
//!   there are no modules.
//! - `resourcePageBlocks` (the `Site/ResourcePageBlockLayout/*` registry) and
//!   `searchFilters`, `browse()->renderSortSelector`: not ported. The item,
//!   item-set and media views print the values the provider supplies instead of
//!   the resource page blocks, and the browse views print the pagination and
//!   the advanced-search link without the filter UI.
//! - `$this->lang()`: the fixed copy and the dates go through
//!   [`crate::i18n`], the translator the site's `locale` setting selects. The
//!   error page's heading and message arrive already resolved on
//!   [`ErrorView`](crate::view::ErrorView), so the theme prints them as given.
//!
//! The port's frame prints the site title as the document's `<h1>`, which
//! upstream's bare `view/layout/layout.phtml` does not. Upstream's views call
//! `pageTitle(..., 2)`, so a view like the item browse or the empty index had no
//! level-one heading; axe reports that as `page-has-heading-one`. The frame owns
//! the heading so every view has one, and the views keep their upstream levels
//! below it.

use crate::blocks;
use crate::html::{Attr, escape_html, hyperlink, pagination};
use crate::i18n::{Translator, sprintf1};
use crate::nav::{NavItem, find_item_by_page, nav_path, render_menu};
use crate::resource_page::render_blocks;
use crate::slug::{controller_for, resource_action_url, site_url};
use crate::theme::{Theme, ThemeInfo};
use crate::theme::{cards, fonts, search, user_bar};
use crate::view::{BrowseView, ErrorView, RenderContext, ResourceView, SiteRegion, View};

/// The bundled theme's manifest: the id `site.theme` stores, the design tokens
/// it reads, and the files it ships.
pub static DEFAULT_INFO: ThemeInfo = ThemeInfo {
    name: "default",
    label: "Default",
    settings: crate::theme::tokens::SETTINGS,
    assets: &super::files::FILES,
};

/// The bundled theme.
#[derive(Debug, Clone, Copy, Default)]
pub struct DefaultTheme;

impl Theme for DefaultTheme {
    fn info(&self) -> &'static ThemeInfo {
        &DEFAULT_INFO
    }

    fn render(&self, view: &View<'_>, ctx: &RenderContext<'_>) -> String {
        let tr = ctx.translator();
        match view {
            View::Index => layout(view, ctx, None, "home", &index_view(tr)),
            View::Page => layout(
                view,
                ctx,
                ctx.page.map(|page| page.title.as_str()),
                &page_body_class(ctx),
                &crate::blocks::carousel::with_assets(
                    crate::map::with_assets(page_view(ctx), ctx),
                    ctx,
                ),
            ),
            View::Browse(browse) => {
                let mut content = browse_view(browse, ctx, tr);
                for resource in &browse.resources {
                    content.push_str(&jsonld_script(ctx, resource.jsonld.as_deref()));
                }
                layout(
                    view,
                    ctx,
                    Some(&browse.title),
                    &format!("{} resource browse", controller_for(browse.kind)),
                    &content,
                )
            }
            View::Item(item) => {
                let mut content = crate::map::with_assets(item_view(item, ctx), ctx);
                content.push_str(&jsonld_script(ctx, item.jsonld.as_deref()));
                layout(view, ctx, Some(&item.title), "item resource show", &content)
            }
            View::ItemSet(item_set) => {
                let mut content = item_set_view(item_set, ctx);
                content.push_str(&jsonld_script(ctx, item_set.jsonld.as_deref()));
                layout(
                    view,
                    ctx,
                    Some(&item_set.title),
                    "item-set resource browse",
                    &content,
                )
            }
            View::Media(media) => {
                let mut content = media_view(media, ctx);
                content.push_str(&jsonld_script(ctx, media.jsonld.as_deref()));
                layout(
                    view,
                    ctx,
                    Some(&media.title),
                    "media resource show",
                    &content,
                )
            }
            View::Search(results) => layout(
                view,
                ctx,
                None,
                "index search",
                &search::search_view(results, ctx),
            ),
            View::CrossSiteSearch(page) => {
                layout(view, ctx, None, "", &search::cross_site_view(page, ctx))
            }
            View::SearchList(list) => layout(
                view,
                ctx,
                None,
                "page browse",
                &search::search_list_view(list, ctx),
            ),
            View::AdvancedSearch(form) => layout(
                view,
                ctx,
                None,
                &format!("{} search", form.resource.body_class()),
                &search::advanced_search_view(form, ctx),
            ),
            View::Error(error) => {
                layout(view, ctx, Some(&error.title), "error", &error_view(error))
            }
        }
    }
}

/// `view/layout/layout.phtml`: the document frame.
///
/// The head carries the core assets first, as upstream links them with
/// `assetUrl(..., 'Omeka')`: `fonts.css` and `iconfonts.css`, then the jQuery
/// and `global.js` scripts. The theme's own assets sit between the two, where
/// upstream appends `style.css`. The `lang` attribute is the site's locale.
fn layout(
    view: &View<'_>,
    ctx: &RenderContext<'_>,
    view_title: Option<&str>,
    body_class: &str,
    content: &str,
) -> String {
    let info = &DEFAULT_INFO;
    let tr = ctx.translator();
    let title = match view_title.filter(|title| !title.is_empty()) {
        Some(title) => format!(
            "{} · {}",
            escape_html(title),
            escape_html(ctx.settings.installation_title())
        ),
        None => escape_html(ctx.settings.installation_title()),
    };
    let assets = ctx.asset_base.trim_end_matches('/');
    // `themes/default/view/layout/layout.phtml`: the site title links home.
    let site_title = format!(
        "<h1 class=\"site-title\"><a href=\"{}\">{}</a></h1>",
        escape_html(&site_url(&ctx.site.slug)),
        escape_html(&ctx.site.title)
    );
    // `$site->publicNav()->menu()->renderMenu(null, ['id' => 'top-nav-contents'])`.
    // `ctx.nav` is resolved from the pages the caller may see, so a hidden page
    // never reaches the menu. The theme builds the list here so the current
    // page carries `aria-current` and the items carry their own classes.
    let nav_menu = header_menu(ctx);
    let menu_label = tr.messages().menu;
    let mut theme_assets = String::new();
    for asset in info.assets.iter().filter(|asset| asset.kind.is_head()) {
        theme_assets.push_str(&asset.kind.element(&format!(
            "{assets}/{}?v={}",
            escape_html(asset.path),
            asset.version()
        )));
        theme_assets.push('\n');
    }
    // The design tokens as custom properties, one `<style>` in the head. Every
    // value is validated at the storage boundary, so nothing a curator stored
    // reaches the sheet as CSS.
    let tokens = ctx.theme_settings.css();
    let fonts = ctx.theme_settings.tokens.fonts();
    let font_faces = fonts::faces(fonts, assets);
    let font_preloads = fonts::preloads(fonts, assets);
    let root_classes = ctx.theme_settings.tokens.classes();
    let header = ctx.theme_settings.tokens.header_layout.class();
    // The site's own CSS follows the theme sheet so its rules win. It passed
    // the sanitiser when it was read, which refuses `</style` among the rest,
    // so it cannot end this element.
    let custom_css = match &ctx.theme_settings.custom_css {
        Some(css) => format!("<style id=\"custom-css\">\n{}\n</style>\n", css.as_str()),
        None => String::new(),
    };
    let logo = logo_markup(ctx);
    let banner = banner_markup(ctx);
    let footer = ctx.theme_settings.footer.render(ctx, tr);
    // `Module.php:188-203` prepends the favicon to `headLink` on `view.layout`.
    let favicon = match &ctx.favicon_url {
        Some(url) => format!("<link rel=\"icon\" href=\"{}\">\n", escape_html(url)),
        None => String::new(),
    };
    // `Module.php:925-931`: an item, item-set or media not in the site is
    // `noindex`.
    let robots = if ctx.noindex {
        "<meta name=\"robots\" content=\"noindex\">\n"
    } else {
        ""
    };
    let user_bar = user_bar::render(view, ctx, tr);
    format!(
        "<!DOCTYPE html>\n<html lang=\"{lang}\" class=\"{root_classes}\">\n<head>\n\
         <meta charset=\"utf-8\">\n\
         <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n\
         <title>{title}</title>\n\
         {robots}\
         {favicon}\
         {font_preloads}\
         <style>\n{font_faces}{tokens}</style>\n\
         {theme_assets}\
         {custom_css}\
         </head>\n\
         <body{body}>\n\
         <a class=\"skip-link\" id=\"skipnav\" href=\"#content\">{skip}</a>\n\
         {user_bar}\
         {banner}\
         <header class=\"{header}\"{header_region}>\n\
         <div class=\"header-inner\">\n\
         {logo}{site_title}\n\
         <details class=\"nav-menu\">\n\
         <summary class=\"nav-menu-toggle\"><span class=\"nav-menu-icon\" aria-hidden=\"true\"></span>{menu_label}</summary>\n\
         <nav id=\"top-nav\" aria-label=\"{menu_label}\">\n{nav_menu}\n</nav>\n\
         </details>\n\
         <details class=\"search-menu\">\n\
         <summary class=\"search-toggle\">{search_icon}<span class=\"visually-hidden\">{search_label}</span></summary>\n\
         <div id=\"search\">{search_form}</div>\n\
         </details>\n\
         </div>\n\
         </header>\n\
         <div id=\"content\" role=\"main\">\n\
         {content}\n</div>\n\
         {footer}\n\
         </body>\n</html>",
        lang = tr.locale().tag(),
        body = match body_class {
            "" => String::new(),
            class => format!(" class=\"{}\"", escape_html(class)),
        },
        header = header,
        header_region = ctx.region_marker(SiteRegion::Header),
        logo = logo,
        banner = banner,
        footer = footer,
        tokens = tokens,
        font_faces = font_faces,
        font_preloads = font_preloads,
        root_classes = escape_html(&root_classes),
        search_icon = SEARCH_ICON,
        search_label = escape_html(tr.search_labels().search),
        site_title = site_title,
        nav_menu = nav_menu,
        menu_label = escape_html(menu_label),
        skip = escape_html(tr.messages().skip_to_main),
        user_bar = user_bar,
        robots = robots,
        favicon = favicon,
        search_form = search::search_form(ctx, tr.search_labels()),
    )
}

/// The header's search toggle: a magnifier. The toggle's name is the hidden
/// text beside it.
const SEARCH_ICON: &str = "<svg class=\"search-glyph\" viewBox=\"0 0 24 24\" aria-hidden=\"true\" \
    focusable=\"false\"><circle cx=\"10.5\" cy=\"10.5\" r=\"6.5\" fill=\"none\" \
    stroke=\"currentColor\" stroke-width=\"2\"/><path d=\"m15.5 15.5 5 5\" stroke=\"currentColor\" \
    stroke-width=\"2\" stroke-linecap=\"round\"/></svg>";

/// The page the header's own navigation is showing, for `aria-current`.
///
/// A page matches the node whose target page is this page. The site index has
/// no page, so the home link is current when the request is the site root.
fn nav_item_is_current(item: &NavItem, ctx: &RenderContext<'_>) -> bool {
    match ctx.page {
        Some(page) => item.page == Some(page.id),
        None => item.page.is_none() && item.url == site_url(&ctx.site.slug),
    }
}

/// The frame's primary navigation: a resolved `<ul>` with the current page
/// marked and child pages nested one level, the "second row" the header shows
/// without JavaScript.
fn header_menu(ctx: &RenderContext<'_>) -> String {
    if ctx.nav.is_empty() {
        return String::new();
    }
    let mut out = String::from("<ul class=\"navigation\" id=\"top-nav-contents\">");
    render_nav_items(&ctx.nav, ctx, 0, &mut out);
    out.push_str("</ul>");
    out
}

fn render_nav_items(items: &[NavItem], ctx: &RenderContext<'_>, depth: usize, out: &mut String) {
    for item in items {
        let current = nav_item_is_current(item, ctx);
        let mut classes = vec!["nav-item"];
        if !item.children.is_empty() {
            classes.push("has-children");
        }
        if current {
            classes.push("current");
        }
        out.push_str(&format!("<li class=\"{}\">", classes.join(" ")));
        let mut attributes = Vec::new();
        if item.target_blank {
            attributes.push(Attr::text("target", "_blank"));
        }
        if current {
            attributes.push(Attr::text("aria-current", "page"));
        }
        out.push_str(&hyperlink(&item.label, Some(&item.url), &attributes));
        if !item.children.is_empty() && depth < 1 {
            out.push_str("<ul class=\"navigation\">");
            render_nav_items(&item.children, ctx, depth + 1, out);
            out.push_str("</ul>");
        }
        out.push_str("</li>");
    }
}

/// The logo the header links home, when the site's token names an asset the
/// caller can resolve. A missing or unreadable asset renders no logo rather
/// than a broken image.
fn logo_markup(ctx: &RenderContext<'_>) -> String {
    let Some(url) = ctx
        .theme_settings
        .logo
        .and_then(|id| ctx.resources.read_asset(omeka_core::ids::AssetId::new(id)))
        .and_then(|asset| asset.url)
    else {
        return String::new();
    };
    format!(
        "<a class=\"site-logo\" href=\"{}\"><img src=\"{}\" alt=\"{}\"></a>\n",
        escape_html(&site_url(&ctx.site.slug)),
        escape_html(&url),
        escape_html(&ctx.site.title)
    )
}

/// The site-wide banner: the stored image, the stored text, or both.
fn banner_markup(ctx: &RenderContext<'_>) -> String {
    let image = ctx
        .theme_settings
        .banner
        .and_then(|id| ctx.resources.read_asset(omeka_core::ids::AssetId::new(id)))
        .and_then(|asset| asset.url);
    let text = ctx.theme_settings.banner_text.as_deref();
    if image.is_none() && text.is_none() {
        return String::new();
    }
    let mut out = format!(
        "<div class=\"site-banner\"{}>",
        ctx.region_marker(SiteRegion::Header)
    );
    if let Some(url) = image {
        out.push_str(&format!(
            "<img class=\"site-banner-image\" src=\"{}\" alt=\"\">",
            escape_html(&url)
        ));
    }
    if let Some(text) = text {
        out.push_str(&format!(
            "<p class=\"site-banner-text\">{}</p>",
            escape_html(text)
        ));
    }
    out.push_str("</div>\n");
    out
}

/// `Controller/Site/PageController.php:41-43`: `page {layout_data.class}
/// site-page-{slug}`, with every character outside `[a-zA-Z0-9-]` in the slug
/// replaced by `-`.
fn page_body_class(ctx: &RenderContext<'_>) -> String {
    let mut class = String::from("page");
    if let Some(page) = ctx.page {
        if let Some(extra) = page.layout_data.class.as_deref().filter(|c| !c.is_empty()) {
            class.push(' ');
            class.push_str(&sanitize_class(extra));
        }
        class.push_str(" site-page-");
        class.push_str(&sanitize_class(page.slug.as_str()));
    }
    class
}

/// `preg_replace('([^a-zA-Z0-9\-])', '-', $value)`.
fn sanitize_class(value: &str) -> String {
    value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' {
                ch
            } else {
                '-'
            }
        })
        .collect()
}

/// `Module.php` `view.show.after` / `view.browse.after`: the resource's
/// embedded JSON-LD in a script, unless the site's `disable_jsonld_embed` is
/// set. The `<` is escaped so a title cannot close the script element.
fn jsonld_script(ctx: &RenderContext<'_>, document: Option<&str>) -> String {
    if ctx.settings.disable_jsonld_embed() {
        return String::new();
    }
    match document {
        Some(json) => format!(
            "<script type=\"application/ld+json\">{}</script>\n",
            json.replace('<', "\\u003c")
        ),
        None => String::new(),
    }
}

/// `view/omeka/site/index/index.phtml`.
fn index_view(tr: Translator) -> String {
    format!("<p>{}</p>", escape_html(tr.messages().no_pages))
}

/// `view/error/404.phtml` and `view/error/403.phtml`: the heading `pageTitle`
/// writes, then the message paragraph when the error view has one.
fn error_view(error: &ErrorView) -> String {
    let mut out = format!("<h1>{}</h1>", escape_html(&error.title));
    if let Some(message) = &error.message {
        out.push_str(&format!("<p>{}</p>", escape_html(message)));
    }
    out
}

/// `view/omeka/site/page/show.phtml` and `page/content.phtml`: breadcrumbs, a
/// sub-menu, the page's blocks through `PageLayout::render`
/// ([`blocks::render_page_blocks`]), and the page pagination.
fn page_view(ctx: &RenderContext<'_>) -> String {
    let Some(page) = ctx.page else {
        return String::new();
    };
    let mut out = String::new();
    if let Some(path) = nav_path(&ctx.nav, page.id)
        && path.len() > 1
    {
        out.push_str(&breadcrumbs(&path));
    }
    let active = find_item_by_page(&ctx.nav, page.id);
    if shows_subnav(ctx, active)
        && let Some(active) = active
    {
        out.push_str(&format!(
            "<nav class=\"sub-menu\">{}</nav>",
            render_menu(&active.children, None, "navigation")
        ));
    }
    out.push_str("<div class=\"blocks\">");
    out.push_str(&blocks::render_page_blocks(page, ctx));
    out.push_str("</div>");
    if ctx.settings.show_page_pagination() {
        out.push_str(&page_pagination(page.id, ctx));
    }
    out
}

fn shows_subnav(ctx: &RenderContext<'_>, active: Option<&crate::nav::NavItem>) -> bool {
    match ctx.settings.subnav_display.as_deref() {
        Some("show") => true,
        Some("hide") => false,
        _ => active.is_some_and(|item| !item.children.is_empty()),
    }
}

fn breadcrumbs(path: &[&crate::nav::NavItem]) -> String {
    let mut out = String::from("<nav class=\"breadcrumbs\">");
    for (index, item) in path.iter().enumerate() {
        if index > 0 {
            out.push_str(" / ");
        }
        out.push_str(&hyperlink(&item.label, Some(&item.url), &[]));
    }
    out.push_str("</nav>");
    out
}

/// `common/site-page-pagination.phtml` through
/// `View\Helper\SitePagePagination`: Prev and Next links to the pages linked in
/// the navigation, in navigation order. A page that the navigation does not
/// link has no pagination, and a hidden page is not a neighbour because it is
/// not in `ctx.pages`.
fn page_pagination(page_id: omeka_core::ids::PageId, ctx: &RenderContext<'_>) -> String {
    let linked = linked_pages(&ctx.nav, &ctx.pages);
    let Some(index) = linked.iter().position(|page| page.id == page_id) else {
        return String::new();
    };
    let prev = index.checked_sub(1).and_then(|i| linked.get(i));
    let next = linked.get(index + 1);
    if prev.is_none() && next.is_none() {
        return String::new();
    }
    let messages = ctx.translator().messages();
    let mut out = String::from("<div class=\"site-page-pagination\">");
    if let Some(prev) = prev {
        out.push_str(&hyperlink(
            messages.prev,
            Some(&crate::slug::page_url(&ctx.site.slug, &prev.slug)),
            &[],
        ));
    }
    if let Some(next) = next {
        out.push_str(&hyperlink(
            messages.next,
            Some(&crate::slug::page_url(&ctx.site.slug, &next.slug)),
            &[],
        ));
    }
    out.push_str("</div>");
    out
}

/// The pages the navigation links, in the order `linkedPages()` walks them: a
/// depth-first pass that adds each page link the tree resolves, once. Only
/// pages the caller can see are in `pages`, so a hidden page never enters the
/// sequence.
fn linked_pages<'a>(
    nav: &'a [crate::nav::NavItem],
    pages: &'a [crate::nav::PageRef],
) -> Vec<&'a crate::nav::PageRef> {
    let mut out: Vec<&crate::nav::PageRef> = Vec::new();
    collect_linked(nav, pages, &mut out);
    out
}

fn collect_linked<'a>(
    nav: &'a [crate::nav::NavItem],
    pages: &'a [crate::nav::PageRef],
    out: &mut Vec<&'a crate::nav::PageRef>,
) {
    for item in nav {
        if let Some(id) = item.page
            && let Some(page) = pages.iter().find(|page| page.id == id)
            && !out.iter().any(|linked| linked.id == id)
        {
            out.push(page);
        }
        collect_linked(&item.children, pages, out);
    }
}

/// `view/omeka/site/item/browse.phtml` and its item-set and page siblings.
fn browse_view(browse: &BrowseView, ctx: &RenderContext<'_>, tr: Translator) -> String {
    let mut out = format!("<h2>{}</h2>", escape_html(&browse.title));
    out.push_str("<div class=\"browse-controls\">");
    out.push_str(&pagination(
        &ctx.current_path,
        &ctx.query,
        browse.page,
        browse.per_page,
        browse.total,
        ctx.translator().messages(),
    ));
    let search_url = resource_action_url(&ctx.site.slug, controller_for(browse.kind), "search");
    let search_url = match crate::html::query_string(&ctx.query) {
        Some(query) => format!("{search_url}?{query}"),
        None => search_url,
    };
    out.push_str(&hyperlink(
        tr.messages().advanced_search,
        Some(&search_url),
        &[Attr::text("class", "advanced-search")],
    ));
    out.push_str("</div>");
    out.push_str(&cards::theme_list_open(ctx));
    for resource in &browse.resources {
        out.push_str(&cards::resource_card(
            browse.kind,
            resource,
            ctx,
            cards::CardParts::all(ctx.theme_settings.tokens.structure.cards),
        ));
    }
    out.push_str("</ul>");
    out.push_str(&pagination(
        &ctx.current_path,
        &ctx.query,
        browse.page,
        browse.per_page,
        browse.total,
        ctx.translator().messages(),
    ));
    out
}

/// `view/omeka/site/item/show.phtml`: the heading, the `Item` label and the
/// configured resource page blocks. The heading is the port's resource header
/// ([`cards::resource_header`]): the kind and type over the display name. A
/// breadcrumb leads back to the item browse, and the IIIF links follow the
/// header.
fn item_view(item: &ResourceView, ctx: &RenderContext<'_>) -> String {
    let browse = format!("/s/{}/item", ctx.site.slug);
    format!(
        "<nav class=\"breadcrumbs\">{} / <span>{}</span></nav>{}{}{}",
        hyperlink(ctx.translator().messages().items, Some(&browse), &[]),
        escape_html(&cards::display_name(item, ctx).0),
        cards::resource_header(item, ctx),
        iiif_links(item, ctx),
        render_blocks(item, ctx)
    )
}

/// The IIIF logo, after `iiif.io`'s mark: three bars and a red last stroke.
/// Decorative, because the link text names the manifest.
const IIIF_LOGO: &str = "<svg class=\"iiif-logo\" viewBox=\"0 0 24 21\" width=\"18\" height=\"16\" \
     aria-hidden=\"true\" focusable=\"false\"><path fill=\"#2873AB\" d=\"M0 4h3v17H0zM5 4h3v17H5zM10 4h3v17h-3z\"/>\
     <path fill=\"#ED1D33\" d=\"M15 4h9v3h-6v4h5v3h-5v7h-3z\"/>\
     <path fill=\"#2873AB\" d=\"M0 0h3v3H0zM5 0h3v3H5zM10 0h3v3h-3z\"/></svg>";

/// A Romeka addition to the item page: a link that opens the item in the IIIF
/// viewer, under the site's viewer settings, and a link to its manifest.
fn iiif_links(item: &ResourceView, ctx: &RenderContext<'_>) -> String {
    let Some(manifest) = &item.iiif_manifest else {
        return String::new();
    };
    let messages = ctx.translator().messages();
    let viewer = crate::iiif_viewer::viewer_src(&ctx.iiif_viewer_url, manifest, &ctx.settings);
    format!(
        "<p class=\"iiif-links\"><a href=\"{}\">{}</a> <a class=\"iiif-manifest\" href=\"{}\">{IIIF_LOGO}{}</a></p>",
        escape_html(&viewer),
        escape_html(messages.iiif_view_in_viewer),
        escape_html(manifest),
        escape_html(messages.iiif_manifest)
    )
}

/// `view/omeka/site/item-set/browse.phtml` for a single item set.
fn item_set_view(item_set: &ResourceView, ctx: &RenderContext<'_>) -> String {
    format!(
        "{}{}",
        cards::resource_header(item_set, ctx),
        render_blocks(item_set, ctx)
    )
}

/// `view/omeka/site/media/show.phtml`.
fn media_view(media: &ResourceView, ctx: &RenderContext<'_>) -> String {
    let mut out = cards::resource_header(media, ctx);
    if let Some(parent) = &media.parent_item {
        out.push_str(&format!(
            "<p class=\"media-parent-item\">{}</p>",
            sprintf1(
                &escape_html(ctx.translator().messages().part_of),
                &hyperlink(&parent.title, Some(&parent.url), &[])
            )
        ));
    }
    // The media renderer's markup has no wrapper of its own, so the page wraps
    // the file and its values together and the sheet lays them side by side.
    out.push_str("<div class=\"media-page\">");
    out.push_str(&render_blocks(media, ctx));
    out.push_str("</div>");
    out
}

#[cfg(test)]
#[path = "default_tests.rs"]
mod tests;
