# omeka-server

Romeka S is a fork of Omeka S. This crate is the HTTP surface: an axum app around the rules in `omeka-api` (the
`/api` JSON-LD read and write endpoints) and the public site routes served through `omeka-site`. The `/api` answers
keep upstream's members and the public site routes keep upstream's paths.

## Configuration

Every key has a default, so a server with no file and no variables starts on the
values below. The file is JSON and is named by `ROMEKA_CONFIG`; a key it omits
keeps its default. Precedence, lowest first: the defaults, the file, the
environment, then the arguments `romeka serve` passes.

| Key | Variable | Default | Meaning |
| --- | --- | --- | --- |
| `host` | `ROMEKA_API_HOST` | `127.0.0.1` | Interface to bind |
| `port` | `ROMEKA_API_PORT` | `8123` | Port to bind |
| `base_url` | `ROMEKA_BASE_URL` | `http://localhost:8123` | Origin for every JSON-LD `@id` and generated link |
| `log_level` | `ROMEKA_LOG` | `info` | One of `off`, `error`, `warn`, `info`, `debug`, `trace` |
| `max_body_bytes` | `ROMEKA_MAX_BODY_BYTES` | `268435456` (256 MiB) | Largest accepted request body |
| `request_timeout_secs` | `ROMEKA_REQUEST_TIMEOUT_SECS` | `30` | Seconds a request may run before the 408 |
| `smtp_host` | `ROMEKA_SMTP_HOST` | unset | SMTP relay host; unset means the log transport |
| `smtp_port` | `ROMEKA_SMTP_PORT` | the TLS mode's port | SMTP relay port |
| `smtp_user` | `ROMEKA_SMTP_USER` | unset | SMTP user name |
| `smtp_password` | `ROMEKA_SMTP_PASSWORD` | unset | SMTP password |
| `smtp_tls` | `ROMEKA_SMTP_TLS` | `starttls` | `starttls`, `tls` or `none` |
| `mail_from` | `ROMEKA_MAIL_FROM` | `no-reply@romeka.invalid` | Envelope sender |
| `archive_roots` | `ROMEKA_ARCHIVE_ROOTS` | unset | Server folders the import reads, as `name=path` pairs separated by `;` (an object in the file). The variable replaces the file's value. Each path is made absolute at load |
| `archive_root` | `ROMEKA_ARCHIVE_ROOT` | unset | One more root, named `archive`. With no root, the folder card of the Import page is empty and uploads still work |

The TLS mode implies the port when none is set: `starttls` 587, `tls` 465,
`none` 25. `starttls` requires the upgrade before it will send; `tls` wraps the
connection from the first byte; `none` speaks plain SMTP and is the mode a relay
on the same host takes.

The environment names mirror the variables `packaging/entrypoint.sh` exports, so
the container's settings reach the server unchanged. An unknown file key and a
value that does not parse fail startup with the key named.

```json
{
  "log_level": "debug",
  "max_body_bytes": 1048576,
  "request_timeout_secs": 10,
  "smtp_host": "relay.example.org",
  "smtp_tls": "starttls",
  "mail_from": "romeka@example.org"
}
```

```
ROMEKA_CONFIG=/etc/romeka/romeka.json romeka serve -d /data/romeka.db \
    --host 127.0.0.1 --port 8123 --base-url http://localhost:8080
```

`romeka serve` always passes a host, a port and a base URL, so those three come
from the command line in practice. It loads the config itself, overrides those
three, and calls `serve_with_services` with its file store (`--store`, default
`files` in the working directory) and a job runner.

## The database

The router holds one `omeka_db::DbPool`, a SQLite or a MySQL pool, and every handler crosses to a
repository through `omeka_db::with_db!` (`crates/omeka-db/README.md`, "Where the engine is chosen").
`build_router` and `serve` take anything that converts into a `DbPool`, so a test passes the
SQLite pool it seeded. The library has no `sqlx` dependency of its own; only the tests do.

The route tests of `api_read`, `files_allow` and every `imports_*` file seed through
`tests/common/engine.rs`. With `ROMEKA_MYSQL_URL` naming a server and no database, each test runs
on a fresh MySQL or MariaDB database instead of SQLite:

