# API golden fixtures

These files freeze the `/api` JSON-LD shape for wave 2. Each one parses into
its wire type in `crates/omeka-api/src/wire/`, serializes back through
`omeka_api::jsonld::to_string_pretty`, and byte-equals the file
(`crates/omeka-api/tests/golden.rs`).

Everything is derived from `source/omeka-s` at the frozen SHA
(`5e413f009c447f4b587803ab23a9cec7d228f6e8`). The notes below name the class
and method each member comes from. Nothing here was captured from a running
server: `measured` means read from the PHP source, `inferred` means a choice
about entity data that the source does not pin down.

## Common conventions

- Base URL `http://localhost`. Every `@id`, `@context`, and generated link is
  built with `force_canonical` (`AbstractResourceRepresentation::apiUrl`,
  `AbstractResourceRepresentation::jsonSerialize`), so the fixtures spell the
  origin out.
- Entity ids (property ids, resource class ids, user id) come from a default
  install. Property ids follow the order of the properties in
  `application/data/vocabularies/dcterms.rdf` (`title` 1, `creator` 2,
  `description` 4, `source` 11, `relation` 13 — inferred), and `dctype:Image`
  is resource class 4, matching `dctype.rdf` order (inferred).
- Timestamps use PHPs `DateTime::format('c')`.
  `Stdlib\DateTime::jsonSerialize` produces `2024-03-01T12:00:00+00:00`.
- `thumbnail_display_urls` keys come from `thumbnails.types` in
  `application/config/module.config.php`: `large`, `medium`, `square`
  (measured). `AbstractRepresentation::thumbnailDisplayUrl` returns the
  custom thumbnail asset URL when one exists, else the primary media's
  `thumbnailUrl($type)`, else `null`. The URL itself is
  `AbstractResourceRepresentation::getFileUrl`:
  `<base>/files/<prefix>/<name>.<ext>`.
- Value objects are `DataType\Literal::getJsonLd`, `DataType\Uri::getJsonLd`,
  or the resource union in
  `AbstractResourceEntityRepresentation::valueRepresentation()`, merged into
  the `ValueRepresentation::jsonSerialize` base through PHP `+`. The base
  members (`type`, `property_id`, `property_label`, `is_public`,
  `@annotation`) always come first; `@id` overrides nothing and lands after
  them for resource values (measured).
- Files here use `serde_json` escaping. PHPs renderer
  (`Laminas\View\Renderer\JsonRenderer` via `Laminas\Json\Json::encode`)
  escapes `/` and hex-escapes HTML characters in addition. The differential
  normalizer compares parsed documents, not raw bytes, so the fixtures carry
  the unescaped form; the byte-equality in the golden test is between the
  fixture and this port's serializer.
- Members typed `serde_json::Value` (`o:ini`, `o:args`, `o:item_pool`,
  `o:layout_data`, media `data`) keep PHP insertion order because `omeka-api`
  enables `serde_json`'s `preserve_order`. `module.json` is the fixture that
  pins this: `o:ini` is not alphabetical.

## item.json

`ItemRepresentation` (id 10) with a resource class, a template, one media,
one item set, and four value types.

- Member order: `AbstractResourceRepresentation::jsonSerialize` (`@context`,
  `@id`, `@type`, `o:id`) then
  `AbstractResourceEntityRepresentation::getJsonLd` (`o:is_public` through
  `thumbnail_display_urls`, the two dates, then the resource block, then
  values, then `@reverse`) merged with
  `ItemRepresentation::getResourceJsonLd` (`o:primary_media`, `o:media`,
  `o:item_set`, `o:site`) between the dates and the values (measured).
- `@type` is a two-element list because a resource class is set:
  `getJsonLdType` appends `term()` (`dctype:Image`), measured.
- `o:thumbnail` is `null` (no custom thumbnail); the three display URLs come
  from primary media `5`'s `thumbnailUrl($type)`, i.e.
  `files/thumbnails/<type>/<storageId>.jpg` (measured).
- Values exercise all three value shapes: literal with `@language`, literal
  with an `@annotation` (it is itself a value list), `resource:item` pointing
  at item 11 with `url: null` (an API request is neither an admin nor a site
  request, so `AbstractResourceRepresentation::url()` returns null —
  measured), and `uri` with `@id`/`o:label` (measured).
