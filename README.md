# Romeka S

Romeka S keeps the Omeka S data model and the Omeka S API, and replaces the PHP application with a
Rust core and a strict-TypeScript admin.

That is the whole idea. An Omeka S client keeps working, because the `/api` surface, the JSON-LD
`o:` prefix and the response members are the same. An Omeka S database keeps working, because the
tables, the columns and the schema are the same. What changes is the implementation behind them.

## Provenance

This repository is a fork of [`omeka/omeka-s`](https://github.com/omeka/omeka-s) taken at commit
`5e413f009c447f4b587803ab23a9cec7d228f6e8`, and it carries the same license, the GNU General Public
License version 3. The Rust core and the TypeScript admin are a reimplementation, not a copy. They
read and write upstream's schema and answer upstream's API. [`NOTICE`](NOTICE) records the upstream
project, the license and the fork point. Upstream is credited there and in the license text.

## Quick start

Prerequisites: stable Rust 1.93 or newer (the workspace is edition 2024), and Node 22 or newer.

```
cargo build --release -p omeka-cli
./target/release/romeka install -d romeka.db \
    --email admin@example.com --name Admin --password 'change this'
./target/release/romeka key add -d romeka.db --owner admin@example.com
```

`install` creates the database, applies the schema, and seeds four vocabularies, the `Base Resource`
template and the first administrator. Every step is idempotent, so the same command is also the
upgrade step. `key add` prints `key_identity` and `key_credential` once each. Store them: the
credential is in no later output.

Start the server and talk to it:

```
./target/release/romeka serve -d romeka.db --host 127.0.0.1 --port 8080 \
    --base-url http://127.0.0.1:8080

curl http://127.0.0.1:8080/health
curl "http://127.0.0.1:8080/api/items?key_identity=ID&key_credential=SECRET"
```

A write needs the key pair, and `property_id` in a value is the property row's id (`dcterms:title` is
1 in a default install):

```
curl -X POST -H 'content-type: application/json' \
    -d '{"dcterms:title":[{"property_id":1,"type":"literal","@value":"A first item"}]}' \
    "http://127.0.0.1:8080/api/items?key_identity=ID&key_credential=SECRET"
```

The server answers the API, the account routes, the public site under `/s/`, the bundled theme's
files under `/assets/`, and the probes `/health` and `/ready`. It serves no admin bundle and no
stored media bytes. Build the admin and put a static origin in front:

```
cd web/admin && npm ci && npm run build      # writes web/admin/dist
```

The origin serves `dist/` at `/`, the media files under `/files/`, and forwards `/api`,
`/api-context`, `/health`, `/ready`, `/s/` and `/assets/` to the server.
`web/admin/e2e/harness/proxy.ts` is a working example of that origin, and
`packaging/nginx.conf.template` is the one we ship. One constraint to know before you write your
own: the admin bundle's own chunk directory is also `assets`, so the SPA's files and the theme's
files share a URL prefix. See [`docs/operator-guide.md`](docs/operator-guide.md).

Sign in to the admin with the key pair in the header's sign-in form. There is also a password login
on the same form for accounts that were invited by email.

## What works

- The API carries 17 adapters, the browse parameters, the JSON-LD documents, the `Link` and
  `Omeka-S-Total-Results` headers, and the error documents.
  [`docs/api.md`](docs/api.md) is generated from the source and lists every route, member and
  parameter.
- The admin SPA covers items, item sets, media, users, sites, site pages, the vocabulary family,
  assets and jobs, with the create, edit and delete forms the API supports.
- The public site renders the bundled theme, the page blocks, the item, item-set and media views
  and site search.
- The command line runs install, users, API keys, template, CSV and media import, JSON-LD and CSV
  export, thumbnails, jobs, backup and restore.
- Sessions and passwords work: sign in, sign out, invite, forgot and set-password links, with the
  link written to the log when no SMTP relay is configured.
- `omeka-migrate` copies an Omeka S MySQL database into the SQLite database Romeka S runs on
  ([`crates/omeka-migrate/README.md`](crates/omeka-migrate/README.md)).

## What is not ported

These are decisions, not surprises. Each one names where it is stated and measured.

- There are no PHP modules and no PHP themes. Extension is a Rust registry
  ([`crates/omeka-plugin/README.md`](crates/omeka-plugin/README.md)).
- Some routes answer `501`. Each one has no handler, so it is not built yet rather than broken, and
  the generated route table in [`docs/api.md`](docs/api.md) marks every one of them.
- `POST /api/assets` and `POST /api/jobs` stay unbuilt, and a media file arrives through
  `romeka import media` rather than an API upload.
  [`crates/omeka-api/README.md`](crates/omeka-api/README.md) gives the reason for each.
- There is no admin pixel parity. The workflow is the same, the markup is not.
- The full translation catalogues are not ported. Two locales ship.
- No call goes to a live external service, so oEmbed, YouTube and remote IIIF are out.
- Fulltext ranking is SQLite FTS5's, and it differs from MySQL's.

[`docs/compat.md`](docs/compat.md) is the page that lists the differences the product states about
itself, each against the file that carries it.

## Speed

Measured on this box by `npm run e2e`, which drives the built SPA against a real server in a browser
and records the run in `web/admin/perf/summary.json`, commit included. Medians of five runs:

| Path | Median |
| --- | --- |
| first contentful paint | 48.0 ms |
| items list render | 5.8 ms |
| item detail render | 5.4 ms |
| item create round trip | 8.8 ms |
| route navigation | 0.6 ms |

The initial payload is 88411 B of JavaScript gzip and 2353 B of CSS gzip, against a 102400 B budget
and a 10240 B per-chunk ceiling. `npm run perf:check` fails the build when either moves past its
budget, and `web/admin/PERF.md` is the decision log behind the numbers.

## Deploy

A container image, a compose file, a TLS proxy and the runbook for backup, restore, upgrade and
rollback live in [`deploy/DEPLOY.md`](deploy/DEPLOY.md). The image source is
[`packaging/`](packaging/README.md). For a bare-metal install, [`docs/operator-guide.md`](docs/operator-guide.md)
covers the process, its ports, its data on disk and the backup pair.

## Documentation

[`docs/README.md`](docs/README.md) is the map. The short version:

| Document | For |
| --- | --- |
| [`docs/user-guide.md`](docs/user-guide.md) | an administrator in the admin app, or a visitor to a public site |
| [`docs/operator-guide.md`](docs/operator-guide.md) | whoever runs, watches and backs up an instance |
| [`docs/api.md`](docs/api.md) | a developer integrating over HTTP |
| [`docs/compat.md`](docs/compat.md) | someone deciding how far Romeka S can stand in for an Omeka S install |
| [`CONTRIBUTING.md`](CONTRIBUTING.md) | a developer changing this repository |
| [`AGENTS.md`](AGENTS.md) | an agent changing this repository |

## Contributing

[`CONTRIBUTING.md`](CONTRIBUTING.md) has the build, test and commit rules. Reporting a
vulnerability is [`SECURITY.md`](SECURITY.md). Changes land as one commit per coherent step, with
the workspace tests and the admin checks green: `.github/workflows/ci.yml` is what a pull request
runs.

## License

GNU General Public License version 3. The full text is [`LICENSE`](LICENSE), and [`NOTICE`](NOTICE)
records the fork point and the upstream attribution.
