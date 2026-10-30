# Changelog

Notable changes to Romeka S. This history starts at the import commit: everything before it is the
Omeka S port this fork was taken from, and it is not replayed here.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project follows
semantic versioning.

## [Unreleased]

### Added

- A values board for each field (Fields › "Its values", `#/fields/<id>`): every text the field holds on
  items, item sets and media, with counts. Drag a text to another field's column, or use "Move to", to move it
  on every record you may change. The move is checked first, names the item types whose form lacks the
  field, records item history, and can be undone for the last move of the visit. New routes
  `GET /api/romeka/field-values/{property}` and `POST /api/romeka/field-values/move`.
- The Items field filter can match one exact text (`field=<id>:is:<text>`), besides empty and filled.

### Changed

- A field has one name on every admin screen outside Standards: the item type's own name for it, else
  its plain name ("Place", "Reference number"), else its vocabulary label. The item record, the item
  form, History, Linked from, the Items filters, the bulk preview and the site editor's field pickers
  used the vocabulary label ("Spatial Coverage", "Identifier") while the Fields page and item types used
  the plain name. A lint rule now refuses a page that reads a field's vocabulary label itself. Only the
  displayed name changed: property labels and item type field names are stored as before.
- The item form names value kinds as item types do: "Web address" for URI and "Link to a record" for
  Resource, with "Record number" for the linked record's id.
- A record with no title is "[Untitled] #12" wherever a number tells it apart; the bulk preview, bulk
  delete, History and the values board each worded it differently.

### Security

- `PUT /api/users/<id>` checks `update`, `change-role`, `change-role-admin` and `activate-user` as upstream's
  `UserAdapter` does. Before, any signed-in user could set their own `o:role` (to `global_admin` too) or
  `o:is_active`, and a site admin could edit a global admin's account.

## [0.2.1] - 2026-10-24

### Changed

- A server runs at most four job processes at once, and later jobs wait their turn in order. Each job opens
  its own database pool, so this bounds the connections a burst of jobs takes. A job still waiting when the
  server stops ends `error` with a log line.
- The server closes its database pool when it stops, so a SQLite file checkpoints its write-ahead log.

### Fixed

- An item, item set or media save replaced its values in separate statements. Two concurrent saves could leave
  the union of both value sets, and a failure halfway left a half-written resource. The values and their
  annotations are now replaced in one transaction.
- A tus upload chunk or delete that hit the request timeout released the upload's lock while its write went
  on, so a retried chunk could write the same part at once. The lock is now held until the write ends. The
  lock map kept an entry for every abandoned upload, and a client that stalled mid-chunk held a thread for as
  long as its connection stayed open. Both are fixed.
- The IIIF image size cache grew without a bound; it now starts over at 100,000 entries.
- Publication save and publish, the import run, and undo read a row and wrote from it in a plain transaction.
  On SQLite a commit in between failed them with `SQLITE_BUSY`; on MySQL the stale row was written back. They
  now take the write lock first.
- Undo overwrote an edit made between its check and its restore. It now keeps that resource as edited.
- Two concurrent tag promotions into one term list dropped each other's words, and a merge could too. A write
  planned from stale words is refused and planned again.
- Setting a password by key wrote back the whole user row, undoing a role or activation change made meanwhile,
  and one key could be used twice. The key and the password now change together, and only those columns.
- A rescan could replace the file index of an import that had started running. It is now refused while the
  import runs or is undone.
- Two concurrent writes of the same slug, email, prefix, namespace, local name or label answered the second
  with a 500 and the raw database message. It now gets the same 422 field error the write's check gives.

## [0.2.0] - 2026-10-24

### Added

- Import plan v3. A plan holds the archive as a tree: set levels, and series that each have their own
  pattern, lists and slot rules. The server reads a v2 plan and lifts it to one series; it stores and answers
  only v3 (`docs/import-v2.md`, "The plan").
- The import proposer. From the counts a scan gathers, the server proposes the tree, with the evidence for each
  choice as counts: folder levels, name parts, lists, what a list's rows describe, set candidates and pairing
  rules. The plan's `edits` keep the person's clicks, and the server replays them on each new proposal. A
  series also holds corrections (join two files, pair a row with a file, leave a row out), which the proposer
  offers and the derive reads as decisions.
