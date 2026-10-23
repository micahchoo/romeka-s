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
| Import | turn files into items, item sets and media, from a folder on the server or from this computer, and undo an import. See [Import files as items](#import-files-as-items) |
| Media | list with a preview column; open one media for the image, its parsed fields, its file derivatives and its values; edit alt text, language and visibility; delete |
| Users | list, invite, edit role and active flag, mint a password link, delete |
| Sites and pages | list sites and create one; each site opens on its hub, with tabs for its navigation, items, users, theme and settings, and a page editor with a live preview. The next section describes them |
| Vocabularies, properties, resource classes, resource templates | one browse list each, a detail view per row, and the create, edit and delete forms the API supports |
| Assets, jobs, modules | one browse list each; assets and jobs have a detail view with the update and delete controls |
| Query | the advanced search form; `Preview` runs a real search and lists the matches |
| Columns | which columns a browse table shows, in page state only |
| Settings | the global settings form. The API serves no settings resource, so this page does not persist |
| System information | the version from the response header, and the registered API resource names |

Media create is not offered in the application: `POST /api/media` sets a row's fields but stores no
file. A file arrives through the Import page or `romeka import run`, which both write the three
thumbnail sizes. Values are not editable in the media form either.
[`../web/admin/README.md`](../web/admin/README.md) has the full route list.

## Build a site

A site opens on its hub. The hub lists the site's pages, says which page is the homepage, which pages
are public and which are in the menu, and lets you set the homepage or add a page to the menu with
one click. It also says how many items the site holds, with a link to the Items tab. The tabs beside
it are Navigation, Items, Users, Theme and Settings.

- **Pages.** Add page opens a template chooser: five built-in starting points, the site's own saved
  templates, or a blank page. The editor shows the page as the public site renders it, beside an
  Outline of its blocks. Click a block in the preview to edit it, or add a block between two others.
  The device bar previews the page at desktop, tablet and phone widths. Save as template keeps the
  page's layout for the next page.
- **Navigation.** The menu as a tree: drag a link, or pick it and move it with the arrow keys. A link
  that the server would refuse is marked with what it lacks (a page, a URL or a label), and so is a
  second link to the same page. The menu preview shows the result.
- **Items.** Which items the site shows. A new site holds no items. The Items and sets tab is one
  picker: search titles and values, filter by class or media, and pick single items or whole item
  sets. Add items puts the picks on the site; Replace with these makes them the site's only items, and asks
  first with the number of items that leave. By search adds the items that match a search, once. Each
  way shows how many items it adds before it adds them. The sync switch adds new items only: when it
  is on, an item created in one of the site's item sets joins the site (with no item sets, every new
  item joins). Editing an item never adds it, so an item you remove stays removed.
- **Users.** Who works on the site, with a site role: Manager, Creator or Viewer.
- **Theme.** Pick one of five presets (Plain, Archive, Gallery, Editorial, Civic), set a brand colour
  that the palette follows, and fine-tune single colours, fonts and spacing. Under the automatic
  colour scheme each colour has a light value and a dark value. A palette that fails WCAG AA 4.5:1
  cannot be saved, and the contrast panel offers the nearest colour that passes. Custom CSS has its own
  tab.
- **Settings.** The site's title, address, summary, owner and visibility, then the site settings
  in groups: browsing, search, item pages, language, navigation and advanced. The saved templates
  are managed here too.

Navigation, Theme, Settings and Users mark unsaved changes, and ask before you leave the tab or close
the window. What the tabs offer follows your site role. A Manager, the site's owner and an
administrator change everything; only the owner or an administrator deletes the site. A Creator adds
and edits pages, and sees the menu and the items without the controls that change them. A Viewer
reads the pages. A control your role cannot use is not shown, and one line says why.

## Import files as items

The Import page, under Content, turns files into items. Each file becomes one media with its
thumbnails, and files that belong together become one item. You can also describe the items in a
spreadsheet. Nothing goes into Omeka until the last step, and you can undo an import later. The
page is for global and site administrators.

### Start

The start page asks "Where are your files?". It has two cards.

