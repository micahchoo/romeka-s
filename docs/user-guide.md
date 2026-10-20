# User guide

Audience: an administrator working in the admin application, or a visitor reading a public site.

This guide assumes an installed instance is running. To get one running, read
[`operator-guide.md`](operator-guide.md). For the HTTP surface the application uses, read
[`api.md`](api.md).

## Install an instance

One command creates the database, applies the schema, and seeds the defaults: four vocabularies with
their classes and properties, the `Base Resource` template, the global administrator, and the small
settings block.

```
romeka install -d /var/lib/romeka/romeka.db --email you@example.org --name "Your Name" --password 'a password'
```

Every step is idempotent. Running it again inserts nothing new, which is also how the schema is
brought up to date after an upgrade. The flags are in
[`../crates/omeka-cli/README.md`](../crates/omeka-cli/README.md).

Start the process, and put a static origin in front of it for the admin application:

```
romeka serve -d /var/lib/romeka/romeka.db --host 127.0.0.1 --port 8080 --base-url http://127.0.0.1:8080
```

The process serves the API and the public site; the origin serves the admin bundle and the media
files and forwards `/api`, `/health` and `/s/`. What the origin has to do is in the
[operator guide](operator-guide.md).

## Sign in

The header's sign-in panel carries two forms.

**The API key pair.** Mint one from the command line:

```
romeka key add -d /var/lib/romeka/romeka.db --owner you@example.org
```

The command prints the identity and the credential once each. Paste both into the key form. Each half
is 32 alphanumeric characters, and a malformed half is refused before any request leaves the browser.
The pair is kept in the browser's `localStorage`, so a reload keeps you signed in; signing out clears
it. A 403 from a page renders the sign-in form beside the message, and the header shows
`Signed in as <identity>`. The pair is the same two parameters upstream expects, so a key minted here
works against an Omeka S server and the reverse ([`../web/admin/README.md`](../web/admin/README.md)).

**A password.** An account with a password signs in by email and password, and the session lives in an
`HttpOnly` cookie. This is the form an invited person uses after following their invite link.

## Invite people, and reset a forgotten password

Two ways to onboard a person.

From the command line, `romeka user add -d ... --email E --name N` creates an inactive, passwordless
account and prints a one-time set-password link. With `--password`, the account is created active with
that password instead. The link is `<base-url>/#/password/<key>`, and it expires two weeks after it is
minted; only the last link minted for an account works.

In the application, the Users page invites by the same route and shows the link with a copy control
on the user's page. The forgot-password form on the sign-in panel mints a reset link the same way.

With no SMTP relay configured, the link goes to the request log and is returned to the administrator
rather than mailed, which is what makes an installation without mail usable. Configure the relay with
`ROMEKA_SMTP_*` if you want the mail to be sent
([`../crates/omeka-server/README.md`](../crates/omeka-server/README.md)).

## The admin pages

The application is a hash-routed single page pointed at the same `/api` endpoints.

| Page | What it does |
| --- | --- |
| Dashboard | resource counts and the site list |
| Items | list, create, open one item, edit its values, title, visibility, class, template, owner and item-set membership, delete it |
| Item sets | list, create, edit, delete |
| Import | import a folder of files from the server as items, item sets and media, and undo the last import. Shown only when the server has an archive folder; see the next section |
| Media | list with a preview column; open one media for the image, its parsed fields, its file derivatives and its values; edit alt text, language and visibility; delete |
| Users | list, invite, edit role and active flag, mint a password link, delete |
| Sites and pages | list sites, create one, edit its theme settings, navigation, page blocks and user permissions; edit a page's blocks. A new site holds no items. Its Items tab picks items and whole item sets, adds them or replaces the site's items with them, and keeps the site in sync: when sync is on, an item created in one of the site's item sets joins the site. Editing an item never adds it, so an item you remove stays removed |
| Vocabularies, properties, resource classes, resource templates | one browse list each, a detail view per row, and the create, edit and delete forms the API supports |
| Assets, jobs, modules | one browse list each; assets and jobs have a detail view with the update and delete controls |
| Query | the advanced search form; `Preview` runs a real search and lists the matches |
| Columns | which columns a browse table shows, in page state only |
| Settings | the global settings form. The API serves no settings resource, so this page does not persist |
| System information | the version from the response header, and the registered API resource names |