```sh
ROMEKA_MYSQL_URL=mysql://root:secret@127.0.0.1:13306 \
  cargo test -p omeka-server --test api_read --test files_allow --test 'imports*'
```

A test's database is named `romeka_srv_<pid>_<n>` and is dropped by a later run once its process
has gone, so point the variable at a throwaway server.

## Server operations

Readiness. `GET /ready` runs `SELECT 1` and answers `{"status":"ready"}` with 200,
or `{"status":"unavailable"}` with 503 when the pool is closed or unreachable.
`GET /health` stays the process liveness answer and reads no shared state.

The old admin path. `GET /admin` and `GET /admin/` answer 302 to the base URL's
root, where the SPA lives. Upstream serves its admin under `/admin`, so an old
bookmark lands there, and the SPA's relative asset paths would load nothing from
under it. A deeper `/admin/...` path stays a 404.

Shutdown. SIGTERM and SIGINT stop the listener, let the in-flight requests
finish, and exit. `docker stop` and `systemctl stop` send SIGTERM, so both are
covered without an entrypoint change.

Logs. One `tracing` line per request carries `method`, `path`, `status` and
`duration_ms`. The path holds no query string, so an API key in `key_credential`
never reaches the log. The level comes from `log_level`.

Conditional GET. Every read document (an API document, a site page, a theme
asset) carries a weak `ETag`, a SHA-256 of the body. A request whose
`If-None-Match` matches answers 304 with no body. Upstream Omeka S sends no
`ETag` and no `Last-Modified` on any response, so this is a port addition. There
is no `Last-Modified`, because no response carries a modification date the HTTP
layer can see.

Compression. A request with `accept-encoding: gzip` gets the body gzipped when
that is smaller. Every document carries `Vary: accept-encoding`, gzipped or not.

Limits. `max_body_bytes` answers 413 and `request_timeout_secs` answers 408, both
with the API error document (`{"errors":{"error":"..."}}`). Upstream has neither
status, so both sentences are the port's own.

The container (`packaging/`) covers the static assets: nginx serves `web/admin/dist`
and `/files`, and forwards `/api`, `/health` and `/s` to the port. It caps the body
at `client_max_body_size 256m`, the same value as the default `max_body_bytes`. It
does not compress, time out, probe readiness or forward `/ready`, log requests,
or terminate TLS (`packaging/nginx.conf.template` listens on plain HTTP only), so
those are the port's.

## API writes

`POST`, `PUT` and `PATCH` under `/api` go through `write()`. It checks the
`Content-Type` the way `ApiController::checkContentType` does (JSON or
multipart, else 415), authenticates, and hands the raw body to
`omeka_api::read::Api::create` or `::update`; the answer is the same document
the read layer serves. `DELETE` goes through `delete()`, which authenticates
and hands the id to `Api::delete`, ignoring the body as `ApiController::delete`
does. The resources without a write handler stay typed 501s. The body rules,
cascade rules and permission matrix are in `crates/omeka-api/README.md`.

