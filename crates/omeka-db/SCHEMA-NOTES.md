# SCHEMA-NOTES

Every deliberate divergence between `crates/omeka-db/migrations/0001_init.sql` and upstream Omeka S
`application/data/install/schema.sql` (revision `5e413f0`, 390 lines). Anything not listed here is a
deliberate copy of the upstream shape: table names, column names, column order, unique keys, indexes
and foreign key actions.

## Two schemas, one per engine

`migrations/0001_init.sql` is the SQLite translation described below.
`migrations-mysql/0001_init.sql` is upstream `schema.sql` verbatim, with `CREATE TABLE IF NOT EXISTS`
added on every statement, so the migration is a no-op against an Omeka S database that already
exists. Nothing is translated there: on MySQL the port's tables, columns, indexes and foreign keys
are the install's own. The type and divergence tables below describe what the SQLite schema had to
change to say the same thing. `README.md` documents the backend seam.

The port's own SQL writes identifiers bare. MariaDB reserves `VALUE`, so the MySQL backend quotes
the `value` table in its `INSERT INTO` (error 1064 otherwise); SQLite, MySQL 8 and MariaDB 11 all
accept every other name the repositories write without quotes.

## Migration semantics

`0001_init.sql` translates `schema.sql` alone. `schema.sql` is the full install schema of the current
release, and the 37 files in `application/data/migrations/` are upgrade paths for installs that
predate it.

Evidence, measured:

- Every `ALTER`/`CREATE` statement in the migrations was extracted and matched against `schema.sql`.
  Each column, index and foreign key the migrations add is already present in `schema.sql`.
- The newest migration, `20240614123811_MigrateDefaultToPrivateSetting.php`, contains no DDL at all.
  It rewrites `setting` rows (`default_to_private` to four granular keys) through the settings service.
- Consequence for this port: there is nothing to replay, and the port ships a single initial schema.
  Upstream's own `migration` table is therefore created empty (see divergence 11), and a re-run of
  `migrate` is a no-op because sqlx records the single applied migration.

## Type translation

| Upstream | SQLite | Notes |
| --- | --- | --- |
| `int AUTO_INCREMENT` (PK) | `INTEGER PRIMARY KEY AUTOINCREMENT` | divergence 1 |
| `int` / `unsigned` columns | `INTEGER` | divergence 2 |
| `varchar(255)`, `varchar(190)`, `varchar(32)`, `longtext` | `TEXT` | divergence 2 |
| `datetime` | `TEXT`, `YYYY-MM-DD HH:MM:SS`, UTC | divergence 3 |
| `longtext` with `(DC2Type:json_array)` | `TEXT`, JSON, parsed by `serde_json` | divergence 4 |
| `tinyint(1)` | `INTEGER` 0/1 with `CHECK (col IN (0, 1))` | divergence 5 |
| `varchar(255)` `resource_type` | `TEXT` with a `CHECK` on the four Doctrine values | divergence 6 |
| `longblob` (`session.data`) | `BLOB` | no length, divergence 2 |
| `varbinary(16)` (`api_key.last_ip`) | `BLOB` | no length, divergence 2 |
| `FULLTEXT KEY (title, text)` | FTS5 external-content table + triggers | divergence 9 |

## Divergences

1. **Integer primary keys.** SQLite needs `INTEGER PRIMARY KEY` for an alias of `rowid`, and
   `AUTOINCREMENT` to forbid id reuse. Upstream `int AUTO_INCREMENT` is not necessarily unsigned for
   every table, so the port does not widen or narrow; ids stay in the signed 64 bit range and the row
   layer rejects a negative id as a `DbError::Column`.
2. **Text and integer affinities, and lengths.** SQLite has neither `varchar(n)` nor unsigned
   integers. Length limits are not enforced. This is safe for a port: upstream also relies on the
   application to bound lengths, and no unique key depends on a prefix.
3. **Datetimes are text.** Upstream `datetime` has second precision. The port stores
   `YYYY-MM-DD HH:MM:SS` in UTC in a `TEXT` column, formats it through `src/time.rs`, and floors the
   clock to a whole second (`time::now`) so the struct a create returns equals the row a later read
   parses back. MySQL zero-dates (`0000-00-00`) are not produced.
4. **JSON columns are text.** The `(DC2Type:json_array)` columns (`navigation`, `item_pool`, `data`,
   `layout_data`, `args`, `data_type`, `setting.value`, `site_setting.value`, `user_setting.value`)
   store JSON text. The row layer parses with `serde_json`; `data_type` is typed as
   `Option<Vec<String>>`.
5. **Booleans.** `tinyint(1)` becomes `INTEGER` with `CHECK (col IN (0, 1))`. MySQL accepts 2 there
   with a warning; the port rejects it. This is the one place the port is stricter than upstream.
