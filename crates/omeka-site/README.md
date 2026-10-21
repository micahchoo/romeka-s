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
stores, the theme's own settings (its `theme.ini` `[config]` spec upstream), and the files the theme
ships with their bytes. The bundled theme declares its design tokens there:
`theme::tokens::SETTINGS` is the schema, and a site's stored map
(`theme_settings_<theme>`, served as `o:theme_settings`) is read through
`theme::ThemeSettings::from_value`, which validates each token at the boundary and falls
back to the token's default. The site settings every core site view reads are separate, in
`settings::SITE_SETTINGS`, stored as individual `site_setting` rows. A second theme
implements `Theme`, adds itself to `THEMES`, and changes nothing else in the crate.

### Design tokens

The bundled theme is edited by tokens, not code. A site stores its choices in the theme's settings map
(`theme_settings_<theme>`, served as `o:theme_settings`):

| Key | What it holds |
| --- | --- |
| `preset` | one of `theme::presets::PRESETS`: `exhibit`, `catalogue`, `gallery`, `journal`, `community`; the ids the first presets stored (`plain`, `archive`, `editorial`, `civic`) read as their replacements (`presets::LEGACY_IDS`) |
| `derive_palette` | whether the palette is derived from the brand colour (default on) |
| `brand_color` and the six other `*_color` keys | colours set by hand; under the `auto` scheme they are the light palette's |
| `accent_color_dark` and the five other `*_color_dark` keys | the dark palette's colours under the `auto` scheme (`tokens::DARK_EDITABLE`); ignored under `light` and `dark` |
| `color_scheme`, the fonts, sizes, `density`, `corner_radius`, `content_width`, `header_layout`, `header_style`, `button_style` | tokens set by hand |
| `hero_style`, `card_style`, `search_display`, `item_layout`, `footer_layout`, `footer_tone` | structural tokens set by hand |
| `logo`, `banner`, `banner_text` | header content |
| `footer_columns`, `footer_links`, `footer_contact`, `footer_licence`, `show_powered_by` | the footer (`theme::footer`); an old `footer_text` reads as the first column |
| `custom_css` | the site's own CSS |