The file store a media delete reaches lives on `AppState`. `build_router` roots
it at `FILES_ROOT` (`files`, upstream's `file_store.local.base_path`), and
`build_router_with_store` takes one for a test. `romeka serve --store DIR` roots
it at `DIR`, and passes the same store to every job process it starts.

## Accounts

`src/accounts.rs` puts `omeka-api`'s login, logout, password and password-link
flows on the wire, one handler per route:

| Route | Body | Answer |
| --- | --- | --- |
| `POST /api/login` | `{email, password}` | 200 the user document and `Set-Cookie`; 401, 403, 429 |
| `POST /api/logout` | any body, ignored | 204, the cookie cleared |
| `POST /logout?redirect=<path>` | any body, ignored | 303 to `redirect` when it is a path on this origin, else to `/`; the cookie cleared |
| `GET /api/me` | | 200 the user document, or 401 |
| `POST /api/password` | `{key, password}` | 200 the user document; 404, 422 |
| `POST /api/password/forgot` | `{email}` | 204, or 429 |
| `POST /api/users/{id}/password-link` | | 200 `{key, link, sent}`; 403, 404 |

The login, `/api/me` and set-password bodies are the `o:` user document, the same
one `GET /api/users/{id}` answers. Login and `/api/me` add `o:admin_locale`, the
locale `MvcListeners::bootstrapLocale` resolves: the user's `locale` setting, else
the global one, absent when neither exists.

`src/settings.rs` serves `GET` and `PATCH /api/settings`, a port route for the
global settings the admin's settings page edits (only `locale` so far). Only a
global admin may call it; `omeka_api::settings` holds the rules. A body that is not a JSON object is 400 with
the API error document; a member that is absent reads as empty and the flow
answers its own status. A 401 from a route the SPA calls signs the caller out
there.

The rate limit counts a login by email and by client address. The address comes
from `X-Real-IP`, then the last `X-Forwarded-For` entry (what the proxy itself
saw), then the socket peer, which `serve_listener` records through
`into_make_service_with_connect_info`. No address at all leaves the address
counter unused rather than shared by every caller.

`same_origin_write` is the middleware in front of every route. A state-changing
request that carries the session cookie must come from the configured base
URL's origin, checked on `Origin` and on `Referer` when there is none; anything
else is 403 with the error document. A request with no session cookie passes,
because a key pair in the query string is not attached by the browser.

## Mail

`src/mail/` is the mailer seam. `Mailer` holds a `Transport` and the base URL
the links are built from; `Message` is one finished message. `Log` is the
default transport and writes the whole message, link included, to the request
log. `Smtp` hands it to a `lettre` relay over the `ROMEKA_SMTP_*` settings, with
a ten second command timeout so a stuck relay does not hold a blocking thread.
A host whose TLS client cannot be built logs at error level and falls back to
`Log`, so a misconfigured relay does not stop the server from starting.

The link is `<base_url>/#/password/<key>`, the route the admin SPA renders. Both
the activation message and the reset message carry it, and both routes also
return it to the caller, so an installation with no relay can still onboard
people: `POST /api/users/{id}/password-link` answers `{key, link, sent}`, where
`sent` is false on the log transport because nothing left the process.

A send runs on `tokio`'s blocking pool. A mail failure is logged and does not
change the answer: `forgot` still answers 204, and the password-link route still
returns the link.

## Public site routes

Every path is under `/s/:site-slug` (`application/config/routes.config.php:16-116`).
`GET` is the only method served by these; another method is a 405 with `Allow: GET`.
The one exception is `POST /s/:slug/preview`, a Romeka addition below.

| Method | Path | Status | View | Upstream |
| --- | --- | --- | --- | --- |
| GET | `/s/:slug` | 302, else 200 | redirect, else `View::Index` | `Controller/Site/IndexController.php`, `view/omeka/site/index/index.phtml` |
| GET | `/s/:slug/page` | 200 | `View::SearchList` | `Controller/Site/PageController.php::browseAction`, `view/omeka/site/page/browse.phtml` |
| GET | `/s/:slug/page/:page-slug` | 200 | `View::Page` | `Controller/Site/PageController.php::showAction`, `view/omeka/site/page/content.phtml` |
| GET | `/s/:slug/search` | 200, else 302 | `View::Search`, else redirect | `Controller/Site/IndexController.php::searchAction`, `view/omeka/site/index/search.phtml` |
| GET | `/s/:slug/index/search` | 200, else 302 | `View::Search`, else redirect | the path `common/search-form.phtml` posts to; same action |
| GET | `/s/:slug/item` | 200 | `View::Browse` | `Controller/Site/ItemController.php::browseAction`, `view/omeka/site/item/browse.phtml` |
| GET | `/s/:slug/item/search` | 200 | `View::AdvancedSearch` | `Controller/Site/ItemController.php::searchAction`, `view/omeka/site/item/search.phtml` |
| GET | `/s/:slug/item-set` | 200 | `View::Browse` | `Controller/Site/ItemSetController.php::browseAction`, `view/omeka/site/item-set/browse.phtml` |
| GET | `/s/:slug/item-set/search` | 200 | `View::AdvancedSearch` | `Controller/Site/ItemSetController.php::searchAction`, `view/omeka/site/item-set/search.phtml` |
| GET | `/s/:slug/item/:id` | 200 | `View::Item` | `Controller/Site/ItemController.php::showAction`, `view/omeka/site/item/show.phtml` |
| GET | `/s/:slug/item-set/:id` | 200 | `View::ItemSet` | `routes.config.php` `site/item-set` reaches `ItemController::browseAction`; the port renders the item-set view instead of the set's item list |
| GET | `/s/:slug/media/:id` | 200 | `View::Media` | `Controller/Site/MediaController.php::showAction`, `view/omeka/site/media/show.phtml` |
| GET | `/s/:slug/media/:id/show` | 200 | `View::Media` | `routes.config.php` `site/resource-id` (`:action` default `show`) |
| GET | `/s/:slug/cross-site-search` | 200 | `View::CrossSiteSearch` (form) | `Controller/Site/CrossSiteSearchController.php::indexAction`, `view/omeka/site/cross-site-search/index.phtml` |
| GET | `/s/:slug/cross-site-search/results` | 200, else 302 | `View::CrossSiteSearch` (sections) | `resultsAction`, `common/cross-site-search/results.phtml` |
| GET | `/s/:slug/cross-site-search/items` | 200 | `View::CrossSiteSearch` (list) | `itemsAction`, `common/cross-site-search/item-results.phtml` |
| GET | `/s/:slug/cross-site-search/item-sets` | 200 | `View::CrossSiteSearch` (list) | `itemSetsAction`, `common/cross-site-search/item-set-results.phtml` |
| GET | `/s/:slug/cross-site-search/site-pages` | 200 | `View::CrossSiteSearch` (list) | `sitePagesAction`, `common/cross-site-search/site-page-results.phtml` |
| GET | `/s/:slug/cross-site-search/items-advanced` | 200 | `View::AdvancedSearch` | `itemsAdvancedAction`, `common/cross-site-search/advanced-item-form.phtml` |
| GET | `/s/:slug/cross-site-search/item-sets-advanced` | 200 | `View::AdvancedSearch` | `itemSetsAdvancedAction`, `common/cross-site-search/advanced-item-set-form.phtml` |

The two search actions that render a `View::Search` redirect to the single
resource's browse when `search_resource_names` holds one name, which is what
`count($resourceNames) === 1` does upstream.

A failure renders the theme error page: an unknown site, page or resource is a
404 (`view/error/404.phtml`), and a storage failure is the generic 500 document.
A site, page or resource the caller cannot see is the same 404, because the
public routes read through the same visibility the API applies
(`SitePageVisibilityFilter`, `ResourceVisibilityFilter`), so the row is absent
rather than denied. The error page carries `content-type: text/html; charset=utf-8`.
A missing page or resource under a site the caller can see renders the 404 in
that site's theme, layout and locale, as `ExceptionListener` does after
`prepareSite`; a slug that names no visible site renders it in English over a
placeholder site.
A `cross-site-search` action the controller does not name, and a
`/:controller/search` whose controller has no search action (`media`, `page`),
are 404s.

A page slug matches case-insensitively, following the `site_page.slug`
`COLLATE NOCASE` column, which stands in for upstream's `utf8mb4_unicode_ci`.

Search parameters are the API's own (`omeka-api/src/read/query.rs`):
`fulltext_search`, `search`, `property[i][property|type|text|joiner]`,
`resource_class_id[]`, `item_set_id[]`, `not_item_set_id[]`, `site_id`,
`in_sites`, `has_media`, plus `page`, `per_page`, `sort_by` and `sort_order`. A
parameter the parser does not know is ignored. A site search puts `site_id` on
the query; the cross-site search puts `in_sites`.

The `site_setting` keys the search reads are `search_type` (which the frame's
form posts to), `search_resource_names` (the sections a results page renders)
and `browse_attached_items` (which adds `site_attachments_only`). The site
loader reads them, and the rest of `SITE_SETTINGS`, from the `site_setting`
rows (`site/settings.rs`); the page size is the site `pagination_per_page`,
then the global setting, then 25. The site `locale` falls back the same way: an
unset or empty site locale takes the global `locale` setting, then English
(`MvcListeners::bootstrapLocale` and the site override after it).

Not ported, all answering 404: `media` browse (its controller has no browse
action), and `/:controller/:id` with an action other than `show`.

## A site's items

`POST /api/sites/:id/items` is a Romeka addition (`src/site_items.rs`). Upstream
changes a site's items with the `UpdateSiteItems` job its server-rendered
resources form dispatches, and has no API for it. The admin's "Items on this
site" pane needs the change and its preview over the API, so this route runs the
job's change synchronously through `omeka_api::site_items`, the module the job
itself now runs.

| Body | Response |
| --- | --- |
| `{action, query \| items \| item_sets, dry_run?}` | 200 `{matched, added, removed, total}`; 400, 403, 404, 415 |

`action` is `add`, `remove` or `replace`. Exactly one selector names the items:
`query` is an item query in the stored `o:item_pool` shape, `items` a list of item
ids, `item_sets` a list of item set ids whose items are meant. The selection is
what `GET /api/items` lists for the caller. `dry_run: true` answers the counts and
writes nothing. The caller must be allowed to update the site: its owner, a site
admin, or a global or site administrator. Only `item_site` rows change, the table
upstream's job writes.

## Page history

Every `PUT` or `PATCH` of a site page is one transaction that also stores the page document it
makes as a version (`omeka_db::repos::PageVersionRepo::save`). A page's first save also keeps the
state it replaced, with no user. A page keeps its newest 100 versions, and deleting the page
deletes them. These routes are a Romeka addition (`src/page_versions.rs`, logic in
`omeka_api::page_history`):

| Route | Response |
| --- | --- |
| `GET /api/site_pages/:id/versions` | 200, newest first: `[{o:version, o:created, o:user: {o:id, o:name} \| null, o:block_count}]`; 403, 404 |
| `GET /api/site_pages/:id/versions/:version` | 200, the page document that version stored; 403, 404 |

The caller must be signed in and able to read the page; anonymous is 403 even for a public page,
because a version can hold content the page no longer shows.

A page write that sends `o:modified` (the `{"@value": ...}` a read answers, or the bare ISO 8601
text) is checked against the stored page. When they differ the save writes nothing and answers 409
with `{"errors": {"error", "o:modified", "o:user"}}`, the stored time and who saved it. A write
without `o:modified` is never refused. The check has one-second precision, the precision of the
`modified` column, so two saves inside one second do not see each other.

## Archive import

Import v2 (`docs/import-v2.md`, "Wire") lives in `src/imports.rs` and
`src/imports/`; the rules are in `omeka_api::import2`. Only a global or site
administrator may call these routes: anonymous is 401, any other role 403.

| Route | Body | Response |
| --- | --- | --- |
| `GET /api/romeka/archive?root=&path=` | none | 200 `{roots, root, path, entries}`; 400, 404 |
| `GET /api/romeka/imports?page=&perPage=` | none | 200 `{total, page, perPage, items}`, newest first |
| `POST /api/romeka/imports` | `{sources}` or `{copyFrom}` | 201 the draft with its default or copied plan; 400, 404, 409 `v1Plan` |
| `GET /api/romeka/imports/{id}` | none | 200 the import, its plan and revision; 404 |
| `PUT /api/romeka/imports/{id}` | `{plan, revision}` | 200; 400, 409 `stale`, `notDraft`, `v1Plan`, `typeExists` |
| `DELETE /api/romeka/imports/{id}` | none | 204; 404, 409 `notDraft` |
| `POST /api/romeka/imports/{id}/scan` | none | 200 `{sources}`; 202 `{jobId}` past 20,000 entries; 409 `busy` |
| `POST /api/romeka/imports/{id}/preview` | `{include, plan?, records?, entries?}` | 200 the sections asked for; 400 |
| `POST /api/romeka/imports/{id}/run` | `{digest, confirm?}` | 200 `{jobId}`; 409 `changed`, `blocked`, `nothing`, `confirm`, `busy`, `cannotRun` |
| `POST /api/romeka/imports/{id}/stop` | none | 202 for an import that is running or undoing (the job stops after the record in hand; a stopped undo ends `undo_failed` with `stopped: true`); 409 `notRunning` |
| `GET /api/romeka/imports/{id}/undo/preview[?force=all]` | none | 200 `{remove, restore, kept, skipped, forced}`, read-only; 409 `cannotUndo` |
| `POST /api/romeka/imports/{id}/undo` | `{force?}` | 200 `{jobId}`; 409 `cannotUndo`, `busy` |
| `GET /api/romeka/imports/{id}/left.csv` | none | 200 `text/csv`; 404 before a run ends |
| `GET /api/romeka/imports/{id}/template.csv?columns=` | none | 200 `text/csv`, the list to fill in |
| `POST /api/romeka/imports/{id}/sheets` | multipart, one list | 200 the import with its new plan; 400, 413, 409 `notDraft` |
| `GET /api/romeka/imports/{id}/pairs.csv` | none | 200 `text/csv`, the unpaired rows to fill in |
| `POST /api/romeka/imports/{id}/pairs` | multipart, one filled list | 200 `{added, already, unmatched, unmatchedTotal, import}`; 400, 409 `notDraft` |
| `OPTIONS`, `GET`, `POST /api/romeka/imports/{id}/uploads` | tus creation headers | the tus 1.0 core: `OPTIONS` names the protocol and the largest file, `GET` lists the uploads with their limits, `POST` creates one (201 and `Location`) |
| `HEAD`, `PATCH`, `DELETE`, `OPTIONS /api/romeka/imports/{id}/uploads/{upload}` | tus headers | `HEAD` answers the stored offset, `PATCH` appends at most 64 MiB (413 past it; 409 on a wrong offset), `DELETE` removes the upload |
| `GET /api/romeka/starters` | none | 200 the starter item types |
| `GET /api/romeka/jobs/{id}/log` | none | 200 the job's log as text; 404 |

A preview reads the evidence index, never the folder walk: `scan` writes the
index, and `run` scans again before it compares the digest. The sheets a plan
names are read from the folder, or from the store when they were uploaded,
through a cache of at most 16 reads and 128 MB of sheet bytes. Run, undo, scan,
unzip and sweep jobs start through the `JobDispatch` the process supplies; the
run start itself is `ImportRepo::start_run`, one locked transaction that
freezes the plan. A `PATCH` to an upload streams its body into the store and is
not bound by `max_body_bytes`.

Deleting the job of an import that is running or undoing answers 409
(`JobRepo::delete_unless_held`), after the dead imports are recorded
(`ImportRepo::settle_dead`).

## Tags

The tag routes are a Romeka addition (`src/tags.rs`); every decision is in
`omeka_api::tagging`. A tag is a plain-text `dcterms:subject` value. Upstream has no
tag concept and no bulk rewrite.

| Route | Body | Response |
| --- | --- | --- |
| `GET /api/tags` | none | 200 `{values: [{value, resources, editable}], distinct}` |
| `GET /api/custom_vocabs/:id/values` | none | the same, for the list's words; 404 |
| `POST /api/tags/promote` | `{tags, list, dry_run?}` | 200 `{list, created, changed, skipped, templates, templates_skipped}`; 400, 403, 415, 422 |
| `POST /api/tags/merge`, `POST /api/custom_vocabs/:id/merge` | `{from, to, dry_run?}` | 200 `{changed, skipped}`; 400, 403, 404, 415 |
| `POST /api/tags/distinct`, `POST /api/custom_vocabs/:id/distinct` | `{values}` | 200 the census; 400, 403, 404, 415 |

A write changes only the resources the caller may update and counts the others
as `skipped`. Each write runs in one transaction, and `dry_run: true` answers the
same counts and writes nothing.

## Published term lists and vocabularies

Two Romeka additions (`src/publish.rs`); every rule is in `omeka_api::publish`.

| Route | Answer |
| --- | --- |
| `GET /api/{custom_vocabs,vocabularies}/:id/publication` | 200 the admin's view: state, address, versions, terms, Turtle preview |
| `POST` the same path, `{action: publish \| deprecate \| undeprecate \| link, ...}` | 200 the view after the command; 400, 403, 404, 415, 422 |
| `GET /vocab/:slug`, `/vocab/:slug/`, `/vocab/:slug/:local` | 200 the newest version (or `?version=N`) of the scheme or one term; 404, 406 |

`/vocab/*` negotiates on `Accept`: `text/turtle` (also the answer to `*/*` or no
header), `application/ld+json`, `application/rdf+xml`, `application/n-triples`
and `text/html`. `?format=ttl|jsonld|rdf|nt|html` overrides the header. Every
answer carries `Vary: Accept` and `Access-Control-Allow-Origin: *`, because the
point of publishing is that other sites read it. The prefix is forwarded by
`deploy/Caddyfile` and the e2e harness proxy.

## Live preview

`POST /s/:site-slug/preview` is a Romeka addition with no upstream counterpart
(`site/preview.rs`). The admin site-page editor renders the page it is editing
before it saves, so this route checks what the `site_pages` write checks, builds
the page in memory and answers the document `GET /s/:site-slug/page/:page-slug`
answers with that content.

| Method | Path | Response |
| --- | --- | --- |
| POST | `/s/:slug/preview` | 200 `text/html`, else the API error document |

The body is the JSON object `PUT /api/site_pages/:id` takes (`o:title`,
`o:slug`, `o:is_public`, `o:layout`, `o:layout_data`, `o:block` with
`o:attachment`), plus `o:id` of the page when editing. A body with no `o:block`
previews the page's stored blocks, which is what an update does. An optional
`o:theme_settings` map overrides the site's stored design tokens for that render
only, which is how the admin Theme page previews an unsaved palette. Credentials
are the admin SPA's: the session cookie, the API key pair in the query string, or
neither, which the access check refuses.

A body with a `resource_page` member, `{"kind": "items", "id": 12}`, renders that
item, item set or media show page instead (the kind is named as
`resource_page_blocks` names it). Its `o:theme_settings` overrides the stored
tokens and, when the map carries `resource_page_blocks`, the stored section
order, so the Theme page shows a draft order on a real record. It needs what a
site update needs (403 otherwise), answers 400 for an unknown kind or a missing
id, and 404 for a record the caller cannot see.

The status is the one the page write would answer: 403 when the caller may not
update that page, or add a page to that site; 404 for a site slug no site holds,
or a page id that is not a page of that site; 422 for a block or slug the write
would reject; 415 for a body that is not JSON. The one check the preview skips is
an empty title or slug, so a new page previews before the curator types: it
renders under the site locale's `[Untitled]` and the slug `untitled`, and the
save still refuses both.

Nothing is written. `write::site_pages_preview` parses with the write's own
`parse_blocks`, purifier, `validate` and attachment resolution, and
`site/dispatch.rs::render_preview` renders through the same theme, site
settings, navigation and caller visibility as `show_page`. It reads the caller's
visibility, so an attachment or asset the caller cannot see is dropped exactly
as it would be on the saved page. It renders at the page's own URL with the
request's query dropped, so the document matches the one the saved page serves
and a credential in the preview URL never reaches a link in the page. The one
difference is the user bar: the preview renders as a visitor sees the page, so
`show_user_bar` is read as `-1` (never) for that render, although the caller is
always a signed-in editor.

The answer carries `Cache-Control: no-store`, `X-Robots-Tag: noindex` and
`Content-Security-Policy: default-src 'self'; script-src 'none'; frame-ancestors
'self'`. The editor renders it in a sandboxed iframe, so no script in it runs. A
cookie-bearing POST is a state-changing request, so the same-origin guard covers
it exactly as it covers a cookie-authenticated `/api` write; `deploy/Caddyfile`
already forwards `/s/*` to the application.

## Theme assets

The bundled theme's document frame links every file in its manifest under
`RenderContext::asset_base` (`/assets`), so `GET` serves those files from the
theme registry rather than from a filesystem. The path after `/assets/` is the
manifest path the frame built.

| Method | Path | Status | Body |
| --- | --- | --- | --- |
| GET | `/assets/<path>` | 200 | the theme asset's bytes |

The media type is the asset kind's (`text/css` for a stylesheet,
`application/javascript` for a script or a library, `image/png` for an
OpenSeadragon button image). The vendored IIIF viewers are theme assets of the
library kind: `vendor/mirador/mirador.min.js`, `vendor/openseadragon/openseadragon.min.js`
and its `images/`, and `js/iiif-viewer.js`. The frame never links a library;
the view that needs one does. The frame links each head asset as
`<path>?v=<version>`, where the version is a digest of the bytes
(`omeka_site::theme::ThemeAsset::version`), so a deploy that changes the sheet
changes the URL every page links. A request whose `v` names the current bytes
answers `cache-control: public, max-age=31536000, immutable`; any other request
(no `v`, or the `v` of an older sheet) answers `cache-control: no-cache`, so the
client revalidates with the weak `ETag` the cache layer adds and never keeps a
stale sheet. A path no bundled theme ships answers
the API's 404 error document; another method is a 405 with `Allow: GET`. The
lookup searches the bundled themes in registry order, which is unambiguous
while one theme ships.

