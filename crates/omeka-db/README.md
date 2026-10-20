# omeka-db

Romeka S is a fork of Omeka S. This crate is the storage layer: it owns the schema, the row structs and the
repositories, and it is the only crate in the workspace that holds SQL. The schema
follows upstream's install SQL.

Two engines are behind one repository API. SQLite is the default and needs no feature. MySQL is the
second backend, behind the `mysql` feature, so the port can run against an Omeka S database that
already exists.

## The seam

Repositories are generic over `Backend`, a trait in `src/backend.rs`:

```rust
pub struct ItemRepo<'a, DB: Backend = Sqlite> { pool: &'a Pool<DB> }
```

`ItemRepo::new(pool)` takes its engine from the pool it is given. The default type parameter
names SQLite where a type is written without one.

One repository implementation serves both engines. It asks the backend for the pieces that differ
and never asks which engine it is running on. Those pieces are:

| Piece | SQLite | MySQL |
| --- | --- | --- |
| new row id | `last_insert_rowid()` | `LAST_INSERT_ID()` |
| no-limit `LIMIT` | `-1` | `9223372036854775807` |
| `datetime` column | text `YYYY-MM-DD HH:MM:SS` parsed by `src/time.rs` | native `datetime` decoded by sqlx |
| `LIKE ... ESCAPE` | `ESCAPE '\'` | `ESCAPE '\\'` |
| ignore a duplicate attach | `INSERT OR IGNORE` | `INSERT IGNORE` |
| replace on a duplicate key | `ON CONFLICT (...) DO UPDATE SET x = excluded.x` | `ON DUPLICATE KEY UPDATE x = VALUES(x)` |
| quote a reserved name | bare name | backticked (`value`, which MariaDB reserves) |
| full-text search | `MATCH` on the FTS5 mirror, `bm25` negated | `MATCH ... AGAINST`, score as the server reports it |
| append to a text column | `log \|\| ? \|\| char(10)` | `CONCAT(log, ?, char(10))`, because MySQL reads `\|\|` as OR |
| consistent database snapshot | `VACUUM INTO ?` | refused with `DbError::Unsupported` |

Everything else is shared: table names, column names, `?` placeholders, the join and filter shapes,
the readiness ping (`SELECT 1`), window functions (`COUNT(*) OVER ()`) and the row structs. `src/rows/**` parses one struct per upstream table through `Backend`, which
owns the per-engine decoding.

No other crate holds SQL. `omeka-api`, `omeka-server` and `omeka-cli` reach every table through a
repository. The integration tests of those crates still arrange most fixtures with raw SQLite
statements; that is test setup, not a path a client reaches.

### Where the engine is chosen

The repositories are generic, and their callers are not. A caller holds a `DbPool`
(`src/pool.rs`), an enum with one variant per engine, and crosses to a repository through
`with_db!`:

```rust
let row = with_db!(api.pool(), |db| ItemRepo::new(db).get(id).await)?;
```

The macro matches the engine and runs the same call in each arm, so the repository call is
compiled once per engine and the code around it once. `Location` parses `--database` and
`--database-url` and opens the pool. The import v2 pipeline in `omeka-api/src/import2` was
already generic over `Backend`, because one run holds a transaction across many repositories; its
entry points are dispatched the same way, one `with_db!` per route or job.

The other shape was a `DB: Backend` parameter through `omeka-api`, `omeka-server` and `omeka-cli`.
Both were measured on a spike of handlers in the shape of `omeka-api/src/read/system.rs`: each
reads one of seven repositories, then filters, sorts, pages and serializes the rows. The generic
form instantiates every handler for both engines; the enum form instantiates only the repository
call. One crate built with its dependencies already compiled, no incremental cache, three runs
each, on a 32-thread host (`CARGO_BUILD_JOBS=8`, rustc 1.99.1). The raw runs are in
`seam-spike/results.txt`:

