# Operator guide

Audience: whoever starts, watches and backs up an installed instance.

For what an administrator does inside the application, read [`user-guide.md`](user-guide.md). For a
production deployment, with the image, the compose file, a TLS proxy and the rollback procedure, read
[`../deploy/DEPLOY.md`](../deploy/DEPLOY.md). This page is the process itself.

## What runs

One binary.

`romeka serve` answers the `/api` routes, the account routes (`/api/login`, `/api/logout`,
`/api/me`, `/api/password`, `/api/password/forgot`), the public site under `/s/`, the bundled theme's
files under `/assets/`, and two probes: `/health` for liveness and `/ready`, which runs a query and
answers 503 while storage is unreachable ([`../crates/omeka-server/src/lib.rs`](../crates/omeka-server/src/lib.rs)).

It serves no admin bundle and no stored media bytes. A static origin in front serves the built admin
application at `/` and the media files under `/files/`. That is the arrangement the packaged
deployment uses, and [`packaging/nginx.conf.template`](../packaging/nginx.conf.template) is the
working example. `web/admin/e2e/harness/proxy.ts` is another one, in Node.

## Flags, environment and the config file

`romeka serve` takes the address, the base URL, the database and the store. Everything else comes
from the environment or from a file: the log level, the body cap, the request timeout, the mail
settings and the archive root.
Precedence is defaults, then the file, then the environment, then the two flags the command sets
itself ([`../crates/omeka-server/src/config.rs`](../crates/omeka-server/src/config.rs)).

| Flag | Default | Notes |
| --- | --- | --- |
| `-d PATH` | required | the SQLite database file |
| `--host` | `127.0.0.1` | loopback unless you change it |
| `--port` | `8080` | |
| `--base-url` | `http://<host>:<port>` | the prefix of every `@id`, of every `Link`, and the origin a cookie write must match |
| `--store DIR` | `files` in the working directory | the file store root. The server passes it to every job process it starts |

| Variable | Default | What it sets |
| --- | --- | --- |
| `ROMEKA_CONFIG` | none | a config file to read after the defaults, before the environment |
| `ROMEKA_API_HOST`, `ROMEKA_API_PORT` | `127.0.0.1`, `8123` | the address, unless `--host` / `--port` overrides it |
| `ROMEKA_BASE_URL` | `http://localhost:8123` | unless `--base-url` overrides it |
| `ROMEKA_LOG` | `info` | the tracing filter |
| `ROMEKA_MAX_BODY_BYTES` | `268435456` | the request body cap, 256 MiB |
| `ROMEKA_REQUEST_TIMEOUT_SECS` | `30` | per-request timeout |
| `ROMEKA_SMTP_HOST`, `ROMEKA_SMTP_PORT` | none | an SMTP relay. With no host, mail goes to the log |
| `ROMEKA_SMTP_USER`, `ROMEKA_SMTP_PASSWORD`, `ROMEKA_SMTP_TLS` | none, none, `starttls` | relay credentials and transport security |
| `ROMEKA_MAIL_FROM` | `no-reply@omeka.invalid` | the envelope sender |
| `ROMEKA_ARCHIVE_ROOT` | none | the server folder the archive import reads (config file key `archive_root`). Unset or empty, the Import page is off |

Two traps. `--base-url` is not cosmetic: every resource link, every `@id` and the `Link` pagination
header are built from it, and the write path checks that a state-changing request carrying the
session cookie comes from that origin. A base URL that names a different host or scheme than the
browser uses breaks sign-in in a way that looks like a bug
([`../crates/omeka-api/src/origin.rs`](../crates/omeka-api/src/origin.rs)). And with no SMTP host the
invite and reset links are written to the request log and returned to the administrator, which is how
an installation without mail still onboards people.

## Data on disk

