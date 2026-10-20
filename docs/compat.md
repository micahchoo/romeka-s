# Compatibility with Omeka S

Audience: someone deciding how far Romeka S can stand in for an Omeka S installation.

## The claim, and the pair it is a claim about

Romeka S is compatible with upstream [`omeka/omeka-s`](https://github.com/omeka/omeka-s) at commit
`5e413f009c447f4b587803ab23a9cec7d228f6e8`. The claim is about a pair: that upstream commit and this
repository at the commit you are reading. [`NOTICE`](../NOTICE) records the fork point, and
[`CHANGELOG.md`](../CHANGELOG.md) starts the history there.

Compatible means two concrete things.

- **The API.** The `/api` routes, the operations, the accepted parameters, the JSON-LD documents,
  the `o:` prefix and the `@context` path, the `Omeka-S-Version` response header, the `Link` and
  `Omeka-S-Total-Results` headers, the status codes and the two error documents are upstream's. A
  client written against Omeka S works against Romeka S unchanged.
  [`api.md`](api.md) is generated from the source and lists every one of them.
- **The data model.** The tables, the columns, the vocabulary classes and properties, the value
  types and the serialized date shape are upstream's. [`crates/omeka-db/SCHEMA-NOTES.md`](../crates/omeka-db/SCHEMA-NOTES.md)
  holds the per-table notes, and `crates/omeka-db/tests/schema_fidelity.rs` reads the upstream
  schema and asserts the same objects.

An Omeka S installation moves onto it through `omeka-migrate`, which reads the MySQL database and
writes the same tables into SQLite ([`crates/omeka-migrate/README.md`](../crates/omeka-migrate/README.md)).
SQLite is the default engine and MySQL is a second backend behind a feature
([`crates/omeka-db/README.md`](../crates/omeka-db/README.md)).

## What deliberately differs

Each row names the file that carries the difference. A row here is a decision that was reviewed, not
a gap that was discovered.

| Difference | Where it is stated |
| --- | --- |
| SQLite is the default storage engine; MySQL is behind the `mysql` feature. Parity is behavioural, not SQL-level. | [`crates/omeka-db/README.md`](../crates/omeka-db/README.md) |
| The admin is a strict-TypeScript SPA over the same `/api`, not server-rendered PHP views. The workflow is the same; the markup is not. | [`web/admin/README.md`](../web/admin/README.md) |
| Public sites render in Rust through a `Theme` trait. PHP themes are not ported, so a site's `theme` setting names a bundled theme. | [`crates/omeka-site/README.md`](../crates/omeka-site/README.md) |
| Module extension is a Rust registry, not a PHP module loader. | [`crates/omeka-plugin/README.md`](../crates/omeka-plugin/README.md) |
| No route answers 401. A missing, malformed, unknown or inactive API key leaves the caller anonymous, and the access check answers 403. | [`api.md`](api.md), Errors |
| A write route with no handler answers 501. Upstream throws `OperationNotImplementedException`, which its JSON strategy maps to 500, so a caller cannot tell "not built" from "failed" there. | [`crates/omeka-api/README.md`](../crates/omeka-api/README.md) |
| `GET /api/data_types/<unknown>` answers 404 where upstream throws and answers 500. | [`crates/omeka-api/src/read/meta.rs`](../crates/omeka-api/src/read/meta.rs) |
| `POST /api/jobs` (upstream fails at flush for every body) is unbuilt. `POST /api/assets` is built and takes a multipart body. | [`crates/omeka-api/README.md`](../crates/omeka-api/README.md) |
| A file's media type comes from the `infer` crate's magic-byte read and a UTF-8 text read, not `finfo`'s libmagic. Each format in `tests/media_type.rs` answers what libmagic 5.46 answers on the oracle. A format outside that list can differ: libmagic reads a `desktop.ini` as `application/x-wine-extension-ini`, the port as `text/plain`. | [`crates/omeka-media/src/media_type.rs`](../crates/omeka-media/src/media_type.rs) |
| The three file validation settings (`media_type_whitelist`, `extension_whitelist`, `disable_file_validation`) with no row mean upstream's defaults, because a Romeka install does not run `AddDefaultSettingsTask`, which stores them. Upstream reads a missing list as empty and refuses every file. `GET /api/settings` answers each as it applies. | [`crates/omeka-api/src/settings.rs`](../crates/omeka-api/src/settings.rs) |
| Every fetch a media ingester makes (`url`, `youtube`, `iiif`, `iiif_presentation`, `oembed`; the `html` ingester fetches nothing) passes one URL guard, on each redirect too. Upstream fetches any URL. The guard admits `http` and `https` only and refuses credentials in the URL. It resolves the host once, refuses the fetch when any address is loopback, link-local (`169.254.169.254` among them), RFC 1918 private, CGNAT `100.64/10`, IPv6 unique local or link-local, IPv6 site-local `fec0::/10`, unspecified (`0.0.0.0/8`), multicast, broadcast, `192.0.0.0/24`, benchmarking `198.18.0.0/15` or reserved `240.0.0.0/4`, or an IPv4-mapped, IPv4-compatible, NAT64 (`64:ff9b::/96`, `64:ff9b:1::/48`) or 6to4 (`2002::/16`) form of these, and connects only to the addresses it checked. A fetch waits 60 seconds to connect, for the response and between chunks of the body, ends at one hour in all, and takes a body of at most 2 GiB. The port-only global setting `import_allow_private_urls` lifts the address check and nothing else. | [`crates/omeka-media/src/http_guard.rs`](../crates/omeka-media/src/http_guard.rs) |
| The archive import and `romeka import media` check each file against the file validation settings, as an upload is checked. Upstream has neither path; its Sideload module does the same check. | [`crates/omeka-media/src/validation.rs`](../crates/omeka-media/src/validation.rs) |
| Thumbnails come from the `image` crate, not GD. The two strategies and the JPEG output are kept, so a derivative can differ in bytes while matching in size. | [`crates/omeka-cli/README.md`](../crates/omeka-cli/README.md) |
| Fulltext search runs on SQLite FTS5, whose ranking differs from MySQL's. | [`crates/omeka-db/README.md`](../crates/omeka-db/README.md) |
| CSV export is this product's, because upstream has no CSV export at the fork point. The column set is the one `romeka import csv` reads back. | [`crates/omeka-cli/README.md`](../crates/omeka-cli/README.md) |
| `romeka user add` without `--password` prints the activation link instead of mailing it, and `POST /api/users/{id}/password-link` returns the link, so an installation with no relay still onboards people. | [`crates/omeka-server/README.md`](../crates/omeka-server/README.md) |
| Site settings are stored as one `site_setting` row per key, as upstream. The site document carries them as the port addition `o:site_settings` and the site write accepts it, because upstream edits them through a server-rendered form and has no API for them. | [`crates/omeka-api/src/site_settings.rs`](../crates/omeka-api/src/site_settings.rs) |
| User and global settings cross the API only for the keys the admin edits: `locale` for both, and for a user the port-only `romeka_hide_lessons` (a boolean) and `romeka_lessons_seen` (a list of lesson ids), whose rows upstream ignores. They travel as `o:user_settings` on the single-user read, the user write, login and `/api/me`; `GET`/`PATCH /api/settings` serves a global admin, and `o:admin_locale` (the `MvcListeners::bootstrapLocale` result) comes on login and `/api/me`. Upstream writes both from server-rendered forms and has no API for them. | [`crates/omeka-api/src/settings.rs`](../crates/omeka-api/src/settings.rs) |
| The admin prints dates in UTC, as a medium date with a short time, in every list and detail view. Upstream uses the `time_zone` setting (default UTC), which the API does not expose, and varies the style per view. | [`web/admin/src/app/format.ts`](../web/admin/src/app/format.ts) |
| Built-in vocabulary labels and comments are translated by term, and only while the stored text is the built-in English. A user vocabulary's label that equals a built-in English msgid (a `shelf:title` named "Title") stays as stored, where upstream gettext would translate it. | [`crates/omeka-site/src/i18n/vocabulary.rs`](../crates/omeka-site/src/i18n/vocabulary.rs) |
| A theme's own settings stay a separate `theme_settings_<theme>` map (`o:theme_settings`); the per-key site settings are rows. The admin's settings schema is generated into `web/admin/src/generated/siteSettings.json` from `omeka-site/src/settings.rs`. | [`crates/omeka-site/src/settings.rs`](../crates/omeka-site/src/settings.rs) |
| The public document frame is the port's own, so its body carries only the view's classes, where upstream's default theme adds `body-truncate-fade`. The page body classes `page {layout_data.class} site-page-{slug}`, the favicon link, embedded JSON-LD and the user bar are ported; the user bar (`theme/user_bar.rs`) links `Admin dashboard` where upstream prints the installation title, and omits upstream's site, resource-list and `View` links. Its one edit link is named for what it opens (`Edit this page`, `Edit item`, `Edit item set`, `Edit media`) where upstream prints `Edit`, and it shows when the caller may `update` the page or resource, where upstream asks for the `edit` privilege. Logout is a form that posts to `POST /logout?redirect=<path>`, which ends the session and returns to that page; upstream's `GET /logout` lands on the login page. A redirect off this origin returns to `/`. Log in links `/login?redirect=<path>`, and the admin sign-in returns there. | [`crates/omeka-site/src/theme/default.rs`](../crates/omeka-site/src/theme/default.rs), [`crates/omeka-site/src/theme/user_bar.rs`](../crates/omeka-site/src/theme/user_bar.rs) |
| A new site's welcome page holds a starter (a Romeka hero, an introduction, a browse preview of the latest items) where upstream's `SiteAdapter::getFirstPageContent` holds a style sampler. Every block is an upstream layout and the hero carries its plain rendering, so upstream renders the same content. | [`crates/omeka-api/src/write/site_defaults.rs`](../crates/omeka-api/src/write/site_defaults.rs) |
| A site created without `o:navigation` gets the navigation `[welcome page, Browse]`, so its root `/s/<slug>` redirects to the welcome page. Upstream's `SiteAdapter::getDefaultNavigation` holds the browse link alone, so a new upstream site's root shows "This site has no pages." A create that sends `o:navigation` stores exactly what it sent, as upstream does. The default is an upstream page link in upstream's column, and upstream's `Site\IndexController::indexAction` also redirects the root to the first linked page, so upstream shows the same site. `o:homepage` stays unset, so unlinking the page moves the root with the navigation. | [`crates/omeka-api/src/write/site_defaults.rs`](../crates/omeka-api/src/write/site_defaults.rs) |
| `/s/<slug>/item-set/<id>` renders the item-set view. Upstream's `site/item-set` route reaches the item browse. | [`crates/omeka-server/README.md`](../crates/omeka-server/README.md) |
| The bundled theme's asset base is `/assets`, the same path prefix the admin bundle builds its chunks under, so one origin serving both collides on it. | [`docs/operator-guide.md`](operator-guide.md), Known limitations |
| A write body whose value names an unknown property term answers 422, with the errors document keyed by that term. Upstream's value hydrator sets a null property and answers 500. | [`crates/omeka-api/src/write/values.rs`](../crates/omeka-api/src/write/values.rs) |
| Stored HTML (html blocks, attachment captions, html media) is purified with `ammonia` against HTMLPurifier's default allowed set, at write and again at render, so rows written before the fix or copied by the migrator are covered too. The kept tags match upstream's; links gain `rel="noreferrer noopener"`, void elements are not self-closed, CSS values are not normalised, and `use_htmlpurifier` is not consulted: purification is always on. | [`crates/omeka-core/src/purify.rs`](../crates/omeka-core/src/purify.rs) |
| An `o:item_set`, `o:site` or `o:attachment[].o:media` entry whose `o:id` is a JSON float or bool is read as the number it names (200, or 404 when nothing has that id). Upstream's `ArrayCollection::get(string\|int)` throws a `TypeError` under `strict_types` and answers 500. Every other reference id reads as PHP's `is_numeric` does, numeric strings included. | [`crates/omeka-api/src/write/body.rs`](../crates/omeka-api/src/write/body.rs) |
| `POST /s/<site-slug>/preview` renders an unsaved site page for the admin editor: the body a site-page `PUT` takes, answered with the full public document and no write. An optional `o:theme_settings` map replaces the stored design tokens for that render only, which is how the Theme page previews an unsaved palette. It needs page-write permission, carries `no-store`, `noindex` and a CSP with no scripts. Upstream has no preview route. An empty title or slug previews under the site locale's `[Untitled]`; only the save refuses them. | [`crates/omeka-server/README.md`](../crates/omeka-server/README.md) |
| Item, item-set and media pages render the bundled theme's resource page blocks, read from `theme_settings_<theme>.resource_page_blocks` as upstream stores them; the Theme page's resource pages section edits that list. A link to a resource with no thumbnail renders no `<img>`, where upstream renders the core `default.png`, and the values list carries `class="values"`. The Lightbox gallery block links each image file's thumbnail to the full image with no script, and leaves out upstream's "Other Media" list. | [`crates/omeka-site/src/resource_page.rs`](../crates/omeka-site/src/resource_page.rs) |
| The oEmbed block's form has no `refresh` checkbox, because the port fetches nothing from the provider on save. A stored `refresh` still parses. | [`crates/omeka-site/src/blocks/oembed.rs`](../crates/omeka-site/src/blocks/oembed.rs) |
| Design tokens are this product's theme settings, stored as the bundled theme's own `theme_settings_<theme>` map: a preset id, whether the palette derives from the brand colour, the tokens set by hand (under the `auto` scheme each colour but the brand may also carry a dark value, `<key>_dark`), logo and banner assets, purified footer HTML and sanitised custom CSS. A map with no preset reads as the plain preset with derivation off; a site created through the API gets `{preset: archive, derive_palette: true}`. Custom CSS the sanitiser refuses is a 422 under `o:theme_settings` on the site write and on `POST /s/<site>/preview`. The frame emits the tokens as CSS custom properties and links the theme sheet as `?v=<content hash>`, immutable. | [`crates/omeka-site/src/theme/tokens.rs`](../crates/omeka-site/src/theme/tokens.rs) |
| `POST /api/sites/{id}/items` changes a site's `item_site` rows by a query, or by item ids and item set ids together (their union), with a dry run; it is the change upstream's `UpdateSiteItems` job makes after the resources form, run synchronously. Upstream has no such route. It writes only `item_site`. | [`crates/omeka-server/README.md`](../crates/omeka-server/README.md) |
| A site whose `assign_new_items` is true takes a new item only when its `o:item_pool` selects it: an item create without `o:site`, `romeka import csv` and the archive import attach the item to each such site (`site_items::attach_on_create`). Upstream attaches every new item to such a site; with the empty pool the behaviour is the same. As upstream, no update attaches an item, so an item removed from a site stays removed. The admin's Items tab writes the pool from the site's item sets, as `item_set_id[]=<id>` for each. | [`crates/omeka-api/src/site_items.rs`](../crates/omeka-api/src/site_items.rs), [`web/admin/src/pages/sites/siteItemsAdd.tsx`](../web/admin/src/pages/sites/siteItemsAdd.tsx) |
| A browse preview of items on a site that holds no items renders one sentence in place of upstream's empty list: on the public site "There are no items to show yet.", and in the admin preview a pointer to the site's Items tab. A site that holds items renders upstream's markup, empty list included. | [`crates/omeka-site/src/blocks/browse_preview.rs`](../crates/omeka-site/src/blocks/browse_preview.rs) |
| Saved page templates are one Romeka-only `site_setting` row, `romeka_page_templates`, a JSON list of `{id, name, description, layout, layout_data, blocks}`, read and written through `o:site_settings`. Upstream reads only the settings it knows: measured on the oracle, its public site and site admin serve a site carrying the row. The five built-in starter templates are code, not rows. | [`crates/omeka-site/src/blocks/templates.rs`](../crates/omeka-site/src/blocks/templates.rs) |
| `GET /api` answers the JSON-LD resource listing. Upstream renders an HTML page from `omeka/api/root` at the same route. | [`crates/omeka-api/src/read/meta.rs`](../crates/omeka-api/src/read/meta.rs) |
| Term lists are the Custom Vocab module built into core: the `custom_vocab` table, `/api/custom_vocabs` and the `customvocab:<id>` data types answer with no module installed. | [`crates/omeka-api/src/write/term_lists.rs`](../crates/omeka-api/src/write/term_lists.rs) |
| Deleting a term list removes its `customvocab:<id>` name from every template row. Upstream's `data_type = ?` matches nothing since Omeka S 4, where the column holds a JSON list, and leaves a dead name. | [`crates/omeka-db/src/repos/custom_vocabs.rs`](../crates/omeka-db/src/repos/custom_vocabs.rs) |
| `o:uris` sent as a string or a list is read as `URI label` lines into the map, as the module's form does. | [`crates/omeka-core/src/term_list.rs`](../crates/omeka-core/src/term_list.rs) |
| `POST /api/vocabularies/import` creates a vocabulary from RDF (an uploaded file's text or a URL), `POST /api/vocabularies/{id}/diff` answers what a file would change, and `POST /api/vocabularies/{id}/diff/apply` applies a reviewed diff. Upstream runs `RdfImporter` from server-rendered admin forms and has no API for it. The import is matched before `/api/{resource}/{id}`, so `POST` there no longer creates a vocabulary through the member path and other methods answer 405. An apply is refused when a member changed after the diff was computed; upstream applies the posted diff as it is. | [`crates/omeka-api/src/vocab_import/routes.rs`](../crates/omeka-api/src/vocab_import/routes.rs) |
| The importer reads the members `RdfImporter::getMembers` reads, equal to upstream's own answers in 18 measured cases over 10 files (`crates/omeka-api/tests/vocab_import_oracle.rs`). Its format guess reads a document opening with `{` or `[` as JSON-LD, where EasyRdf reads `{` as RDF/JSON and cannot guess `[`; it reads the content, not the content type, under `text/plain` and the generic XML and JSON types. A JSON-LD remote `@context` is not fetched. A URL is fetched through the same guard as a media ingester's (http(s) only, never to a private or local address unless `import_allow_private_urls` is set), within 20 seconds and 16 MiB; upstream fetches any URL. Language tags compare without case. | [`crates/omeka-api/src/vocab_import/mod.rs`](../crates/omeka-api/src/vocab_import/mod.rs) |
| `GET /api/tags`, `POST /api/tags/promote`, `/api/tags/merge`, `/api/tags/distinct` and `GET /api/custom_vocabs/{id}/values`, `POST /api/custom_vocabs/{id}/merge`, `/distinct` count, promote and merge tags (plain-text `dcterms:subject` values) and a list's words, with a dry run. The tag census is a Romeka-only read endpoint: an upstream-shaped query would cost one `/api/items` search per tag and could not name the tags to search for. Upstream has none of these routes. They write ordinary `value`, `custom_vocab` and `resource_template_property` rows. | [`crates/omeka-server/README.md`](../crates/omeka-server/README.md) |
| Spellings judged different are one Romeka-only `setting` row, `romeka_distinct_values`, a JSON map of scope (`tags` or `customvocab:<id>`) to lists of spellings. Upstream reads a setting only by its id, so it never reads this row (inferred from `Settings\Settings::get`, not measured on the oracle). | [`crates/omeka-api/src/tagging/mod.rs`](../crates/omeka-api/src/tagging/mod.rs) |
| A term list or a vocabulary can be published: `GET`/`POST /api/{custom_vocabs,vocabularies}/{id}/publication` and `GET /vocab/<slug>/[<local>]`, which negotiates Turtle, JSON-LD, RDF/XML, N-Triples or HTML. Upstream has neither route. The state lives in Romeka's own tables, `romeka_publication` and `romeka_publication_version` (migration 0003); no upstream table or column changes. | [`crates/omeka-api/src/publish/mod.rs`](../crates/omeka-api/src/publish/mod.rs) |
| Once a term list or vocabulary is published, its terms can only be added or deprecated. A `PUT`, `PATCH` or `DELETE` that would remove or rename a published term, move a published vocabulary's namespace, or delete the list or vocabulary is a 422. Upstream allows each of them. A draft (never published) changes as upstream allows. A list merge (`POST /api/custom_vocabs/{id}/merge`) on a published list keeps each published merged word in the list, deprecated and replaced by the kept one (`dcterms:isReplacedBy` in the next version), and its answer adds `deprecated`; the values still move. A promotion only adds words. | [`crates/omeka-api/src/publish/guard.rs`](../crates/omeka-api/src/publish/guard.rs) |
| The archive import is Romeka's own: `GET /api/romeka/archive`, `GET /api/romeka/starters`, `GET`/`POST /api/romeka/imports`, `GET`/`PUT`/`DELETE /api/romeka/imports/{id}`, `POST /api/romeka/imports/{id}/preview`, `/run` and `/undo`, `GET /api/romeka/imports/{id}/left.csv` and `GET /api/romeka/jobs/{id}/log`. Upstream has none of them. They need a global or site administrator; anonymous is 401 here, as on the account routes. With `ROMEKA_ARCHIVE_ROOT` unset, the listing answers `enabled: false` and create, preview and run answer 404. The import JSON carries `died`. | [`crates/omeka-server/README.md`](../crates/omeka-server/README.md), Archive import, [`crates/omeka-api/src/import/catalog.rs`](../crates/omeka-api/src/import/catalog.rs) (the starters) |
| `DELETE /api/jobs/{id}` answers 409 while an archive import that is running or undoing holds the job. Upstream deletes any job. Romeka refuses because the delete nulls the import's `job_id` and stops its worker at the next heartbeat. A job whose worker died is settled first, then deletable. Message key `server.jobHeldByImport` | [`crates/omeka-api/src/write/jobs.rs`](../crates/omeka-api/src/write/jobs.rs) |
| An import document is plain camelCase JSON, not JSON-LD: `id`, `folder`, `status`, `undoable`, `jobId`, `plan`, `summary` (`items`, `files`, `thumbnails`, `left`, `failed`, `newSets`, `newTypes`, `newFields`, `itemSetIds`, `existingSets`, `failures`, `progress`) and `undoReport` (`items`, `media`, `itemSets`, `templates`, `fields`, `kept`), null until an undo ends. `left.csv` is `text/csv`: `dcterms:identifier`, the grid's terms and a last `#reason` column, which `romeka import csv` ignores, as it ignores every header that starts with `#`. | [`crates/omeka-server/src/imports/wire.rs`](../crates/omeka-server/src/imports/wire.rs), [`user-guide.md`](user-guide.md) |
| Two job classes are Romeka's own, `Romeka\Job\ImportArchive` and `Romeka\Job\UndoImport`, with args `{"importId": n}`. Each runs in a `romeka job run` child the server starts. An upstream install reading the same `job` table cannot run them. | [`crates/omeka-cli/README.md`](../crates/omeka-cli/README.md), Jobs |
| Migration 0005 adds the tables `romeka_import` and `romeka_import_resource`. Upstream's schema has neither and never reads them. MySQL has no partial index, so its single open undo window is kept by the repository's transaction, where SQLite also has the unique index. | [`crates/omeka-db/SCHEMA-NOTES.md`](../crates/omeka-db/SCHEMA-NOTES.md) |
| A file under `/files/` of a private item or media is served only to a caller who may see both the media and its item: its owner, a role with `view-all`, or a request carrying such a user's session or API key pair. Upstream serves every stored file to anyone who has its URL. The origin asks `GET /api/romeka/files/allow` (Romeka-only, called by the proxy) before each file, which answers 204, 403, or 404 for a path no media or asset row names, so a stored file whose row is gone is no longer served. Assets have no visibility upstream and stay public. | [`crates/omeka-server/src/files_allow.rs`](../crates/omeka-server/src/files_allow.rs), [`deploy/Caddyfile`](../deploy/Caddyfile) |
| Every site page save is kept as a version in the Romeka-only table `romeka_site_page_version` (migration 0007): the page document it made, when and by whom, the newest 100 per page. `GET /api/site_pages/{id}/versions` and `GET /api/site_pages/{id}/versions/{version}` read them for a signed-in reader of the page. A page `PUT` or `PATCH` that sends `o:modified` is refused with 409 (`errors` adds `o:modified` and `o:user`) when the stored page was saved since; upstream ignores `o:modified` on a write, and a write without it is never refused. The save, its blocks and its version are one transaction; upstream's single flush has the same all-or-nothing effect. | [`crates/omeka-server/README.md`](../crates/omeka-server/README.md), Page history |
| A TIFF original gets the three JPEG thumbnails. GD has no TIFF decoder, so upstream makes none unless ImageMagick is configured. | [`crates/omeka-media/src/thumbnailer.rs`](../crates/omeka-media/src/thumbnailer.rs) |

## Site pages: the one-way rule

Romeka S may give a site page more than upstream can draw. The rule for that extra is one-way: a page
made here must show the same content when upstream renders it, perhaps plainer. A page edited in
upstream's admin is not promised to keep what only Romeka S draws.

The rule rests on what upstream does with data it does not know. Measured against the oracle at the
fork point, with a page posted through `/api` and read back:

| Sent | Upstream's API | Upstream's public page |
| --- | --- | --- |
| an extra key in a known block's `o:data` | stores it unchanged | ignores it; the block renders |
| an extra key in block or page `o:layout_data` | stores it unchanged | ignores it; the known keys apply |
| a block whose `o:layout` upstream does not know | stores it unchanged | an empty `<div class="block block-<layout>">`; the content is gone |

So an extra key degrades and an unknown layout does not. An upstream admin save is the second test.
Measured on the oracle, a page saved once through `admin/site/s/<site>/page/<page>/edit`:

| Where the extra key was | After the upstream admin save |
| --- | --- |
| block `o:data` | dropped: the form rebuilds `o:data` from its fields |
| block `o:layout_data` | kept: the form carries it as one hidden JSON field (`view/common/block-layout.phtml`) |
| page `o:layout_data` | dropped: the form rebuilds it from `PageLayoutDataForm` |

Every block this product adds obeys four consequences:

- It is stored under an upstream layout, usually `html`. Its own settings sit under
  `o:layout_data.romeka`, which upstream ignores and its admin keeps.
- The upstream part of the block, such as `o:data.html`, is the plain rendering. Romeka S writes it
  from the block's own renderer on every save, so nobody writes the fallback by hand.
- `o:layout_data.romeka` records `plain_hash`, a hash of the plain rendering's content: its visible
  text, image sources and alt text, and link targets, in order. Formatting is not hashed, because an
  upstream admin save re-formats the HTML through HTMLPurifier (measured) without changing it. When
  the content no longer matches, someone edited the block outside this product: Romeka S renders the
  edited plain HTML and its editor marks the block, rather than overwrite that edit with the rich one.
  Measured on the oracle: a page saved once in upstream's admin with no change renders the rich block
  again here, and a page whose text was edited there renders the edit.
- A block that has no plain rendering (a live search, an interactive map) writes a plain statement of
  what is missing and a link to this product's page, never nothing.

Page-level settings of this product do not go in page `o:layout_data`, because an upstream save drops
them.

## What compatibility does not cover

- Third-party PHP modules and themes, Laminas and Doctrine internals, and the admin's exact markup.
- Full translation catalogues. Nine locales ship: English, Spanish (hand-written from `es.po`) and seven machine-translated Indic locales (hi mr ml ta kn te ne), with CLDR dates on the public site.
- Live external services: oEmbed, YouTube and remote IIIF.
- Mail delivery: the interface and a log transport exist, and SMTP is optional.
- MySQL fulltext ranking particulars.
- A JSON-LD `@context` served from upstream's URL: `@context` points at this server's
  `/api-context`.

## Where the evidence is

The differential evidence this fork was built against lives in the port's own repository: the
behaviour ledger, the scenario diffs and the Docker oracle are not in this tree. The captured upstream
documents the API tests read are, under [`fixtures/api/`](../fixtures/api/README.md).
`crates/omeka-api/tests/golden.rs` parses each one into its wire type and asserts the serializer
byte-equals the file, and `crates/omeka-api/tests/property_wire.rs` reads the same set.
`cargo test --workspace` passes every target, those two included.

What you can run and read in this repository:

```
cargo test --workspace                          # the whole workspace, fixture targets included
cd web/admin && npm ci && npm run e2e           # the admin and the public site in a browser
cd docs && ./gen.sh --check                     # the API reference is current
crates/omeka-cli/scripts/verify-backup.sh       # backup and restore, byte for byte
```

| Artifact | What it answers |
| --- | --- |
| [`api.md`](api.md) | every route, parameter, member and access rule, generated from the source |
| [`crates/omeka-db/SCHEMA-NOTES.md`](../crates/omeka-db/SCHEMA-NOTES.md) | the schema, table by table, against upstream's |
| `crates/omeka-db/tests/schema_fidelity.rs` | the schema objects this build writes, against upstream's schema file |
| [`crates/omeka-migrate/README.md`](../crates/omeka-migrate/README.md) | how an Omeka S MySQL database is read and where the mapping refuses a source |
| [`crates/omeka-cli/scripts/verify-backup.sh`](../crates/omeka-cli/scripts/verify-backup.sh) | the backup archive, the manifest and a restore, end to end |
| [`web/admin/PERF.md`](../web/admin/PERF.md) | the frontend measurements, with the method and the decision behind each |
| [`docs/gen/check_routes.py`](gen/check_routes.py) | every exact claim in the route table, against a running server |
| `GET /iiif-viewer` is upstream's Mirador page and allow-list (`window.sideBarOpen`, `selectedTheme`); an allowed key applies only with a value of its kind. The page sends a CSP with no inline script. | [`crates/omeka-site/src/iiif_viewer.rs`](../crates/omeka-site/src/iiif_viewer.rs) |
| The IIIF presentation block opens Mirador's sidebar when `iiif_viewer_sidebar` is set; upstream's block always sends false. Both IIIF blocks take a height kept in `o:layout_data.romeka.settings.height`; unset, upstream's height. | [`crates/omeka-site/src/blocks/iiif.rs`](../crates/omeka-site/src/blocks/iiif.rs) |
| `/iiif/3/item/<id>/manifest`, `/iiif/3/item-set/<id>/collection` and `/iiif/image/3/<media>/...` (Image API 3, level 1) are Romeka's own; upstream needs the IIIF Server module. They read as a public page's caller and keep image answers in the store under `iiif/`. | [`crates/omeka-server/README.md`](../crates/omeka-server/README.md) |
| An item page with a paintable media links the IIIF viewer and its manifest. | [`crates/omeka-site/src/theme/default.rs`](../crates/omeka-site/src/theme/default.rs) |

A claim in a crate README is a claim; the command beside it is the proof. Run the command.
