# omeka-migrate

Romeka S is a fork of Omeka S. This crate is the adoption path: migration reads an Omeka S install in MySQL, in
upstream's own schema, and writes the same tables into SQLite. It keeps upstream's table and column names, so a
migrated install reads back the records it held before.

```rust
let report = omeka_migrate::migrate_from_upstream(
    "mysql://user:password@host:3306/omeka",
    std::path::Path::new("romeka.sqlite"),
    &omeka_migrate::MigrateOptions::default(),
)
.await?;
println!("{report}");
```

`migrate_from_upstream(source_url, target_path, options)` does four things.

1. It checks the source's own column list against the columns the port reads, and fails by name on
   the first one the source lacks (`MigrateError::MissingColumn`). The port does not replay
   upstream's `application/data/migrations/`, so a source older than the frozen SHA keeps older
   columns, and the migrator refuses it rather than dropping a field.
2. It reads every table the port serves through `omeka-db`'s MySQL repositories, so MySQL
   datetimes, JSON columns, booleans and binary-collated text decode through the same row structs a
   SQLite read uses.
3. It writes them through `omeka-db`'s SQLite repositories in dependency order, remapping every
   reference through an id map. A target that already holds rows is refused.
4. When `MigrateOptions::source_files` names the source install's `files` directory, it copies each
   `asset` row's stored file from `<source_files>/asset/` into the target store, which defaults to
   `files` beside the target database or `MigrateOptions::target_files` when set. A source asset
   whose file is missing stops the run by name (`MigrateError::MissingAssetFile`).

`MigrateOptions { dry_run: true, .. }` stops after the mapping. The report names what a run would
write, the target is never opened, and no file is copied.

The report is one row per table.

```
table                       read  written  skipped
user                           2        2        0
...
total                         11       11        0
```

## Table mapping

Every port table keeps its upstream name. The write path is the repository that owns it.

| upstream table | write path |
| --- | --- |
| `user` | `UserRepo::create` |
| `asset` | `AssetRepo::create`, then the file copy above |
| `password_creation` | `PasswordCreationRepo::create` |
| `vocabulary` | `VocabularyRepo::create` |
| `resource_class` | `ClassRepo::create` |
| `property` | `PropertyRepo::create` |
| `resource_template` | `TemplateRepo::create` |
| `resource_template_property` | `TemplateRepo::add_property` |
| `resource`, `item` | `ItemRepo::create` |
| `resource`, `item_set` | `ItemSetRepo::create` |
| `resource`, `media` | `MediaRepo::create` |
| `value` | `ValueRepo::create` |
| `item_item_set` | `ItemRepo::add_to_item_set` |
| `site` | `SiteRepo::create`, then `SiteRepo::update` for the homepage and navigation |
| `site_page` | `SiteRepo::add_page` |
| `site_page_block` | `SiteRepo::add_block` |
| `site_block_attachment` | `SiteBlockAttachmentRepo::create` |
| `site_item_set` | `SiteRepo::add_item_set` |
| `site_permission` | `SiteRepo::set_permission` |
| `item_site` | `ItemSiteRepo::create` |
| `setting` | `SettingRepo::set` |
| `site_setting` | `SiteSettingRepo::set` |
| `user_setting` | `UserSettingRepo::set` |
| `api_key` | `ApiKeyRepo::create` |

An item, an item set and a media row each share their id with a `resource` row, so the migrator
reads all three kinds and writes each through its own repository, which writes the `resource` half.

## Column mapping

Most columns copy straight across. The exceptions are below.

- **Ids.** `omeka-db` assigns the target its own ids, so a source id never travels to the target as
  a row key. The migrator keeps a source id to target id map and rewrites every reference:
  `owner_id`, `resource_class_id`, `resource_template_id`, `thumbnail_id`, `item_id`, `resource_id`,
  `value_resource_id`, `property_id`, `vocabulary_id`, `site_id`, `page_id`, `item_set_id`,
  `block_id`, `media_id`, `user_id`, `homepage_id`, `primary_media_id` and the page ids inside
  `site.navigation`.
- **`created`.** Copied into the target `created`.
- **`modified`.** Not carried. A repository create writes one instant into both `created` and
  `modified`, so the target `modified` equals the source `created`.
- **`resource.thumbnail_id` and `site.thumbnail_id`.** Remapped through the asset map. An asset
  whose row was skipped reads as absent, the way the source's `ON DELETE SET NULL` would leave it.
- **`item.primary_media_id`.** Remapped and set after the media pass, because it points forward at a
  media row.
- **`site.homepage_id`.** Remapped and set after the page pass, for the same reason.
- **`site.navigation`.** Copied, with the page id under `data.id` of every `page` link rewritten to
  the target page id. A link of another type is copied unchanged.
