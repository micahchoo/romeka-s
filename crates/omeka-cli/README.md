# omeka-cli

Romeka S is a fork of Omeka S. This crate owns the command line, `romeka`, and
the job runner. The crate is the CLI owner in `DESIGN.md`: `install`, `user`,
`key`, `import`, `export`, `media`, `backup`/`restore`, `serve`, plus the job
trait, a synchronous dispatcher, and the four upstream jobs. It reads and writes
the database only through `omeka-db` repositories.

The four jobs, the invite flow and the install seed mirror the upstream PHP
classes named in the module docs.

## Commands

```
romeka install     -d PATH --email E --name N --password P
                  [--title T] [--time-zone TZ] [--locale L]
romeka user add    -d PATH --email E --name N [--password P] [--role R]
                  [--base-url URL]
romeka user list   -d PATH [--role R]
romeka user set-password -d PATH --email E --password P
romeka user grant-admin  -d PATH --email E
romeka key add      -d PATH --owner E [--label TEXT]
romeka key list     -d PATH
romeka key revoke   -d PATH --identity IDENTITY
romeka import template -d PATH --file TEMPLATE.json
romeka import run      -d PATH --plan PLAN.json --as EMAIL [--dry-run] [--confirm COUNT]
                              [--store DIR] [--archive-root DIR]
romeka import undo ID  -d PATH [--force all] [--preview] [--store DIR]
romeka import list     -d PATH [--page N] [--per-page N]
romeka export jsonld   -d PATH --out PATH [--resource NAME] [--item-set ID]
                              [--base-url URL]
romeka export csv      -d PATH --out FILE [--resource items] [--item-set ID]
                              [--columns TERM,TERM]
romeka media derivatives -d PATH [--store DIR] [--media ID] [--retry]
romeka media thumbnails -d PATH [--store DIR] [--force]
romeka media sweep -d PATH [--store DIR] [--delete]
romeka backup      -d PATH --store DIR --out FILE
romeka restore     -d PATH --store DIR --in FILE [--force]
romeka serve       -d PATH [--host H] [--port P] [--base-url URL] [--store DIR]
romeka job run ID  -d PATH [--store DIR] [--base-url URL] [--archive-root DIR]
```

Every `-d PATH` can be `--database-url mysql://user:password@host:port/database` instead, or
`ROMEKA_DATABASE_URL` in the environment, for a MySQL 8.0 or MariaDB 11 database. `-d` wins over
the variable; the two flags together are refused. A server has no folder, so a command given a
URL and no `--store` uses `files` in its working directory. The server hands a URL to each `job
run` child through the environment, so the password never appears in `ps`
(`src/jobs/dispatcher.rs`). The binary builds with the MySQL driver by default (feature `mysql`);
`--no-default-features` builds a SQLite-only one. `tests/database_url.rs` covers the flags, and
`scripts/mysql-smoke.py` drives a built binary against an empty MySQL or MariaDB database: install,
serve, sign in, an item with an uploaded file, search, the public site, an import v2 run and its
undo, and the backup refusal.

`install` creates the SQLite file (a MySQL database must exist already), runs the embedded migrations, inserts the
four default vocabularies with their classes and properties, creates the
`Base Resource` template from `InstallDefaultTemplatesTask`, writes the admin
user (`global_admin`), and writes the small default settings. Every step is
idempotent: a second run inserts nothing.

