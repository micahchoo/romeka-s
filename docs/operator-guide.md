# Operator guide

Audience: whoever starts, watches and backs up an installed instance.

Read [`user-guide.md`](user-guide.md) for what an administrator does inside the application. This page is about the
process, its ports, its data on disk, its backups and the gate that checks the code.

## What runs

The application is two processes.

1. `omeka serve`: the API (`/api`, `/api-context`), the liveness route (`/health`), and the public site under `/s/`.
2. A static origin: it serves the built admin application (`web/admin/dist`), serves the media files the API links to,
   and forwards `/api`, `/api-context`, `/health` and `/s/` to the port. The end-to-end harness in
   `web/admin/e2e/harness` is one working example: it serves the bundle and the file store and forwards those paths
   (`../web/admin/README.md`).

The port serves no admin bundle and no media bytes itself. If the origin does not forward `/api`, the admin application
gets the origin's 404 for every page.

A packaged form (a container image, a compose file, a systemd unit and a smoke script) is a separate unit, specified in
[`../briefs/packaging.md`](../briefs/packaging.md). It has not landed in this tree yet; the packaging directory does not
exist at this commit.

## Ports and addressing

| Setting | Default | Notes |
| --- | --- | --- |
| `omeka serve --host` | `127.0.0.1` | loopback only unless you change it |
| `omeka serve --port` | `8080` | |
| `omeka serve --base-url` | `http://<host>:<port>` | the prefix of every `@id` the API emits; set it when a client reaches the server through another host name or port |
| `GET /health` | | answers `{"status":"ok"}`; not part of the upstream API |

`--base-url` matters more than it looks. Every resource link, every `@id`, and the `Link` pagination header are built
from it, so a value that does not match the address a client uses produces links that point somewhere else
([`../crates/omeka-server/README.md`](../crates/omeka-server/README.md)).

## Data on disk

The database is one SQLite file, given by `-d`. The server opens it with `PRAGMA foreign_keys=ON`, a five second busy
timeout, WAL journaling and a five connection pool ([`../crates/omeka-db/src/lib.rs`](../crates/omeka-db/src/lib.rs)).

The file store is a directory tree:

```
original/<storage_id>.<extension>     the stored file
large/<storage_id>.jpg                one derivative per thumbnail type
medium/<storage_id>.jpg
square/<storage_id>.jpg
```

Two things to know, both of them easy to get wrong.

- `omeka install` is what applies the schema. `omeka serve` opens the file but does not migrate it. Serving a path with
  no schema fails on the first query. Running `omeka install` again against an existing database applies any new
  migrations and inserts nothing else, so that is also the upgrade step.
- The server's store root is the constant `files`, resolved against the process working directory, and it is not
  configurable: `omeka serve` has no store flag ([`../crates/omeka-server/src/lib.rs`](../crates/omeka-server/src/lib.rs)).
  The import commands default their store to `files` **beside the database file**
  ([`../crates/omeka-cli/src/commands/media.rs`](../crates/omeka-cli/src/commands/media.rs)). Start the server from the
  directory that holds the database, or pass `--store` to every import command, or the media links will 404 while the
  rows exist.

## Backup and restore

The database and the store must be copied together. A media row that names a file the archive lacks is a broken restore,
and the reverse is wasted bytes.

Today there is no `backup` command. The safe manual procedure is:

1. Stop the server, so nothing is writing.
2. Copy the database file and the whole store directory.
3. Start the server again.

A copy taken while the server runs is not safe: the database is in WAL mode, so recent commits live in the `-wal`
sidecar file, and the store can change between the two copies.

The backup unit specified in [`../briefs/backup.md`](../briefs/backup.md) adds `omeka backup` and `omeka restore`, with
a consistent database copy, the store, and a manifest of the commit, counts and checksums. When it lands, this section
should point at its README instead of repeating it.

Never write into the acceptance collection `/mnt/Ghar/2TA/BHC006_GAWANMUSEUMTRUST`; it is a read-only source
([`../VERIFY.md`](../VERIFY.md)).

## The gate

`./verify.sh` is the one command that answers whether the code is in a good state.

| Mode | Runs |
| --- | --- |
| `./verify.sh --rust` | `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` |
| `./verify.sh --ts` | `npm ci`, typecheck, lint, tests and build in `web/admin` |
| `./verify.sh --verification` | the upstream matrix, the behaviour ledger, the scenario diff against a started oracle, and the port-against-goldens diff |
| `./verify.sh` | all three |

CI runs the same gate through [`../verification/ci/run.sh`](../verification/ci/run.sh), keeps the log, publishes a stage
table and fails the build on any red stage ([`../.github/workflows/verify.yml`](../.github/workflows/verify.yml)). The
generated scoreboard is [`../verification/report.md`](../verification/report.md).

What green means is defined in [`../VERIFY.md`](../VERIFY.md), not here: five layers, an oracle diff, a behaviour
ledger, a real-data run, and the rule that a ported row needs a proof the checker reruns itself. Read that page before
claiming anything from a green run.

The documentation has its own check. `cd docs && ./gen.sh && git diff --quiet` regenerates the API reference and fails
when the committed file is stale.

## Checking against upstream

[`../oracle/README.md`](../oracle/README.md) runs upstream Omeka S at the frozen commit (PHP and MariaDB in Docker) as
the reference server, with `up.sh`, `smoke.sh` and a documented reset. The scenario diff uses it; a normal deployment
does not need it.

## Troubleshooting

| Symptom | Cause, and what to do |
| --- | --- |
| Every request is a 500 whose body names a missing table | the schema was never applied: run `omeka install` against the same database |
| Every authenticated page answers 403 | the key is missing, malformed, unknown, or its owner is inactive; the port answers no 401 ([`api.md`](api.md)) |
| API links point at the wrong host or port | set `--base-url` to the address clients use |
| Media pages load but their images 404 | the server's store root is `<working directory>/files`; start it from the database's directory or point the imports at the same directory |
| The admin application answers 404 for everything | the static origin is not forwarding `/api` to the port |
| `omeka serve` exits with an address error | the port is already in use, or `--host` is not an address |
