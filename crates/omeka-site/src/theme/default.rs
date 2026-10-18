//! The bundled theme's views.
//!
//! Ported from `view/layout/layout.phtml`, `view/omeka/site/index/index.phtml`,
//! `view/omeka/site/page/show.phtml`, `view/omeka/site/page/content.phtml`,
//! `view/omeka/site/item/browse.phtml`, `view/omeka/site/item/show.phtml`,
//! `view/omeka/site/item-set/browse.phtml`,
//! `view/omeka/site/media/show.phtml`, `common/site-page-pagination.phtml`,
//! `view/omeka/iiif-viewer/index.phtml`, `view/error/403.phtml` and
//! `view/error/404.phtml`.
//!
//! The IIIF viewer page (`view/omeka/iiif-viewer/index.phtml`) is where the
//! port departs from the upstream view. Upstream renders Mirador over a `url`
//! query parameter; the port renders the OpenSeadragon container the media
//! renderer emits ([`crate::view::IiifViewerView`]), because the port replaced
//! Mirador with OpenSeadragon throughout (`omeka-media/src/render/iiif.rs`).
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
use crate::i18n::Translator;
use crate::nav::{find_item_by_page, nav_path, render_menu};
use crate::resource_page::render_blocks;
use crate::slug::{controller_for, resource_action_url, site_url};
use crate::theme::search;
use crate::theme::style::{PLACEHOLDER_SVG, STYLE_CSS};
use crate::theme::{AssetKind, Theme, ThemeAsset, ThemeInfo};
use crate::view::{
    BrowseView, ErrorView, IiifViewerView, RenderContext, ResourceView, Thumbnail, View,
};
use omeka_core::resource::ResourceKind;

/// The bundled theme's manifest: the id `site.theme` stores, the design tokens
/// it reads, and the files it ships.
pub static DEFAULT_INFO: ThemeInfo = ThemeInfo {
    name: "default",
    label: "Default",
    settings: crate::theme::tokens::SETTINGS,
    assets: &[
        ThemeAsset {
            path: "css/style.css",
            kind: AssetKind::Stylesheet,
            content: STYLE_CSS,
        },
        ThemeAsset {
            path: "img/placeholder.svg",
            kind: AssetKind::Image,
            content: PLACEHOLDER_SVG,
        },
    ],
};

/// The bundled theme.
#[derive(Debug, Clone, Copy, Default)]
pub struct DefaultTheme;

impl Theme for DefaultTheme {
    fn info(&self) -> &'static ThemeInfo {
        &DEFAULT_INFO
    }

    fn render(&self, view: &View<'_>, ctx: &RenderContext<'_>) -> String {
        let info = self.info();
        let tr = ctx.translator();
        match view {
            View::Index => layout(info, ctx, None, "home", &index_view(tr)),
            View::Page => layout(
                info,
                ctx,
                ctx.page.map(|page| page.title.as_str()),
                &page_body_class(ctx),
                &page_view(ctx),
            ),
            View::Browse(browse) => {
                let mut content = browse_view(browse, ctx, tr);
                for resource in &browse.resources {
                    content.push_str(&jsonld_script(ctx, resource.jsonld.as_deref()));
                }
                layout(
                    info,
                    ctx,
                    Some(&browse.title),
                    &format!("{} resource browse", controller_for(browse.kind)),
                    &content,
                )
            }
            View::Item(item) => {
                let mut content = item_view(item, ctx);
                content.push_str(&jsonld_script(ctx, item.jsonld.as_deref()));
                layout(info, ctx, Some(&item.title), "item resource show", &content)
            }
            View::ItemSet(item_set) => {
                let mut content = item_set_view(item_set, ctx);
                content.push_str(&jsonld_script(ctx, item_set.jsonld.as_deref()));
                layout(
                    info,
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
                    info,
                    ctx,
                    Some(&media.title),
                    "media resource show",
                    &content,
                )
            }
            View::Search(results) => layout(
                info,
                ctx,
                None,
                "index search",
                &search::search_view(results, ctx),
            ),
            View::CrossSiteSearch(page) => {
                layout(info, ctx, None, "", &search::cross_site_view(page, ctx))
            }
            View::SearchList(list) => layout(
                info,
                ctx,
                None,
                "page browse",
                &search::search_list_view(list, ctx),
            ),
            View::AdvancedSearch(form) => layout(
                info,
                ctx,
                None,
                &format!("{} search", form.resource.body_class()),
                &search::advanced_search_view(form, ctx),
            ),
            View::IiifViewer(iiif) => layout_with_head(
                info,
                ctx,
                Some(&iiif.title),
                "iiif-viewer",
                &openseadragon_head(ctx),
                &iiif_viewer_view(iiif, ctx),
            ),
            View::Error(error) => {
                layout(info, ctx, Some(&error.title), "error", &error_view(error))
            }
        }
    }
}

