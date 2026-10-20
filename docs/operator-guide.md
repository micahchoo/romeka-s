# Operator guide

Audience: whoever starts, watches and backs up an installed instance.

For what an administrator does inside the application, read [`user-guide.md`](user-guide.md). For a
production deployment, with the image, the compose file, a TLS proxy and the rollback procedure, read
[`../deploy/DEPLOY.md`](../deploy/DEPLOY.md). This page is the process itself.

## What runs

One binary.

`romeka serve` answers the `/api` routes, the account routes (`/api/login`, `/api/logout`,
`/api/me`, `/api/password`, `/api/password/forgot`), the public site under `/s/` with its Logout
form at `/logout`, the bundled theme's
files under `/assets/`, and two probes: `/health` for liveness and `/ready`, which runs a query and
answers 503 while storage is unreachable ([`../crates/omeka-server/src/lib.rs`](../crates/omeka-server/src/lib.rs)).

It serves no admin bundle and no stored media bytes. A static origin in front serves the built admin
application at `/` and the media files under `/files/`, asking `GET /api/romeka/files/allow` first
whether each file may be served. That is the arrangement the packaged
deployment uses, and [`deploy/Caddyfile`](../deploy/Caddyfile) is the
working example. `web/admin/e2e/harness/proxy.ts` is another one, in Node.

## Flags, environment and the config file

`romeka serve` takes the address, the base URL, the database and the store. Everything else comes
from the environment or from a file: the log level, the body cap, the request timeout, the mail
settings and the archive roots.
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
| `ROMEKA_ARCHIVE_ROOTS` | none | the folders the archive import reads, as `name=path` pairs separated by `;` (config file key `archive_roots`, an object). Unset or empty, the Import page offers no server folder |
| `ROMEKA_ARCHIVE_ROOT` | none | one more root, named `archive` (config file key `archive_root`) |
| `ROMEKA_STORE_BACKEND` | `local` | `local` keeps files under `--store`; `s3` keeps them in the bucket below. Environment only, no config file key |
| `ROMEKA_S3_ENDPOINT` | none | the S3 service, such as `https://s3.eu-west-1.amazonaws.com` or `http://minio:9000`. Buckets are addressed path style. Required for `s3` |
| `ROMEKA_S3_BUCKET` | none | the bucket. Required for `s3` |
| `ROMEKA_S3_REGION` | `us-east-1` | the region the signatures name. MinIO accepts the default |
| `ROMEKA_S3_ACCESS_KEY`, `ROMEKA_S3_SECRET_KEY` | none | the key pair. Required for `s3`. The secret is never logged or printed |
| `ROMEKA_S3_PUBLIC_BASE` | none | where plain file URLs point instead of the bucket, such as a CDN in front of it |
| `ROMEKA_S3_URL_TTL_SECS` | `900` | how long a signed file URL stays valid, 1 to 604800 seconds |

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

## Object storage

With `ROMEKA_STORE_BACKEND=s3` every byte the local tree would hold goes to the bucket instead, under
the same keys (`original/<storage_id>.<extension>`, `large/...`, `medium/...`, `square/...`,
`asset/...`). `romeka serve`, the `romeka job run` children it starts and the file commands
(`media`, `thumbnails`, `sweep`) read the same variables, so they all reach the same bucket. A wrong
value stops the process at startup and names the variable. A wrong key or a missing bucket shows on
the first read or write as an `object store error` that names the S3 action and its status
([`../crates/omeka-server/src/config_store.rs`](../crates/omeka-server/src/config_store.rs),
[`../crates/omeka-media/src/store/s3.rs`](../crates/omeka-media/src/store/s3.rs)).

The API and the public site name each file with the URL its visibility allows
([`../crates/omeka-media/src/store/urls.rs`](../crates/omeka-media/src/store/urls.rs)):