The database is one SQLite file. The process opens it with foreign keys on, a five second busy
timeout, write-ahead logging and a five connection pool
([`../crates/omeka-db/src/lib.rs`](../crates/omeka-db/src/lib.rs)). Because it is in WAL mode, recent
commits live in the `-wal` sidecar until a checkpoint, which is why the backup tool copies through
the database API rather than the filesystem.

The file store is a directory tree, `files` beside the database by default:

```
original/<storage_id>.<extension>     the stored file
large/<storage_id>.jpg                one derivative per thumbnail type
medium/<storage_id>.jpg
square/<storage_id>.jpg
```

- `romeka install` is what applies the schema and the migrations. `romeka serve` opens the file and
  does not migrate it, so serving a path with no schema fails on the first query. Running `install`
  again against an existing database applies anything new and inserts nothing else, which makes it
  the upgrade step too.
- The server's store root is `--store`, or `files` in the process working directory without it. The
  import commands default their store to `files` beside the database file. Give `serve` and every
  import the same `--store`, or media rows will exist while their files 404
  ([`../crates/omeka-cli/src/commands/serve.rs`](../crates/omeka-cli/src/commands/serve.rs),
  [`../crates/omeka-cli/README.md`](../crates/omeka-cli/README.md)).

## Jobs and the archive import

With `ROMEKA_ARCHIVE_ROOT` set, the admin's Import page reads folders under that root and imports
them. The server never writes the archive folder, so it may be mounted read-only. The originals and
thumbnails go to the store.

An import and its undo each run as a job: a `job` row with class `Romeka\Job\ImportArchive` or
`Romeka\Job\UndoImport`, run by a child process the server starts and does not wait for:

```
romeka job run <id> --database <db> --store <store> --base-url <base-url> --archive-root <root>
```

The child gets the server's database and store as absolute paths, so it reads and writes what the
server serves whatever its working directory. It gets the base URL and the archive root as the
server holds them, so set `ROMEKA_ARCHIVE_ROOT` to an absolute path. The child appends progress to
the job's log, which the Jobs page and `GET /api/romeka/jobs/{id}/log` show, and exits when the job
ends.

Do not run a stuck job by hand. Wait for the rule below to settle it, then read the import again.

- One import or undo works at a time. The database checks this in one locked transaction, so two
  server processes on one database cannot start two imports.
- A job that ends in an error shows its import as failed. A failed import can still be undone, and
  the undo converges: it deletes what the run recorded, one item at a time.
- A job process that stops working (killed, crashed, or its container restarted) leaves its import
  on `running` or `undoing`. The job writes a heartbeat every 10 seconds. Two minutes after the last
  heartbeat, the next read of the import (the Import page or the API) records the import as `failed`
  and its job as `error`. The job log gets a line that says the process stopped, and the import JSON
  carries `died`. Another import or undo can start after that. No operator step marks a row by hand.
- The last import can be undone until the next import starts. Undo deletes the imported items with
  their files, and the new item sets, item types and fields that nothing else uses. A failed import
  cannot run again. To remove what its run wrote, undo it before you start the next import.

## Backup and restore

```
romeka backup  -d /var/lib/romeka/romeka.db --store /var/lib/romeka/files --out backup.tar
romeka restore -d /var/lib/romeka/romeka.db --store /var/lib/romeka/files --in backup.tar
```

`backup` writes one uncompressed tar holding a consistent copy of the database, every file under the
store, and a `manifest.json` with the commit, the row counts and a checksum per file. The database
copy comes from SQLite's `VACUUM INTO`, not a file copy, so the archive holds one point in time with
no `-wal` frames missing and no torn page. Before writing, it checks that every media row's original
and thumbnails are present in the store and refuses the run when one is not, so an archive never
holds a media row without its file. `restore` refuses an occupied target unless `--force`, verifies
every checksum against the manifest, removes a stale `-wal` and `-shm`, and reports the counts it
measured after writing ([`../crates/omeka-cli/README.md`](../crates/omeka-cli/README.md)).