- **`site.item_pool`.** Copied unchanged. It holds query parameters, not ids.
- **Ids inside `site_page_block.data` and `layout_data`.** The `asset` block's slot `id` (asset) and
  `page` (page) and the `listOfPages` `pagelist` page ids are rewritten, and so is
  `layout_data.background_image_asset`. Every other id a block stores is left as the source wrote
  it; see the losses below.
- **`site_block_attachment`.** `block_id`, `item_id` and `media_id` are remapped. An attachment
  whose block, item or media was skipped is skipped.
- **`setting`, `site_setting`, `user_setting`.** Copied as rows, one per key, with the JSON value
  unchanged. A `site_setting` or `user_setting` whose site or user was skipped is skipped.
- **`item_site`.** Copied as rows with both ids remapped.
- **`password_creation`.** Copied with `user_id` remapped and `created` and `activate` as stored.
- **`value.value_annotation_id`.** Dropped. See the losses below.
- **`api_key.id`.** A string key, kept as is.
- **`password_hash`, `credential_hash`.** Copied as is. The hashes are the source's own and the
  port never checks them here.
- **JSON columns** (`media.data`, `site_page_block.data`, `site_page_block.layout_data`,
  `site_page.layout_data`, `resource_template_property.data_type`). Copied as parsed JSON.
- **Datetimes.** Read as MySQL `datetime` and written as the port's `YYYY-MM-DD HH:MM:SS` text.

## Known losses

1. **Upstream ids.** The target renumbers every row. Every reference is remapped, so the graph is
   intact, but an external link to `o:id` 42 will not find the same row.
2. **`modified` timestamps.** See above. An item or site whose forward reference is set by an
   `update` gets `modified` set to the migration time.
3. **Asset files without a source store.** `asset` rows always move. The files move only when
   `MigrateOptions::source_files` names the source install's `files` directory. Without it, a
   migrated thumbnail points at a URL the port does not serve, and a thumbnail URL an import wrote
   into a JSON column is not copied either.
4. **Value annotations.** `omeka-db` creates a `value_annotation` but has no read for one, so the
   migrator cannot enumerate them. Every `value.value_annotation_id` is dropped and the annotation
   resources are not written.
5. **Ids inside block `data` JSON the migrator does not know.** The `asset` and `listOfPages`
   layouts' asset and page ids are rewritten, but a `browsePreview` `query` that names an
   `item_set_id`, or any id another layout or a module stores in its `data`, is left as the source
   wrote it. An id whose row was skipped is left too, the way `site.navigation` leaves a missing
   page.
6. **Tables the migrator does not copy.** `module`, `job`, `session`, `migration` and
   `fulltext_search` stay behind. `module` names upstream module code the port does not load, so a
   copied row would claim a module the port cannot run. `job` rows name a class and a process that
   only existed on the source, and an unfinished one would be re-run on the target. `session` holds
   PHP-serialized payloads the port cannot restore. `migration` is upstream's own ledger, and the
   port keeps its own. `fulltext_search` is an index the port rebuilds with its own job. `omeka-db`
   has a repository for `module` and `job`; the migrator chooses not to call them.
7. **Memory.** The read pass holds the whole source in memory before the write pass, so a very large
   install needs a matching amount of RAM.

## Running the tests

```sh
export CARGO_TARGET_DIR=/mnt/Ghar/2TA/DevStuff/notebook/omeka-research/cargo-s2/site-migrate
cargo test -p omeka-migrate
```

The SQLite-side tests need no server. They cover the pure mapping functions, the block and layout
id rewrites, the asset file copy, and a whole write pass over a hand-built source with gaps in every
id sequence, which is what proves the remap.

The MySQL suite is gated on a reachable server, the same way `omeka-db`'s MySQL suite is.
`ROMEKA_MYSQL_URL` names a server, not a database. Each test creates its own database from upstream's
own schema, seeds a fixture through the port's repositories, and drops the database again. The
render test seeds a site with a thumbnail, an asset block, a media block with an attachment, a block
background asset, an item in the site pool and site settings, migrates it with the source store, and
drives the port's router through `/s` to read the result.

```sh
docker run -d --name omeka-upstreammigrate-mysql \
  -e MYSQL_ROOT_PASSWORD=secret -p 127.0.0.1:13317:3306 \
  mysql:8.0 --default-authentication-plugin=mysql_native_password

ROMEKA_MYSQL_URL=mysql://root:secret@127.0.0.1:13317 \
  cargo test -p omeka-migrate --test mysql
```

The same suite passes against MariaDB 11, using the MariaDB container's port in place of 13317.

With no variable set, or a server that does not answer, the helper prints `SKIP` with the reason and
each test returns.

## Follow-ups

- `romeka migrate-from-upstream` as a CLI command. This crate exposes the function; `omeka-cli` owns
  the command surface.
- The port-versus-upstream differential `ROADMAP.md` wave 9 names, which diffs both APIs row by row
  after an import.