- `@reverse` uses the `resources` segment and `o:title`, per
  `subjectValuesForReverse` (measured). The reverse entry points at a
  `resources` id because that is the adapter the method picks.
- The two `DateTime` values are the `{"@value", "@type"}` wrapper, not a
  plain string (only users differ; see below; measured).

## item_set.json

`ItemSetRepresentation` (id 1). Adds `o:is_open` and the `o:items` search
link built by `getResourceJsonLd` with `?item_set_id=1` (measured). No class,
no template, no media: `thumbnail_display_urls` has all three keys with
`null` values, which is what the loop over thumbnail types emits when
`thumbnailDisplayUrl` returns null (measured). `o:modified` is present with
`null`.

## media.json

`MediaRepresentation` (id 5, item 10, upload ingester).

- `o:ingester` `upload`, `o:renderer` `file` (`Media\Ingester\Upload::getRenderer`,
  measured; `MediaAdapter::hydrate` stores it on create).
- `o:item` is a `ResourceReference` (`@id`, `o:id`), measured.
- `o:size` is an int when numeric, else null (`is_numeric(...) ? (int) : null`,
  measured).
- `o:filename` is the storage id plus the extension; `storageId` is 40 hex
  characters (inferred as entity data).
- `o:original_url` and `o:thumbnail_urls` are `getFileUrl` results;
  `thumbnail_urls` is empty (`[]` in PHP) when the media has no thumbnails —
  this fixture has thumbnails so all three keys are present (measured). Note
  for wave 2: an empty PHP list serializes as `[]`, while an empty Rust map
  serializes as `{}`; the port must decide the empty-thumbnails shape then.
- `data` is the raw `data` column. Doctrine 2's `JsonArrayType` hydrates a NULL
  column as `[]`, so a media that stored no payload answers `[]`, not `null`
  (measured: the oracle answers `[]` for a media whose `media.data` is NULL, and
  `Ingester\Upload` stores none).
- `thumbnail_display_urls` mirrors `thumbnail_urls` because
  `MediaRepresentation::primaryMedia()` returns `$this` (measured).

## vocabulary.json

`VocabularyRepresentation` (id 1). No `o:is_public`, no dates: the class does
not extend the resource entity representation (`VocabularyRepresentation::getJsonLd`,
measured). `o:label` and `o:comment` repeat what the installer read from
`application/data/vocabularies/dcterms.rdf` (inferred as entity data).

## resource_template.json

`ResourceTemplateRepresentation` (id 2) with three properties.

- Member order from `getJsonLd` (measured): `o:label`, `o:owner`,
  `o:resource_class`, `o:title_property`, `o:description_property`,
  `o:resource_template_property`.
- Each entry of `o:resource_template_property` is
  `ResourceTemplatePropertyRepresentation::jsonSerialize` (measured):
  `o:property`, `o:alternate_label`, `o:alternate_comment`, `o:data_type`,
  `o:is_required`, `o:is_private`, `o:default_lang`. `o:data_type` is a list
  because the entity stores it as a JSON array; unknown names are dropped
  (`dataTypes()`, measured). The third entry shows a `resource:item` data
  type.

## site.json

`SiteRepresentation` (id 1), a site a user created after install. Core ships
no default site (measured: nothing in `application/src` creates one), so the
entity data here is inferred.

- Member order from `SiteRepresentation::getJsonLd` (measured): `o:slug`,
  `o:theme`, `o:title`, `o:summary`, `o:thumbnail`, `o:navigation`,
  `o:homepage`, `o:item_pool`, `o:owner`, `o:created`, `o:modified`,
  `o:is_public`, `o:assign_new_items`, `o:page`, `o:site_permission`,
  `o:site_item_set`, `o:site_item`.
- `o:navigation` is the stored `site.navigation` JSON array (measured:
  `SiteRepresentation::navigation()` returns `Site::getNavigation()`). The
  default entry's shape (`type`, `data`, `links`) is
  `SiteAdapter::getDefaultNavigation` (measured); `data` differs per type, so
  it stays untyped in `wire::common::NavLink`.
- `o:item_pool` is the stored `site.item_pool` JSON array (measured). A site
  with no filters stores an empty array, which is the `[]` here (inferred).
- `o:homepage` and `o:page` are `site_pages` references; `o:site_item` is the
  `?site_id=1` search link built in `SiteRepresentation::getJsonLd`
  (measured).