The frame also links four core assets the port does not ship: `css/fonts.css`,
`css/iconfonts.css`, `js/global.js` and `vendor/jquery/jquery.min.js`. They are
upstream admin files that need the Font Awesome and Lato font binaries, which
`verification/coverage/coverage.csv` marks `unported-admin-asset`, so they
answer 404.

## IIIF

| Method | Path | Status | Body |
| --- | --- | --- | --- |
| GET | `/iiif-viewer?url=&mirador_config=` | 200 | the Mirador page (`IiifViewerController`) |
| GET | `/iiif/3/item/<id>/manifest` | 200, 404 | the item's IIIF Presentation 3 manifest |
| GET | `/iiif/3/item-set/<id>/collection` | 200, 404 | the item set's IIIF Presentation 3 collection |
| GET | `/iiif/image/3/<media>/info.json` | 200, 404 | the stored image's Image API 3 description, level 1 |
| GET | `/iiif/image/3/<media>/<region>/<size>/<rotation>/<quality>.<format>` | 200, 400, 404 | the image |

`src/iiif/` holds them. The viewer page is upstream's contract: `url` is the one window's manifest, and
of `mirador_config` only `window.sideBarOpen` (a boolean) and `selectedTheme` (`light` or `dark`) apply.
It answers a Content-Security-Policy that loads scripts only from this origin and no inline script,
and lets Mirador fetch a manifest and its images from any https origin.

