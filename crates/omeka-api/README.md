# omeka-api

Romeka S is a fork of Omeka S. This crate holds the `/api` REST layer: wire
types, the JSON-LD serializer, the route table, error documents, adapter
registry, API key parsing, and the read and write endpoints over `omeka-db`.
`omeka-server` turns these into axum responses. `read::Api` holds the
deployment's `omeka_db::DbPool`; every repository call goes through
`omeka_db::with_db!`, and the import v2 pipeline (`src/import`) stays generic
over `omeka_db::Backend` because a run holds one transaction across many
repositories.

The wire keeps upstream's member names, member order and error documents, so a
client written against Omeka S reads the same answers here.

## Wire types are mirrors (wave 1)

This crate's wire types under `src/wire/` mirror one PHP representation class
named in each module doc, so the serializer can emit upstream key order. The
read and write endpoints speak in `omeka-db` rows and map at the boundary.

## Key order

Serialization is `serde_json` over structs whose field order is the insertion
order the PHP representation produces. That order is frozen by the golden
fixtures: parse fixture, serialize, byte-equal the file
(`tests/golden.rs`, fixtures under `fixtures/api/`).

Reason for matching PHP order instead of sorting keys: a human can diff a
port response against a PHP response after only whitespace normalization, and
the differential normalizer sorts keys anyway. Maps (`values`,
`extensions`, `o:thumbnail_urls`) keep insertion order via `IndexMap`.
Untyped `serde_json::Value` members (`o:ini`, `o:args`, `o:item_pool`,
`o:layout_data`, media `data`) keep PHP order because this crate enables
`serde_json`'s `preserve_order`; without it they would sort.

## Archive import

`src/import/` is the logic of import v2, a Romeka addition (`docs/import-v2.md`). The module is generic over `omeka_db::Backend` and holds no
SQL. Its parts:

- Rules and evidence. `plan` is the stored plan and its one parse boundary. `archive` holds the named
  roots and the safe relative paths. `evidence` walks a source into file entries, with `scan` to store
  the walk. `sheet` reads CSV, TSV, `.xlsx`, `.xls` and `.ods` lists, with the encodings, delimiters
  and quotes of the contract. `tags` reads EXIF, IPTC and XMP tags and file facts.
- The pure derive. `derive` takes the evidence, the plan, the decisions and a catalog of the install,
  and gives a model of records, flags, anomalies and a digest. It uses `grouping` (the file-name
  pattern), `join`, `slots`, `matching` and `flags`. It never touches a disk or a database.
- What the admin reads. `preview` answers the sections of `POST …/preview`, `check` the counts and
  the digest, `draft` the stored drafts, `template` the list a person fills in, and `starters` the
  starter item types.
- The write side. `run` writes the model one record at a time, with `ledger`, `media_in` and `left`.
  `undo` takes an import back, with `touched` to find what changed since.
- Uploads. `staging` keeps the files a person sent, `unzip` unpacks a ZIP among them, and `sweep`
  removes them when no import needs them.

The server's routes in `omeka-server/src/imports.rs` call it, and the CLI's jobs run it. The server is
the one validator: the admin only renders what it answers. The routes are in
[`../omeka-server/README.md`](../omeka-server/README.md), and the commands in
[`../omeka-cli/README.md`](../omeka-cli/README.md).

## Route table

`src/routes.rs` expands the upstream lookup (`api` route, `:resource`
segment, adapter instance) into one row per path, method, and handler:
`2 + 18 * 7 = 128` rows. Method semantics come from Laminas
`AbstractRestfulController` and `ApiController`:

- collection: `GET` search, `POST` create
- member: `GET` read, `POST` create, `PUT` update, `PATCH` patch,
  `DELETE` delete
- wrong method on a routed path: 405 with an `Allow` list (port behavior;
  upstream answers 405 from the generic MVC layer with a non-JSON body)
- unknown resource name: 404 with
  `The API does not support the "<name>" resource.` This is a port decision.
  Upstream throws `BadRequestException`, which `ApiJsonStrategy` does not
  map, so PHP answers 500 for the same request. The brief demands 404.

## Errors and 501

