# Romeka S

Romeka S is a port of [Omeka S](https://github.com/omeka/omeka-s), the PHP application for
collections of digital items. It keeps the Omeka S data model and the Omeka S API. It replaces the
PHP application with a Rust server (`crates/`) and a strict-TypeScript admin app (`web/admin/`).

## How it differs from Omeka S

Romeka S answers the same `/api` as Omeka S, with the same `o:` JSON-LD prefix, members and status
codes. It reads and writes the same tables and columns. Thus a client written for Omeka S works
against Romeka S, and an Omeka S database holds the same records here.
[`docs/compat.md`](docs/compat.md) lists every difference, each against the file that carries it.

The changes are in what runs behind the API:

- One `romeka` binary is the server, the job runner and the command line. There is no PHP.
- The database is a SQLite file by default. The same binary also runs on MySQL 8.0 or MariaDB 11
  with `--database-url`.
- Some things that Omeka S gets from modules are built in. These include an archive import, term
  lists (the Custom Vocab module), IIIF manifests and an IIIF Image API server, notes and captions
  on media, and file storage in an S3-compatible bucket.
- A file of a private item is served only to a person who may see that item. Omeka S serves every
  stored file to anyone who has its URL.

## Run it on your machine

You need Rust 1.93 or newer (the workspace is edition 2024) and Node 22 or newer. The container
image builds with these versions (`packaging/Dockerfile`).

1. Build the binary, then create a database and its first administrator:

   ```
   cargo build --release -p omeka-cli
   ./target/release/romeka install -d romeka.db \
       --email admin@example.com --name Admin --password 'change this'
   ```

   `install` creates the schema and seeds four vocabularies (Dublin Core, Dublin Core Type, BIBO and
   FOAF), the `Base Resource` template and the administrator. Every step is idempotent. Run the same
   command after an upgrade to bring the schema up to date.

2. Start the server. The base URL is the admin's address in the next step, because the server
   refuses a signed-in write that comes from another origin:

   ```
   ./target/release/romeka serve -d romeka.db --host 127.0.0.1 --port 8080 \
       --base-url http://localhost:5173
   ```

3. In a second terminal, start the admin app. Its dev server forwards `/api`, `/s` and the other
   server routes to the address in `ROMEKA_DEV_TARGET`:

   ```
   cd web/admin && npm ci
   ROMEKA_DEV_TARGET=http://127.0.0.1:8080 npm run dev
   ```

4. Open `http://localhost:5173` and sign in with the email and the password from step 1.

The server does not serve stored files, so in this setup an uploaded image shows no picture. The
container in [`deploy/`](deploy/DEPLOY.md) puts Caddy in front of the server to serve the admin app
and the files. [`docs/operator-guide.md`](docs/operator-guide.md) says what any other front server
must do.

To use the API without the admin app, mint an API key. `key add` prints `key_identity` and
`key_credential` once each, and no later output shows the credential again:

```
./target/release/romeka key add -d romeka.db --owner admin@example.com
curl http://127.0.0.1:8080/health
curl "http://127.0.0.1:8080/api/items?key_identity=ID&key_credential=SECRET"
curl -X POST -H 'content-type: application/json' \
    -d '{"dcterms:title":[{"property_id":1,"type":"literal","@value":"A first item"}]}' \
    "http://127.0.0.1:8080/api/items?key_identity=ID&key_credential=SECRET"
```

In a new install, `property_id` 1 is `dcterms:title`.

## What it does

Each area below links to the page that describes it in full.

- **API.** Eighteen adapters, the browse parameters, the JSON-LD documents, the `Link` and
  `Omeka-S-Total-Results` headers and the error documents. [`docs/api.md`](docs/api.md) is
  generated from the source and lists every route, member and parameter.
- **Admin app.** Items, item sets, media, assets, users, sites, jobs and the vocabularies, with the
  create, edit and delete forms that the API supports. Sign-in is by email and password. The API
  key form is behind "Use an API key". The items list can add selected items to a site or remove
  them from it. [`docs/user-guide.md`](docs/user-guide.md), The admin pages.
- **Describe.** Item types (resource templates) have an editor with a live preview of the item
  form. The template sets the rows, labels, required values and data types of the item, item set
  and media forms. Term lists are the Custom Vocab module built into core. The Subject field takes
  tags, written as plain `dcterms:subject` values. A standard (vocabulary) can be imported or updated
  from an RDF file or URL, with a diff before an update. A term list or a standard can be published at `/vocab/<slug>/` in Turtle,
  JSON-LD, RDF/XML, N-Triples or HTML. [`CHANGELOG.md`](CHANGELOG.md), Unreleased.
- **Archive import.** The Import page turns a folder on the server, or files and ZIP archives sent
  from the browser, into items, item sets and media. It reads spreadsheet lists (CSV, TSV, XLSX,
  XLS and ODS) and the EXIF, IPTC and XMP tags of the files. Simple mode has three steps:
  Structure, Check and Add. Structure shows the archive as a tree. The server proposes which folders
  are series, where the files of each item end, and which list describes them. It also offers
  corrections that you accept with one click: put a file into another file's item, pair a row with a
  file, or leave a row out. Advanced mode shows the same tree with the rules of each folder. An
  import can create, append, revise, update, replace or delete. A run and an undo are jobs, and an
  undo keeps what people edited after the import. [`docs/user-guide.md`](docs/user-guide.md), Import
  files as items, and [`docs/import-v2.md`](docs/import-v2.md) for the contract.
- **Media.** Each media gets three JPEG thumbnails (800, 400 and a 200 pixel square). When a browser
  cannot show the original, the derivative worker makes an access copy: an MP4 for video, an M4A for
  audio, a full-size JPEG for camera RAW, PSD, HEIC and other image formats, or a PDF for an office
  document. The original never changes. External tools make some of these copies (`ffmpeg`,
  `pdftoppm`, `exiftool`, `vips`, LibreOffice). A missing tool leaves the media with no copy and
  says which tool it needs. The item page can add files, URLs, YouTube, oEmbed and IIIF media.
  Outbound fetches go through one guard that refuses private and loopback addresses.
  [`packaging/README.md`](packaging/README.md), The media tools, and
  [`crates/omeka-media/README.md`](crates/omeka-media/README.md).
- **Notes and captions.** A note on a media is a W3C Web Annotation. It can mark the whole file, a
  region of an image, or a time range of audio or video. Captions import from WebVTT or SubRip and
  are served as `captions.vtt`. The annotation routes are under `/annotations/`, and the IIIF
  Content Search route searches the notes of an item. [`docs/compat.md`](docs/compat.md), the
  annotation rows.
- **Public sites.** The site builder opens each site on a hub with its pages, Navigation, Items,
  Users, Theme and Settings. The page editor shows the page as the public site renders it, at
  desktop, tablet and phone widths, with undo and a version history. There are 16 block layouts
  from upstream and 9 Romeka blocks, among them a hero, a gallery, a map and a timeline. A Romeka
  block stores a plain rendering, so Omeka S shows the same content. The theme has five presets and
  refuses a palette that fails WCAG AA contrast. Item pages link a Mirador viewer and an IIIF
  manifest. [`docs/user-guide.md`](docs/user-guide.md), Build a site.
- **Languages.** The admin app and the public site ship nine locales: English, Spanish and seven
  Indic locales (Hindi, Marathi, Malayalam, Tamil, Kannada, Telugu and Nepali). Spanish is written
  by hand from upstream's `es.po`. The Indic catalogues are machine-translated by
  `web/admin/scripts/translate-locales.mjs`. [`web/admin/README.md`](web/admin/README.md), Locales.
- **Command line.** `romeka` has these commands: `install`, `serve`, `user`, `key`, `import`
  (`template`, `run`, `undo`, `list`), `export` (`jsonld`, `csv`), `media` (`thumbnails`,
  `derivatives`, `sweep`), `job` (`run`, `list`, `show`, `stop`), `backup` and `restore`.
  `romeka <command> --help` lists the flags. [`crates/omeka-cli/README.md`](crates/omeka-cli/README.md).
- **Moving from Omeka S.** The `omeka-migrate` crate copies an Omeka S MySQL database into a SQLite
  file. It is a library function, `migrate_from_upstream`, and no `romeka` command calls it yet.
  [`crates/omeka-migrate/README.md`](crates/omeka-migrate/README.md).

## What it does not do yet

- There are no PHP modules and no PHP themes. Extension is a Rust registry
  ([`crates/omeka-plugin/README.md`](crates/omeka-plugin/README.md)).
- Some routes answer `501` because they have no handler. [`docs/api.md`](docs/api.md) marks each
  one. `POST /api/jobs` is one of them, because upstream's `JobAdapter::hydrate` is empty.
- The admin markup is not upstream's. The workflow is the same.
- Most admin lists have no search, filter or sort control. A browse sends the fixed query in
  [`web/admin/src/app/list.tsx`](web/admin/src/app/list.tsx).
- There is no batch edit and no batch delete.
- The modules page shows module state only. It has no install, activate or configure control
  ([`web/admin/src/pages/modules/page.tsx`](web/admin/src/pages/modules/page.tsx)).
- Resource templates import only from the command line (`romeka import template`). The admin app
  does not export them.
- The admin app cannot stop a job. `romeka job stop` can
  ([`web/admin/src/pages/jobs/detail.tsx`](web/admin/src/pages/jobs/detail.tsx)).
- There is no maintenance mode.
- Upstream's full translation catalogues are not ported. Only the nine locales above ship.
- Fulltext ranking on SQLite is FTS5's, and it differs from MySQL's.

## Deploy it

[`deploy/DEPLOY.md`](deploy/DEPLOY.md) is the runbook for one container that holds the server and
Caddy. It covers the first install, TLS, mail, backup, restore, upgrade and rollback. Two compose
files add to `deploy/compose.yaml`. `compose.archive.yaml` mounts a host folder read-only for the
import. `compose.office.yaml` adds the office worker, which makes the PDF copies of office
documents with LibreOffice. [`packaging/README.md`](packaging/README.md) describes the image and its
media tools. For an install without containers, [`docs/operator-guide.md`](docs/operator-guide.md)
covers the process, its settings, its data on disk, MySQL, object storage and backup.

## Contribute

[`CONTRIBUTING.md`](CONTRIBUTING.md) has the build, the checks and the commit rules. An agent follows
[`AGENTS.md`](AGENTS.md). [`.github/workflows/ci.yml`](.github/workflows/ci.yml) is what a pull
request runs: `cargo fmt`, `cargo clippy`, `cargo test --workspace`, the admin type check, lint,
unit tests, build and end-to-end tests, the bundle budget (`npm run perf:check`, decisions in
[`web/admin/PERF.md`](web/admin/PERF.md)), the API reference check and the image build. Report a
vulnerability as [`SECURITY.md`](SECURITY.md) says. [`docs/README.md`](docs/README.md) is the map of
the other documents.

## License and provenance

This repository is a fork of [`omeka/omeka-s`](https://github.com/omeka/omeka-s) taken at commit
`5e413f009c447f4b587803ab23a9cec7d228f6e8`. It carries the same license, the GNU General Public
License version 3. The full text is [`LICENSE`](LICENSE). The Rust core and the TypeScript admin are
a reimplementation, not a copy. They read and write upstream's schema and answer upstream's API.
[`NOTICE`](NOTICE) records the upstream project, the license, the fork point and the third-party
fonts and viewers. Upstream is credited there and in the license text.