6. **`resource_type`.** Upstream stores the Doctrine discriminator in a plain `varchar(255)` and
   validates in PHP (`resource_discriminator_map`). The port adds
   `CHECK (resource_type IN ('Omeka\Entity\Item', 'Omeka\Entity\ItemSet', 'Omeka\Entity\Media',
   'Omeka\Entity\ValueAnnotation'))`, freezing the four kinds the port writes.
7. **Collations.** Upstream's default is `utf8mb4_unicode_ci`, a case- and accent-insensitive
   collation. The port applies `COLLATE NOCASE` to the columns where that matters: every column
   under a unique key, and every column the repositories compare by equality. `NOCASE` folds the
   ASCII letters A-Z only, so a non-ASCII case pair (`İ` and `i`) is distinct where upstream folds
   it. SQLite has no built-in Unicode collation; closing that gap needs a custom collation and is a
   follow-up. `password_creation.id` is upstream `utf8mb4_bin` and stays case-sensitive, matching
   upstream. `property.local_name` and `resource_class.local_name` are `utf8mb4_bin` upstream and
   binary here (SQLite's default `BINARY`) from `migrations/0004_term_local_name_binary.sql`; 0001
   gave them `NOCASE`, which refused FOAF's `givenName` beside `givenname`. Measured on the oracle
   database (`information_schema.columns`), those two and `password_creation.id` are upstream's
   only binary columns. The columns that carry `NOCASE` are:
   `user.email`, `vocabulary.namespace_uri`, `vocabulary.prefix`, `resource_template.label`, `asset.storage_id`, `media.storage_id`,
   `site.slug`, `site_page.slug`, `setting.id`, `module.id`, `api_key.id`, `site_setting.id`,
   `user_setting.id`, `session.id`, `migration.version`, `fulltext_search.resource`.
   Other `utf8mb4_unicode_ci` columns (for example `value.type`, `job.class`) have no uniqueness or
   equality match in this unit, so they are stored without an explicit collation.
8. **Index names.** Upstream mostly uses hashed Doctrine names (`IDX_...`, `UNIQ_...`) plus a few
   logical names (`title`, `value`, `uri`, `is_public`, `media_type`, `item_position`,
   `page_position`, `block_position`, `position_site_item_set`, `is_public_site_page`,
   `value_annotation_id`). The port keeps the logical names and gives the remaining indexes a
   readable `ix_<table>_<column>` name. One rename is forced: upstream's index on `value.value` is
   named `value`, which collides with the `value` table because SQLite keeps tables and indexes in
   one namespace (`sqlite3` reports "there is already a table named value"). That index is
   `ix_value_value`.
9. **Full text search.** Upstream declares a MySQL `FULLTEXT KEY` on `fulltext_search (title, text)`.
   SQLite has no `FULLTEXT` index type, so the port adds an FTS5 external-content table
   `fulltext_search_fts` with three triggers that mirror insert, update and delete on
   `fulltext_search`. A `MATCH` query joins the FTS table back to `fulltext_search` on `rowid`. FTS5
   is compiled into the bundled `libsqlite3-sys` (`-DSQLITE_ENABLE_FTS5`) and a test exercises the
   synchronisation; the FTS shadow tables (`fulltext_search_fts_data` and friends) are infrastructure,
   not upstream tables.
10. **Prefix indexes.** Upstream indexes `resource.title(190)`, `value.value(190)` and
    `value.uri(190)` are prefix indexes on long text. SQLite supports neither prefix lengths nor a
    `longtext` distinction, so all three are full-column indexes.
11. **Migration bookkeeping.** sqlx creates `_sqlx_migrations` for its own records. Upstream's
    `migration` table is still created, empty, so an export or a later MySQL import keeps the same
    table set.
12. **Forward foreign keys.** `site.homepage_id` references `site_page`, which upstream creates
    after `site`, and `item.primary_media_id` references `media`. SQLite allows a reference to a
    table that does not exist yet at `CREATE TABLE` time, so no `PRAGMA foreign_keys=OFF` window is
    needed (upstream wraps `schema.sql` in `SET FOREIGN_KEY_CHECKS = 0`).
13. **Connection pragmas.** `PRAGMA foreign_keys = ON` is set on every pooled connection (SQLite
    defaults to OFF, MySQL InnoDB always checks). File databases also get `journal_mode = WAL` and a
    `busy_timeout`.

## Ordering

Doctrine `@OrderBy({"position" = "ASC"})` (`Entity/Item.php` media, `Entity/ResourceTemplate.php`
properties, `Entity/Site.php` item sets, `Entity/SitePage.php` blocks,
`Entity/SitePageBlock.php` attachments) becomes a plain `ORDER BY position, id`. MySQL sorts NULL
first in ascending order and so does SQLite, so an unpositioned child leads the list. The id, which
is not part of upstream's annotation, breaks ties deterministically.

## Delete semantics

The database declares only part of the cascade upstream expects. The port keeps the database's
`ON DELETE` actions exactly and performs the Doctrine-level removals in the repository, inside one
transaction:

- Database level: `resource` cascades to `item`, `item_set`, `media`, `value_annotation`;
  `item_item_set`, `item_site`, `site_item_set`, `site_permission`, `site_setting`, `user_setting`
  and `password_creation` cascade from their parents; `value.value_resource_id` cascades; and the
  `SET NULL` actions on owner, class, template, thumbnail, homepage and primary media are kept.
- Doctrine level (no database action, so the repository does them): deleting a resource removes its
  `value` rows first; deleting a vocabulary removes its classes and properties; deleting a user
  removes their `api_key` rows; deleting a site removes its pages, blocks, block attachments, item
  set memberships and permissions; deleting a template removes its property rows.

`Entity/Resource.php` owns values (`cascade={"remove"}`), and the port mirrors the order: values
first, then `resource`, so both the value's `resource_id` and any pointing value's
`value_resource_id` are gone before the row disappears.

## Module tables

`0002_custom_vocab.sql` adds `custom_vocab`, the table the Custom Vocab module (v2.1.0,
`Module::install`) creates, because the port builds term lists into core. The SQLite file applies
the translation rules above: `terms` and `uris` are `(DC2Type:json)` text, `label` collates NOCASE
and stays unique, and both foreign keys keep `ON DELETE SET NULL`. The MySQL file is the module's
DDL with its two `ADD CONSTRAINT` statements folded into `CREATE TABLE IF NOT EXISTS`, so an install
that already runs the module keeps its table and rows. Deleting a list is a repository-level removal:
`CustomVocabRepo::delete` sets the list's values back to `literal` and removes its
`customvocab:<id>` name from every template row, in one transaction.

## Romeka tables

`0003_publication.sql` adds the two tables publishing needs. Upstream has neither and reads neither, so
an install that shares the database sees its own tables unchanged. Both names start with `romeka_`.

- `romeka_publication`: one row per term list or vocabulary with Romeka state. `custom_vocab_id` and
  `vocabulary_id` name the target (a `CHECK` allows at most one) and go to `NULL` when upstream
  deletes it, so the published versions keep serving. `slug` is unique across both kinds, because
  both are served under `/vocab/<slug>/`. `terms` is JSON: each term's minted local name,
  deprecation and authority links, keyed by the term.
- `romeka_publication_version`: one row per publish, `UNIQUE (publication_id, version)`, cascading
  from its record. `snapshot` is the published scheme as JSON. `PublicationRepo` has no statement
  that updates or deletes a version.

The MySQL file is the same schema with `IF NOT EXISTS`, InnoDB and utf8mb4, as `0002` writes.

`0005_archive_import.sql` added the v1 archive importer's two tables. `0006_import_v2.sql` drops
them with their rows (no install held an import worth keeping) and creates import v2's five
tables, also with the `romeka_` prefix and read by no upstream code.

- `romeka_import`: one row per import, `draft` to `undone`. `sources`, `plan`, `ran_plan` and
  `summary` are JSON. `owner_id`, `job_id` and `copied_from` go to `NULL` when the user, job or
  source import is deleted. The summary also holds the worker's `heartbeat` and, once a reader
  finds the worker dead, `died` (`src/repos/import_lifecycle.rs`). SQLite and MySQL both carry a
  `CHECK` on `status`.
- `romeka_import_resource`: the ledger. One row per item, media, item set, template or property
  an import created or changed, keyed `(import_id, kind, target_id)`, with the record key and the
  time written, and indexed on `(import_id, record)`. `target_id` has no foreign key, because it
  points at one of five tables and undo deletes the target before the row.
- `romeka_import_before`: a changed resource's state before the import, by the same key.
- `romeka_import_upload`: one tus upload, keyed `(import_id, id)`. The stored offset is
  `received`, because MySQL reserves `offset`.
- `romeka_import_entry`: the evidence index, keyed `(import_id, source, path_hash)`. `path_hash`
  is the SHA-256 of `path`, written by `EntryRepo`, so the MySQL key stays under 3,072 bytes.

Every child table cascades from its import. On MySQL every key text column and every path is
`utf8mb4_bin`, so `A.jpg` and `a.jpg` stay two keys.

The MySQL `0003` has no `CHECK` that at most one publication target is set: MySQL 8 refuses a
`CHECK` on a column whose foreign key sets it to `NULL`. `PublicationTarget` names one target.

The MySQL directory has no `0004`: that migration only makes SQLite's term columns case-sensitive.

## Not translated

- The `resource_type` prefix index and the FULLTEXT behaviour are the only MySQL-specific index
  features; both are handled above.