Media create is not offered in the application and there is no web upload: `POST /api/media` sets a
row's fields but stores no file. A file arrives through `romeka import media` or the Import page,
which both write the three thumbnail sizes. Values are not editable in the media form either.
[`../web/admin/README.md`](../web/admin/README.md) has the full route list.

## Import a folder from the server

The Import page, under Content, turns a folder of scans or photographs into items. Each file becomes
one media with its thumbnails, and files with the same identifier become one item. The page shows
only when the operator sets `ROMEKA_ARCHIVE_ROOT` on the server
([`operator-guide.md`](operator-guide.md), Jobs and the archive import). The files stay on the
server: you choose a folder there, you do not upload anything.

Choose the folder, then **Import <folder>**. This makes a draft. A draft writes nothing until you
start it, and it saves each change as you make it. The draft has seven steps, as tabs:

| Step | What you do there |
| --- | --- |
| 1. Folder | say how a file name becomes an item: mark an example, or write a regex |
| 2. Item sets | choose which folders become item sets, and their names |
| 3. Item types | choose an item type for each item set, or make a new one from a starter |
| 4. Sources | choose a spreadsheet (CSV), check how it is read, and say which column holds the identifier; map tags inside the files (EXIF) to fields |
| 5. Grid | one row per item: map spreadsheet columns to fields, fix problems, edit cells, skip rows |
| 6. Check | a dry run with the counts, then **Import items: N** |
| 7. Run | the progress, the result, and the undo |

**Mark an example.** The Folder step shows one file name cut into parts: `Letters/LT_001_back.jpg`
shows as `Letters`, `LT`, `001` and `back`. Select a part and say what it is: the identifier, a part (front, back), a page
number, or the item set (top folder only). Parts you mark as identifier, side by side, make one
identifier, so `LT` and `001` give the identifier `LT_001`. Romeka S then finds the same shape in
every other file and counts the matches. If some files have no part, as `PH_101.tif` has none, the
step says so and offers **Make it optional**. **Edit as regex** shows the regex the marks make, for
a shape the marks cannot express.

**Problems.** The Grid names each problem in words: a required field that is empty, a date not in
the form YYYY, YYYY-MM or YYYY-MM-DD, spaces at the ends, text read with the wrong encoding, a
spreadsheet line with the wrong number of cells, a spreadsheet row that matches no file. Each kind
has bulk fixes, for example **Fill them** or **Read as Windows-1252**. At the Check step you can
leave the rows that still have problems out of the import.

**The run.** The import runs as a job, so you can leave the page. The Jobs page shows the job and its
log while it runs. When it ends, the Run step shows the counts. If rows were left out, **Download the
left-out rows (CSV)** gives them with a `#reason` column.

**Undo, until the next import.** The last import can be undone until any next import starts. The
Import page shows a banner while that is possible. Undo asks first, then deletes the imported items
with their files and thumbnails, and the new item sets, item types and fields that nothing else uses.
A field or set that something else uses stays, and the report names it. When a new import starts,
the previous one becomes permanent: its Run step says so, and it has no undo button. This applies to
all imports, not only yours.

**The left-out rows.** The downloaded CSV holds `dcterms:identifier`, the Grid's fields and the
`#reason` for each row: `exists`, `skipped`, `problems: ...` or `failed: ...`. To import rows you left
out, fix them in the folder or the spreadsheet, start a new import of the same folder, and run it. A
new import skips every row whose identifier an item in the archive already has: the Grid marks such
a row **Already exists** with a link to the item, and the Check step counts it in **Items that already
exist and are skipped**. An import never changes an existing item, because undo cannot bring back the
values it would overwrite; to change an existing item, edit it. `romeka import csv` also accepts the
file as it is, because it ignores a column whose header starts with `#`. It makes the items without
their files.

## Import and export