- **In a folder on the server.** The person who runs your Omeka sets up the folders with
  `ROMEKA_ARCHIVE_ROOTS` ([`operator-guide.md`](operator-guide.md), Jobs and the archive import). Open a
  folder and choose **Use the folder X**, or **Use everything in** a root. The files stay on the
  server. If no folder is set up, the card says how to set one up.
- **On this computer.** Choose **Start with files from this computer**. You send the files on the
  next page. Drag files, a folder or a zip file onto it, or use **Choose files** and **Choose a
  folder**. A zip file is unpacked after it arrives. Files and folders whose names start with a dot
  are left out. Each sending shows its progress, and you can pause one file or all of them. If you
  leave the page, sending stops; add the same files again and they continue. One file can be up
  to 2.1 GB and one import up to 53.7 GB (2 GiB and 50 GiB), unless the operator changed the
  limits. **Look at the files again** reads them again after you add or change files.

Under the cards, **Past imports** lists every import with its files, status, start, finish and how
many it added. Each row has **Make a copy**, and **Try again** when the import stopped before the
end.

### Simple and Advanced

An import has two modes over the same settings. You can switch at any time and lose nothing.

- **Simple** asks one question at a time and suggests an answer to each.
- **Advanced** shows every setting at once.

The import saves each change as you make it. The line over the screen says Saving, Saved or Not
saved. If someone saved the import somewhere else after you opened it, the page says so and offers
to load their answers. An import that has started cannot change: **Make a copy** to change it.

### The three steps of Simple

Simple has three steps: Structure, Check and Add.

1. **Structure.** Omeka reads your folders and suggests how they become items and sets. The
   left side is a tree of your archive: each folder that makes items, the folders it leaves out,
   and the lists that describe no folder. Select a line to see its questions on the right. Each
   question shows our suggestion and the other answers, each with its counts. The counts tell you
   the result before you click. Nothing is used until you choose **Use this structure**.
   - **The archive.** Where the files are and how many there are, the top folders and the lists.
     **Your own sets** adds sets that cross the folders, such as "Exhibition 2018".
   - **Items.** How the files of a folder make items. If the suggestion is wrong, choose **Mark an
     example**. Pick a file of an item that has more than one file. Click the part of its name
     that changes between the files of that item, such as "front" and "back". Omeka reads every
     name the same way and says how many items that makes. The folder is then "written by you".
     **Back to our suggestion** undoes that.
   - **List.** The spreadsheet that describes the folder, with one row for each item or each file.
     Omeka says how many rows name a file. **Each row describes** says whether a row is an item or
     a file.
   - **Where each column goes.** A column whose name is the name of a field goes into that field,
     such as a column "Title" into Title. Change any column here: into another field, onto each
     file, or nowhere.
   - **Set.** The item set of the folder: the folder's name, another name, one set for each value
     of a column, or no set.
   - **The item type comes from.** A column of the list, the folder, or nothing. A table shows each
     value and how many items have it. Choose an item type for each value. "Letter" and "Letters"
     are different values: choose the same type for both. A value can become a new item type.
   - **Leave out** keeps a folder out of the import. **Import it** brings it back.
2. **Check.** Omeka asks one question for each kind of problem, not one for each item, and suggests
   an answer. **Use the N suggested answers** takes them all. A question you skip gets its suggested
   answer. A problem that would write broken data must be fixed before you can add.
3. **Add.** The page sums up what goes in: the items, the files, the new item sets, item types and
   fields, and the items left out. Choose who can see the items: only signed-in staff (private, the
   default) or everyone (public). Then add them.

An old address of a step that is gone opens Structure at the question that step asked.

### Fill in a list yourself

If you have no list, make one. Select the folder on Structure and choose **Fill in a list
yourself**. Choose the columns you want, download the list, fill it in and upload it.

- The list has one row for each item. The first two columns are **Identifier** and **Files**. Keep
  them as they are: Omeka uses them to find each item.
- Each other column name ends with a field in square brackets, such as `Title [dcterms:title]`.
  Keep the brackets. They tell Omeka which field the column fills. You can add columns for any
  field, the item sets, the item type and who can see the item.