- Simple has three steps: Structure, Check and Add. Structure shows the proposal as a tree, and each answer is
  one click. Advanced is the rule view of the same tree. The old Simple and Advanced addresses open Structure
  at the question they held. A folder's item type can come from a list column, per value. Every Advanced rule
  card has one value map control, and a list column's values can be cleaned.
- An import run that a server restart killed goes on by itself at boot, and a run the person stopped does
  not. A stopping server sends each job a TERM, and an import run then ends after its current record as
  `interrupted`.
- The add-media panel on an item's page: files, a link (a file to download, YouTube, oEmbed, an IIIF image or
  manifest) or pasted HTML. Files go up as resumable per-user uploads (`/api/romeka/uploads`, tus 1.0,
  migration 0009). The server answers the upload limits and probes a pasted link before a media is created.
- Every media kind is accepted and named (image, audio, video, PDF, document, model, captions, embed,
  manifest). The media list filters by kind. Camera RAW shows from its embedded preview. SVG is stored
  filtered. An asset takes any allowed file.
- Derivatives leave the request. A media create stores the original and queues its thumbnails and access copy
  in `romeka_media_derivative` (migration 0010). One worker drains the queue and yields to a running import.
  `romeka media derivatives` is the backfill, with `--retry`, `--redo` and `--media`.
- Access copies: an H.264/AAC MP4 for video and an AAC M4A for audio, up to 1080 lines on the short side; a
  full-size JPEG master for camera RAW and other image types; a PDF for an office document, through
  LibreOffice. The players, the readers and IIIF use the copy, and the original is never changed. Video gets a
  poster and a PDF its first page. A media page has a Retry for a failed copy.
- `deploy/compose.office.yaml` adds the office worker: the `office-tools` image (LibreOffice, `vips`,
  `libraw`, `ghostscript`) runs `romeka media derivatives --retry` hourly against the same database and
  store. A copy that failed 3 times stays failed.
- Notes on media: a W3C Web Annotation Protocol server under `/annotations/` (migration 0008). A note marks
  the whole file, a region of an image, a time range of audio or video, or a box on a video's frames over a
  time range (migration 0011). The admin media page draws and lists them; the item page annotates each file.
  Visitors see notes on the picture and listed under it.
- Captions are annotations, with a track per language. They import from WebVTT or SubRip with a dry run, serve
  as `captions.vtt`, and read as a transcript on the site. The last caption replace can be undone.
- IIIF: audio, video and PDF canvases; notes in manifests; a manifest per media; and IIIF Content Search 2.0
  over an item's notes (`/iiif/search/2/item/{id}`).
- The site: an item page shows its first file whole and the rest as a strip; a PDF reads in the page; a 3D
  model turns on request; `?t=` opens a player at that moment; the original downloads under its own name; a
  "Use elsewhere" block gives IIIF links, a citation and `og` meta.
- The admin media page shows the media itself. An oEmbed or HTML media renders in a sandboxed frame from
  `/api/romeka/media/{id}/render`, and an allow-listed provider's player and a YouTube video play in place.
- `romeka serve` and the CLI open a MySQL 8.0 or MariaDB 11 server with `--database-url` (or
  `ROMEKA_DATABASE_URL`). `backup` and `restore` refuse a server.
- The image builds a headless `ffmpeg` 7.1.5 with dav1d and x264 from pinned source
  ([`packaging/README.md`](packaging/README.md), The ffmpeg build).

- Describe: the admin's Vocabulary group becomes Describe, ordered around the work. Item types (resource templates)
  are a plain-words editor with a live preview of the item form, starter kinds and a class picker; a Fields page
  searches properties by meaning; Standards holds vocabularies, properties and classes as tabs, with a "?" that
  shows each term's stored definition. The old routes still open the right screen.
- The resource template drives the item, item set and media forms: its rows, labels, required and private flags,
  data types, default language and class. A write that misses a required template value answers 422, as upstream.
- Term lists: the Custom Vocab module built into core (`custom_vocab`, `/api/custom_vocabs`, `customvocab:<id>`
  data types), with a type-ahead picker in the item editor. Migration 0002.
- Tags: a Tags field writes plain `dcterms:subject` values; the Term lists page counts tags, merges look-alike
  spellings and promotes tags into a list.
