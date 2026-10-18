# omeka-site

Romeka S is a fork of Omeka S. This crate is the public site layer: it turns stored site, page, block and resource
values into one HTML document. It holds no database, no HTTP and no async code: every stored value arrives in a
`RenderContext`, so a page render is a plain function a test can call with literal values.

The bundled theme ports upstream's layout and site views. Upstream reference is `source/omeka-s/application` at the
frozen SHA, read only, and each module cites the file it ports.

## The render contract

A caller does three things.

1. Parse the request path with `slug::parse_public_path`. It returns a `PublicRoute` or a
   `RouteError`. This is the only place the site URL shape is decided.
2. Build a `view::RenderContext` from values it loaded: the `Site`, the optional `SitePage`,
   the `SiteSettings`, the resolved navigation (`nav::resolve` over `nav::parse_navigation`), the
   site's `PageRef` list, the request's query pairs, and a `view::ResourceProvider`.
3. Call `render::render_page`, `render::render_index`, or `theme::Theme::render` with a
   `view::View`. The answer is a complete HTML document, starting at `<!DOCTYPE html>`.

`theme::DefaultTheme` renders every view. `theme::theme_for(site.theme)` picks a theme by name; the
port bundles one, `default`, listed in `theme::THEMES`.

A theme is a `Theme` implementation plus a `ThemeInfo` manifest. The manifest carries the name a site
stores, the theme's own settings (its `theme.ini` `[config]` spec upstream; the bundled theme declares
none), and the files the theme ships with their bytes. The site settings every core site view reads
are separate, in `settings::SITE_SETTINGS`, stored as individual `site_setting` rows. A second theme
implements `Theme`, adds itself to `THEMES`, and changes nothing else in the crate.

### The bundled theme's stylesheet

`theme::style::STYLE_CSS` is the bundled theme's own CSS, held as data (`theme/style.css`). The
theme's manifest lists it at `css/style.css`, and the document frame emits a `<link>` for every
manifest asset under `RenderContext::asset_base`, between the core `fonts.css`/`iconfonts.css` links
and the core scripts. The head carries no other stylesheet, so the caller serves
`{asset_base}/css/style.css` from `theme::theme_asset("default", "css/style.css")` and the page is
styled. The sheet's block rules are the subset of upstream `application/asset/css/page-blocks.css`
that applies to the markup this port emits, its page-grid rules are
`application/asset/css/page-grid.css`, and it adds a rule for every block wrapper class no upstream
rule names. A stored block `class` is arbitrary, so only a registered layout's wrapper has a rule.

### Page blocks

`blocks::render_page_blocks` reproduces `View/Helper/PageLayout::render`: the page's blocks in
`position` order inside the `blocks-inner` element, with the page's `layout` and `layout_data` on
that element (`page-layout-normal`, or `page-layout-grid grid-template-columns-{n}` with the pixel
gaps). Each block is wrapped as `BlockLayout::render` wraps it, in `block block-{layout}` with the
classes and inline styles `AbstractLayout` derives from the block's `layout_data` (the grid
placement, a stored class, alignment, background, padding and the CSS lengths). A `blockGroup` opens
a `block block-blockGroup` wrapper around the `span` blocks that follow it, and on a grid page that
wrapper carries `block-group-grid grid-position-1 grid-span-{n}`.

`layout_data.template_name` (the theme block-template key) is carried and not rendered: the port
bundles no block templates. `background_image_asset` needs the asset's URL, which the
`ResourceProvider::read_asset` lookup supplies; the bundled theme's provider resolves the id through
the `asset` repository and fills `background-image: url(...)`, so the class is emitted and the style
appears whenever the asset exists.

### The block field registry

Each layout declares the `data` fields its upstream form edits as a `blocks::schema::LayoutSchema`,
beside its renderer, and returns it from `BlockLayout::schema`. `blocks::schema::registry` collects
them, in the registry's order, with the schemas of a block's and a page's `layout_data`, and
serialises them to `web/admin/src/generated/blocks.json`. The admin reads that file instead of
mirroring the registry, so adding a layout is one module (plus its catalogue keys in
`web/admin/src/i18n`).

A test writes the file and fails when it differs from the registry, so the boundary is enforced
rather than conventional. Regenerate with `UPDATE_GENERATED=1 cargo test -p omeka-site`. A second
test fails when a schema names a `data` key its layout's parse does not read.

The kinds are upstream's form elements: `Text`, `Html`, `Integer { min, max }`, `Boolean`,
`Choice { options, multiple }` with `ChoiceOption` values, `PageRef`, `AssetRef`, `ItemQuery`,
`Attachments { media }`, `Repeat { fields }` and `PageList` for the `listOfPages` tree. An
attachments list is stored as rows beside `data`, so its key is not a `data` member. The hidden
members a form does not present (`oembed.url_old`, `oembed.oembed`) are not fields.