- Put two values in one cell with a `|` between them.
- The file is UTF-8 with a byte order mark, so Excel shows Devanagari and other scripts correctly.
- Upload a `.csv`, `.tsv`, `.xlsx`, `.xls` or `.ods` file of at most 50 MB. Omeka reads the first
  visible sheet of a workbook. Omeka says how many rows belong to items of this import and shows
  the rows that do not. The values in your list come first, before other lists and the file names.

A cell that starts with `=`, `+`, `-`, `@` or a tab gets a leading `'` in the download, so a
spreadsheet never runs it as a formula. Omeka takes the `'` off when you upload the list.

### Pair rows and files by hand

A row goes with a file only when the row gives the file's exact name or code. To pair the others
yourself, use **Pair them by hand**: choose a row that names no file, then choose its file. Omeka
shows ten of each, with a search. Each pair you make is listed and has **Undo**.

With more than 30 rows that name no file, Omeka offers a list instead. Download the rows (.csv),
write the name of its file in the **Files** column of each row, and send the list back. A row is
paired only when its cell is exactly the name of one file. When two folders hold a file of one name,
write the folder and the name, as in `letters/page-1.jpg`. Omeka does not guess. It lists each cell
that made no pair and why: the row is not in this import, no file has that name, more than one file
has that name, or the file is in no item. One row takes one file.

### The Check step and the review lists

Omeka works on groups of items that share facts, not on one item at a time, so 40,000 items stay
easy to review.

- **Your items in groups** describes each group by exact facts: how many files, which kinds, and
  whether your list has a row. Groups that are rare, or that have 7 or more files, are marked "worth
  a look". It is on the archive line of Advanced's Structure.
- **Files in each item** is a chart, on the same line. A wrong grouping shows here first.
- **Files that are the same** lists files that are byte for byte the same as another file.
- **Every item** and **Every file** list everything, 50 rows at a time. Choose a filter, then
  change all the rows the filter selects with one action: leave them out, put them in an item set,
  give them an item type, or make each file an item of its own. The page names the rows a change
  hits before you apply it.

Your changes stay when you add more files or change an answer. A change that names a file or a row
that is no longer there is kept and listed, not dropped.

### Advanced

Advanced shows the same tree as Simple's Structure. Simple shows the questions of the line you
select. Advanced shows its rules. The mode switch keeps the line you selected. The Advanced steps
are Structure, Problems, Existing items, Changes, Items and Run.

- **Structure.** Select a line of the tree.
  - **The archive.** The sources and the files, the files that are the same, the chart of files in
    each item, and the groups of items by facts, each with one change. The defaults for who sees a
    new record and who owns it. Your own sets, each with its source and its sets.
  - **A folder.** Five sections, each one line until you open it.
    **Which files**: the folder and its kinds of files.
    **Items**: the pattern that makes items from names (a regex with a group named `identifier`).
    Omeka counts the items and the files that fit before you use a typed pattern.
    **Lists**: the lists the folder reads. For each list: its text encoding, delimiter, quote and
    header line, the join column, and for each column its field, a separator, a language and
    whether its values are private.
    **Fields**: a rule for each field, an ordered list of sources, with "first" (the first source
    with a value wins) or "all" (the values of every source add up), and a value map that changes
    a value before it is used.
    **Set**: every set choice with its counts, a set already in Omeka, or a description.
  - A choice that Simple also offers sends Simple's answer. Any other change writes the folder by
    hand. The folder then says "written by you", and Omeka suggests nothing more for it until
    **Back to our suggestion**.
- **Problems.** One policy for each kind of problem: keep writes the value as it is, repair fixes it,
  leave leaves it out. Also the answers to the groups of problems, such as identical files.
- **Existing items.** The action, and how a record finds the item already in Omeka.
- **Changes.** Every change you made, in words, each with a remove button.
- **Items** and **Run.** The same lists and the same run as in Simple.

The old addresses of Files, Lists and Fields open Structure at the first folder, with that section
open.

The action says what the import does to items already in Omeka.

