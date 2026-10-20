# omeka-cli

Romeka S is a fork of Omeka S. This crate owns the command line, `romeka`, and
the job runner. The crate is the CLI owner in `DESIGN.md`: `install`, `user`,
`key`, `import`, `export`, `media`, `backup`/`restore`, `serve`, plus the job
trait, a synchronous dispatcher, and the four upstream jobs. It reads and writes
the database only through `omeka-db` repositories (and `src/store.rs` for the
four tables those repositories do not cover).

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
romeka import csv      -d PATH --file ITEMS.csv --item-set ID
romeka import media    -d PATH --source DIR [--manifest FILE.json] [--store DIR]
                              [--item-set ID] [--item ID]
romeka export jsonld   -d PATH --out PATH [--resource NAME] [--item-set ID]
                              [--base-url URL]
romeka export csv      -d PATH --out FILE [--resource items] [--item-set ID]
                              [--columns TERM,TERM]
romeka media thumbnails -d PATH [--store DIR] [--force]
romeka media sweep -d PATH [--store DIR] [--delete]
romeka backup      -d PATH --store DIR --out FILE
romeka restore     -d PATH --store DIR --in FILE [--force]
romeka serve       -d PATH [--host H] [--port P] [--base-url URL] [--store DIR]
romeka job run ID  -d PATH [--store DIR] [--base-url URL] [--archive-root DIR]
```

`install` creates the SQLite file, runs the embedded migrations, inserts the
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

## CSV items

The first record is the header. Each header cell names a property as
`prefix:local_name` (`dcterms:title`, `foaf:name`). Each later record is one
item: a non-empty cell becomes a literal value on that property, and two columns
with the same term become two values. The item's `title` column is the first
non-empty `dcterms:title` cell. Every item joins the `--item-set` set. A column whose header starts with `#` is a
note and is ignored. A row
whose column count differs from the header is reported and skipped; the rest
import. The exit code is 1 when any row failed. Values are literals only (no URI
or resource-value columns yet).

A row imports whole or not at all: its item, item-set link and values commit in
one transaction, so a failed row leaves nothing. After the commit the item joins
each site that keeps in sync with its item pool (`assign_new_items`) and whose
pool selects it, as an API create that names no `o:site` does.

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
match. The columns here read back through `romeka import csv`:

- A cell names a property as `prefix:local_name` (`dcterms:title`), the term
  `romeka import csv` resolves.
- The default column set is every property that holds a value on any exported
  item, with `dcterms:title` first and the rest in term order. `--columns`
  names the terms instead, in the order given.
- A property with several values on any item gets one column per value and the
  header repeats the term. `romeka import csv` reads two cells with the same
  term as two values, so this is what makes a multi-valued item round-trip.
- Values flatten to text: a literal is its `@value`, a URI its `@id`, a resource
  value its display title. Re-importing stores every cell as a literal, so a URI
  or resource value comes back as a literal of that text and a language tag is
  dropped.

`export csv` writes items only, because `romeka import csv` reads items. The
command prints the item count and the header it wrote. Both commands create a
missing parent directory for `--out`.

## Media files

`romeka import media` copies each file into the store through `omeka-media` and
inserts its `media` row through `omeka-db`.

```
romeka import media -d PATH --manifest media.json --source COLLECTION --store DIR --item-set ID
romeka import media -d PATH --source COLLECTION --item-set ID
```

`--manifest FILE.json` reads the acceptance unit's `media.json` shape
(`verification/acceptance/media.py`), so the sample that run selects imports
unchanged:

```json
{"sample": [
  {"identifier": "BHC_006_AF_001", "source": "BHC006_GMT_AF.xlsx:12", "file": "Artifacts/foo.jpg"}
]}
```

`identifier` is the join key: the item whose `dcterms:identifier` literal equals
it, restricted to `--item-set` when given. `file` resolves against `--source`
when given, else against the manifest's directory. `source` is provenance;
`media.source` holds the file name instead (`Media/Ingester/Upload.php:61`). An
entry may carry `item` to name the item directly. With `--source DIR` and no
manifest the command walks the directory for the extensions `media.py` selects
and links each file by its stem. `--item ID` forces one item for every file.

`--store DIR` is the store root (`file_store.local.base_path`), the directory
that holds `original/`. It defaults to `files` beside the database. The stored
path is `original/<storage_id>.<extension>`, the same rule `FileStore::storage_path`
uses and the same value `Entity/Media.php:223` reads back as the file name.

The row carries the fields `File/TempFile.php:420` sets: `source`,
`media_type`, `storage_id`, `extension`, `sha256`, `size` and `has_original`,
with `ingester` `upload` and `renderer` `file`. `media_type` comes from the
file's bytes (`omeka_media::media_type`, the port's `finfo`). A file the install's
file validation settings refuse (`media_type_whitelist`, `extension_whitelist`,
`disable_file_validation`) is reported as failed, with upstream's message.

Thumbnails for the decodable image types (gif, jpeg, png, webp) come from
`omeka-media`'s `ImageThumbnailer`, the port of `File/Thumbnailer/Gd.php`. Each
imported image gets the three configured sizes under `large/`, `medium/` and
`square/`, at `<type>/<storage_id>.jpg`, and its `has_thumbnails` is true. A file
whose bytes are not an image is stored and imported as before, with no
thumbnails.

A second run skips every file whose `(item_id, source)` pair already exists, so
no row and no bytes are written twice. Each file is reported as `imported`,
`skipped` or `failed` with a reason, then a summary line that includes how many
imports got thumbnails. The exit code is 1 when any file failed.

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
`--store DIR` root defaults to `files` beside the database, the same rule
`import media` uses.

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
whole store. `restore` reads it back into a target.

The archive is a plain (uncompressed) tar, so `tar tf backup.tar` lists it and
`tar xf` extracts it without this CLI. Three kinds of entry:

```
romeka.db          the database copy
store/<path>      every file under the store root, at its store-relative path
manifest.json     the commit, the counts and the checksums
```

`--store DIR` is the store root, the directory holding `original/` and the
thumbnail types; the same path `import media` writes and `serve` reads. Every
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
- `Romeka\Job\ImportArchive` and `Romeka\Job\UndoImport` (Romeka's own):
  `importId`. The server's import routes compose them; each runs only for an
  import the server moved to `running` or `undoing` and linked to that job. The
  run appends each progress line to `job.log` and ends the import `done` or
  `failed`, its undo window open either way; a run that already wrote is never
  repeated. The undo ends it `undone` and closes the window; an undo that fails
  leaves `undoing`, and a later undo converges.

`romeka job run <id>` takes `--store` (default `files` beside the database),
`--base-url` (default `http://localhost`) and `--archive-root` (the import is off
without it). `romeka serve` passes all three, with the store `--store` gave it,
when it starts a job.

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
- `import media` derives `media_type` from the file extension; upstream sniffs
  the content with `finfo` and then applies its alias table
  (`File/TempFile.php:296`). Its thumbnails come from the `image` crate, not GD;
  the port keeps GD's two strategies and the JPEG output, so a derivative can
  differ from GD's bytes while matching its size.
- `src/store.rs` holds the SQL `omeka-db` has no repository for. It should move
  into `omeka-db` when those repositories exist.