Error bodies use the two shapes from `ApiJsonRenderer`:
`{"errors":{"error":"..."}}` for general errors and
`{"errors":{"<key>":["..."]}}` for validation errors. Status codes follow
`ApiJsonStrategy::getStatusCodeForException` (400, 403, 404, 415, 422) plus
500.

A handler the port does not build answers 501 with
`{"errors":{"error":"The <resource> adapter does not implement the <operation>
operation."}}`. Upstream has no 501; an adapter that lacks an operation
throws `OperationNotImplementedException`, which `ApiJsonStrategy` maps to
500. The port uses 501 so a caller can tell "not built yet" apart from
"failed". The port builds create, update and delete for items, item_sets,
media, sites, site_pages, users, vocabularies, resource_classes, properties,
resource_templates, assets and jobs, except `POST /api/jobs`, the one create
left unbuilt for the reason below.

An adapter whose registered `operations` list does not contain the requested
operation refuses it upstream, so the read layer answers the upstream 500 with
the same sentence instead of a 501.

One create is excluded, with its upstream source:

- `POST /api/jobs`: `JobAdapter::hydrate` is empty, while `job.class` is
  `NOT NULL` (`Entity/Job.php:56`). Upstream's create therefore fails at flush
  with a database error for every body. The port leaves the 501 rather than
  reproduce a 500 for each request.

`POST /api/assets` is built. Upstream reads the upload out of a
`multipart/form-data` body (`ApiController::processPostData`), so the create
takes a `boundary` `Content-Type`, parses the `data` field as JSON and the
`file` field as the upload, and stores the bytes under the `asset` prefix
(`AssetAdapter::hydrate`). A JSON body, or a multipart body with no `file`
field, answers `{"errors":{"file":["No file was uploaded"]}}`. The file is
checked against the media file-validation settings, with the same sniffing a
media upload runs, and an SVG is filtered before it is stored; upstream's own
four-image list is the deliberate difference `docs/compat.md` records.

A `POST /api/media` create runs the ingester `o:ingester` names, through
`omeka-media`'s registry, so `MediaAdapter::hydrate` and the ingester together
write the row: the ingester reads the request content (`ingest_url`, `html`,
`o:source`, `file_index`, `start`, `end`), fetches, stores and thumbnails a
file, and returns the typed payload; the adapter stores `data`, `o:source`,
`position`, `o:lang` and `o:alt_text` and takes the owner from the item. A name
no ingester registered answers 422 `Media must set a valid ingester.`, the way
`Ingester\Fallback` is refused. The same create runs for each inline `o:media`
entry of an item create, so a CSV import or a script can post an item and its
media in one request. The ingesters are synchronous, so each runs on
`tokio::task::spawn_blocking`: a `url` or `iiif` create waits for its fetch
without holding the async worker that carries every other request.

## Read endpoints

`src/read/` holds one module per resource group: `resources` (items, item_sets,
media, value_annotations), `resources_generic` (the `resources` adapter),
`vocab` (vocabularies, classes, properties, templates), `meta` (users, sites,
site_pages, api_resources, data_types, root, context) and `system` (assets,
jobs, modules). Every member answers one document; the collection headers are
described below.

`src/read/export.rs` serves `romeka export jsonld`: it lists the searchable
adapters (`resources()`), opens the list window so one call returns every row,
and hands back the JSON array the collection endpoint answers. It adds no
serializer; the CLI writes the documents this module reads.

| resource | list | member | notes |
| --- | --- | --- | --- |
| items, item_sets, media | yes | yes | value filters; `search` matches the title |
| value_annotations | yes | yes | no `sortFields` upstream, so `sort_by` is ignored |
| assets | yes | yes | `owner_id` filter; no visibility flag on the table |
| jobs | yes | yes | `class` and `status` filters; `sort_by=owner_email` falls back to `id` |
| modules | yes | yes | state is `active`/`not_active`; `o:ini` carries `name` and `version`; no total header, no window |
| vocabularies | yes | yes | |
| resource_classes, properties | yes | yes | `vocabulary_id`, `local_name`, `vocabulary_prefix`, `vocabulary_namespace_uri`, `term` filters |
| resource_templates | yes | yes | |
| users | yes | yes | list needs an authenticated key |
| sites | yes | yes | |
| site_pages | yes | yes | `site_id`, `site_slug`, `slug`, `is_public` filters |
| api_resources | yes | yes | the member echoes any id; no total header, no window |
| data_types | yes | yes | the six registered names, in registration order; no total header, no window |
| resources | yes | yes | every resource kind in one page; see below |