| File | URL |
| --- | --- |
| an original, of any resource | signed, valid for `ROMEKA_S3_URL_TTL_SECS` |
| a thumbnail of a media whose media and item are public | plain: `ROMEKA_S3_PUBLIC_BASE/<key>`, or the object in the bucket |
| a thumbnail of a private media, or of a media of a private item | signed |
| an asset (a site logo, a resource thumbnail) | plain |

A signed URL is produced only in a document or page the caller may see, so only that caller gets it.
It names the endpoint, so the browser must reach the endpoint. A plain URL answers only if anyone may
read its key, so give the bucket a policy that allows anonymous `s3:GetObject` on `large/*`,
`medium/*`, `square/*` and `asset/*`, and nothing on `original/*`; or put a CDN with its own read
access in front of those prefixes and name it in `ROMEKA_S3_PUBLIC_BASE`. The thumbnail of a private
resource lives under the same public prefixes, so its plain key is as private as its storage id, 40
random hex characters; the API and the site never render it plain. Add a lifecycle rule that aborts
incomplete multipart uploads after a day: a stream longer than 5 MiB goes up in parts, and a failed
upload that also fails to abort leaves its parts.

The key pair needs `s3:GetObject`, `s3:PutObject`, `s3:DeleteObject`, `s3:ListBucket` and
`s3:AbortMultipartUpload` on the bucket. Without `s3:ListBucket`, S3 answers 403 instead of 404 for a
missing object, and every existence check fails.

Backups (`romeka backup`, `deploy/backup.sh`) copy the local store only. With `s3`, back the bucket
up with the service's own tools (versioning, replication, `mc mirror`).

## Jobs and the archive import

The admin's Import page adds files to Omeka from two places: a folder on the server, and the
person's own computer. This section is what you set up and watch. What a person does on the page
is in [`user-guide.md`](user-guide.md). The rules are in [`import-v2.md`](import-v2.md).

### Archive roots

A root is a named folder that the server reads and never writes. It may be mounted read-only.
Set the roots with `ROMEKA_ARCHIVE_ROOTS`, a list of `name=path` pairs separated by `;`:

```
ROMEKA_ARCHIVE_ROOTS='photos=/srv/photos;scans=/mnt/nas/scans'
```

- The config file key is `archive_roots`, an object of the same pairs. The variable replaces the
  file's value.
- `ROMEKA_ARCHIVE_ROOT` still works. It names one more root, called `archive`.
- Each name is used once. Each path is made absolute when the process starts, so a later change of
  the working directory cannot move a root.
- A plan names a source by root and a path inside it. If you move a folder, the person changes the
  path in the plan. You do not change the root.
- The server does not follow a symbolic link to a folder. It counts the links and the files whose
  names start with a dot, and shows the counts on Check.
- A global administrator and a site administrator may read every root.

With no root, the Import page still works for files from the person's computer. Its folder card says
how to set a root.

### The import jobs

An import runs as a job: a `job` row, started in a child process the server does not wait for.

| Class | What it does | Who starts it |
| --- | --- | --- |
| `Romeka\Job\ImportRun` | writes the items, media and item sets of an import | the Import page, `POST …/run` |
| `Romeka\Job\ImportUndo` | undoes an import | `POST …/undo` |
| `Romeka\Job\ImportScan` | reads a source of more than 20,000 files into the index | `POST …/scan` |
| `Romeka\Job\ImportUnzip` | unpacks an uploaded `.zip` into the import's staging area | the last `PATCH` of the upload |
| `Romeka\Job\ImportSweep` | removes staging files that no import needs | `romeka serve`, hourly, and an upload start |

The child is this command:

```
romeka job run <id> --database <db> --store <store> --base-url <base-url> [--archive-root <root>]
```

The child gets the server's database and store as absolute paths, so it reads and writes what the
server serves whatever its working directory. It reads `ROMEKA_ARCHIVE_ROOTS` and `ROMEKA_CONFIG` from
the environment it inherits, so keep both in the server's environment. `--archive-root` carries only
the root named `archive`. The child appends progress to the job's log, which the Jobs page and
`GET /api/romeka/jobs/{id}/log` show, and exits when the job ends.