### Romeka blocks

A Romeka block is an upstream block with a plain rendering upstream can show, plus the block's own
settings under `o:layout_data.romeka`. `blocks::romeka` owns the rule. `RomkaBlock` carries a name, a
catalogue label, a `LayoutSchema` in the same format as an upstream layout, a rich renderer and a
plain renderer. `plain` has no default, so a block without one does not compile. `ROMKA_BLOCKS` holds
the registry; `hero` is its first entry.

`romeka::plain_hash` hashes the block's canonical content form with FNV-1a 64. The form keeps only
what a reader sees, in order: text runs with whitespace collapsed, each image's `src` and `alt`, and
each link's `href` with the text it wraps. An upstream admin save rewrites the html through
HTMLPurifier (pretty-printing, self-closed voids, reordered attributes, dropped `rel`), so a hash of
the raw bytes would change on every save; the canonical form does not. `Html::render` calls
`render_carrier`: a carrier whose hash vouches for the html renders the rich form, and one whose hash
does not, or is absent, renders the stored html, purified. A carrier version 1 hash was the raw-byte
hash, so `stored_matches` still accepts it and the next write migrates the carrier to version 2 with
the canonical hash. The admin repeats the hash (`web/admin/src/pages/sites/editor/romeka.ts`, through
`DOMParser`), and `web/admin/src/generated/blocks.json` carries a worked sample of both algorithms
that both sides test against.

`blocks::schema::registry` lists the Romeka registry beside the upstream layouts, each entry with its
carrier, so the admin picker offers them and its form uses the block's own fields. The site write
(`omeka-api/src/write/site_pages.rs::carry_block`) regenerates the plain rendering from `plain` and
records the hash. `docs/compat.md` states the one-way rule this implements.

### Views

| `view::View` | upstream view |
| --- | --- |
| `Index` | `view/omeka/site/index/index.phtml` |
| `Page` | `view/omeka/site/page/show.phtml` + `page/content.phtml` |
| `Browse` | `view/omeka/site/item/browse.phtml`, `item-set/browse.phtml` |
| `Item` | `view/omeka/site/item/show.phtml` |
| `ItemSet` | `view/omeka/site/item-set/browse.phtml` for one item set |
| `Media` | `view/omeka/site/media/show.phtml` |
| `Search` | `view/omeka/site/index/search.phtml` |
| `CrossSiteSearch` | `view/omeka/site/cross-site-search/*.phtml` + `view/common/cross-site-search/*.phtml` |
| `SearchList` | `view/omeka/site/page/browse.phtml` |
| `AdvancedSearch` | `view/omeka/site/item/search.phtml`, `item-set/search.phtml`, `view/common/advanced-search.phtml` |
| `IiifViewer` | `view/omeka/iiif-viewer/index.phtml` |
| `Error` | `view/error/404.phtml`, `view/error/403.phtml` |

The port's frame prints the site title as the document's `<h1>`. Upstream's bare
`view/layout/layout.phtml` prints no heading, so a view like the item browse or
the empty index had none; the frame owns one so every view has a level-one
heading. The views keep their upstream levels below it, with one exception: the
page browse prints each page title in an `h3` where upstream `page/browse.phtml`
uses an `h4`, which would skip a level under the `Pages` `h2`.

### Search

`search` owns the three resource names a search covers and the values the search
views render. `SearchResource` is one table: the wire name (`site_pages`,
`items`, `item_sets`), the site controller segment, the cross-site action and the
body class. `search_resource_names` maps through `SearchResource::from_name`, and
a `/:controller/search` route maps back through `SearchResource::from_controller`.

| value | what it carries |
| --- | --- |
| `SearchResults` | a needle and one `SearchSection` per resource name; `is_empty` is upstream's `empty($results)` |
| `SearchHit` | a title, a URL, an optional thumbnail and the site a cross-site hit belongs to |
| `SearchList` | one paginated resource list |
| `CrossSitePage` with `CrossSiteBody` | one cross-site action: the form, the sections or one list |
| `AdvancedSearchForm` | the query being edited, where the form posts, and the select options |

`SearchType` reads `search_type` and names the two targets of
`common/search-form.phtml`. The labels the search prints are
`i18n::SearchLabels`, one catalogue per locale beside `Messages`.

The frame renders `common/search-form.phtml` as `<div class="site-search">`.
Upstream's core layout has no search form; a theme adds it. The port ships one
theme, so its frame carries the form.

### What escapes