- Import and update a vocabulary from an RDF file or URL, with a diff before an update. The URL fetch refuses
  private and loopback addresses on every redirect hop.
- Publish a term list (SKOS) or a vocabulary (RDFS/OWL) at `/vocab/<slug>/`, by content negotiation, with versions;
  a published term can only be added or deprecated. Migration 0003.
- Lessons: a short note the first time an action rests on a standard, once per user, with a "?" to recall it and a
  "Show lessons" switch in the user settings.
- Import v2 replaces the archive import below (`docs/import-v2.md`). One plan, saved as a draft, drives a folder
  on the server (`ROMEKA_ARCHIVE_ROOTS`, `name=path` pairs; `ROMEKA_ARCHIVE_ROOT` stays as the root named
  `archive`), files and ZIPs uploaded from the browser (tus, into the import's staging area), and lists (CSV,
  TSV, XLSX, XLS, ODS). The admin had a guided Simple mode (Your files, What we found, Your list, Where they go,
  Check, Add) and an Advanced mode with eight tabs; plan v3 below replaces both. Check counts flags by kind with a policy each (keep, repair, leave); only plan
  problems block a run. An import can create, append, revise, update, replace or delete, and a delete runs only
  with its count typed. Undo puts back what the import changed and keeps what was edited since, until
  `force`. The run, undo, rescan, unzip and staging sweep are jobs (`Romeka\Job\ImportRun`, `ImportUndo`,
  `ImportScan`, `ImportUnzip`, `ImportSweep`). `romeka import run`, `undo` and `list` are the command line's
  front end, with `--json` output and Ctrl-C as Stop; `romeka import csv` and `romeka import media` are gone.
  Migration 0006.
- Archive import (replaced by import v2 above; kept here as history): with `ROMEKA_ARCHIVE_ROOT` set, the admin's
  Import page (under Content) turned a server folder into items, item sets and media in seven steps: Folder (mark an example file name, or write a regex), Item sets, Item
  types, Sources (a CSV, read with detection and Windows-1252 repair, and EXIF tags), Grid (problems with bulk
  fixes, cell edits), Check (a dry run) and Run. The run and the undo are jobs (`Romeka\Job\ImportArchive`,
  `Romeka\Job\UndoImport`) in a `romeka job run` child, and the Jobs page shows their log while they run. The last
  import can be undone until the next one starts. TIFF originals get thumbnails. `romeka serve --store` and
  `romeka job run --store --base-url --archive-root` are new; `deploy/compose.archive.yaml` mounts a folder
  read-only. Migration 0005.
- Portainer stacks. `deploy/portainer/stack.yaml` faces the internet and gets its own certificate;
  `deploy/portainer/stack.proxy.yaml` serves plain HTTP behind a proxy that already terminates TLS.
  Neither builds an image or mounts a file from a checkout. `deploy/PORTAINER.md` is the runbook.
- Published images. The `image` workflow pushes `ghcr.io/micahchoo/romeka-s` and
  `ghcr.io/micahchoo/romeka-s-office-tools` on every push to `main` and every `v*` tag, and the
  `release` workflow publishes the GitHub release for a tag from this file.
- `ROMEKA_MODE=upgrade` runs the install and the schema migrations, then serves, for a host with no
  easy one-off container.
- `ROMEKA_TRUSTED_PROXIES` names the proxies in front of the origin whose `X-Forwarded-For` it believes.

### Changed

- The phone nav strip keeps the active link in view when a link appears after the first render, as the Import
  link does once its probe answers.

- Corrected the pages that still called the golden fixtures missing and two `omeka-api` test targets
  red: the fixtures are at [`fixtures/api/`](fixtures/api/README.md), `docs/gen/check_wire.py` reads
  them and exits non-zero on an empty set, and the README's "What is not ported" now names the admin
  SPA gaps.
- Added the one-way rule for site pages to [`docs/compat.md`](docs/compat.md): a page made here
  must show the same content in upstream, and a block this product adds is stored under an upstream
  layout with a generated plain rendering.
- Added two rows to [`docs/compat.md`](docs/compat.md) for the deliberate differences: a write that
  names an unknown property term answers 422, and `GET /api` answers JSON-LD rather than HTML.

### Fixed

- The login limit could be dodged. Caddy passed a client's own `X-Real-IP` through, and the server
  reads that header first, so a caller could name a fresh address on every attempt. Caddy now sets it
  from the client address it saw (`deploy/Caddyfile`).