These rules hold for every import job.

- One import or undo works at a time. The database checks this in one locked transaction, so two
  server processes on one database cannot start two. The second start answers 409 `busy` and the
  Import page links to the import that holds the lock.
- A run commits one item at a time with its media. A record that fails does not stop the others. It
  is listed in the import's summary with a reason, and the import ends `done` when any record was
  written and `run_failed` when none was.
- **Stop** (`POST …/stop`) asks the job to stop after the record in hand. The import ends
  `run_failed`, or `undo_failed` for an undo, so it can be run again or undone.
- A job that stops working (killed, crashed, or its container restarted) leaves its import on
  `running` or `undoing`. The job writes a heartbeat every 10 seconds. Two minutes after the last
  heartbeat, the next read of the import records it as `run_failed` or `undo_failed` and its job as
  `error`. The import JSON carries `died`. Another import can start after that. No operator step
  marks a row by hand.
- To finish a stopped or dead import, use **Try again** on the Import page (`POST …/run` again). The
  run skips every record the first try wrote. A `done` import with failures can be run again the same
  way. Or undo it: an undo works from what the run wrote, so it also undoes a run that did not end.
- Do not start `romeka job run <id>` by hand for a job whose import still beats. The command
  refuses while the heartbeat is younger than two minutes.
- A killed worker can leave the bytes of the record it held. `romeka media sweep` lists them and
  `--delete` removes them ([`../crates/omeka-cli/README.md`](../crates/omeka-cli/README.md)).
- `DELETE /api/jobs/{id}` answers 409 while an import that is running or undoing holds the job.

An import ends in one of these states: `draft`, `running`, `done`, `run_failed`, `undoing`,
`undone`, `partly_undone`, `undo_failed`. `partly_undone` means the undo kept something that
someone changed or used after the import. It is not an error.

Any import can be undone, not only the last one. An undo keeps a resource that was edited, linked to
or added to since the import, and says why. An import that ran `delete` cannot be undone.

### Uploaded files

A person without access to the server adds files to an import from the browser. The files go to the
store under `staging/<importId>/`, never under `/files`, so the web server does not serve them.
Inside it, `files/<upload>` holds a finished file, `parts/<upload>.<offset>` holds the chunks of an
upload in progress, and `meta/` holds the names a ZIP entry had before they were decoded. The upload
is the tus 1.0 core protocol. A `PATCH` carries at most 64 MiB, and the browser sends smaller chunks.
Only the import's owner and global administrators may upload, and only into a draft.

A `.zip` is unpacked by the `ImportUnzip` job. Its log says what was left out and why. The job
leaves out an entry whose path leaves the folder, an absolute path, a symbolic link and an
unreadable entry. It reads a name without the UTF-8 flag as cp437. It refuses the whole ZIP, and
keeps nothing from it, in three cases. An entry unpacks to more than 100 times its size and more
than 10 MB. The files would pass the import's limit. The ZIP holds more than 100,000 entries.

`romeka serve` starts the sweep when it boots and then every hour. An upload start also starts it
when none started in the last hour. The sweep removes:

- the staging area of an import 30 days after it is `done` or `undone`, and 30 days after the last
  try of an import that stopped;
- the staging area of a draft 7 days after it was last saved;
- an upload that did not finish 24 hours after it began;
- parts that no upload names, one hour after they were written.

An import that is running or undoing keeps everything. The draft shows its date.

Six global settings bound the uploads and the fetches. `GET /api/settings` answers each one, and
`PATCH /api/settings` sets it (a whole number above zero; `null` restores the default):

| Setting | Default | What it bounds |
| --- | --- | --- |
| `import_upload_max_file` | 2147483648 (2 GiB) | One uploaded file, one file from a ZIP, one URL an import fetches |
| `import_upload_max_total` | 53687091200 (50 GiB) | All files of one import, uploads in progress counted |
| `import_staging_keep_days` | 30 | Days the files stay after an import is done or undone, or after its last attempt failed |
| `import_draft_staging_keep_days` | 7 | Days a draft's files stay after it was last saved |
| `import_allow_private_urls` | off | When on, an import and the API may fetch from private addresses. It lifts the address check and nothing else |
| `media_type_whitelist`, `extension_whitelist`, `disable_file_validation` | upstream's lists | The file types an upload, the API and an import accept (below) |

