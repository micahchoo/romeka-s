# User guide

Audience: an administrator working in the admin application, or a visitor reading a public site.

This guide assumes an installed instance is already running. To get one running, see
[`operator-guide.md`](operator-guide.md). For the HTTP surface the application uses, see [`api.md`](api.md).

## Install an instance

One command creates the database, applies the schema, and seeds the defaults: four vocabularies with their classes and
properties, the `Base Resource` template, the global administrator, and the small settings block.

```
omeka install -d /var/lib/omeka/omeka.db --email you@example.org --name "Your Name" --password 'a password'
```

Every step is idempotent. Running the same command again inserts nothing new, which is also how the schema is brought
up to date after an upgrade. The flags are listed in [`../crates/omeka-cli/README.md`](../crates/omeka-cli/README.md).

Start the server and open the admin application in a browser:

```
omeka serve -d /var/lib/omeka/omeka.db --host 127.0.0.1 --port 8080
```

The application and the media files are served by a static origin in front of the port, not by the port itself. The
[operator guide](operator-guide.md) says what that origin must do.

## Sign in

There is no password login for the admin application. The login is an API key pair, minted from the command line:

```
omeka key add -d /var/lib/omeka/omeka.db --owner you@example.org
```

The command prints the identity and the credential once each. Paste both into the sign-in form in the header. The pair
is 32 alphanumeric characters per half; a malformed half is refused before any request leaves the browser.

The pair is kept in the browser's `localStorage` under `omeka.apiKey`, so a reload keeps you signed in. Signing out
clears it. The credential half is never rendered or logged, and the header shows `Signed in as <identity>`. The same
two parameters are what upstream expects, so a key minted here works against an upstream server and the reverse
(`../web/admin/README.md`).

A page that needs a permission you do not hold answers 403 and renders the sign-in form beside the message.

## The admin pages

The application is a hash-routed single page, pointed at the same `/api` endpoints upstream serves.

| Page | What it does |
| --- | --- |
| Dashboard | resource counts and the site list |
| Items | list, create, open one item, edit its values, title, visibility, class, template, owner and item-set membership, delete it |
| Item sets | list, create, edit, delete |
| Media | list with a preview column, open one media to see the image, its parsed fields, its file derivatives and its values, edit alt text, language and visibility, delete |
| Users | list, create, edit role and active flag, delete |
| Vocabularies, properties, resource classes, resource templates, modules, jobs, assets | one browse list per API resource |
| Query | the advanced search form; `Preview` runs a real search and lists the matches |
| Columns | which columns a browse table shows, in page state only |
| Settings | the global settings form; the port serves no settings endpoint, so it does not persist |
| System information | the version from the response header, and the registered API resource names |

Media create is not offered in the application, and a web upload does not exist: `POST /api/media` sets the row's
fields but stores no file. A file arrives through `omeka import media` below, which also writes the three thumbnail
sizes. Values are not editable in the media form either.

## Import data

Three commands, all reading the upstream shapes.

```
omeka import template -d DB --file TEMPLATE.json
omeka import csv      -d DB --file ITEMS.csv --item-set ID
omeka import media    -d DB --source DIR --item-set ID
```

The CSV header names properties as `prefix:local_name` (`dcterms:title`). Each later row is one item: a non-empty cell
becomes a literal value, and two columns with the same term become two values. The first non-empty `dcterms:title` cell
becomes the item's title. A row whose column count differs from the header is reported and skipped, and the exit code is
1 when any row failed.

For media, the join key is `dcterms:identifier`: a file is linked to the item whose identifier literal equals the
manifest's `identifier`, restricted to `--item-set` when you give one. With `--source DIR` and no manifest, the command
walks that directory. A second run skips a file whose `(item, source)` pair already exists, so it is safe to repeat.
Images get the three thumbnail sizes; anything else is stored without thumbnails. `omeka media thumbnails` backfills
thumbnails for rows that are missing them. The full rules are in
[`../crates/omeka-cli/README.md`](../crates/omeka-cli/README.md).

The real-data run that exercises all three commands against the Gawan Museum Trust collection is
[`../verification/acceptance/report.md`](../verification/acceptance/report.md).

## The public site

Every public path sits under the site's slug.

| Path | Renders |
| --- | --- |
| `/s/<slug>` | the site's home page, or a redirect to its home page |
| `/s/<slug>/page/<page-slug>` | one page and its blocks |
| `/s/<slug>/item` | the item browse |
| `/s/<slug>/item/<id>` | one item |
| `/s/<slug>/item-set` | the item-set browse |
| `/s/<slug>/item-set/<id>` | one item set |
| `/s/<slug>/media/<id>` | one media |

The home page, a page and a browse carry the site's navigation. Twenty block layouts are implemented, the ones the
upstream page editor offers. A page slug matches case-insensitively.

A missing site, page or resource is a 404 page; a resource or site that is not public is a 403 page. Both are rendered
by the bundled theme.

Not served yet, and answering 404: page browse (`/s/<slug>/page`), cross-site search, the generic resource search, and
media browse. Serving theme assets depends on the static origin, which is described in the
[operator guide](operator-guide.md); theme work is tracked in [`../ROADMAP.md`](../ROADMAP.md).

## Search

The query page builds the advanced search upstream defines: `search` for a free-text match over any value, and repeated
`property[i][property|type|text|joiner]` rows for a narrowed search. The accepted parameters per resource, with the
source function that reads each, are in [`api.md`](api.md#routes). A fulltext index backs `fulltext_search`; its ranking
differs from MySQL's, which [`../DESIGN.md`](../DESIGN.md) excludes from differential matching.

The public site's own search pages are not ported yet.

## What is limited, and why

- No password login and no session cookie. The API key is the only credential
  ([`../crates/omeka-api/src/auth.rs`](../crates/omeka-api/src/auth.rs)).
- No settings endpoint, so the settings page does not persist.
- Media files and their thumbnails come from `omeka import media` and `omeka media thumbnails`. The API stores no
  uploaded file (`../crates/omeka-api/README.md`).
- Modules and themes from the wider ecosystem are out: the port has Rust registries instead of a PHP module loader
  ([`../DESIGN.md`](../DESIGN.md), Exclusions).
- Mail is not delivered; the interface and a log transport exist.