### Query parameters

`id` narrows every entity collection, exactly as `AbstractEntityAdapter::buildBaseQuery`
applies it before each adapter's own `buildQuery`. `id[]=1&id[]=2`, `id[0]=1`,
`id=1,2` and a repeated `id` all read as the same list. `SearchQuery::ids`
carries it, the SQL window renders it as `resource.id IN (...)`, and the
in-memory list paths apply `query::id_matches` to the rows they loaded. The
non-entity adapters (`api_resources`, `data_types`, `modules`) extend
`AbstractAdapter` and have no base query, so `id` is ignored there.

`resource_classes` and `properties` also read `vocabulary_prefix`,
`vocabulary_namespace_uri` and `term` (`vocab::MemberFilter`): the first two are
equalities on the member's vocabulary row, and `term` is a `prefix:localName`
that passes `Vocabulary::TERM_REGEX` and narrows to that one member. `term`
elsewhere is not a filter.

### The list window and the `Link` header

The window is `AbstractEntityAdapter::limitQuery`: `page` and `per_page` apply
only when `page` is numeric (`SetBrowseDefaults` writes `page=1` when the caller
sent none). A non-numeric `page` leaves the query unbounded, apart from numeric
`limit` and `offset`. `paging::Limit` is that decision and `paging::Window` is
the paginator `ApiController::getList` reads for the links, whose page count is
`ceil(total / per_page)`.

`Link` is built by `response::link_header` from `php::link_pairs`, which
rebuilds the PHP request array and renders it the way Laminas does: a nested
parameter keeps its bracket notation (`property%5B0%5D%5Bproperty%5D=1`), an
array is indexed from zero, and a space is `%20`. `read/php.rs` documents the
two Laminas steps.

A collection whose adapter answers a plain `Response` has no
`Omeka-S-Total-Results`: `ApiController::getList` adds the header only when
`Response::getTotalResults()` is not null. That is `api_resources`,
`data_types` and `modules` (`ApiResponse::collection`), and it also means their
searches ignore the page window, so they always answer every registered row.
Every other collection uses `ApiResponse::listing` and carries the header.

`GET /api` answers the JSON-LD resource listing. Upstream answers an HTML page
there (`view/omeka/api/root.phtml`); this is a deliberate divergence, recorded
in the unit report rather than here.

`read/resources_generic.rs` is `ResourceAdapter`. Its `search` returns a page
of `Resource` rows whatever their kind, ordered by `id` (`ResourceAdapter`
declares no `sortFields`), and its `read` proxies the row to the adapter its
`getResourceName` names, so the member carries that kind's `@id` and the
per-kind access rule applies. The `owner_id`, `resource_class_id`,
`resource_template_id`, `is_public` and `search` filters are rendered; the
`property[i]` value filters, `fulltext_search` and the `created_*` /
`modified_*` dates are not, and are reported as a gap.

Adapter access mirrors `Api\Manager::execute`'s ACL check: `users` needs an
authenticated caller; `value_annotations`, `jobs` and `modules` need a
privileged one (`global_admin`, `site_admin`, `editor`, `reviewer`); everything
else is open, including anonymous requests.

This crate issues no SQL. The asset, job, module, site_page and
value_annotation listings in `src/read/system.rs`, the page relations, the term
catalogue and the list window all read through `omeka-db` repositories.

## Write endpoints

`src/write/` holds one module per resource. A write parses the body at the
boundary (`write::body::Payload`), checks general adapter access, hydrates and
persists through `omeka-db`, then answers through `Api::read_member`, so a
create or update response is byte-identical to a `GET` of the row. A delete
answers the same document detached (`write::detached`), because
`Api\Manager::finalize` renders the entity the adapter returns after the flush
has cleared its id. `write::vocab` holds `vocabularies`,
`resource_classes` and `properties`, and `write::vocab_member` holds the
columns and validation the two member kinds share; `write::templates` holds
`resource_templates`. `POST` and `PUT`/`PATCH` are built for:

