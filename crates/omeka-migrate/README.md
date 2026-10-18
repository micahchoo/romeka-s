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

`migrate_from_upstream(source_url, target_path, options)` does three things.

1. It checks the source's own column list against the columns the port reads, and fails by name on
   the first one the source lacks (`MigrateError::MissingColumn`). The port does not replay
   upstream's `application/data/migrations/`, so a source older than the frozen SHA keeps older
   columns, and the migrator refuses it rather than dropping a field.
2. It reads every table the port serves through `omeka-db`'s MySQL repositories, so MySQL
   datetimes, JSON columns and booleans decode through the same row structs a SQLite read uses.
3. It writes them through `omeka-db`'s SQLite repositories in dependency order, remapping every
   reference through an id map. A target that already holds rows is refused.

`MigrateOptions { dry_run: true }` stops after the mapping. The report names what a run would write
and the target is never opened.

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
| `site_item_set` | `SiteRepo::add_item_set` |
| `site_permission` | `SiteRepo::set_permission` |
| `api_key` | `ApiKeyRepo::create` |

An item, an item set and a media row each share their id with a `resource` row, so the migrator
reads all three kinds and writes each through its own repository, which writes the `resource` half.

## Column mapping

Most columns copy straight across. The exceptions are below.

- **Ids.** `omeka-db` assigns the target its own ids, so a source id never travels to the target as
  a row key. The migrator keeps a source id to target id map and rewrites every reference:
  `owner_id`, `resource_class_id`, `resource_template_id`, `item_id`, `resource_id`,
  `value_resource_id`, `property_id`, `vocabulary_id`, `site_id`, `page_id`, `item_set_id`,
  `user_id`, `homepage_id`, `primary_media_id` and the page ids inside `site.navigation`.
- **`created`.** Copied into the target `created`.
- **`modified`.** Not carried. A repository create writes one instant into both `created` and
  `modified`, so the target `modified` equals the source `created`.
- **`resource.thumbnail_id` and `site.thumbnail_id`.** Dropped. The port has no `asset` repository.
- **`item.primary_media_id`.** Remapped and set after the media pass, because it points forward at a
  media row.
- **`site.homepage_id`.** Remapped and set after the page pass, for the same reason.
- **`site.navigation`.** Copied, with the page id under `data.id` of every `page` link rewritten to
  the target page id. A link of another type is copied unchanged.
- **`site.item_pool`.** Copied unchanged. It holds query parameters, not ids.
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
3. **Assets.** `asset` rows do not move, so `resource.thumbnail_id` and `site.thumbnail_id` are
   dropped. A thumbnail URL an import wrote into a JSON column still points at a file the port did
   not copy.
4. **Value annotations.** `omeka-db` creates a `value_annotation` but has no read for one, so the
   migrator cannot enumerate them. Every `value.value_annotation_id` is dropped and the annotation
   resources are not written.
5. **`site_block_attachment`.** The migrator does not copy these rows. `omeka-db` now reads and
   writes them, so a migrated `itemWithMetadata` or `media` block renders empty only until this
   step is built.
6. **Ids inside block `data` JSON.** Only `site.navigation` page ids are rewritten. Any other id a
   block stores in its `data` is left as the source wrote it.
7. **`fulltext_search`.** Not migrated. The port rebuilds the index with its own job.
8. **Repo-less tables.** `setting`, `module`, `job`, `session`, `asset`, `site_block_attachment`,
   `password_creation`, `user_setting`, `site_setting` and `item_site` have no repository, so they do
   not move. Site and user settings are the notable omission; the port has no settings read or write
   yet.
9. **Memory.** The read pass holds the whole source in memory before the write pass, so a very large
   install needs a matching amount of RAM.

## An `omeka-db` decoding gap this unit found

`property.local_name` and `resource_class.local_name` are `varchar(190) ... utf8mb4_bin`. sqlx
reports a binary-collated text column to the row layer as `VARBINARY`, and `omeka-db`'s MySQL text
reader decodes a `String`, so `PropertyRepo::list` and `ClassRepo::list` fail against a real server
with `Column { column: "local_name", reason: "mismatched types; Rust type String (as SQL type
VARCHAR) is not compatible with SQL type VARBINARY" }`. Measured on MySQL 8.0.46.

Until `omeka-db` decodes a binary-collated text column, this crate reads those two tables itself with
`CAST(local_name AS CHAR)` and parses each row through the same `from_db_row`. The proposed
`omeka-db` fix is in the unit report: `Backend::text` for MySQL should fall back to decoding
`Vec<u8>` and converting to UTF-8, as the row layer already does for a SQLite row that holds bytes.

## Running the tests

```sh
export CARGO_TARGET_DIR=/mnt/Ghar/2TA/DevStuff/notebook/omeka-research/cargo-s2/upstreammigrate
cargo test -p omeka-migrate
```

The SQLite-side tests need no server. They cover the pure mapping functions and a whole write pass
over a hand-built source with gaps in every id sequence, which is what proves the remap.

The MySQL suite is gated on a reachable server, the same way `omeka-db`'s MySQL suite is.
`ROMEKA_MYSQL_URL` names a server, not a database. Each test creates its own database from upstream's
own schema, seeds a small fixture through the port's repositories, and drops the database again.

```sh
docker run -d --name omeka-upstreammigrate-mysql \
  -e MYSQL_ROOT_PASSWORD=secret -p 127.0.0.1:13317:3306 \
  mysql:8.0 --default-authentication-plugin=mysql_native_password

ROMEKA_MYSQL_URL=mysql://root:secret@127.0.0.1:13317 \
  cargo test -p omeka-migrate --test mysql
```

With no variable set, or a server that does not answer, the helper prints `SKIP` with the reason and
each test returns.

## Follow-ups

- `romeka migrate-from-upstream` as a CLI command. This crate exposes the function; `omeka-cli` owns
  the command surface.
- The port-versus-upstream differential `ROADMAP.md` wave 9 names, which diffs both APIs row by row
  after an import.
- The `omeka-db` decoding fix above, after which the two cast reads can go.