- A list sent again in place of another may be a workbook. The server now reads the new file's header
  (`POST …/sheets/header`); the admin read it as CSV text and refused every `.xlsx`, `.xls` and `.ods`.
- An admin edit no longer removes an item from its sites, a media save no longer erases its values, and
  `resource:item` types and URI languages survive a save.
- Upstream drops a blank literal or URI value, and reads a URI value's language from `o:lang`; so does the port.
- Vocabulary terms are case-sensitive, as upstream's are (migration 0004): FOAF stores both `givenName` and
  `givenname`. The default vocabularies install from upstream's RDF files, and `romeka install` corrects an older
  install's FOAF labels and comments.
- The admin shell keeps its header and nav at their own height on a phone, and the nav strip scrolls to the active
  link; a fieldset no longer overflows a 375 px screen.
- A site page renders its normal or grid layout and each block's `layout_data` (grid position and
  span, class, alignment, sizes, padding, background colour) with upstream's markup.
- A block's `o:attachment` is read, written and rendered. A missing item, or a media that is not the
  item's, is stored as null, as upstream's `hydrateAttachments` does.
- A `PATCH` that names no value object keeps every stored value; one that names a valid value still
  replaces the whole set, as upstream's `ValueHydrator` does.
- Every reference a body names resolves before the first write, so a refused reference leaves the
  stored row untouched and answers the 404 `findEntity` throws.
- A site `PUT` removes every page its `o:page` list omits, and so does a `PATCH` that carries the
  member.
- A `DELETE` answers the detached entity, with `o:id` null and `@id` the collection URL, the shape
  `Api\Manager::finalize` renders after the flush.
- The admin's delete controls consume that detached answer, so the twelve `delete*` methods return
  `void` and the boundary only checks the shape (`assertDetached` in `web/admin/src/api/parse.ts`).
- An email that is not `local-part@hostname` answers the Laminas `INVALID_FORMAT` message.
- The `id` filter narrows every entity collection, and `resource_classes` and `properties` read
  `term`, `vocabulary_prefix` and `vocabulary_namespace_uri`.
- A non-numeric `page` leaves a list unbounded, because `SetBrowseDefaults` writes `page=1` only
  when the caller sent none.
- `api_resources`, `data_types` and `modules` answer no `Omeka-S-Total-Results` header and ignore
  the page window, because their adapter answers a plain `Response`.
- The `Link` header renders nested and repeated parameters the way Laminas does, through the
  request array `read/php.rs` rebuilds.
- Text sorts order through one case-fold rule (`repos::compare_text_ci` in `omeka-db`) that matches
  SQLite's `COLLATE NOCASE`, so a Rust comparator and a `COLLATE NOCASE` clause agree.
- A value object's `@annotation` reads and writes a `value_annotation` resource, dropped when the
  value that carried it leaves the set.

### Security

- `rustls-webpki` 0.103.16 and `webpki-roots` 1.0.11 stop trusting the distrusted emSign
  intermediates (RUSTSEC-2026-0382, RUSTSEC-2026-0381). They verify every outbound TLS connection:
  link probes, IIIF and oEmbed fetches, SMTP.
- `ctutils` 0.4.5, under the S3 request signing, generates its masks without a branch on the secret
  (RUSTSEC-2026-0385).

## [0.1.0] - 2026-10-17

The import. Romeka S is the Omeka S port's tree under its own name, with the same data model, the
same `/api` surface and the same license as upstream Omeka S, forked at
`5e413f009c447f4b587803ab23a9cec7d228f6e8` (see [`NOTICE`](NOTICE)).

### Added

- The account flows. Sign in and sign out on the admin header, with the session held in a
  server-side row behind an `HttpOnly`, `SameSite=Lax` cookie; `GET /api/me` as the session probe;
  the invite and forgot-password links; and the set-password page behind its one-time key. With no
  SMTP relay configured, the invite and reset links are written to the server log and returned to
  the administrator, so an installation without mail onboards people anyway
  ([`crates/omeka-server/README.md`](crates/omeka-server/README.md), Accounts).
- The admin sign-in panel carries both forms: the API key pair and the password login.
- Set-password and forgot are their own lazy chunks, so a reader of an admin page downloads neither.