Size the store for `import_upload_max_total` times the number of drafts in progress. The axum body
limit (`max_body_bytes`) does not bound an upload: a `PATCH` streams its body into the store.

### File types and URLs

The import checks each file against the file validation settings, as an upload is checked.
Upstream's default lists refuse `.nef`, `.heic`, `.flac`, `.svg`, `.json`, `.mkv` and `.gz`. A file
that fails the check is flagged `not-media` and left out by default. To import those files, widen the
lists with `PATCH /api/settings` (`media_type_whitelist`, `extension_whitelist`).

A row of a list may name a URL to fetch. Every fetch, in the import and in the API, passes one
guard. It allows `http` and `https`, and refuses a URL with a user name. It resolves the host once and
refuses loopback, link-local (`169.254.169.254` too), private, CGNAT and unique-local addresses. It
follows at most five redirects and checks each one. A fetch waits 60 seconds to connect and between
chunks of the body, ends at one hour, and takes a body of at most `import_upload_max_file`. Check
never fetches. Turn on `import_allow_private_urls` only when the server must reach its own network.

### Object storage and the import

With `ROMEKA_STORE_BACKEND=s3` the staging area, the stored `left.csv` (`imports/<id>/left.csv`) and
every imported file live in the bucket. The browser's chunks go to the bucket part by part, and
the ZIP job reads the archive by range, so the server does not hold a whole upload. Four notes:

- The key pair needs `s3:ListBucket`, because the sweep and the scan list prefixes.
- Give the bucket policy no anonymous read on `staging/*`, `imports/*` or `original/*`.
- The thumbnails of a private item sit under the same public prefixes as the others, so their
  plain key is as private as its 40 random hex characters. The API and the site never render it.
- `romeka backup` copies the local store only. Back up the bucket with the service's own tools.