`crates/omeka-cli/scripts/verify-backup.sh` proves the cycle end to end with the built binary and the
system `tar`, including that a second restore without `--force` is refused.

## The static origin

The origin does four things. It serves the built admin application at `/`, from
`web/admin/dist`. It serves the media store under `/files/`. It forwards `/api` (which covers
`/api-context`), `/health`, `/ready`, `/s/` and `/assets/` to the process. And it falls back to
`index.html` for an unknown path, because the admin is a hash-routed single page.

Two things to know before you write one.

- **`/assets/` collides.** The bundled theme links its stylesheet at `/assets/css/style.css`, and the
  admin bundle builds its own chunks under `assets/` too. One origin serving both has one URL prefix
  for two file sets: serve the SPA's files there and the public site renders unstyled, forward to the
  process and the admin's chunks 404. The shipped nginx
  ([`packaging/nginx.conf.template`](../packaging/nginx.conf.template)) serves the SPA's files, which
  is the first of those two, and it forwards neither `/assets/` nor `/ready`. Until one of the two
  paths moves, an origin that has to be correct for both needs a rule that separates them.
- **The process may be reached over loopback.** Forward the client's `Host` and `X-Forwarded-For`:
  login limits count by address, and the address comes from `X-Real-IP`, then the last
  `X-Forwarded-For` entry, then the socket peer.

## Upgrade and roll back

1. `romeka backup -d ... --store ... --out before-upgrade.tar`
2. Stop the service.
3. Install the new binary and run `romeka install -d ... ` against the same database. It applies the
   migrations and inserts nothing else.
4. Start the service and check `/ready`.
5. To roll back, restore `before-upgrade.tar` with `--force` into the same paths, put the old binary
   back, and start it.

A migration that only adds cannot be undone by the old binary, which is why the rollback step is the
restore and not just an older binary.

## Checks on a checkout

```
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace

cd web/admin && npm ci && npm run typecheck && npm run lint && npm test && npm run build && npm run e2e
cd web/admin && npm run perf:check

cd docs && ./gen.sh --check
```

`cargo test --workspace` passes every target, the two `omeka-api` targets `golden` and `property_wire`
among them; both read the captured upstream documents under `fixtures/api/`. [`compat.md`](compat.md)
says what that means for a compatibility claim.

## Troubleshooting

| Symptom | Cause, and what to do |
| --- | --- |
| Every request is a 500 whose body names a missing table | the schema was never applied: run `romeka install` against the same database |
| Every authenticated page answers 403 | the key is missing, malformed, unknown, or its owner is inactive. No route answers 401 ([`api.md`](api.md), Errors) |
| Sign-in appears to succeed and the next write is refused | `--base-url` names a different origin than the browser used, so the write fails the same-origin check |
| API links point at the wrong host or port | set `--base-url` to the address clients use |
| Media pages load but their images 404 | `serve --store` and the import's `--store` differ, or the server runs without `--store` from another directory |
| The admin has no Import page | `ROMEKA_ARCHIVE_ROOT` is unset or empty in the server's environment |
| An import stays on "Importing" | the job process is still working, or it stopped less than two minutes ago. Reload the page. Two minutes after the last heartbeat the import shows `failed`. Read the job's log on the Jobs page for the cause |
| The store holds files no row names (after an import that crashed, or an undo) | run `romeka media sweep -d PATH --store DIR` to list them, then add `--delete` to remove them. Files younger than 10 minutes are skipped. See the `romeka-cli` README |
| The admin application answers 404 for everything | the static origin is not forwarding `/api` to the process |
| The public site renders without styling | the origin is serving the SPA's `assets` files instead of forwarding `/assets/` to the process |
| `/ready` answers 503 | the database is unreachable. `/health` still answers 200, because it touches no shared state |
| `romeka serve` exits with an address error | the port is in use, or `--host` is not an address |
