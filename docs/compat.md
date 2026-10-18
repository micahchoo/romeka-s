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
| `POST /api/jobs` (upstream fails at flush for every body) is unbuilt. `POST /api/assets` is built and takes a multipart body. | [`crates/omeka-api/README.md`](../crates/omeka-api/README.md) |
| A file's media type comes from the `image` crate's magic-byte sniff plus a UTF-8 text guess, not `finfo`'s libmagic, so a rejected, non-image upload can name a different media type in its error. | [`crates/omeka-media/src/media_type.rs`](../crates/omeka-media/src/media_type.rs) |
| `romeka import media` derives `media_type` from the file extension. Upstream sniffs the content with `finfo` and applies its alias table. | [`crates/omeka-cli/README.md`](../crates/omeka-cli/README.md) |
| Thumbnails come from the `image` crate, not GD. The two strategies and the JPEG output are kept, so a derivative can differ in bytes while matching in size. | [`crates/omeka-cli/README.md`](../crates/omeka-cli/README.md) |
| Fulltext search runs on SQLite FTS5, whose ranking differs from MySQL's. | [`crates/omeka-db/README.md`](../crates/omeka-db/README.md) |
| CSV export is this product's, because upstream has no CSV export at the fork point. The column set is the one `romeka import csv` reads back. | [`crates/omeka-cli/README.md`](../crates/omeka-cli/README.md) |
| `romeka user add` without `--password` prints the activation link instead of mailing it, and `POST /api/users/{id}/password-link` returns the link, so an installation with no relay still onboards people. | [`crates/omeka-server/README.md`](../crates/omeka-server/README.md) |
| There is no `site_setting` repository yet, so the site-search settings a caller sets are not read back and some blocks render with the defaults. | [`crates/omeka-server/README.md`](../crates/omeka-server/README.md) |
| `/s/<slug>/item-set/<id>` renders the item-set view. Upstream's `site/item-set` route reaches the item browse. | [`crates/omeka-server/README.md`](../crates/omeka-server/README.md) |
| The bundled theme's asset base is `/assets`, the same path prefix the admin bundle builds its chunks under, so one origin serving both collides on it. | [`docs/operator-guide.md`](operator-guide.md), Known limitations |
| A write body whose value names an unknown property term answers 422, with the errors document keyed by that term. Upstream's value hydrator sets a null property and answers 500. | [`crates/omeka-api/src/write/values.rs`](../crates/omeka-api/src/write/values.rs) |
| Stored HTML (html blocks, attachment captions, html media) is purified with `ammonia` against HTMLPurifier's default allowed set, at write and again at render, so rows written before the fix or copied by the migrator are covered too. The kept tags match upstream's; links gain `rel="noreferrer noopener"`, void elements are not self-closed, CSS values are not normalised, and `use_htmlpurifier` is not consulted: purification is always on. | [`crates/omeka-core/src/purify.rs`](../crates/omeka-core/src/purify.rs) |
| An `o:item_set`, `o:site` or `o:attachment[].o:media` entry whose `o:id` is a JSON float or bool is read as the number it names (200, or 404 when nothing has that id). Upstream's `ArrayCollection::get(string\|int)` throws a `TypeError` under `strict_types` and answers 500. Every other reference id reads as PHP's `is_numeric` does, numeric strings included. | [`crates/omeka-api/src/write/body.rs`](../crates/omeka-api/src/write/body.rs) |
| `GET /api` answers the JSON-LD resource listing. Upstream renders an HTML page from `omeka/api/root` at the same route. | [`crates/omeka-api/src/read/meta.rs`](../crates/omeka-api/src/read/meta.rs) |

## Site pages: the one-way rule

Romeka S may give a site page more than upstream can draw. The rule for that extra is one-way: a page
made here must show the same content when upstream renders it, perhaps plainer. A page edited in
upstream's admin is not promised to keep what only Romeka S draws.

The rule rests on what upstream does with data it does not know. Measured against the oracle at the
fork point, with a page posted through `/api` and read back:

| Sent | Upstream's API | Upstream's public page |
| --- | --- | --- |
| an extra key in a known block's `o:data` | stores it unchanged | ignores it; the block renders |
| an extra key in block or page `o:layout_data` | stores it unchanged | ignores it; the known keys apply |
| a block whose `o:layout` upstream does not know | stores it unchanged | an empty `<div class="block block-<layout>">`; the content is gone |

So an extra key degrades and an unknown layout does not. An upstream admin save is the second test.
Measured on the oracle, a page saved once through `admin/site/s/<site>/page/<page>/edit`:

| Where the extra key was | After the upstream admin save |
| --- | --- |
| block `o:data` | dropped: the form rebuilds `o:data` from its fields |
| block `o:layout_data` | kept: the form carries it as one hidden JSON field (`view/common/block-layout.phtml`) |
| page `o:layout_data` | dropped: the form rebuilds it from `PageLayoutDataForm` |

Every block this product adds obeys four consequences:

- It is stored under an upstream layout, usually `html`. Its own settings sit under
  `o:layout_data.romeka`, which upstream ignores and its admin keeps.
- The upstream part of the block, such as `o:data.html`, is the plain rendering. Romeka S writes it
  from the block's own renderer on every save, so nobody writes the fallback by hand.
- `o:layout_data.romeka` records a hash of the plain rendering it wrote. When the stored plain
  rendering no longer matches, someone edited the block outside this product: Romeka S renders the
  edited plain HTML and its editor marks the block, rather than overwrite that edit with the rich one.
- A block that has no plain rendering (a live search, an interactive map) writes a plain statement of
  what is missing and a link to this product's page, never nothing.

Page-level settings of this product do not go in page `o:layout_data`, because an upstream save drops
them.

## What compatibility does not cover

- Third-party PHP modules and themes, Laminas and Doctrine internals, and the admin's exact markup.
- Full translation catalogues. Two locales ship.
- Live external services: oEmbed, YouTube and remote IIIF.
- Mail delivery: the interface and a log transport exist, and SMTP is optional.
- MySQL fulltext ranking particulars.
- A JSON-LD `@context` served from upstream's URL: `@context` points at this server's
  `/api-context`.

## Where the evidence is

The differential evidence this fork was built against lives in the port's own repository: the
behaviour ledger, the scenario diffs and the Docker oracle are not in this tree. The captured upstream
documents the API tests read are, under [`fixtures/api/`](../fixtures/api/README.md).
`crates/omeka-api/tests/golden.rs` parses each one into its wire type and asserts the serializer
byte-equals the file, and `crates/omeka-api/tests/property_wire.rs` reads the same set.
`cargo test --workspace` passes every target, those two included.

What you can run and read in this repository:

```
cargo test --workspace                          # the whole workspace, fixture targets included
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
