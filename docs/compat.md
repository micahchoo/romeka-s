# Compatibility with Omeka S

Audience: someone deciding how far Romeka S can stand in for an Omeka S installation.

## The claim, and the pair it is a claim about

Romeka S is compatible with upstream [`omeka/omeka-s`](https://github.com/omeka/omeka-s) at commit
`5e413f009c447f4b587803ab23a9cec7d228f6e8`. The claim is about a pair: that upstream commit and this
repository at the commit you are reading. [`NOTICE`](../NOTICE) records the fork point, and
[`CHANGELOG.md`](../CHANGELOG.md) starts the history there.

Compatible means two concrete things.

- **The API.** The `/api` routes, the operations, the accepted parameters, the JSON-LD documents,
  the `o:` prefix and the `@context` path, the `Omeka-S-Version` response header, the `Link` and
  `Omeka-S-Total-Results` headers, the status codes and the two error documents are upstream's. A
  client written against Omeka S works against Romeka S unchanged.
  [`api.md`](api.md) is generated from the source and lists every one of them.
- **The data model.** The tables, the columns, the vocabulary classes and properties, the value
  types and the serialized date shape are upstream's. [`crates/omeka-db/SCHEMA-NOTES.md`](../crates/omeka-db/SCHEMA-NOTES.md)
  holds the per-table notes, and `crates/omeka-db/tests/schema_fidelity.rs` reads the upstream
  schema and asserts the same objects.

An Omeka S installation moves onto it through `omeka-migrate`, which reads the MySQL database and
writes the same tables into SQLite ([`crates/omeka-migrate/README.md`](../crates/omeka-migrate/README.md)).
SQLite is the default engine and MySQL is a second backend behind a feature
([`crates/omeka-db/README.md`](../crates/omeka-db/README.md)).

## What deliberately differs

Each row names the file that carries the difference. A row here is a decision that was reviewed, not
a gap that was discovered.

| Difference | Where it is stated |
| --- | --- |
| SQLite is the default storage engine; MySQL is behind the `mysql` feature. Parity is behavioural, not SQL-level. | [`crates/omeka-db/README.md`](../crates/omeka-db/README.md) |
| The admin is a strict-TypeScript SPA over the same `/api`, not server-rendered PHP views. The workflow is the same; the markup is not. | [`web/admin/README.md`](../web/admin/README.md) |
| Public sites render in Rust through a `Theme` trait. PHP themes are not ported, so a site's `theme` setting names a bundled theme. | [`crates/omeka-site/README.md`](../crates/omeka-site/README.md) |
| Module extension is a Rust registry, not a PHP module loader. | [`crates/omeka-plugin/README.md`](../crates/omeka-plugin/README.md) |
| No route answers 401. A missing, malformed, unknown or inactive API key leaves the caller anonymous, and the access check answers 403. | [`api.md`](api.md), Errors |
| A write route with no handler answers 501. Upstream throws `OperationNotImplementedException`, which its JSON strategy maps to 500, so a caller cannot tell "not built" from "failed" there. | [`crates/omeka-api/README.md`](../crates/omeka-api/README.md) |
| `GET /api/data_types/<unknown>` answers 404 where upstream throws and answers 500. | [`crates/omeka-api/src/read/meta.rs`](../crates/omeka-api/src/read/meta.rs) |
| `POST /api/assets` (needs a multipart boundary) and `POST /api/jobs` (upstream fails at flush for every body) are unbuilt. | [`crates/omeka-api/README.md`](../crates/omeka-api/README.md) |
| `romeka import media` derives `media_type` from the file extension. Upstream sniffs the content with `finfo` and applies its alias table. | [`crates/omeka-cli/README.md`](../crates/omeka-cli/README.md) |
| Thumbnails come from the `image` crate, not GD. The two strategies and the JPEG output are kept, so a derivative can differ in bytes while matching in size. | [`crates/omeka-cli/README.md`](../crates/omeka-cli/README.md) |
| Fulltext search runs on SQLite FTS5, whose ranking differs from MySQL's. | [`crates/omeka-db/README.md`](../crates/omeka-db/README.md) |
| CSV export is this product's, because upstream has no CSV export at the fork point. The column set is the one `romeka import csv` reads back. | [`crates/omeka-cli/README.md`](../crates/omeka-cli/README.md) |
| `romeka user add` without `--password` prints the activation link instead of mailing it, and `POST /api/users/{id}/password-link` returns the link, so an installation with no relay still onboards people. | [`crates/omeka-server/README.md`](../crates/omeka-server/README.md) |
| There is no `site_setting` repository yet, so the site-search settings a caller sets are not read back and some blocks render with the defaults. | [`crates/omeka-server/README.md`](../crates/omeka-server/README.md) |
| `/s/<slug>/item-set/<id>` renders the item-set view. Upstream's `site/item-set` route reaches the item browse. | [`crates/omeka-server/README.md`](../crates/omeka-server/README.md) |
| The bundled theme's asset base is `/assets`, the same path prefix the admin bundle builds its chunks under, so one origin serving both collides on it. | [`docs/operator-guide.md`](operator-guide.md), Known limitations |

## What compatibility does not cover

- Third-party PHP modules and themes, Laminas and Doctrine internals, and the admin's exact markup.
- Full translation catalogues. Two locales ship.
- Live external services: oEmbed, YouTube and remote IIIF.
- Mail delivery: the interface and a log transport exist, and SMTP is optional.
- MySQL fulltext ranking particulars.
- A JSON-LD `@context` served from upstream's URL: `@context` points at this server's
  `/api-context`.

## Where the evidence is, and what is missing here

The differential evidence this fork was built against lives in the port's own repository, not in this
one: the captured upstream documents, the behaviour ledger, the scenario diffs and the Docker oracle
are not in this tree. Because of that, two test targets fail here rather than skip:
`crates/omeka-api/tests/golden.rs` and `crates/omeka-api/tests/property_wire.rs` read
`verification/fixtures/api/*.json`, which does not exist here yet. `cargo test --workspace` is red on
those two targets until the captured documents land.

What you can run and read in this repository:

```
cargo test --workspace                          # everything except the two fixture targets
cd web/admin && npm ci && npm run e2e           # the admin and the public site in a browser
cd docs && ./gen.sh --check                     # the API reference is current
crates/omeka-cli/scripts/verify-backup.sh       # backup and restore, byte for byte
```

| Artifact | What it answers |
| --- | --- |
| [`api.md`](api.md) | every route, parameter, member and access rule, generated from the source |
| [`crates/omeka-db/SCHEMA-NOTES.md`](../crates/omeka-db/SCHEMA-NOTES.md) | the schema, table by table, against upstream's |
| `crates/omeka-db/tests/schema_fidelity.rs` | the schema objects this build writes, against upstream's schema file |
| [`crates/omeka-migrate/README.md`](../crates/omeka-migrate/README.md) | how an Omeka S MySQL database is read and where the mapping refuses a source |
| [`crates/omeka-cli/scripts/verify-backup.sh`](../crates/omeka-cli/scripts/verify-backup.sh) | the backup archive, the manifest and a restore, end to end |
| [`web/admin/PERF.md`](../web/admin/PERF.md) | the frontend measurements, with the method and the decision behind each |
| [`docs/gen/check_routes.py`](gen/check_routes.py) | every exact claim in the route table, against a running server |

A claim in a crate README is a claim; the command beside it is the proof. Run the command.