Everything the views print is escaped except the fragments upstream trusts, which are purified
instead. A stored `html` block's body and a media block attachment's `caption` both pass through
`omeka_core::purify` (the port of `Stdlib\HtmlPurifier`) before they are emitted raw, and a
provider's oEmbed `html` is emitted as received. `html::escape_html` mirrors
`Laminas\View\Helper\EscapeHtml` (`ENT_QUOTES`, no `ENT_HTML5`, so a single quote is `&#039;`).

## Routes

`slug::parse_public_path` reads the site route table (`config/routes.config.php:16-116`). All
routes sit under `/s/:site-slug`.

| route name | path | parsed as |
| --- | --- | --- |
| `site` | `/s/:site-slug` | `Home` |
| `site/page-browse` | `/s/:site-slug/page` | `PageBrowse` |
| `site/page` | `/s/:site-slug/page/:page-slug` | `Page` |
| `site/cross-site-search` | `/s/:site-slug/cross-site-search[/:action]` | `CrossSiteSearch` |
| `site/resource` | `/s/:site-slug/:controller[/:action]` | `ResourceBrowse` or `ResourceAction` |
| `site/resource-id` | `/s/:site-slug/:controller/:id[/:action]` | `ResourceShow` or `ResourceShowAction` |
| `site/item-set` | `/s/:site-slug/item-set/:item-set-id` | `ResourceShow` with controller `item-set` |

`slug::controller_for` maps a `ResourceKind` to the controller segment (`item`, `item-set`,
`media`); `slug::kind_for_controller` maps back, and answers `None` for a segment that is not a
resource controller.

The companion URL builders (`site_url`, `page_url`, `page_browse_url`, `resource_browse_url`,
`resource_action_url`, `resource_show_url`, `item_set_url`) write the same table. A generated URL
omits a segment equal to the route default, so item browse is `/s/main/item`, not
`/s/main/item/browse`.

`/page` and `cross-site-search` are literal routes beside the generic `:controller` route. Where
both match, the parser picks the literal one, matching the Laminas router's tie. The rule is
inferred; `slug.rs` says why.

## The resource provider

Block layouts and browse views ask a `ResourceProvider` for stored resources. The trait is
synchronous by design: the site layer never opens a database, so the caller loads what a render
needs and answers from that snapshot. `view::NoResources` answers empty, which is what a test or a
caller with no storage uses.

Six lookups: `search` (a browse), `read` (one resource), `read_media`, `read_asset`, `attachments`
(a block's `site_block_attachment` rows), and `search_sites`. Only two layouts call the provider,
`browsePreview` and `listOfSites`; `itemWithMetadata`, `media` and `asset` read their content from
`attachments`, which only a caller with a database can answer.

## Not ported

- `resourcePageBlocks` (the `Site/ResourcePageBlockLayout/*` registry). The item, item-set and
  media views print the values the provider supplies instead.
- `searchFilters` and `browse()->renderSortSelector`. The browse views print the pagination and the
  advanced-search link without the filter UI.
- Module view listeners (`trigger('view.show.before')` and friends). There are no modules.
- `$this->lang()` as a view helper. The port carries the same information on
  `i18n::Locale` and `i18n::Translator`; full translation catalogs are excluded
  by DESIGN.md, so two locales ship (see below).
- The exact Laminas menu and pagination markup (per-item classes, `aria` attributes).
- From the advanced form: the `dataType()->getSelect` input (the site layer has no data-type
  registry), the `resource-template-restrict` hidden inputs (`search_restrict_templates` needs a
  template list) and the add/remove value buttons (`advanced-search.js` is not ported). The
  `property` and `resource_class_id[]` selects list what the caller supplies; `vocabulary_scope`
  narrowing is not applied.
- The `sort_by` select offers `title`, `created` and `id`. Upstream's public items list also offers
  `resource_class_label`, which the port's sort maps to `id`, so offering it would mislead.
- A cross-site item or item set prints its title once, linked to the resource on the site being
  served. Upstream prints one link per site the resource belongs to (`$item->sites()`); no reader
  maps a resource to its sites.

## Locales

`i18n` holds the site's rendering locale. The key is upstream's `site_setting.locale`
(`Mvc\MvcListeners::bootstrapLocale`); `SiteSettings::locale()` parses the stored tag at the boundary
and `RenderContext::translator()` exposes the result. The theme and the blocks render their fixed copy
through `Translator::messages()` and dates through `Translator::format_datetime`. The search views
read `Translator::search_labels()` (`i18n::SearchLabels`), the same shape for the second catalogue.

Two locales ship: English (the source language) and Spanish, whose terms follow upstream's
`application/language/es.po`. A region tag (`es_ES`, `en_US`) narrows to its language; an unknown tag
falls back to English. The running server keeps `SiteSettings::default()` until a `site_setting`
repository exists, so a live render is English today; the render layer honours whatever a caller sets.