`--role` accepts `researcher`, `author`, `reviewer`, `editor`, `site_admin` or
`global_admin` (`omeka-core`'s `Role`).

`user add` with `--password` creates an active account whose password is already
set. Without it the command invites: the account is created inactive and
passwordless, a one-time key is minted with `activate` set, and the command
prints the set-password link, `<base-url>/#/password/<key>`. `--base-url`
defaults to `http://localhost:8080`, the port `romeka serve` binds by default.
Opening the link and posting a password activates the account, which is the flow
`Admin\UserController::addAction` mails. The link is printed rather than mailed,
so an installation with no SMTP relay can still onboard a person; the link
expires two weeks after it is minted, and only the last one minted for a user
works.

## API keys

```
romeka key add    -d PATH --owner EMAIL [--label TEXT]
romeka key list   -d PATH
romeka key revoke -d PATH --identity IDENTITY
```

`add` mints the two 32 character alphanumeric strings of `Entity/ApiKey.php`,
stores the bcrypt hash of the credential, and prints the identity once, then the
credential once. The credential is in no other output and in no column a later
read can reach. The owner must exist and be active, the same requirement
`Authentication\Adapter\KeyAdapter` has; an unknown or inactive owner is refused
and writes nothing. `--label` defaults to empty, because upstream only requires a
label in the admin form.

`list` prints one tab-separated row per key: identity, owner email, label,
created, and last accessed (`-` until a request uses the key). It never prints a
credential.

`revoke` deletes the row and exits 0. An identity with no row prints
`no key with identity ...` on stderr and exits 1.

Pass the pair to the API as `key_identity` and `key_credential`, which is what
`romeka serve` reads:

```
curl "$BASE/api/items?key_identity=$ID&key_credential=$SECRET"
```

## Resource template JSON

Upstream's API shape, so an exported template reads back:

```json
{
  "o:label": "Base Resource",
  "o:resource_class": {"o:id": 3},
  "o:resource_template_property": [
    {"o:property": {"o:id": 1}, "o:alternate_label": "Title",
     "o:position": 0, "o:data_type": ["literal"], "o:is_required": true}
  ]
}
```

Only `o:label` and each `o:property.o:id` are required. A duplicate label is an
error.

## Import v2

`romeka import run`, `undo` and `list` are the importer's second front end
(`docs/import-v2.md`, "Command line"). They call the same `omeka_api::import`
functions as the routes and run the job bodies in this process, so a run needs
no server.

```
romeka import run -d PATH --plan plan.json --as admin@example.org --archive-root /srv/archive
romeka import run -d PATH --plan plan.json --as admin@example.org --archive-root /srv/archive --dry-run
romeka import undo 7 -d PATH --preview
romeka import undo 7 -d PATH [--force all]
romeka import list -d PATH [--page N] [--per-page N]
```

`run` makes a draft owned by `--as` (a global or site administrator) from the
plan file, rescans the sources, derives and checks, then starts the run as the
route does. It refuses on a blocking flag, on a plan with nothing to write, and
on a delete plan unless `--confirm` names the count the Check summary shows.
`--archive-root` is the folder the plan's sources read, named `archive`; without
it the configured roots (`ROMEKA_ARCHIVE_ROOTS`) apply. `--store` defaults to
`files` beside the database.

`--dry-run` prints the Check summary (counts per action and kind, new sets,
types and fields, flags by policy, the blocking flags, the digest) and writes
nothing: the draft it needed is deleted again. A refused run deletes its draft
too, so a refusal leaves no import. A run prints a `progress: done of total`
line when the count moves and ends with `import N done: D of T records, F
failed`, then one line per failed record. The exit code is 1 when a record
failed or nothing was written.

`undo` runs the undo job in this process. `--preview` prints what the undo
would remove, restore, keep with its reasons and skip, and writes nothing.
`--force all` restores or deletes touched resources too. An undo that keeps
touched resources ends `partly_undone` and exits 0; one that stopped on an
error ends `undo_failed` and exits 1. `list` prints one line per import,
newest first.

## Export

```
romeka export jsonld -d PATH --out PATH [--resource NAME] [--item-set ID] [--base-url URL]
romeka export csv    -d PATH --out FILE [--resource items] [--item-set ID] [--columns TERM,TERM]
```

`export jsonld` writes the documents `/api/{resource}` serves, built by
`omeka-api`'s read layer, so the export is the same serializer the HTTP layer
uses. `--resource` is any name in `omeka-api`'s
`read::export::resources()`: the sixteen API adapters that implement `search`
(`items`, `item_sets`, `media`, `value_annotations`, `vocabularies`,
`resource_classes`, `properties`, `resource_templates`, `assets`, `jobs`,
`modules`, `users`, `sites`, `site_pages`, `api_resources`, `data_types`). It
defaults to `items`. `--item-set ID` keeps only the items in that set and needs
`--resource items`. `--base-url` is the origin every `@id` is built from, the
same flag `serve` takes; it defaults to `http://localhost`.

When `--out` is an existing directory, each document goes to its own
`<resource>-<o:id>.json`. Any other `--out` is a file for one JSON array. The
command prints the document count.

`export csv` writes one row per item and one header cell per value column. The
column set is a port decision: upstream at the frozen SHA has no CSV export.
The only export upstream has is `Admin/ResourceTemplateController::exportAction`,
which writes one resource template as JSON, so there is no upstream column set to
match. The columns are:

- A cell names a property as `prefix:local_name` (`dcterms:title`).
- The default column set is every property that holds a value on any exported
  item, with `dcterms:title` first and the rest in term order. `--columns`
  names the terms instead, in the order given.
- A property with several values on any item gets one column per value and the
  header repeats the term, so a multi-valued item keeps every value.
- Values flatten to text: a literal is its `@value`, a URI its `@id`, a resource
  value its display title. A language tag is dropped.

`export csv` writes items only. The
command prints the item count and the header it wrote. Both commands create a
missing parent directory for `--out`.

## Media thumbnails

`romeka media thumbnails` backfills the thumbnails a media row is missing. It is
the pass for rows that already exist: a store written before the thumbnailer, or
a run interrupted between an original and its derivatives.

```
romeka media thumbnails -d PATH [--store DIR] [--force]
```

For each `media` row it reads the original back from
`original/<storage_id>.<extension>`, derives the three sizes, and writes the
files that are missing. A row whose media type is not a decodable image is
skipped, not failed, matching `TempFile::storeThumbnails`. A row whose original
is missing fails; the run continues to the next row. The summary line is
`generated G, skipped S, failed F of N`, and the exit code is 1 when any row
failed.

Idempotent. When all three thumbnail files are present and `--force` is absent
the row is skipped and nothing is written, so a second run reports zero
generated. `--force` removes the three files first and rebuilds them. The
`--store DIR` root defaults to `files` beside the database, the same rule every
command with a `--store` uses.

It stays for the inline pass: one row at a time in this process, with a report
per row. `romeka media derivatives` below is the queue version.

## Media derivatives

`romeka media derivatives` queues the derivative every media is missing and then
runs `Romeka\Job\MediaDerivatives` once in this process.

```
romeka media derivatives -d PATH [--store DIR] [--media ID] [--retry]
```

A media created through the API answers `o-module-romeka:preview` `pending`, and
the server starts one worker that makes its thumbnails, poster or PDF page. This
command is the operator's way to do that for rows the worker never saw: a store
written before the queue existed, a create whose worker could not start, or a
row a tool failed on (`--retry`). It queues only media whose type a decoder or a
tool here can read, and reads nothing back itself.

The worker streams an original out of the store in 8 MiB pieces into a temporary
file when the still comes from `ffmpeg` or `pdftoppm`, so a media staged in an
object store has a poster or a first page even though the import could not make
one (it has no local path to hand the tool). A tool that is not installed leaves
the row `none` with `toolMissing: <command>`; a command that ran and failed
leaves it `failed` with the command's reason.

The same command records each media's access decision, which the API's create
also records: an `access_copy` row that says whether a browser shows the
original (`none`, "a browser shows the original"), or which copy a worker makes
and with which command (`pending`, `ffmpeg` for an MP4 or M4A, `vips` for a TIFF
layout the decoder does not read, `exiftool` or libraw for a RAW file, `gs` for a
PostScript page), or `none` with `needs <tool>, not installed`. `--retry` also
queues a copy that failed, or one whose tool was missing when it was decided; a
copy settled "a browser shows the original" has no tool and stays settled. A row
is claimed only by a worker whose install has its command, so `--retry` on an
install with `vips` re-queues a row a smaller install settled with `needs vips,
not installed` (the separate tools image in `packaging/README.md` runs this way).
The worker makes the audio and video copy in this process when the backfill runs
it (a transcode of the whole queue), and the hourly sweep starts the same worker
in its own process.

The summary line is `queued Q, retried R, waiting W, already ready A, nothing to
do N`, one line per row after the run, and the exit code is 1 when any row ended
failed. `--media ID` narrows the run to one media.

## Media sweep

`romeka media sweep` finds store files that no row names. An import stores an
item's files and then commits its row; a crash between the two leaves files
that no `media` row names, and the undo job, which works from rows, cannot see
them.

```
romeka media sweep -d PATH [--store DIR] [--delete]
```

It lists `original/`, `asset/` and the `large/`, `medium/` and `square/`
directories. A file is named when a `media` row has its `storage_id` (the
original with the row's extension, and a `.jpg` thumbnail of each type) or an
`asset` row has it. The rest are orphans.

Without `--delete` the run only prints `orphan <path>` per file and a summary
line. Deletion cannot be undone, so it is explicit: with `--delete` the run
removes the orphans and prints `removed <path>` for each. The exit code is 1
when a file could not be removed.

A file modified in the last 10 minutes is skipped and counted, because an
import may be between storing and committing. A symlink is never followed and
never listed, and nothing outside the store root is read or removed. A file
elsewhere under the root, or in a subdirectory, is not the store's and is left
alone. Run the sweep while no import is running if you can: the age rule
protects a file in flight, not a very slow import.

Idempotent. The decision comes from the rows and the store as they are now, so
a second `--delete` finds nothing to remove. The `--store DIR` root defaults to
`files` beside the database.

## Backup and restore

```
romeka backup  -d PATH --store DIR --out FILE
romeka restore -d PATH --store DIR --in FILE [--force]
```

`backup` writes one archive holding a consistent copy of the database and the
whole store. `restore` reads it back into a target. Both are for a SQLite
database: on MySQL `backup` refuses with `DbError::Unsupported`'s reason (use
`mysqldump --single-transaction`) before it writes anything, and `restore`
refuses before it reads the archive.

The archive is a plain (uncompressed) tar, so `tar tf backup.tar` lists it and
`tar xf` extracts it without this CLI. Three kinds of entry:

```
romeka.db          the database copy
store/<path>      every file under the store root, at its store-relative path
manifest.json     the commit, the counts and the checksums
```

`--store DIR` is the store root, the directory holding `original/` and the
thumbnail types; the same path `import run` writes and `serve` reads. Every
regular file under it is archived, not only the files media rows name, so assets
and any orphan survive too.

The database copy comes from SQLite's `VACUUM INTO`, not a file copy. A file copy
of a live write-ahead-log database misses the frames still in the `-wal` and can
catch a torn page; `VACUUM INTO` reads through one read transaction, writes a
complete file, and compacts it, so the archive holds one point in time and no
free pages.

Consistency between the two parts rests on the read order: the database first,
the store second. A media row can only name a file that existed when the row was
written, so a store read after the database snapshot holds every file the
snapshot names. The reverse order would archive a store the database has not
caught up with. Before writing, backup checks every media row's `original` and
thumbnail paths (`FileStore::storage_path` and `thumbnail_path`) against the
store and refuses the run when one is absent. An archive therefore never holds a
media row without its file. The archive is written to a staging file beside
`--out` and renamed into place, so a failed run leaves no archive behind.

`manifest.json` holds:

- `format` (`romeka-backup`) and `format_version` (1).
- `created`, in the upstream datetime format.
- `version`, the upstream `Module::VERSION` of the port.
- `commit`, the git commit the binary was built from (`build.rs`).
- `counts`, the row counts of `resource`, `item`, `item_set`, `media`, `value`,
  `user`, `vocabulary`, `resource_class`, `property`, `resource_template`,
  `asset`, `site`, `site_page` and `site_page_block`, read from the archived copy.
- `database`, the entry name, size and SHA-256.
- `store`, one `{path, size, sha256}` per file, the total size, and a checksum
  over the sorted `path\0sha256\n` lines.

`restore` refuses an occupied target, meaning an existing database file or a
non-empty store, unless `--force`. It extracts the archive to a staging directory
beside the store, re-computes the database and every store checksum against the
manifest, and only then writes the target. The database lands through a temporary
sibling renamed over the target. Stale `<db>-wal` and `<db>-shm` files are
removed, because SQLite would replay a stale log over the restored database.
`--force` also mirrors the store, removing a file the archive does not hold.
After writing, restore opens the restored database, checks that every media row's
file is in the restored store, and counts the rows, so the summary line reports
measured counts rather than the manifest's claim.

Measured 2026-10-17: the Gawan Museum Trust sample (949 items, 100 media, a
386,744,988 byte store) archives to 388,762,112 bytes: the database copy
1,916,928 bytes, the store 386,744,988 bytes, the rest tar headers and the
manifest.

`scripts/verify-backup.sh [--database FILE --store DIR]` proves the cycle end to
end with the built binary and the system `tar`: it lists the archive, checks the
manifest against the database, restores into a fresh directory, compares every
store file's SHA-256, and checks that a second restore without `--force` is
refused.

## Jobs

`Dispatcher::dispatch(&dyn Job, args)` composes a `job` row, runs it inline (the
synchronous strategy), and records `completed`, `stopped` or `error`. Jobs read
`job.args` and poll `stopping` through `should_stop`.

- `BatchDelete`: `resource` (`items`, `item_sets`, `media`), optional `query`.
- `BatchUpdate`: `resource`, optional `query`, and any of `data` (replace),
  `data_append`, `data_remove`. Each is an object mapping `prefix:local_name` to
  one string or an array of strings. One resource's edit commits in one
  transaction, so a failed insert keeps the values a replace would delete. An
  edited item joins each synced site whose pool selects it now and did not
  before, as an API update does; an item removed from a site by hand stays out
  until an edit makes it newly match.
- `IndexFulltextSearch`: no arguments. Clears `fulltext_search` and indexes
  items, item sets and media.
- `UpdateSiteItems`: `action` (`add`, `replace`, `remove`, `remove_all`) and
  `sites`, an object keyed by site id whose values are item queries or null. The
  query selects what `GET /api/items` lists for a global administrator, because
  the job runs `omeka_api::site_items`, the change `POST /api/sites/{id}/items`
  runs.
- `Romeka\Job\ImportRun` and `Romeka\Job\ImportUndo` (Romeka's own): `importId`,
  and for an undo an optional `force`. The server's import routes compose them;
  each runs only for an import the server moved to `running` or `undoing` and
  linked to that job. The run commits one record at a time, appends each progress
  line to `job.log`, writes a heartbeat every 10 seconds and ends the import
  `done`, or `run_failed` when no record was written or a stop was asked. A
  second run of a `run_failed` import skips the records the first one wrote.
  The undo ends it `undone`, `partly_undone` when it kept a touched resource, or
  `undo_failed` on an error; a later undo converges.
- `Romeka\Job\ImportScan`: `importId`. A rescan of the sources of a draft, for
  a source of more than 20,000 entries. The route does a smaller one in the
  request.
- `Romeka\Job\ImportUnzip`: `importId` and `upload`, the id of the finished
  `.zip`. It writes each entry into the import's staging area and names what it
  left out in the job log. A refused ZIP ends the job `error` with the reason as
  the last log line.
- `Romeka\Job\ImportSweep`: no arguments. It removes the staging files no import
  needs, then starts one `MediaDerivatives` worker when derivative rows are
  queued and none is active (not while an import runs or undoes). `romeka serve`
  starts it at boot and then every hour, so a pending preview never waits for
  the next media create.
- `Romeka\Job\MediaDerivatives` (Romeka's own): no arguments. It drains the
  `romeka_media_derivative` queue, one row at a time, and settles each `ready`,
  `failed` or `none`. One worker runs per install: the server composes a new job
  only when a row is queued and no worker of this class is starting, running or
  stopping. A worker that finds an archive import running or undoing starts
  nothing, logs one `waiting:` line, and polls until the import ends (or for an
  hour, when it ends with its rows still queued); it checks for a stop request
  between rows. The work is `omeka_api::derivatives::settle_next`, the same
  function a test and the backfill drain. The server starts one on a create and
  the hourly `ImportSweep` starts one when the queue has work, so the two
  together are the queue's timer.

`romeka job run <id>` takes `--store` (default `files` beside the database),
`--base-url` (default `http://localhost`) and `--archive-root` (the root named
`archive`; the named roots come from `ROMEKA_ARCHIVE_ROOTS` in the environment).
It refuses a job whose import has a heartbeat younger than two minutes.
`romeka serve` passes all three, with the store `--store` gave it, when it
starts a job.

## Where this deviates from upstream

- `user add` without `--password` prints the activation link instead of mailing
  it. Upstream's `Admin\UserController::addAction` calls
  `Mailer::sendUserActivation`; the CLI has no relay to send through, and the
  link is the part the admin needs, so it goes to stdout. `romeka serve` mails
  the same message when `ROMEKA_SMTP_HOST` is set.
- `user.password_hash`: bcrypt, through `omeka_api::password`. The port writes
  the `$2y$` form PHP's `password_hash` writes and verifies every bcrypt version
  prefix, so a hash a migrated Omeka S install carries still verifies and a hash
  the CLI or the API sets works in both. This crate held a PBKDF2 scheme of its
  own until the login routes landed; the session login and the CLI must share one
  scheme, and bcrypt is the one upstream uses.
- Default vocabularies are upstream's four RDF files, bundled in
  `omeka_api::vocab_import::defaults` and read with the API's RDF importer, as
  `InstallDefaultVocabulariesTask` reads them with `RdfImporter`.
  `tests/default_vocabularies.rs` checks a fresh install against an upstream
  install read through its API. `romeka install` also corrects an install made
  from the older JSON bundle, which gave every FOAF member its local name as its
  label and no comment and lacked `foaf:givenname`: it adds a missing member and
  replaces only text that is still that fallback, so an edited label is kept. A
  member it adds goes after the existing ones, as upstream's Update adds it.
- `BatchUpdate`'s data maps are a `term -> literals` shape, not the full API
  hydrate array.
