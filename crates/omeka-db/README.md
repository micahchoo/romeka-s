# omeka-db

The storage layer for the Omeka S port. It owns the schema, the row structs and the repositories,
and it is the only crate in the workspace that holds SQL.

Two engines are behind one repository API. SQLite is the default and needs no feature. MySQL is the
second backend, behind the `mysql` feature, so the port can run against an Omeka S database that
already exists.

## The seam

Repositories are generic over `Backend`, a trait in `src/backend.rs`:

```rust
pub struct ItemRepo<'a, DB: Backend = Sqlite> { pool: &'a Pool<DB> }
```

The default type parameter is what keeps callers unchanged. `ItemRepo::new(&sqlite_pool)` still
names a SQLite repository, and `ItemRepo::<MySql>::new(&mysql_pool)` is the MySQL one. No caller in
`omeka-api`, `omeka-cli` or `omeka-server` had to change.

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
| full-text search | `MATCH` on the FTS5 mirror, `bm25` negated | `MATCH ... AGAINST`, score as the server reports it |

Everything else is shared: table names, column names, `?` placeholders, the join and filter shapes,
and the row structs. `src/rows/**` parses one struct per upstream table through `Backend`, which
owns the per-engine decoding.

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
upgrade and uninstall. `asset`, `session`, `password_creation` and `site_block_attachment` carry
the reads and writes their callers make. The rest of the evidence is in `tests/`, one file per
repository group.

## Schema and migrations

`migrations/0001_init.sql` is the SQLite translation of upstream
`application/data/install/schema.sql`, with every divergence listed in `SCHEMA-NOTES.md`.

`migrations-mysql/0001_init.sql` is upstream's `schema.sql` itself, with `CREATE TABLE IF NOT
EXISTS` on every statement. Point the port at an Omeka S database that already exists and the
migration leaves its tables alone. It creates `_sqlx_migrations` in that database for its own
bookkeeping.

`migrate` and `migrate_mysql` are both no-ops on a second run.

## Running the tests

```sh
export CARGO_TARGET_DIR=/mnt/Ghar/2TA/DevStuff/notebook/omeka-research/cargo-s2/<unit>
cargo test -p omeka-db
```

The MySQL suite is gated on the `mysql` feature and on a reachable server. It reads
`OMEKA_MYSQL_URL`, which names a server and not a database. Each test creates its own database and
drops it again, so tests never share state.

```sh
docker run -d --name omeka-mysqlbackend-mysql \
  -e MYSQL_ROOT_PASSWORD=secret -p 127.0.0.1:13306:3306 \
  mysql:8.0 --default-authentication-plugin=mysql_native_password

OMEKA_MYSQL_URL=mysql://root:secret@127.0.0.1:13306 \
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
- Case folding. SQLite `COLLATE NOCASE` folds ASCII only; MySQL `utf8mb4_unicode_ci` folds more.
- Length limits. SQLite has none; MySQL enforces the upstream `varchar(n)` and prefix index widths.
- The MySQL suite needs a server that grants `CREATE DATABASE` to the URL's user.

## Follow-ups

- Every table the port serves has a repository. `item_site` is the one exception: its row struct
  exists and a caller that attaches an item to a site still writes the row itself. See
  `SCHEMA-NOTES.md`.
- The MySQL migration does not replay upstream's `application/data/migrations/`. An install older
  than the frozen SHA keeps its older columns, exactly as upstream would leave them.