```
romeka import template -d DB --file TEMPLATE.json
romeka import csv      -d DB --file ITEMS.csv --item-set ID
romeka import media    -d DB --source DIR [--item-set ID] [--store DIR]

romeka export jsonld   -d DB --out DIR --resource items
romeka export csv      -d DB --out items.csv --columns dcterms:title,dcterms:identifier
```

The CSV header names properties as `prefix:local_name` (`dcterms:title`). Each later row is one item:
a non-empty cell becomes a literal value, and two columns with the same term become two values. A
column whose header starts with `#` is a note for people, and the command ignores it. The
first non-empty `dcterms:title` cell becomes the item's title. A row whose column count differs from
the header is reported and skipped, and the exit code is 1 when any row failed.

For media, the join key is `dcterms:identifier`: a file is linked to the item whose identifier
literal equals the manifest's `identifier`, restricted to `--item-set` when you give one. With
`--source DIR` and no manifest, the command walks that directory. A second run skips a file whose
`(item, source)` pair already exists, so it is safe to repeat. Images get the three thumbnail sizes;
anything else is stored without thumbnails, and `romeka media thumbnails` backfills the rows that are
missing them. The full rules are in [`../crates/omeka-cli/README.md`](../crates/omeka-cli/README.md).

`export jsonld` writes the same documents `/api/<resource>` serves, so it is the wire format rather
than a second one. `export csv` writes one row per item, and its column set is this product's: it
reads back through `romeka import csv`.

## The public site

Every public path sits under the site's slug.

| Path | Renders |
| --- | --- |
| `/s/<slug>` | the site's home page, or a redirect to its home page |
| `/s/<slug>/page` | the page browse |
| `/s/<slug>/page/<page-slug>` | one page and its blocks |
| `/s/<slug>/item` | the item browse |
| `/s/<slug>/item/<id>` | one item |
| `/s/<slug>/item/search` | the advanced item search form |
| `/s/<slug>/item-set` | the item-set browse |
| `/s/<slug>/item-set/<id>` | one item set |
| `/s/<slug>/media/<id>` | one media |
| `/s/<slug>/search` | the site search |
| `/s/<slug>/cross-site-search`, `/results`, `/items`, `/item-sets`, `/site-pages` | the search across every public site |
| `/assets/<path>` | a file the bundled theme ships |

The home page, a page and a browse carry the site's navigation. Sixteen block layouts are
implemented, the ones the upstream page editor offers. A page slug matches case-insensitively.

A missing site, page or resource is a 404 page; a site, page or resource that is not public is a 403
page. Both are rendered by the bundled theme. A GET is the only method served on these paths;
anything else is a 405.

## Search

The admin's query page and the public search forms build the advanced search the API defines:
`search` for a free-text match over any value, `fulltext_search` for the index, and repeated
`property[i][property|type|text|joiner]` rows for a narrowed search. The accepted parameters per
resource, with the source function that reads each, are in [`api.md`](api.md#routes).

A write keeps the fulltext index in step, so a value you save is findable immediately. The whole
index is rebuilt by the `IndexFulltextSearch` job, which is a `job` row with class
`Omeka\Job\IndexFulltextSearch`; the API's job create is not built at this commit, so there is no
supported command that enqueues one, and `romeka job run <id>` runs a row that already exists
([`../crates/omeka-cli/README.md`](../crates/omeka-cli/README.md), Jobs). Its ranking is SQLite
FTS5's, which differs from MySQL's ([`compat.md`](compat.md)).

## What is limited, and why

- The settings page does not persist, because the API serves no settings resource.
- Media files and their thumbnails come from the command line or from a server folder through the
  Import page. There is no web upload: the API stores no uploaded file
  ([`../crates/omeka-api/README.md`](../crates/omeka-api/README.md)).
- Modules and themes from the wider PHP ecosystem are out: extension here is a Rust registry
  ([`compat.md`](compat.md)).
- Live external services (oEmbed, YouTube, remote IIIF) are not called.
- Full upstream translation catalogues are not ported. The admin ships English, Spanish (hand-written), and seven machine-translated Indic locales: Hindi, Kannada, Malayalam, Marathi, Nepali, Tamil and Telugu.