| Action | What it does |
| --- | --- |
| Create | adds every record as new |
| Append | adds the list's values to items already in Omeka |
| Revise | changes the values the list gives, and keeps the rest |
| Update | replaces the values of each mapped field; an empty cell clears the field |
| Replace | removes all values of the item, then writes the list's; the item's files stay |
| Delete | deletes the items the list names, with their files |

Delete is not undoable. Omeka asks you to type the number of things it deletes (items, files and
item sets, counted together) before it starts. Every other action can be undone.

### Add, stop and try again

Add starts a job. The page shows how many are done, the time left at this speed, and a link to the
log on the Jobs page. You can leave the page: the import goes on. **Stop after the one in hand**
stops it cleanly, and what is in Omeka stays there.

If your files or answers changed after the counts were made, Omeka counts again and asks you to
look first. Only one import or undo works at a time. If another is working, the page links to it.

A record that fails does not stop the others. When the run ends, the page lists each failure with
its reason. **Try again: the N not done yet** runs it again, and what is done already is skipped.
**Download the list of what was left out or failed (.csv)** gives every left-out file and row with
a `#reason` column.

### Undo, and what it keeps

You can undo any import, not only the last one. **Undo this import...** first reads what the undo
would do. It removes what the import added, puts back the items the import changed, and skips what
is not in Omeka any more.

Some things are kept: an item someone edited since the import, an item that another item links to,
an item with files added later, an item set that others added items to, and an item type or field that
something else uses. Each row says
why it is kept. An import that kept something ends **Partly undone: some items were kept**. Then you
can choose **Undo it anyway** for one row, or **Undo all N anyway**. That also undoes what was done
to those items after the import, and an item type undone this way takes its values from items
outside the import. **Undo the rest** finishes a partly undone import. **Stop the undo after the
one in hand** stops an undo.

### Copy an import

**Make a copy** makes a new draft with the same answers and the same changes. A change that no
longer fits the files is kept and listed. A copy does not hold the files you sent from this
computer: add them again, and the changes reattach to the files with the same paths.

### How long uploaded files stay

Files sent from this computer are not served on the web. Omeka removes them after a set time:

- 30 days after an import is added or undone;
- 30 days after the last try of an import that stopped;
- 7 days after a draft was last saved;
- one day after a sending that never finished.

The draft shows the date. The operator can change these times
([`operator-guide.md`](operator-guide.md)).

### Files of private items

Everything the import adds is private unless you choose public. A file of a private item is
served only to a signed-in user who may see the item.

## Command line

```
romeka import template -d DB --file TEMPLATE.json
romeka import run      -d DB --plan PLAN.json --as EMAIL [--dry-run]
romeka import undo ID  -d DB [--preview] [--force all]
romeka import list     -d DB

romeka export jsonld   -d DB --out DIR --resource items
romeka export csv      -d DB --out items.csv --columns dcterms:title,dcterms:identifier
```

`import template` reads a resource template from JSON. `import run`, `undo` and `list` do what the
Import page does, without a server: the plan is the JSON the page saves. Read the
[operator guide](operator-guide.md) for the flags and the rules. The old `import csv` and `import
media` commands no longer exist. Put the items in a list and use the Import page, or `import run`.

`export jsonld` writes the same documents `/api/<resource>` serves, so it is the wire format rather
than a second one. `export csv` writes one row per item, and its column set is this product's. A property with
several values gets one column for each value, and the header repeats the term.

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
- Media files and their thumbnails come from the Import page, from a folder on the server or from
  this computer, or from `romeka import run`. The media form does not upload a file
  ([`../crates/omeka-api/README.md`](../crates/omeka-api/README.md)).
- Modules and themes from the wider PHP ecosystem are out: extension here is a Rust registry
  ([`compat.md`](compat.md)).
- Live external services (oEmbed, YouTube, remote IIIF) are not called.
- Full upstream translation catalogues are not ported. The admin ships English, Spanish (hand-written), and seven machine-translated Indic locales: Hindi, Kannada, Malayalam, Marathi, Nepali, Tamil and Telugu.