| Handlers | Form | Release build | Release binary (stripped) | Debug build |
| --- | --- | --- | --- | --- |
| 0 | either | 1.9 to 2.1 s | 4,612,016 B | 0.7 s |
| 60 | generic | 12.4 to 16.2 s | 6,979,072 B | 4.1 to 4.2 s |
| 60 | enum | 8.0 to 8.5 s | 6,185,216 B | 2.9 to 3.0 s |
| 180 | generic | 27.4 to 27.8 s | 11,214,880 B | 10.7 to 11.2 s |
| 180 | enum | 16.1 to 17.3 s | 8,503,872 B | 6.9 to 7.5 s |

Above the empty baseline, 180 handlers cost the generic form 25.6 s and 6.60 MB of release
build, and the enum form 14.6 s and 3.89 MB: 1.75 times the build time and 1.70 times the code.
The spike understates the gap. Its handlers are about 20 lines; the real ones (wire documents,
permissions, the site renderer) are longer, and the generic form copies all of it per engine. It
also needs the 13-line bound list of [Why a local statement
builder](#why-a-local-statement-builder) on every generic function and on the axum state, which
doubled the spike's source (1,898 lines against 996 for 60 handlers). Neither form lets a caller forget
an engine: a `with_db!` match that misses a variant does not compile.

On the real binary the enum form costs a SQLite-only build nothing (49,988,832 bytes against
50,003,840 before the change), and the MySQL driver with every MySQL arm adds 7.7 MB (57,696,688
bytes, release, unstripped; `seam-spike/results.txt`).

`seam-spike/gen.py N OUT` writes both forms from one handler template, so they differ only in
the seam, and `seam-spike/measure.sh` run inside `OUT` repeats the measurement. A single crate
bounds what the numbers say: they compare the two seams, not the workspace's own build time.

A MySQL backup is the operator's. MySQL has no statement that writes a database to a file, and a
consistent copy is `mysqldump --single-transaction` or a physical backup tool, which an operator who
runs MySQL already has. The `romeka backup` archive carries a SQLite file that `romeka restore` opens
as SQLite, so a second dump format would also need a second restore. `DatabaseRepo::snapshot_into`
therefore refuses on MySQL rather than writing a partial copy.

Two seams were rejected. A second repository set per backend duplicates the 3,096 lines of
`src/repos` and drifts. `sqlx::Any` would bind one implementation at runtime, but `open_memory` and `open_file` must
keep returning `SqlitePool` for the existing callers, and `Any` erases the per-driver type checks
the row layer relies on.

### Why a local statement builder

`src/repos/mod.rs` has a small `Statement` type instead of `sqlx::QueryBuilder`. `QueryBuilder`
cannot run when the database is a type parameter. Its `build()` ties the query's argument lifetime
to the borrow of the builder, and for a projection such as `DB::Arguments` the borrow checker
rejects that as a self-referential borrow. `Statement` owns the SQL text and the bound values, so
the query borrows a local.

The same constraint means every generic item that binds or reads restates the value bounds it needs,
as in `for<'x> i64: Decode<'x, DB> + Encode<'x, DB> + Type<DB>`. Rust does not expose a type
parameter's `Encode` and `Decode` impls through a trait bound. `Sqlite` and `MySql` are the only
instantiations and both satisfy the bounds.

## Repositories

`src/repos/**` holds one repository per served table. `setting`, `site_setting` and `user_setting`
offer upstream's `get`, `set`, `delete` and a list, and the two target tables key their rows by
`(id, site_id)` or `(id, user_id)`. `job` carries the runner's writes: `start`, `set_status`,
`request_stop`, `append_log`, `clear_pid` and `finish`. `module` carries install, activate,
upgrade and uninstall. `item_site` is `ItemSiteRepo`: `create` attaches an item to a site and
`list` reads every membership, which is the shape the migrator needs. `asset`, `session`,
`password_creation` and `site_block_attachment` carry the reads and writes their callers make.
`RelationRepo` reads one relation (media, memberships, values, reverse references, thumbnails,
template properties) for a whole page of resources in one statement. `TermRepo` reads the
vocabulary prefixes and the property and class terms a request can print. `ItemRepo::window` reads
one page of items and the total its filter matches in one statement. `DatabaseRepo` holds the
whole-database operations: the readiness ping, a table's row count and the backup snapshot. The rest
of the evidence is in `tests/`, one file per repository group.

A value annotation is a `resource` row of the `value_annotation` kind plus the `value_annotation`
identity row, and `value.value_annotation_id` points at it (`migrations/0001_init.sql`, `Entity/Value.php`
line 67). `ResourceRepo::create_annotation` writes both rows in one transaction, so a crash never
leaves a bare annotation. `ResourceRepo::delete` removes the annotation's values and its `resource`
row, and the reference from the value clears through `ON DELETE SET NULL`. The api layer deletes an
annotation with the value that carried it, because upstream maps the pair `orphanRemoval`.

`ValueRewriteRepo` holds the bulk value rewrites behind tags: `text_values` reads a type's text
values with the resource columns a permission check needs, and `promote` and `merge` each write
in one transaction. Each statement re-checks the value's type, so a value another writer retyped
after the read stays as that writer left it.

`SiteRepo::create_whole` writes a new site, its first page and blocks, its permissions, item sets
and settings in one transaction, the way upstream's one flush stores them, so a failing row leaves
no partial site. The first page's id exists only inside that transaction, so a navigation that
links the page is passed as a function of its id.

Import v2's state (`docs/import-v2.md`, "Database") is in migration `0006_import_v2.sql`, which
replaced the v1 importer's tables. `ImportRepo` holds the draft, saves its plan only at the
revision the caller edited (`PlanSave::Stale` otherwise), and ends a run or an undo through an
`ImportEnding`, which names the one state it ends. `ImportRepo::start_run` and `start_undo` are
each one serialized transaction (`BEGIN IMMEDIATE` on SQLite, `FOR UPDATE` on every import row on
MySQL): they record dead imports first, refuse while another import is live, and a run start
freezes the plan into `ran_plan`. An import is dead when it claims to run but its job row has
ended or its heartbeat (summary member `heartbeat`, else the job's start) is older than two
minutes; `settle_dead` records it `run_failed` or `undo_failed` and its job `error`
(`src/repos/import_lifecycle.rs`).

`ImportResourceRepo::record` and `BeforeImageRepo::put` take the caller's executor, so a run
writes a record, its ledger row and its before-image in one transaction. `UploadRepo::advance`
moves an upload forward only from the stored offset (`UploadAdvance::Conflict` otherwise).
`EntryRepo::replace` swaps an import's whole evidence index in one transaction and keys each entry
by the SHA-256 of its path. `IdentifierRepo::items` maps trimmed `dcterms:identifier` literals to
the lowest item id, optionally within item sets.

### Ordering text

`repos::compare_text_ci` and `repos::compare_opt_text_ci` are the one rule a caller applies to order
text the way upstream does. The rule is an ASCII case fold that matches SQLite's `COLLATE NOCASE`,
so a Rust comparator and a `COLLATE NOCASE` clause agree. A sorter that orders by a resource title,
a user name or a vocabulary label calls this instead of comparing bytes, and the limits in
[Known differences](#known-differences) carry to every caller.

## Schema and migrations

`migrations/0001_init.sql` is the SQLite translation of upstream
`application/data/install/schema.sql`, with every divergence listed in `SCHEMA-NOTES.md`.

`migrations-mysql/0001_init.sql` is upstream's `schema.sql` itself, with `CREATE TABLE IF NOT
EXISTS` on every statement. Point the port at an Omeka S database that already exists and the
migration leaves its tables alone. It creates `_sqlx_migrations` in that database for its own
bookkeeping.

`migrations/0004_term_local_name_binary.sql` makes `property.local_name` and
`resource_class.local_name` case-sensitive, as upstream's `utf8mb4_bin` is, so FOAF's `givenName`
and `givenname` are two terms. SQLite changes a collation only by rebuilding the table, and the
drop in a rebuild runs every `ON DELETE` action of the tables that point at it. `migrate`
therefore runs the SQLite set on one connection with foreign key enforcement off, which is the
order SQLite's `ALTER TABLE` documentation gives, and fails if any reference is broken afterwards.
A later rebuild migration relies on the same arrangement. `tests/term_collation_upgrade.rs`
proves every row of every table survives the upgrade; with `ROMEKA_UPGRADE_DB` naming a real
database it runs the same check on a copy of it. MySQL needs no such step: its
`0001_init.sql` already carries upstream's `utf8mb4_bin` on both columns.

`migrate` takes a SQLite pool, a MySQL pool or a `DbPool` and applies that engine's set. It is
a no-op on a second run. It applies any version it has not recorded, including one below a version
already recorded: the 8124 deploy recorded 0007 before its next build brought 0006
(`tests/migration_order.rs`, on both engines).

## Running the tests

```sh
export CARGO_TARGET_DIR=/mnt/Ghar/2TA/DevStuff/notebook/omeka-research/cargo-s2/<unit>
cargo test -p omeka-db
```

The MySQL suite is gated on the `mysql` feature and on a reachable server. It reads
`ROMEKA_MYSQL_URL`, which names a server and not a database. Each test creates its own database and
drops it again, so tests never share state.

```sh
docker run -d --name omeka-mysqlbackend-mysql \
  -e MYSQL_ROOT_PASSWORD=secret -p 127.0.0.1:13306:3306 \
  mysql:8.0 --default-authentication-plugin=mysql_native_password

ROMEKA_MYSQL_URL=mysql://root:secret@127.0.0.1:13306 \
  cargo test -p omeka-db --features mysql
```

The same suite passes against MariaDB 11. MariaDB reserves `VALUE`, so that run is what exercises
the backend's quoted `value` insert. The import suites (`tests/imports.rs`, `tests/import_*.rs`)
write each test once with `on_both!` (`tests/import_db/mod.rs`), which makes a SQLite test and a
MySQL test from the same body.

```sh
docker run -d --name omeka-mariadb-test \
  -e MARIADB_ROOT_PASSWORD=secret -p 127.0.0.1:13318:3306 mariadb:11

ROMEKA_MYSQL_URL=mysql://root:secret@127.0.0.1:13318 \
  cargo test -p omeka-db --features mysql
```

A container that already exists starts again with
`docker start omeka-mysqlbackend-mysql`, and is removed with
`docker rm -f omeka-mysqlbackend-mysql`.

With no variable set, or a server that does not answer, each test prints `SKIP` with the reason and
returns. The suite skips loudly rather than failing.

## Known differences

- Full-text ranking. SQLite ranks with FTS5's `bm25`, MySQL with InnoDB's `MATCH` score. Both are
  reported with a higher score as a better match, and the two orders differ. `DESIGN.md` excludes
  ranking particulars from differential matching. MySQL also drops tokens shorter than
  `innodb_ft_min_token_size` (3 by default) and FTS5 keeps them.
- Text order and case folding. Upstream orders text through MySQL's `utf8mb4_unicode_ci`, which
  folds case and accents. The port's one rule is `repos::compare_text_ci`, an ASCII case fold that
  matches SQLite's `COLLATE NOCASE`, so a Rust sorter and a `COLLATE NOCASE` clause answer the same
  order. Two gaps stay against upstream: accents do not fold (`A` and `Á` never tie), and a
  non-ASCII case pair does not fold (`İ` and `i` stay distinct). `SCHEMA-NOTES.md` divergence 7
  records the same limit for the column collations the migration carries.
- Length limits. SQLite has none; MySQL enforces the upstream `varchar(n)` and prefix index widths.
- The MySQL suite needs a server that grants `CREATE DATABASE` to the URL's user.

## Follow-ups

- The MySQL migration does not replay upstream's `application/data/migrations/`. An install older
  than the frozen SHA keeps its older columns, exactly as upstream would leave them.