The manifests, the collection and the Image API are Romeka additions (upstream needs the IIIF Server
module). They read through the API as a public page's caller: the session cookie, or no one, never a
key in the query. A resource the caller may not see is a 404, as its `/s/` page is. Each answer
carries `Access-Control-Allow-Origin: *`; a cross-origin fetch carries no cookie, so that shares no
private document.

A manifest has one canvas per media a canvas can paint, in media order: a stored image the
thumbnailer decodes, with its derivatives, or a media ingested from a IIIF image service. A stored
image's canvas is the original's size, its body is the "large" derivative at its own size with this
server's Image API as the service, and the original is the canvas `rendering`. Sizes come from the
file headers on first use and are kept in memory by storage path (`iiif::Dimensions`); nothing
writes `media.data`.

The Image API offers level 1 plus `sizeByPct`, `sizeByConfinedWh` and `png`, with no answer edge over
4096 pixels and no upscaling. Each answer is kept in the store under
`iiif/<storage id>/<region>/<size>/<rotation>/<quality>.<format>`, so a repeat is a read. Those files
are not removed with their media. `deploy/Caddyfile` forwards `/iiif-viewer` and `/iiif/*`.

## Render contract

A handler does three things and nothing else.

1. Parse the path with `omeka_site::slug::parse_public_path`, read the site row
   by slug through `omeka_db::repos::SiteRepo`, and stop on a missing row or a
   non-public site.
2. Load rows into `omeka-site` values. `site/load.rs` turns a `SiteRow` into
   `Site`, a `SitePageRow` plus its blocks into `SitePage`, a resource row plus
   its values into `ResourceView`, and a `MediaRow` into the `MediaRef` the
   media block shows. `site/provider.rs` runs the searches a page's blocks will
   ask for, and loads each media or item-with-metadata block's
   `site_block_attachment` rows with the item views and media refs they resolve
   to, plus the assets the asset block and any block's `background_image_asset`
   name, because `ResourceProvider` is synchronous and `omeka-db` is async.
3. Render through the bundled theme and copy the document onto the wire. The
   theme owns all HTML and escaping; the server never builds markup.

Known gaps, each a repository the port does not have yet: `ItemFilter` carries
no `site_id`, so a browse-preview shows every public resource; and an
item-with-metadata block's item-set and media list thumbnails are not resolved,
so those lists render without images. The `asset` block and a block's
`background_image_asset` both resolve through the asset repository, and a
`site.thumbnail_id` or `resource.thumbnail_id` shows the asset it names.