- `o:site_permission` and `o:site_item_set` use
  `SitePermissionRepresentation::jsonSerialize` (`o:user`, `o:role`) and
  `SiteItemSetRepresentation::jsonSerialize` (`o:item_set`), measured. Both
  are empty here.
- No `o-module-*` members: no core module attaches any at the frozen SHA, so
  the `extensions` seam serializes nothing.

## user.json

`UserRepresentation` (id 1). Order from `getJsonLd` (measured): `o:name`,
`o:email`, `o:created`, `o:role`, `o:is_active`.

- `o:created` is a plain ISO 8601 string, not a `{"@value", "@type"}`
  object: `getDateTime` returns the `Stdlib\DateTime` wrapper and its
  `jsonSerialize` returns `format('c')` (measured). This is the one place a
  date is not a JSON-LD value object in these fixtures.
- `o:role` is one of the `User` role constants; `global_admin` here
  (measured set, choice inferred).

## asset.json

`AssetRepresentation` (id 1).

- Order from `getJsonLd` (measured): envelope `@context`, `@id`, `@type`,
  `o:id`, then `o:owner`, `o:name`, `o:filename`, `o:media_type`,
  `o:asset_url`, `o:alt_text`.
- `getJsonLd` lists `o:id` again, but the envelope already set it and PHP
  `array_merge` keeps the first occurrence, so `o:id` appears once
  (measured).
- `o:name`, `o:media_type` are non-null columns (`Entity\Asset`: `name` and
  `mediaType` are plain `@Column`), and `o:filename` is built from the
  non-null `storageId` plus extension (`getFilename`, measured).
- `o:asset_url` is `getFileUrl('asset', filename)` (measured).

## property.json and resource_class.json

`PropertyRepresentation` (id 1) and `ResourceClassRepresentation` (id 4).
Both extend `AbstractVocabularyMemberRepresentation`, so the child order is
`getJsonLd` there (measured): `o:local_name`, `o:label`, `o:comment`,
`o:term`, `o:vocabulary`; the envelope adds `@context`, `@id`, `@type`,
`o:id` (`AbstractResourceRepresentation::jsonSerialize`, measured).
`o:term` is `prefix:localName` (`term()`, measured). Vocabulary ids 1
(dcterms) and 2 (dctype) and the labels are inferred entity data.

## job.json

`JobRepresentation` (id 1).

- Order (measured): `o:status`, `o:job_class`, `o:args`, `o:owner`, then the
  `o:started` / `o:ended` value objects, merged that way by `getJsonLd`.
- `o:status` is the string constant from `Entity\Job::STATUS_*`
  (`completed` here), not a number (measured).
- `o:args` is the raw args column (measured); `o:ended` is present only when
  set, and the key is kept with `null` otherwise (measured).
- `o:job_class` is a PHP class name, so the JSON text carries backslashes.

## value_annotation.json

`ValueAnnotationRepresentation` (id 1). It extends the resource entity
representation and returns `[]` from `getResourceJsonLd` (measured), so the
document is exactly `AbstractResourceEntityRepresentation::getJsonLd`: the
common block, dates, values, and no resource-specific members. `o:title`
mirrors the `dcterms:title` value.

## module.json

`ModuleRepresentation` (`o:id` "Common").

- Order (measured): `o:state`, `o:ini`, after the envelope.
- `o:id` is the module directory name (measured, `Module\Module::getId`).
- `o:state` is a string from `Module\Manager::STATE_*` (`active`,
  `not_active`, `not_installed`, `not_found`, `invalid_module`,
  `invalid_ini`, `invalid_omeka_version`, `needs_upgrade`), measured. It is
  not the numeric state some ports assume.
- `o:ini` is the parsed `module.ini` array (measured). Key order is the file
  order, which is why this fixture pins `serde_json`'s `preserve_order`.

## api_resource.json and data_type.json

`ApiResourceRepresentation` (`o:id` "items") and `DataTypeRepresentation`
(`o:id` "literal").

- `ApiResourceAdapter` lists registered resource names; the envelope `@id` is
  `/api/api_resources/<name>` and the child adds `o:id` with the same string
  (measured entry of `getJsonLd`; the two ids are equal, and PHP
  `array_merge` collapses them, same as `asset.json`).
- `DataTypeRepresentation::getJsonLd` adds `o:id` (the registered name) and
  `o:label` from the data type class (measured; `DataType\Literal::getLabel`
  returns "Text").