`theme::tokens::Choices::from_value` is the boundary: it keeps the values the theme accepts and drops
the rest. `theme::tokens::resolve` is the one function that turns choices into effective tokens: the
preset's values, then a palette derived from the brand colour when derivation is on, then the values
set by hand. A map with no `preset` was stored before presets and reads as `catalogue` (the preset that
replaced `plain`) with derivation off; a site created through the API starts from
`presets::NEW_SITE_PRESET`, `exhibit`, with its own palette (`tokens::new_site_settings`, written by
`omeka-api`'s site create).

The presets differ in structure as well as colour. Each structural token is a class on the root
element (`hero-cover`, `cards-row`, `search-icon`, `footer-columns`, `item-side` and their siblings)
that `style.css` keys the layout on; the footer tone is a set of `--footer-*` properties, as the header
style is a set of `--header-*` ones.

`theme::palette::derive` builds a light and a dark palette from one colour in OKLab
(`theme::color`): the backgrounds take the brand's hue at the preset's lightness, and each text colour
steps away from both backgrounds until it reaches AA 4.5:1. `palette::PAIRS` lists every text and
background pair `style.css` draws; a test derives palettes from a spread of brands (near white, near
black, pure yellow and a ring of hues) for every preset and checks every pair in both shades. Text on the
brand colour (`--color-on-brand`) is never stored; the theme picks it.

`Tokens::css` emits the tokens as CSS custom properties in one `<style>` in the head, and
`theme/style.css` uses those properties everywhere and names no colour of its own. A test checks that
every property the sheet reads is one the tokens emit, for every preset. A font is a curated system
stack or a webfont the theme serves itself (`theme::fonts`: Fraunces, Source Sans 3, IBM Plex Sans,
IBM Plex Mono, Inter, with the Noto faces for the Indic scripts). The frame declares and preloads only
the faces the site's two fonts name, every face swaps in, and a Noto face loads only for its script.
No font is requested from another origin.

`theme::tokens::registry()` is serialised to `web/admin/src/generated/themeSettings.json`, which the
admin Theme page renders. `theme::tokens_schema::samples()` is serialised to
`web/admin/src/generated/themeSamples.json`: derived palettes, nearest-passing suggestions and resolved
maps, which an admin test replays against its copy of the arithmetic. A Rust test keeps both files
byte-identical.

A resource list draws each resource as a card (`theme::cards`) built for poor data: with no thumbnail
the card shows a tinted surface with the kind's icon and type, and with no title it shows the
identifier without a file extension. The theme still ships `img/placeholder.svg` at its old URL, for
custom CSS that names it.

### Custom CSS

Stored text reaches the page as CSS in `theme::custom_css` and nowhere else. `custom_css::sanitize`
reads the text with `cssparser` and refuses `@import`, `@document`, any `url()` (or URL string in
`src()`, `image()`, `image-set()`, `cross-fade()`) that is not a path under this site's `/files/` or
`/assets/`, `expression()`, `behavior`, `-moz-binding`, `</style`, more than 64 KB, and text the tokenizer
cannot read. Each refusal is a `CssProblem` kind with its line and column; the sentence a curator reads
is one entry per kind in `omeka-api`'s message registry, so the admin translates it whole. The site
write and the preview answer refusals as a 422 under `o:theme_settings`; `ThemeSettings::from_value` sanitises again on read and drops refused CSS
whole. The frame prints the CSS in `<style id="custom-css">` after the theme sheet, so its rules win.

### The bundled theme's stylesheet

`theme::style::STYLE_CSS` is the bundled theme's own CSS, held as data (`theme/style.css`). The
theme's manifest lists it at `css/style.css`, and the document frame emits a `<link>` for every
manifest asset whose kind is a head asset, under `RenderContext::asset_base`. The link carries
`?v=<ThemeAsset::version>`, a digest of the bytes, so a deploy that changes the sheet changes the URL;
the caller serves `{asset_base}/css/style.css` from `theme::theme_asset("default", "css/style.css")`,
immutable for the current version and revalidated otherwise. The sheet's block rules are
the subset of upstream `application/asset/css/page-blocks.css` that applies to the markup this port
emits, its page-grid rules are `application/asset/css/page-grid.css`, and it adds a rule for every
block wrapper class no upstream rule names. A stored block `class` is arbitrary, so only a
registered layout's wrapper has a rule.

### The IIIF viewers

`iiif_viewer` is the viewer page upstream renders bare (`view/omeka/iiif-viewer/index.phtml`):
`mirador_config` builds `IiifViewerController`'s configuration with its allow-list, `viewer_page`
renders the document, and `viewer_src` is the link a block or the item page opens it with, under the
site's `iiif_viewer_sidebar` and `iiif_viewer_theme`. Mirador 4 and OpenSeadragon 5 are vendored
under `vendor/` by `vendor/update.sh` and listed in `theme/files.rs` as library assets, which the
frame never links: the viewer page loads Mirador, and the IIIF image block and the IIIF image media
embed load OpenSeadragon where they call it.

Both IIIF blocks take a height (`blocks/iiif.rs`), a Romeka setting of an upstream block kept in
`o:layout_data.romeka.settings`; a schema field marked `romeka` is stored there. In the editor's
script-free preview both render a poster card in place of the viewer. The presentation block's
manifest field is the `manifestSource` kind: a URL, or an item of this site stored as its manifest
URL, so upstream opens the same manifest. The item view links an item with a paintable media to the
viewer and to its manifest (`ResourceView::iiif_manifest`).

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

Four upstream blocks take Romeka settings in `o:layout_data.romeka.settings`; unset, each renders
as upstream does, and upstream renders the stored block plainer. The browse preview
(`blocks/browse_preview.rs`) may draw its cards in a style of its own instead of the theme's
`card_style` token, or as a carousel (`blocks/carousel.rs`, with `carousel.js` loaded only on a page
that holds one), set the column count, and choose the details under each name
(`theme/cards.rs::Detail`). A resource list carries its card style as its own class
(`resource-cards-*`), so a block's style and the theme's never meet on one list. Its sort is the
stored query's `sort_by`, a column or a property term, as upstream's `sortQuery` reads it; the
loader orders by a property's smallest public value. A preview whose search matches nothing says so.
The asset block (`blocks/asset.rs`) keeps each image's own shape unless the block asks for a 3:2
frame, lays the images out stacked, in a row or in a grid, aligns the captions, and links a slot to
a URL kept at `settings.attachments[<slot>].url` when it names no page. A site holds any file the
media allow list takes, so a slot's file has a kind (`view::AssetKind`) and a size: a non-image in
the asset block is a link to the file named by its alt text or its name, with the kind in words and
the size, and a slot that accepts images renders nothing for another kind. The list of pages
(`blocks/list_of_pages.rs`) may draw its pages as cards whose image and summary come from each
page's own blocks (`blocks/page_card.rs`, `ResourceProvider::page_card`). The item with metadata
(`blocks/item_with_metadata.rs`) may print only a chosen, ordered list of properties (the
`PropertyList` field kind) and may show each item as a compact card with a citation line.

### The block field registry

Each layout declares the `data` fields its upstream form edits as a `blocks::schema::LayoutSchema`,
beside its renderer, and returns it from `BlockLayout::schema`. `blocks::schema::registry` collects
them, in the registry's order, with the schemas of a block's and a page's `layout_data`, and
serialises them to `web/admin/src/generated/blocks.json`. The admin reads that file instead of
mirroring the registry, so adding a layout is one module, plus its name in the catalogue
(`web/admin/src/i18n`) and its field labels, help lines and choice names in the page editor's
table (`web/admin/src/pages/sites/editor/messages.ts` and `editor/locales/`).

A test writes the file and fails when it differs from the registry, so the boundary is enforced
rather than conventional. Regenerate with `UPDATE_GENERATED=1 cargo test -p omeka-site`. A second
test fails when a schema names a `data` key its layout's parse does not read.

The kinds are upstream's form elements: `Text`, `Html`, `Integer { min, max }`, `Boolean`,
`Choice { options, multiple }` with `ChoiceOption` values, `PageRef`, `AssetRef`, `ItemQuery`,
`Attachments { media }`, `Repeat { fields }` and `PageList` for the `listOfPages` tree. An
attachments list is stored as rows beside `data`, so its key is not a `data` member. The hidden
members a form does not present (`oembed.url_old`, `oembed.oembed`) are not fields.

Three kinds are this product's own controls over upstream's keys. `Color` is a `<hex-color>`, the only
colour the renderer keeps. `Sides { keys }` edits the four `padding_*` keys as one control. `Link
{ url_key }` writes a page under the field's key or a URL under `url_key`, never both, because the
hero renderer prefers the page. A `Text` field may name a `placeholder` example and an `ItemQuery`
the sibling choice that names its resource type (`resource_from`).

Every field also carries a `help` catalogue key: the one line under its control that says what the
visitor sees. `when` shows a control only while a sibling holds a value (`When::equals`) or any value
(`When::set`); a hidden control keeps what it stores. `group` and `advanced` place a layout-data field
in a section of the styling form. Tests fail when a field lacks help or a `when`, a resource source
or a link's URL key names no sibling.

### Romeka blocks

A Romeka block is an upstream block with a plain rendering upstream can show, plus the block's own
settings under `o:layout_data.romeka`. `blocks::romeka` owns the rule. `RomkaBlock` carries a name, a
catalogue label, a `LayoutSchema` in the same format as an upstream layout, a rich renderer and a
plain renderer. `plain` has no default, so a block without one does not compile. `ROMKA_BLOCKS` holds
the registry; `hero` is its first entry.

The content blocks follow the hero: `gallery`, `featuredItem`, `button`, `quote`, `accordion`,
`timeline` and `searchBox`, one module each under `blocks/`. They share the setting readers in
`blocks::setting` (text, ids, a page-or-URL `Button`, an items-or-query `selection`). A block that
lists items names them through the `items` hook; the page load reads each picked item with its main
media's large image, and the write resolves them into `PlainItem`s with their public values and main
image, so a plain rendering needs no database. `PlainRefs` also carries the site's locale, so a
plain rendering prints the site's own words ("View item", "Undated"). Two plain renderings cannot
carry what the rich one shows: the purifier allows no `details` and no `form`, so the accordion
stores each heading as an `h3` with its answer and the search box stores a link to the search page.
The gallery's lightbox is `js/lightbox.js`, added to a page only when a gallery on it asks for it
(`blocks::gallery::with_assets`); without the script each image links to its file.

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

### Resource page blocks

An item, item set or media page renders the fragments `resource_page::render_blocks` names, in the
order the theme stores under `resource_page_blocks`. The registration is `RESOURCE_PAGE_BLOCKS` and
the bundled theme's set is `default_blocks`. `resource_page_schema::registry` lists the registered
layouts per resource with their labels and the theme's default order, and serialises them to
`web/admin/src/generated/resourcePageBlocks.json`, which the admin Theme page turns into an ordered
list editor. A test keeps the file byte-identical; regenerate with `UPDATE_GENERATED=1 cargo test -p
omeka-site`.

### Starter page templates

`blocks::templates` owns the five built-in starting points a new page can begin from: a page `layout`
and `layout_data` plus a block list. A template block names either a registered layout with its
`o:data` or a registered Romeka block with its settings, so a template cannot reference a block the
admin has no form for. `template_registry` serialises them to
`web/admin/src/generated/templates.json`, expanding a Romeka block to its carrier with the settings
under `o:layout_data.romeka` and an empty carrier body, because the first save generates the plain
rendering. A test keeps the file byte-identical and checks every block against the registries: a
layout block's `o:data` parses, a Romeka block's settings name schema fields and carry their kinds,
and every `o:data` key names a field its layout declares (the `asset` layout's index keys excepted).

`blocks::templates::welcome_blocks` is the first page a site created through the API starts with (a hero,
an introduction and a preview of the latest items), stored by `omeka-api`'s site create in place of upstream's
style sampler. The chooser does not offer it. The about and gallery templates open with a `pageTitle` block;
the hero templates do not, because the hero's heading is the page's visible title.

A site's own templates are not stored here. They are one `site_setting` row,
`romeka_page_templates`, a JSON list of `{id, name, description, layout, layout_data, blocks}` that
upstream ignores; the admin writes it through `o:site_settings`.

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

- `searchFilters` and `browse()->renderSortSelector`. The browse views print the pagination and the
  advanced-search link without the filter UI.
- Module view listeners (`trigger('view.show.before')` and friends). There are no modules.
- `$this->lang()` as a view helper. The port carries the same information on
  `i18n::Locale` and `i18n::Translator`; full translation catalogs are excluded
  by DESIGN.md, so nine locales ship (see below).
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

Nine locales ship. English is the source language. Spanish is hand-written, and its terms follow
upstream's `application/language/es.po`. Hindi, Marathi, Malayalam, Tamil, Kannada, Telugu and Nepali
(`hi mr ml ta kn te ne`, the admin's languages) are machine-translated: their strings live in
`src/i18n/locales/<tag>.json`, which `web/admin/scripts/translate-locales.mjs` fills from
`locales/en.json` with sarvam-30b, reusing the admin's translation where the English is the same. A
locale file that lacks a key or adds one fails `every_shipped_locale_parses`. Their dates come from
CLDR, not the model: `web/admin/scripts/cldr-dates.mjs` writes `src/i18n/dates_cldr.rs` from node's
ICU, with the names, the `dateStyle`/`timeStyle` patterns, the day periods and the digits of each
locale's default numbering system (Devanagari for `mr` and `ne`, Latin for the rest). A region tag
(`es_ES`, `en_US`, `hi_IN`) narrows to its language; an unknown tag falls back to English. The running server keeps `SiteSettings::default()` until a `site_setting`
repository exists, so a live render is English today; the render layer honours whatever a caller sets.

The built-in vocabularies' labels (Dublin Core, Dublin Core Type, BIBO, FOAF) print in the site's
locale through `Translator::term_label(term, stored)`, upstream's `$translate($property->label())`.
The text is keyed by term (`dcterms:title`, or a vocabulary's prefix), and a stored label is
translated only while it is still the built-in English, so a user's vocabulary and an edited label
print as stored. The tables are the admin's `web/admin/src/i18n/locales/vocabulary/labels/<tag>.json`;
`data/vocabulary-labels/` is their copy for a build without `web/admin`, and
`i18n::vocabulary` tests fail when the copy is stale (`UPDATE_GENERATED=1 cargo test -p omeka-site
vocabulary` rewrites it) or a locale lacks a term.
