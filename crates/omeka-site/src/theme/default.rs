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
use crate::html::{Attr, escape_html, hyperlink, open_tag, pagination};
use crate::i18n::{Messages, Translator};
use crate::nav::{find_item_by_page, nav_path, render_menu};
use crate::settings::SETTINGS;
use crate::slug::{controller_for, resource_action_url};
use crate::theme::search;
use crate::theme::style::STYLE_CSS;
use crate::theme::{AssetKind, Theme, ThemeAsset, ThemeInfo};
use crate::view::{
    BrowseView, ErrorView, IiifViewerView, RenderContext, ResourceRef, ResourceView, Thumbnail,
    View,
};
use omeka_core::resource::ResourceKind;

/// The bundled theme's manifest: the id `site.theme` stores, the site settings
/// it reads, and the one file it ships.
pub static DEFAULT_INFO: ThemeInfo = ThemeInfo {
    name: "default",
    label: "Default",
    settings: &SETTINGS,
    assets: &[ThemeAsset {
        path: "css/style.css",
        kind: AssetKind::Stylesheet,
        content: STYLE_CSS,
    }],
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
                "page",
                &page_view(ctx),
            ),
            View::Browse(browse) => layout(
                info,
                ctx,
                Some(&browse.title),
                &format!("{} resource browse", controller_for(browse.kind)),
                &browse_view(browse, ctx, tr),
            ),
            View::Item(item) => layout(
                info,
                ctx,
                Some(&item.title),
                "item resource show",
                &item_view(item, tr),
            ),
            View::ItemSet(item_set) => layout(
                info,
                ctx,
                Some(&item_set.title),
                "item-set resource browse",
                &item_set_view(item_set, tr),
            ),
            View::Media(media) => layout(
                info,
                ctx,
                Some(&media.title),
                "media resource show",
                &media_view(media, tr),
            ),
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
    // The site title is the page's level-one heading. Upstream's bare layout
    // prints no heading, so a view like the item browse or the empty index had
    // none; the theme's own frame adds one, and every view's title sits below it.
    let site_title = format!(
        "<h1 class=\"site-title\">{}</h1>",
        escape_html(&ctx.site.title)
    );
    let mut theme_assets = String::new();
    for asset in info.assets {
        theme_assets.push_str(
            &asset
                .kind
                .element(&format!("{assets}/{}", escape_html(asset.path))),
        );
        theme_assets.push('\n');
    }
    format!(
        "<!DOCTYPE html>\n<html lang=\"{lang}\">\n<head>\n\
         <meta charset=\"utf-8\">\n\
         <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n\
         <title>{title}</title>\n\
         <link rel=\"stylesheet\" href=\"{assets}/css/fonts.css\">\n\
         <link rel=\"stylesheet\" href=\"{assets}/css/iconfonts.css\">\n\
         {theme_assets}\
         <script src=\"{assets}/vendor/jquery/jquery.min.js\"></script>\n\
         <script src=\"{assets}/js/global.js\"></script>\n\
         {head_extra}\
         </head>\n\
         <body class=\"minimal {body}\">\n\
         <div class=\"site-search\">{search_form}</div>\n\
         <div id=\"content\" role=\"main\">\n\
         {site_title}\n{content}\n</div>\n\
         <footer>\n<span class=\"powered-by\">{powered_by}</span>\n</footer>\n\
         </body>\n</html>",
        lang = tr.locale().tag(),
        body = escape_html(body_class),
        site_title = site_title,
        search_form = search::search_form(ctx, tr.search_labels()),
        powered_by = escape_html(tr.messages().powered_by),
    )
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
    out.push_str(&blocks::render_page_blocks(&page.blocks, ctx));
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

/// `common/site-page-pagination.phtml`: Prev and Next links to the neighbouring
/// pages of this site, in the order the caller supplied them.
fn page_pagination(page_id: omeka_core::ids::PageId, ctx: &RenderContext<'_>) -> String {
    let Some(index) = ctx.pages.iter().position(|page| page.id == page_id) else {
        return String::new();
    };
    let prev = index.checked_sub(1).and_then(|i| ctx.pages.get(i));
    let next = ctx.pages.get(index + 1);
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
        out.push_str(&render_resource(browse.kind, resource));
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
fn render_resource(kind: ResourceKind, resource: &ResourceView) -> String {
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
        .unwrap_or_default();
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

/// `view/omeka/site/item/show.phtml`, without the resource page blocks.
fn item_view(item: &ResourceView, tr: Translator) -> String {
    format!(
        "<h2>{}</h2><h3>{}</h3>{}",
        escape_html(&item.title),
        escape_html(tr.messages().item),
        values_list(item)
    )
}

/// `view/omeka/site/item-set/browse.phtml` for a single item set, without the
/// item list the item-set search would supply.
fn item_set_view(item_set: &ResourceView, tr: Translator) -> String {
    let mut out = format!(
        "<h2>{}</h2><h3>{}</h3>{}",
        escape_html(&item_set.title),
        escape_html(tr.messages().item_set),
        values_list(item_set)
    );
    out.push_str(&resource_lists(item_set, tr.messages()));
    out
}

/// `view/omeka/site/media/show.phtml`.
fn media_view(media: &ResourceView, tr: Translator) -> String {
    let mut out = format!(
        "<h2>{}</h2><h3>{}</h3>",
        escape_html(&media.title),
        escape_html(tr.messages().media)
    );
    if let Some(parent) = &media.parent_item {
        out.push_str(&format!(
            "<p class=\"media-parent-item\">{} {}</p>",
            escape_html(tr.messages().part_of),
            hyperlink(&parent.title, Some(&parent.url), &[])
        ));
    }
    out.push_str(&values_list(media));
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

fn values_list(resource: &ResourceView) -> String {
    if resource.values.is_empty() {
        return String::new();
    }
    let mut out = String::from("<dl class=\"values\">");
    for value in &resource.values {
        out.push_str(&format!(
            "<div class=\"property\"><dt>{}</dt><dd class=\"value\">{}</dd></div>",
            escape_html(&value.property),
            escape_html(&value.value)
        ));
    }
    out.push_str("</dl>");
    out
}

fn resource_lists(resource: &ResourceView, messages: &Messages) -> String {
    let mut out = String::new();
    out.push_str(&named_list(
        "item-sets",
        messages.item_sets,
        &resource.item_sets,
    ));
    out.push_str(&named_list("media-list", messages.media, &resource.media));
    out
}

fn named_list(class: &str, label: &str, entries: &[ResourceRef]) -> String {
    if entries.is_empty() {
        return String::new();
    }
    let mut out = format!(
        "{}<div class=\"property\"><dt>{}</dt>",
        open_tag("dl", &[Attr::text("class", class)]),
        escape_html(label)
    );
    for entry in entries {
        out.push_str(&format!(
            "<dd class=\"value\">{}</dd>",
            hyperlink(&entry.title, Some(&entry.url), &[])
        ));
    }
    out.push_str("</div></dl>");
    out
}

#[cfg(test)]
#[path = "default_tests.rs"]
mod tests;
