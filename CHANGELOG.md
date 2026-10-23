# Changelog

Notable changes to Romeka S. This history starts at the import commit: everything before it is the
Omeka S port this fork was taken from, and it is not replayed here.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project follows
semantic versioning.

## [Unreleased]

### Added

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
  TSV). The admin has a guided Simple mode (Your files, What we found, Your list, Where they go, Check, Add) and
  an Advanced mode with eight tabs. Check counts flags by kind with a policy each (keep, repair, leave); only plan
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