More is in [Object storage](#object-storage).

### MySQL

The import tables (migration 0006) exist for SQLite and for MySQL 8 and MariaDB 11, with
`utf8mb4_bin` on every key text column. The repositories and the importer are written against
`omeka_db::Backend`, and the `omeka-db` tests run the import repositories on both engines. But
`romeka serve` and the CLI open a SQLite file (`-d PATH`) and nothing else, so an install cannot run
the importer on MySQL yet. A MySQL backup is the operator's: use `mysqldump --single-transaction`
([`../crates/omeka-db/README.md`](../crates/omeka-db/README.md)).

### Upgrading from the v1 import

Migration 0006 drops the two v1 import tables and their rows, and creates six new ones. It converts
nothing. A v1 import that you still want to undo must be undone before you upgrade. Take a backup
first, as for any upgrade. After the upgrade the old `romeka import csv` and `romeka import media`
commands are gone, and a v1 plan is refused with 409 `v1Plan`.

## Import from the command line

`romeka import run`, `undo` and `list` do what the Import page does, without a server. Stop other
imports first: one import or undo works at a time, here as on the page.

```
romeka import run  -d <db> --store <store> --archive-root <root> --plan plan.json --as admin@example.org --dry-run
romeka import run  -d <db> --store <store> --archive-root <root> --plan plan.json --as admin@example.org
romeka import undo <id> -d <db> --store <store> --preview
romeka import undo <id> -d <db> --store <store> [--force all]
romeka import list -d <db> [--page N] [--per-page N]
romeka import template -d <db> --file template.json
```

- The plan is the JSON the Import page saves. A draft's plan is the `plan` member of
  `GET /api/romeka/imports/{id}`.
- `--archive-root` is the folder the plan's sources read, named `archive`. Without it the roots from
  `ROMEKA_ARCHIVE_ROOTS` apply.
- `--dry-run` prints the Check summary and writes nothing. Run it first.
- `--as` is the email of a global or site administrator. That user owns the import. A site
  administrator may name only themself as an owner in the plan.
- A delete plan needs `--confirm <count>`, the count the Check summary shows.
- A run that is refused, or a dry run, leaves no import behind. The run's files go to `--store`, so
  point it at the store the server serves.
- `undo --preview` prints what the undo would do and writes nothing. `--force all` also restores or
  deletes what someone edited since the import.
- `romeka import template` reads a resource template from JSON.
- `romeka import csv` and `romeka import media` no longer exist. Put the items in a list and use
  `import run`, or use the Import page.

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
`web/admin/dist`. It serves the media store under `/files/`, after the access check below. It forwards `/api` (which covers
`/api-context`), `/health`, `/ready`, `/s/` and `/assets/` to the process. And it falls back to
`index.html` for an unknown path, because the admin is a hash-routed single page.

Two things to know before you write one.

- **`/assets/` collides.** The bundled theme links its stylesheet at `/assets/css/style.css`, and the
  admin bundle builds its own chunks under `assets/` too. One origin serving both has one URL prefix
  for two file sets: serve the SPA's files there and the public site renders unstyled, forward to the
  process and the admin's chunks 404. The shipped Caddyfile
  ([`deploy/Caddyfile`](../deploy/Caddyfile)) separates them: a file the bundle ships wins, and
  every other path under `/assets/` goes to the process. An origin you write needs the same rule.
- **`/files/` needs the access check.** A file of a private item or media is served only to a user
  who may see it. Before it serves a file, the origin sends a `GET /api/romeka/files/allow` to the
  process with the request's `Cookie` header and the original request URI, query string included,
  in `X-Forwarded-Uri`. Caddy's `forward_auth` does this ([`../deploy/Caddyfile`](../deploy/Caddyfile));
  nginx's `auth_request` does it with `proxy_set_header X-Forwarded-Uri $request_uri`. The answer
  is 204 to serve the file, 403 to refuse it and 404 when no media or asset names it; the origin
  passes a 403 or 404 to the client. A 204 with `X-Romeka-Private: 1` is a private file: answer it
  with `Cache-Control: private` and `Vary: Cookie`, and remove any `X-Romeka-Private` the client
  sent before the check. Send `X-Content-Type-Options: nosniff` on every `/files/` answer, and
  `Content-Security-Policy: sandbox` on `.html`, `.htm`, `.svg` and `.xml` files. An origin that
  serves `/files/` straight from disk serves private files to anyone.
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
| The Import page has no folder to choose | `ROMEKA_ARCHIVE_ROOTS` and `ROMEKA_ARCHIVE_ROOT` are unset or empty in the server's environment. Files from the person's computer still work |
| The admin has no Import link | the signed-in user is not a global or site administrator |
| An import stays on "Adding" | the job process is still working, or it stopped less than two minutes ago. Reload the page. Two minutes after the last heartbeat the import shows "Stopped before the end" (`run_failed`). Read the job's log on the Jobs page for the cause, then use Try again |
| A second import answers "busy" | one import or undo works at a time. Wait for it, or stop it |
| An upload answers 413 or says the import is full | `import_upload_max_file` or `import_upload_max_total` is reached. Raise it with `PATCH /api/settings` |
| The store holds files no row names (after an import that crashed, or an undo) | run `romeka media sweep -d PATH --store DIR` to list them, then add `--delete` to remove them. Files younger than 10 minutes are skipped. See the `romeka-cli` README |
| The admin application answers 404 for everything | the static origin is not forwarding `/api` to the process |
| The public site renders without styling | the origin is serving the SPA's `assets` files instead of forwarding `/assets/` to the process |
| `/ready` answers 503 | the database is unreachable. `/health` still answers 200, because it touches no shared state |
| `romeka serve` exits with an address error | the port is in use, or `--host` is not an address |