| resource | POST | PUT/PATCH | notes |
| --- | --- | --- | --- |
| items | yes | yes | `o:item_set` and `o:site` membership replaced on a full `PUT`; inline `o:media` built on create only |
| item_sets | yes | yes | `o:is_open` resets to false on a full `PUT` |
| media | yes | yes | the create runs the ingester named by `o:ingester` and stores the file and payload it produced |
| sites | yes | yes | create adds a welcome page and a site-admin permission, and a create without `o:navigation` links the welcome page before the browse link; every row of one create commits together or not at all; a `PUT` removes every page `o:page` does not name |
| site_pages | yes | yes | blocks replaced; `o:data` `{}` is stored as `[]` (PHP `json_decode(..., true)`) |
| users | yes | yes | new users are inactive and have no password |
| vocabularies | yes | yes | `o:class` and `o:property` list their members; an unlisted member is removed |
| resource_classes, properties | yes | yes | `o:vocabulary` read on create only |
| resource_templates | yes | yes | `o:resource_template_property` replaced when the body carries it |
| assets | yes | yes | create takes a multipart `file` and `data` field; update writes `o:name` and `o:alt_text` |
| jobs | no | yes | the empty `job` hydrate writes nothing; the create needs a `class` |
| all others | 501 | 501 | |

## Delete endpoints

`DELETE /api/<resource>/<id>` is built for items, item_sets, media, users,
sites, site_pages, vocabularies, resource_classes, properties,
resource_templates, assets and jobs. `ApiController::delete` hands the id to
`Api::delete`, which checks the adapter grant, finds the row, checks the entity
privilege, removes it, and answers 200 with the detached entity
(`Api\Manager::finalize` renders the entity the adapter returns after the
flush), and the same `GET` is a 404 afterwards. A missing row is the same 404
message a read gives.

`write::detached` rewrites the live document into that shape: `o:id` is null,
`@id` is the collection URL, `@reverse` is absent (upstream builds it only when
the id is set), and each association the delete cascaded away reads as a
detached reference. So an item's `o:media` and `o:primary_media` keep their
places with a null id, an item set's `o:items` link drops its `item_set_id`
query, and a site's pages become detached references. A lazy collection that
never loaded reads empty, so an item's `o:item_set` and `o:site` are `[]`. A
reference to a row that survives keeps its id, such as `o:owner` or a
resource-valued value.

Cascade and refusal rules, each taken from the PHP:

| resource | what goes with it | privilege |
| --- | --- | --- |
| items | values (`Entity/Resource.php:85`), media (`Entity/Item.php:24`, including each media's stored files through `Module::deleteMediaFiles`), `item_item_set` and `item_site` rows (database cascade) | author and up; author and reviewer only their own row (`OwnsEntityAssertion`), reviewer and editor any | 
| item_sets | values; `item_item_set` and `site_item_set` rows (database cascade) | as items |
| media | values, the resource row, the original file and one file per thumbnail type (`Module.php:323`) | as items |
| sites | values, pages, blocks and their attachments, site permissions, site item sets (`Entity/Site.php:88,99,109`) | the owner, or an admin (`OwnsEntityAssertion`, `AclFactory::addRulesForSites`) |
| site_pages | values, blocks and their attachments (`Entity/SitePage.php:74`) | the site's owner, or an admin |
| users | API keys (`Entity/User.php:59`); every other row the user owned keeps its data and loses the reference (`ON DELETE SET NULL`) | `global_admin` and `site_admin` only; an admin may not delete their own account, and a `site_admin` may not delete another admin (`IsSelfAssertion`, `UserIsAdminAssertion`) |
| vocabularies | its classes and properties (`Entity/Vocabulary.php:54,65`, `orphanRemoval`), and each property's values and template rows (`ON DELETE CASCADE`) | `global_admin` and `site_admin` only; a `site_admin` may delete only a vocabulary they own (`AssertionNegation(OwnsEntityAssertion)`) |
| resource_classes | nothing else; a resource that used the class loses the reference (`ON DELETE SET NULL`) | `global_admin` and `site_admin` only |
| properties | the values that used it and the template rows that listed it (`ON DELETE CASCADE`) | `global_admin` and `site_admin` only |
| resource_templates | its `resource_template_property` rows; a resource that used the template loses the reference (`ON DELETE SET NULL`) | author and up; an author only their own row, an editor any |
| assets | the stored file at `asset/<filename>` (`Module.php:347`) and the reference from any thumbnail (`ON DELETE SET NULL`) | author and up; author and reviewer only their own row, an editor any |
| jobs | nothing else | `global_admin` and `site_admin` only |

`write::access` holds the write grants as tables: the adapter-level create,
update and delete grant per resource (`AclFactory`), and the entity-level
privilege per role (`AbstractEntityAdapter::authorize`). The adapter grant runs
before the row is read, so an anonymous delete is a 403, not a 404.

Upstream has no refusal for deleting a user who still owns resources; the row
goes and the references become null. Verified against
`Api/Adapter/UserAdapter.php`, which overrides neither `delete` nor
`deleteEntity`, and against `Entity/User.php`, whose only `cascade={"remove"}`
collection is `$keys`.

A site page delete and a site delete accept only the site's owner below the
admin roles. A site update and every page write also accept a `site_permission`
role: `admin` for the site update, `editor` or `admin` for a page write, which
is `write::access::has_site_permission` reading the same role numbers
`HasSitePermissionAssertion` assigns.

Media file removal reaches `omeka-media`'s `FileStore`. The store comes from
`omeka-server`'s `AppState`; `Api::load` (no store) leaves it unset, and a
delete then removes the row and no bytes. The media read also uses the store: a
row whose three thumbnail files are present emits `o:thumbnail_urls` even when
its `has_thumbnails` column is false, and falls back to the column alone when no
store is bound.

Accepted members, per resource (a member with no value keeps the stored one on
`PATCH`, and resets to the upstream default on `PUT`). A `PATCH` whose body
names no value object keeps every stored value (`ValueHydrator` line 162); one
that names a valid value replaces the whole set, so a `PATCH` that names a
title drops every other value, as upstream does:

- common resource entities (items, item_sets, media): `o:is_public`,
  `o:owner`, `o:resource_class`, `o:resource_template`, `o:thumbnail`, plus one
  key per property term (`dcterms:title` and friends). Each value object names
  `property_id` (`"auto"` takes the term from its key), `type`
  (`literal`/`uri`/`resource*`), and its object (`@value`, `@id`, or
  `value_resource_id`). A value object may also carry `@annotation`.
- items: `o:item_set`, `o:primary_media`, `o:site`, `o:media`. A site whose
  `assign_new_items` is true keeps in sync with its `o:item_pool`
  (`src/site_items.rs`): a create that does not name `o:site`, and every update,
  joins the item to each such site whose pool newly selects it. Upstream's create
  joins every such site whatever its pool, which is the same for the empty pool.
  A named site is joined when the caller may assign items there
  (`AclFactory`'s `can-assign-items`: the site's owner or a site `admin` or
  `editor`), or, on a create, when the site assigns new items. A `PUT` replaces
  the membership, dropping the sites the caller may assign and the body omits.
  `o:media` builds the media on a create: each entry runs the media create the
  standalone route runs, so its ingester ingests, and the entry takes the
  position of its index in the array unless it names `position`. An entry that
  names an `o:id` is `Media must belong to this item.`, and every failure
  answers under `o:media` as one entry per failing media. A write that names the
  member on an update is not handled yet.
- item_sets: `o:is_open`.
- media: `o:ingester` (required on create), `o:item` (required), `data`,
  `o:source`, `position`, `o:lang`, `o:alt_text`, and the parameters the
  ingester reads out of the body: `html` for `html`, `ingest_url` and
  `store_original` for `url`, `file_index` for `upload`, `o:source` for
  `oembed`/`youtube`/`iiif`/`iiif_presentation`, and `start`/`end` for
  `youtube`. `o:renderer` is read on no write; the renderer comes from the
  ingester.
- sites: `o:slug`, `o:theme`, `o:title`, `o:summary`, `o:navigation`,
  `o:homepage`, `o:item_pool`, `o:is_public`, `o:assign_new_items`,
  `o:page`, `o:site_permission`, `o:site_item_set`, `o:thumbnail`. `o:page`
  names the pages to
  keep: on a `PUT`, and on a `PATCH` that carries the member, every page the
  list omits is removed. On create the list is only checked and the welcome page
  is still added. `o:homepage` names a page that must exist (404 when it does
  not) and must belong to the site. `o:navigation` is validated through
  `write::navigation`: the tree must be a list of objects with a `type` and
  `data`, each link type's `data` must satisfy its own `isValid`, page links
  must be unique, and a `links` child must be a list. `o:item_pool` must be an
  array, a JSON list or object, as PHP's `is_array` reads it. `o:thumbnail.o:id` is stored as the site's `thumbnail_id` without a
  lookup, as a resource thumbnail is; the asset repository is not built yet, so
  an unknown asset id is stored rather than answered as a 404.
- site_pages: `o:site` (create), `o:title`, `o:slug`, `o:is_public`,
  `o:layout`, `o:layout_data`, `o:block`.
- users: `o:name`, `o:email`, `o:role`, `o:is_active`. An email that is not
  `local-part@hostname` answers the Laminas `INVALID_FORMAT` message,
  `The input is not a valid email address. Use the basic format
  local-part@hostname`; the validator's other messages are not reproduced, and
  reported as a gap.
- vocabularies: `o:namespace_uri`, `o:prefix`, `o:label`, `o:comment`,
  `o:class`, `o:property`. A class or property entry names its `o:id` to keep a
  member, or carries a class/property body to create one. A member the list
  omits is removed on a full `PUT`, and an absent `o:class` or `o:property` on a
  full `PUT` removes every member of that kind.
- resource_classes, properties: `o:local_name`, `o:label`, `o:comment`, and
  `o:vocabulary` on create only.
- resource_templates: `o:resource_class`, `o:label`, `o:title_property`,
  `o:description_property`, `o:resource_template_property`.
- assets: on create the multipart `file` (with `o:name` and `o:alt_text`); on
  update `o:name` and `o:alt_text`.
- jobs: no member changes a row.

A top-level key that is not one of these and not a known property term is a
422 validation error keyed by that member.

A value object's `@annotation` is a `value_annotations` resource, hydrated the
way any resource is (`ValueHydrator` lines 134-153 run the subrequest through
`AbstractResourceEntityAdapter`). A non-empty object on create or update makes
the annotation; the port writes the resource, its values and the
`value.value_annotation_id` link in the same pass that writes the value. A value
that carried an annotation and no longer names one drops it, and a value that
leaves the set takes its annotation with it, as `Value::$valueAnnotation`
`orphanRemoval` does upstream. A read returns the annotation's term-keyed value
map under `@annotation`, and nothing when the annotation has no values
(`ValueRepresentation::jsonSerialize`).

Every reference a body names is resolved before the first write, so a failure
leaves the stored row untouched. The references are a value object's
`value_resource_id`, a site's users, item sets and pages, an item's
`o:item_set`, a media's `o:item`, an owner, a class and a template. An unknown
reference answers the 404 `findEntity` throws, naming the class and the numeric
id, for example
`{"errors":{"error":"Omeka\\Entity\\ItemSet entity with criteria {\"id\":999999} not found"}}`.
An unknown value resource answers
`{"errors":{"error":"Resource not found with id 999999."}}`, the message
`DataType\Resource\AbstractResource::hydrate` throws at the same point.

`AbstractEntityAdapter::hydrateOwner` is shared by every owned row
(`write::resource::owner_member`): an absent `o:owner`, or one that names no
`o:id`, keeps the stored owner; `{"o:id": null}` clears it; a named id must
exist; and a create with no usable id belongs to the caller.

## Permission rules

`write::access::adapter` mirrors `Service\AclFactory` at the adapter level for
create, update and delete; the entity methods mirror the per-entity assertions.
Anonymous writes are denied except where `addRulesForSites` allows the whole
adapter and the entity rule is the gate.

| resource | create | update | delete | ownership |
| --- | --- | --- | --- | --- |
| items, item_sets, media, assets | author and up | author and up | author and up | update: author own row, reviewer and up any; delete: author and reviewer own row, editor and up any |
| resource_templates | author and up | author and up | author and up | update: author own row, reviewer and up any; delete: author own row, editor and up any; a reviewer holds no grant on the adapter |
| vocabularies, resource_classes, properties, jobs | site_admin, global_admin | site_admin, global_admin | site_admin, global_admin | delete and update of a vocabulary: a `site_admin` only their own row |
| sites, site_pages | editor and up (site create); any caller at the adapter level, so the entity rule decides the rest | any caller at the adapter level; the entity rule needs the owner or a site role | any caller at the adapter level; the entity rule needs the owner | a page write: the site's owner or a site `editor`/`admin`; a site update: the site's owner or a site `admin`; a site create: `editor` and up; delete: `OwnsEntityAssertion` |
| users | site_admin, global_admin | researcher and up | site_admin, global_admin | update: self or admin; delete: nobody's own account, and no `site_admin` on another admin |

Roles, highest first: `global_admin`, `site_admin`, `editor`, `reviewer`,
`author`, `researcher`. This is the port's read of `AclFactory`. The
site-role assertions (`HasSitePermissionAssertion`) read the `site_permission`
rows for the caller at the site the write touches, with the role numbers the
assertion assigns (`admin` 1, `editor` 2, `viewer` 3). Reads consult the same
rows through the site and site-page visibility (`Actor::sees_site`,
`Actor::sees_site_page`), which the API reads and the `/s` routes share.

## Auth

`src/auth.rs` parses the `key_identity` / `key_credential` query pair the way
`MvcListeners::authenticateApiKey` reads it and validates the identity shape
against `ApiKey::STRING_LENGTH` / `STRING_CHARLIST`. A failed attempt is not
an HTTP error upstream: the request continues anonymously. The port keeps
that: this module returns data, not status codes.

A request resolves to a caller in two steps. The session cookie
(`romeka_session`) comes first, then the key pair, so the admin SPA and an API
client can both reach the same routes. A caller with no session and no key is
anonymous.

### A site's items

`src/site_items.rs` is the one place that changes a site's `item_site` rows in
bulk: a command (`add`, `remove`, `replace`) over a selector (an item query in
the `o:item_pool` shape, item ids, or item set ids). `omeka-server` serves it as
`POST /api/sites/{id}/items`, and `romeka`'s `UpdateSiteItems` job runs it too.
A selector resolves through the items list endpoint's own filters for the
caller, and the writes go in batches of 1000 rows inside one transaction.

### Tags

`src/tagging/` is the step from tag to term. A tag is a plain-text `dcterms:subject`
value; a URI or list value on the same property is an expert heading and no tag
operation touches it. A scope is every tag, or the words stored with one list's
`customvocab:<id>` type on any property. `census` counts each spelling once per
resource the caller can see. `merge` rewrites spellings to a kept one and deletes a
value the merge makes repeat on its resource, unless it carries an annotation;
on a list of words the list's entries follow. `promote` (`tagging/promote.rs`) makes
or extends a list from tags, retypes the tag values to the list, and makes the
item type of each changed resource offer the list on its subject field, adding
the field when the type has none. Every write touches only resources the caller
may update, runs in one transaction (`ValueRewriteRepo`), and reindexes the
changed resources after it commits. `distinct` remembers spellings someone said
are different, in the `romeka_distinct_values` setting.

### The account routes

`src/login.rs` holds the flows; `omeka-server` puts them on the wire.

| Route | Body | Answer |
| --- | --- | --- |
| `POST /api/login` | `{email, password}` | 200, the user document, `Set-Cookie`; 401 wrong credentials, 403 inactive with the right password, 429 rate limited |
| `POST /api/logout` | | 204, the cookie cleared and the session row deleted |
| `GET /api/me` | | 200 the user document, or 401 |
| `POST /api/password` | `{key, password}` | 200 the user document; 404 unknown or expired key, 422 a password the rules reject |
| `POST /api/password/forgot` | `{email}` | 204 always; 429 rate limited |
| `POST /api/users/{id}/password-link` | | 200 `{key, link, sent}`; 403 a caller who is not an administrator or who names their own account |

Login and `GET /api/me` add `o:admin_locale` to the user document, from
`src/settings.rs`: the user's `locale` setting, else the global one
(`MvcListeners::bootstrapLocale`). The same module is the boundary for
`o:user_settings` on the user read and write, and for the port's
`/api/settings`. Only the keys in `USER_SETTINGS` (`locale`, and the port-only
`romeka_hide_lessons` and `romeka_lessons_seen` the admin lessons keep) and
`GLOBAL_SETTINGS` (`locale`) cross it.

`loginAction`, `logoutAction`, `createPasswordAction` and
`forgotPasswordAction` are `application/src/Controller/LoginController.php`;
the password link is `Mailer::getPasswordCreation` as
`Admin\UserController::addAction` calls it.

### The session

`src/session.rs` keeps one `session` row per login (upstream's PHP session
table), keyed by the cookie's value and carrying `{"user_id":<id>}` as its
data. A row expires after 14 days of inactivity, and each request that uses a
row moves that window, which is what PHP's `gc_maxlifetime` does. Logout
deletes the row, and a row whose user was deactivated stops authenticating.
The tradeoff against a stateless signed cookie is in the module's header
comment: the row costs a read, an `UPDATE` and a user read per authenticated
request, and buys immediate revocation, a deactivation that takes effect at
once, and no session secret to manage.

The cookie is `romeka_session`, `Path=/`, `HttpOnly`, `SameSite=Lax`, with
`Secure` when the configured base URL is https. Login rotates the id
(`SessionManager::regenerateId`), so a session fixed before a login cannot
survive it.

### Passwords

`src/password.rs` stores and verifies `user.password_hash` the way
`Entity/User.php` does: bcrypt at cost 10, written in the `$2y$` form PHP's
`password_hash` writes, and verified with `password_verify`'s rule set (`$2a$`,
`$2b$`, `$2x$`, `$2y$`). A hash a migrated Omeka S install carries therefore
still verifies. A null hash verifies nothing, as `verifyPassword` returns false
before it looks. The rules come from `Config['password']`: six characters, and
the four character-class minima are configurable but unset in the shipped
configuration.