/// `view/layout/layout.phtml`: the document frame.
///
/// The head carries the core assets first, as upstream links them with
/// `assetUrl(..., 'Omeka')`: `fonts.css` and `iconfonts.css`, then the jQuery
/// and `global.js` scripts. The theme's own assets sit between the two, where
/// upstream appends `style.css`. The `lang` attribute is the site's locale. A view that appends to
/// `headScript` upstream passes its own element as `head_extra` ([`layout_with_head`]).
fn layout(
    info: &ThemeInfo,
    ctx: &RenderContext<'_>,
    view_title: Option<&str>,
    body_class: &str,
    content: &str,
) -> String {
    layout_with_head(info, ctx, view_title, body_class, "", content)
}

/// The layout with one extra head element, placed last where upstream's
/// `headScript()->appendFile` puts a view's own script.
fn layout_with_head(
    info: &ThemeInfo,
    ctx: &RenderContext<'_>,
    view_title: Option<&str>,
    body_class: &str,
    head_extra: &str,
    content: &str,
) -> String {
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
    // never reaches the menu.
    let nav_menu =
        crate::nav::render_menu_with_id(&ctx.nav, None, "navigation", Some("top-nav-contents"));
    let mut theme_assets = String::new();
    for asset in info.assets.iter().filter(|asset| asset.kind.is_head()) {
        theme_assets.push_str(
            &asset
                .kind
                .element(&format!("{assets}/{}", escape_html(asset.path))),
        );
        theme_assets.push('\n');
    }
    // The design tokens as custom properties, one `<style>` in the head. Every
    // value is validated at the storage boundary, so nothing a curator stored
    // reaches the sheet as CSS.
    let tokens = ctx.theme_settings.css();
    let header = ctx.theme_settings.header_layout.class();
    let logo = logo_markup(ctx);
    let banner = banner_markup(ctx);
    let footer_text = footer_text(ctx);
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
    // `UserBar.php:35-43`: `-1` never, `1` always, otherwise only when signed in.
    let user_bar = if ctx.settings.shows_user_bar(ctx.signed_in) {
        user_bar(ctx, tr)
    } else {
        String::new()
    };
    format!(
        "<!DOCTYPE html>\n<html lang=\"{lang}\">\n<head>\n\
         <meta charset=\"utf-8\">\n\
         <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n\
         <title>{title}</title>\n\
         {robots}\
         {favicon}\
         <style>\n{tokens}</style>\n\
         {theme_assets}\
         {head_extra}\
         </head>\n\
         <body class=\"minimal {body}\">\n\
         <a id=\"skipnav\" href=\"#content\">{skip}</a>\n\
         {user_bar}\
         {banner}\
         <header class=\"{header}\">\n\
         {logo}{site_title}\n\
         <nav id=\"top-nav\">\n{nav_menu}\n</nav>\n\
         <div id=\"search\">{search_form}</div>\n\
         </header>\n\
         <div id=\"content\" role=\"main\">\n\
         {content}\n</div>\n\
         <footer>\n{footer_text}<span class=\"powered-by\">{powered_by}</span>\n</footer>\n\
         </body>\n</html>",
        lang = tr.locale().tag(),
        body = escape_html(body_class),
        header = header,
        logo = logo,
        banner = banner,
        footer_text = footer_text,
        tokens = tokens,
        site_title = site_title,
        nav_menu = nav_menu,
        skip = escape_html(tr.messages().skip_to_main),
        user_bar = user_bar,
        robots = robots,
        favicon = favicon,
        search_form = search::search_form(ctx, tr.search_labels()),
        powered_by = escape_html(tr.messages().powered_by),
    )
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
    let mut out = String::from("<div class=\"site-banner\">");
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

/// The footer's HTML, purified at render the way an `html` block is, so a row
/// stored before the write path purified it still cannot execute.
fn footer_text(ctx: &RenderContext<'_>) -> String {
    match ctx.theme_settings.footer_text.as_deref() {
        Some(html) => format!(
            "<div class=\"footer-text\">{}</div>\n",
            omeka_core::purify::purify(html)
        ),
        None => String::new(),
    }
}

/// `common/user-bar.phtml`: the admin bar a signed-in caller sees, or a login
/// link when `show_user_bar` is `1` and nobody is signed in.
fn user_bar(ctx: &RenderContext<'_>, tr: Translator) -> String {
    let messages = tr.messages();
    if ctx.signed_in {
        let name = ctx.user_name.as_deref().unwrap_or("");
        format!(
            "<nav id=\"user-bar\" aria-label=\"{label}\">\
             <div class=\"user-links\">\
             <span class=\"user-id\">{signed_in} {name}</span> \
             <a href=\"/logout\" class=\"logout\">{logout}</a>\
             </div></nav>\n",
            label = escape_html(messages.user_bar_label),
            signed_in = escape_html(messages.signed_in_as),
            name = escape_html(name),
            logout = escape_html(messages.log_out),
        )
    } else {
        format!(
            "<nav id=\"user-bar\" aria-label=\"{label}\">\
             <a href=\"/login\" class=\"login\">{login}</a></nav>\n",
            label = escape_html(messages.user_bar_label),
            login = escape_html(messages.log_in),
        )
    }
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
    out.push_str("<ul class=\"resource-list\">");
    for resource in &browse.resources {
        out.push_str(&render_resource(browse.kind, resource, ctx));
    }
    out.push_str("</ul>");
    out.push_str(&pagination(
        &ctx.current_path,
        &ctx.query,
        browse.page,
        browse.per_page,
        browse.total,
    ));
    out
}

/// One `li` of a browse list (`view/omeka/site/item/browse.phtml`).
///
/// A resource without a thumbnail shows the theme's placeholder, the way
/// upstream's `thumbnail` helper falls back to `default.png`.
fn render_resource(kind: ResourceKind, resource: &ResourceView, ctx: &RenderContext<'_>) -> String {
    let thumbnail = resource
        .thumbnail
        .clone()
        .map(|Thumbnail { url, alt }| {
            format!(
                "<img src=\"{}\" alt=\"{}\">",
                escape_html(&url),
                escape_html(&alt)
            )
        })
        .unwrap_or_else(|| {
            format!(
                "<img src=\"{}/img/placeholder.svg\" alt=\"\" class=\"resource-placeholder\">",
                ctx.asset_base.trim_end_matches('/')
            )
        });
    let mut out = format!(
        "<li class=\"{} resource\">",
        escape_html(controller_for(kind))
    );
    out.push_str(&format!(
        "<a href=\"{}\" class=\"resource-link\">{}<span class=\"resource-name\">{}</span></a>",
        escape_html(&resource.url),
        thumbnail,
        escape_html(&resource.title)
    ));
    if let Some(body) = resource
        .description
        .as_deref()
        .filter(|body| !body.is_empty())
    {
        out.push_str(&format!(
            "<div class=\"description\">{}</div>",
            escape_html(body)
        ));
    }
    out.push_str("</li>");
    out
}

/// `view/omeka/site/item/show.phtml`: the heading, the `Item` label and the
/// configured resource page blocks.
fn item_view(item: &ResourceView, ctx: &RenderContext<'_>) -> String {
    format!(
        "<h2>{}</h2><h3>{}</h3>{}",
        escape_html(&item.title),
        escape_html(ctx.translator().messages().item),
        render_blocks(item, ctx)
    )
}

/// `view/omeka/site/item-set/browse.phtml` for a single item set.
fn item_set_view(item_set: &ResourceView, ctx: &RenderContext<'_>) -> String {
    format!(
        "<h2>{}</h2><h3>{}</h3>{}",
        escape_html(&item_set.title),
        escape_html(ctx.translator().messages().item_set),
        render_blocks(item_set, ctx)
    )
}

/// `view/omeka/site/media/show.phtml`.
fn media_view(media: &ResourceView, ctx: &RenderContext<'_>) -> String {
    let mut out = format!(
        "<h2>{}</h2><h3>{}</h3>",
        escape_html(&media.title),
        escape_html(ctx.translator().messages().media)
    );
    if let Some(parent) = &media.parent_item {
        out.push_str(&format!(
            "<p class=\"media-parent-item\">{} {}</p>",
            escape_html(ctx.translator().messages().part_of),
            hyperlink(&parent.title, Some(&parent.url), &[])
        ));
    }
    out.push_str(&render_blocks(media, ctx));
    out
}

/// The IIIF viewer page: the OpenSeadragon container for one stored media, in
/// the media renderer's markup shape.
///
/// The markup is copied from `omeka-media/src/render/iiif.rs` (itself
/// `Renderer/IIIF.php:12-22`) so the viewer page and the media renderer emit
/// the same shape: `{id}` names the container and the viewer, the prefix is the
/// OpenSeadragon images directory, and the tile source is the escaped IIIF
/// document.
fn iiif_viewer_view(view: &IiifViewerView, ctx: &RenderContext<'_>) -> String {
    let id = escape_html(&format!("iiif-{}", view.media.get()));
    let prefix_url = escape_html(&format!(
        "{}/vendor/openseadragon/images/",
        ctx.asset_base.trim_end_matches('/')
    ));
    format!(
        r#"        <div class="openseadragon" id="{id}" data-prefix-url="{prefix_url}" data-tile-sources="{tile_sources}" style="height: {height};"></div>
        <script>
        {{
            const openSeadragonDiv = document.getElementById('{id}');
            const viewer = OpenSeadragon({{
                id: '{id}',
                prefixUrl: openSeadragonDiv.dataset.prefixUrl,
                tileSources: [JSON.parse(openSeadragonDiv.dataset.tileSources)]
            }});
        }}
        </script>"#,
        id = id,
        prefix_url = prefix_url,
        tile_sources = escape_html(&view.tile_source),
        height = escape_html(&view.height),
    )
}

/// The OpenSeadragon library the viewer page loads, under the asset base.
///
/// Upstream appends the file to `headScript` from the block or the renderer
/// (`block-layout/iiif-image.phtml`, `Renderer/IIIF.php:14`); the theme owns the
/// document head, so the viewer page adds it here.
fn openseadragon_head(ctx: &RenderContext<'_>) -> String {
    format!(
        "<script src=\"{}/vendor/openseadragon/openseadragon.min.js\"></script>\n",
        ctx.asset_base.trim_end_matches('/')
    )
}

#[cfg(test)]
#[path = "default_tests.rs"]
mod tests;