### Rate limits

`src/limits.rs` counts attempts in a five-minute sliding window: five logins per
email, a hundred per client address, three forgot-password requests per email,
thirty per address. A successful login clears its email counter. The email
bounds are the account guard; the address bounds cap one host's throughput. The
counters live in the process and are bounded, evicting the least recently
inserted subject.

### Same origin

`src/origin.rs` compares the request's `Origin`, or its `Referer` when there is
none, against the configured base URL. `omeka-server` runs the check on every
state-changing request that carries the session cookie, because `SameSite=Lax`
is a browser default rather than a server decision. A key-authenticated request
is not checked: its credential is not attached by the browser.

### Tokens

`src/token.rs` draws the session token and the one-time password key: 32
characters from `ApiKey::STRING_CHARLIST`, each equally likely, read from the
operating system's CSPRNG. `Entity\PasswordCreation::setId` reuses `ApiKey`'s
generator upstream, so the two have the same shape.

## Staged uploads and adding a media

`src/uploads.rs` holds a user's own resumable uploads (migration 0009), the
bytes of which sit in the store under `uploads/<userId>/`. It reuses the import
staging area's part mechanics (`src/import/staging/parts.rs`, which takes a
folder root) and answers a row scoped by the user, so another account's upload
is `UploadError::Missing`, which the routes render as 404. A media create that
names a completed upload in `o-module-romeka:upload` first claims it with one
`UPDATE ... WHERE claimed IS NULL` (`uploads::claim`), so two creates cannot
both ingest the same bytes: the loser is `ClaimError::Busy` and the route answers
409. The claim is released when the ingest fails and is reclaimable after
`CLAIM_STALE` (one hour). The bytes then go through `src/import/media_in.rs`'s
`arrive_stored`; the helper is the import's, called as it is.

`src/media_add.rs` holds the two facts an add panel reads before it sends
anything: the upload limits (the file size, the validation lists and the media
kind names) and a probe of a pasted link. The probe fetches through the guarded
client with a 10-second cap, reads at most 1 MiB, names the ingesters that could
read the link, and writes no row.

## o-module extensions

`wire::site::Site` carries an empty `extensions` map as the seam for
`o-module-*` keys. No core module attaches one at the frozen SHA (the
`rep.resource.json` listeners that would do it are not registered), so the
map serializes empty. The brief and DESIGN.md mention the extension block for
sites; there is no upstream core key to mirror here, so the seam is the
honest shape. A module unit fills it later.
