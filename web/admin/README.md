# Romeka S admin

Romeka S is a fork of Omeka S. This directory holds the strict-TypeScript admin SPA: it is built with Vite and React
19 and talks to the existing `/api` JSON-LD endpoints. Nothing here builds or modifies the PHP application. The client
speaks upstream's `/api` members as they are, and many fixtures under `tests/fixtures/` are the shapes frozen in
upstream's captured API documents.

## Commands

Run these from `web/admin`.

| Command | What it does |
| --- | --- |
| `npm ci` | Install from `package-lock.json`. |
| `npm run dev` | Vite dev server. |
| `npm run typecheck` | `tsc --noEmit`. |
| `npm run lint` | `eslint .`. |
| `npm test` | `vitest run`. |
| `npm run build` | `tsc --noEmit && vite build` into `dist/`. |
| `npm run e2e` | Playwright: builds the app, starts the port on a temporary database and drives a browser through the admin. Writes `perf/summary.json`. |
| `npm run perf:bundle` | Gzips the built `dist` bundles and fails over the budget in `perf/budget.json`. |
| `npm run perf:pages` | Prints the initial payload and each route's chunk from the built `dist`. |
| `npm run perf:check` | The UI budget gate: the bundle, and the recorded timings when `perf/summary.json` exists. |

## Layout

- `src/api/types.ts`: wire types and branded ids.
- `src/api/parse.ts`: hand-written parsers, `ParseError`, and the JSON path helpers. Every value
  enters through here.
- `src/api/auth.ts`: the API key. It validates the pair, keeps it in `localStorage`, and holds the
  signed-in state the shell subscribes to. Every key enters through here.
- `src/api/session.ts`: the session. The cookie is `httpOnly`, so this holds the user `GET /api/me`
  answered and the sign-in and sign-out that change it.
- `src/api/client.ts`: the typed `ApiClient`. It builds query strings, appends the API key to every
  `/api` request, reads the `Link` and `Omeka-S-Total-Results` headers, maps HTTP errors to
  `ApiError`, and sends the write requests. The auth routes (`login`, `logout`, `getMe`,
  `setPassword`, `forgotPassword`, `createPasswordLink`) send `credentials: 'same-origin'` and carry
  no key, because a sign-in cannot depend on one.
- `src/api/write.ts`: the request-body encoders, the mirror of `parse.ts`.
- `src/app/`: the shell, the hash router, `useAsync`, the label lookups, the shared `ListPage` and
  `Pager`, the shared `ValuesList`, the media preview and image helpers, and the page table for the
  16 upstream admin controllers. Each row of the table holds its page component as a `React.lazy`
  import, so a route's module is a chunk the browser fetches on the first visit to that route.
  `draft.ts` holds the edit-time model, `fields.tsx` the shared form
  controls, `options.ts` the reference lists and `save.ts` the submit state. `auth.tsx` holds the
  sign-in panel and the header state, `password.tsx` the set-password page, `forgot.tsx` the
  request form, and `errors.tsx` the shared page-load error surface that adds the panel to a 403.
- `src/pages/`: one directory per page area. `items/`, `item-sets/`, `users/` and `media/` each have a
  list and a form; `items/` and `media/` also have a detail page. `vocabularies/`, `properties/`,
  `resource-classes/`, `resource-templates/`, `assets/` and `jobs/` each have a list, a detail view and the
  write forms the API's write surface supports. The rest are browse views or single-purpose pages.
- `tests/`: parser tests, page tests, the fixture loader, the fetch stub, and `tests/fixtures/`.
- `e2e/`: the Playwright suite. `harness/` holds the origin, the CLI driver, the seed data, the run record, the
  table reader and the axe check; `specs/` holds the specs. See "End-to-end tests".
- `perf/`: the UI budget. `budget.json` is the budget, `bundle.mjs` measures the initial payload and the
  chunks, `pages.mjs` prints the payload per route, `check.mjs` is the gate, and `summary.json` is the
  artifact the e2e run writes. See `perf/README.md`, and `PERF.md` for the frontend decision log.

## Routes

Routes are hash routes, so the server needs no rewrite rules. The sign-in panel is not a route: it
lives in the shell header, and a page that needs it renders it in place. A caller with neither a
session nor a key gets the panel alone at every admin address: no nav, no page, and no request but
`GET /api/me`. The address stays as it is, so signing in renders the page it names.

The nav offers each page to the roles in its `offered` row in `src/app/pages.tsx`, which are the roles
the server lets read what the page shows: Import to the two admin roles (`imports::wire::admin`),
Settings to a global admin (`settings::authorize_global`), Jobs and Modules to reviewer and up
(`Api::allow`), every other page to every role. A caller signed in by a key alone has no role the
admin knows, so the nav offers them only the pages every role may use. The route opens for anyone,
and the server answers each request for the caller. Each route's page component
is its own chunk, fetched on the first visit and cached after that; the shell renders the page heading
from the page table while that chunk loads.

Three addresses sit outside the admin page table, because whoever opens them cannot sign in yet. They
render the header and their own page, with no admin navigation:

- `#/password/<key>` and `#/password?key=<key>` set a password from a one-time key. The key is in the
  URL, which is why the set-password page takes it from there; `#/create-password?key=<key>` is
  accepted too, because that is the upstream action's name and the mail link is the server's to build.
  A link that carries the key on the document's own query (`?key=<key>`, which is what a link against
  the app root produces) opens the same page whatever the fragment says.
- `#/forgot` requests a reset link.

- `#/` (or `#/dashboard`) shows the admin dashboard: resource counts and the site list, each site
  name a link to that site (`#/sites/<id>`).
- `#/users/<id>` edits a user. On your own account the page hides Role, Active and Delete, which the
  server refuses you, and says another administrator can change them. Under the role select one line
  says what the chosen role may do, from the permission matrix the API enforces; a site admin is not
  offered the two admin roles (`change-role-admin`). The page lists the sites the user holds a site
  role on, each a link to that site's Users tab, read from the sites the reader may see.
- `#/items` lists items. `#/items/new` creates one. `#/items/<id>` shows one item. Its values are one
  list above the media, each field under the name its item type gives it, with the term as quiet text
  and each value's kind in the form's words (`app/values.tsx`). "Edit this item" at the top swaps that
  list for the form in the same place (`pages/items/item-values.tsx`); Save or Cancel brings the list
  back there. `?edit=1` opens the page at the form.
- `#/item-sets` lists item sets. `#/item-sets/new` creates one. `#/item-sets/<id>` opens the set's place: it
  replaces its own history entry with `#/items?set=<id>`, the Items screen filtered to the set. Whenever the
  Items screen has a set filter, the set's card heads the results (`pages/items/set-card.tsx`): its item
  count, how many of them the other filters match, visibility, "Open to additions", the sites that show
  it, its description, Edit set and Delete. A set the reader may not read, hidden or gone, reads "This set
  is not available" over empty results. `#/item-sets/<id>/edit` edits one. The set delete, on the card and
  on the form, states that the items stay and leave the set and how many sites stop showing it.
- The item page lists the item's sets (each a link to the set's place) and sites, and names its resource
  template; the media page names its item by title.
- The set list has an Items column: each set's count from one `GET /api/romeka/item-sets/counts` per page,
  as a link to the set's place. A set the answer leaves out, or a failed read, shows a dash with the reason.
  A reader whose role sees every item (reviewer and up) may sort by item count (`sort_by=item_count`).
  Upstream's order counts every item a set holds, so for any other role the order and the visible
  counts would disagree, and it is not offered.
- "Linked from" (`pages/items/linked-from.tsx`, its own chunk) is open on the item page and folded in
  the set card. It reads `GET /api/romeka/links/<id>` and shows one row per kind and field with its
  count ("Creator · 2 items"). An items row opens `#/items?links=<id>&via=<property id>`; an item set or
  media row lists its records in place, five at a time. Each count is the total of the `res` property
  search its row opens, for the same reader.
- The item page's "Delete this item" moves the item to the trash (`pages/items/trash-item.tsx`, its own
  chunk). It is a bulk delete of that item alone: a preview with `{"id": [<id>]}`, then the run, so the
  rights are the item `DELETE`'s and the run is in the bulk panel's recent changes. The confirm states
  the media and their size, the records that link to the item by field ("Creator · 2 items"), and the
  trash days. The Items screen then reads "Deleted <title>." with Undo (`pages/items/trashed-notice.tsx`),
  which is the run's undo; it states the links an undo left out in the server's words. A reload loses the
  notice; the Undo stays in recent changes. `DELETE /api/items/<id>` itself is unchanged.
- The Items screen's `links` filter (+ optional `via`) is that `res` search, so it stacks with every
  other filter and a bulk change over every match selects through it. Its chip reads "Links to: <title>
  (as <field>)". The linked record's card heads the results (`pages/items/links-card.tsx`): "Items that
  link to <title>", the field, the count, and a link to the record. A record the reader may not read,
  hidden or gone, reads "This record is not available" over no results, and its chip names it by id.
  With both a set and a links filter, both cards show, the set's first.
- A row or card on Items opens `#/items/<id>?in=<the list's query>&at=<position>` (`pages/items/walk.ts`).
  `in` is the Items screen's own URL query, so every filter walks, a new one included. The item page reads
  the rows two before and two after `at` with that query, five to a page (one or two requests), and shows
  "12 of 382 in <place>" with Previous and Next links that carry `in` and `at`. The place is the set's
  title, the linked record's title, or "Items". The item is looked for at `at` and one place either side,
  because a save can take an earlier item out of the list ("field is empty"); anywhere else, or a failed
  read, shows no line, never a wrong number. An item opened with no `in` shows no line and reads no list.
  When a next item exists, the edit form has "Save and next": it saves, then opens the next item at
  `&edit=1`, which opens its edit form.
- The Items screen's first filter is a row of item type chips ("Artifact 297"), from one
  `GET /api/romeka/item-types/counts` (`pages/items/item-type-chips.tsx`). Each count is the type's
  total for the reader, not a total under the other filters, and equals the list `template=<id>`
  opens. "No item type 12" shows while some item has none. The names come from the item type list,
  so the chips filter before the counts arrive; a failed count shows no numbers. The table's Item
  type column replaces Resource class; the Class column shows only while `with_class` is above 0
  (and when the count failed). The Class select is under "More filters", and a label two classes
  share carries the vocabulary prefix ("Image (dctype)").
- The Items screen's `field` filter (`field=<property id>:empty` or `:filled`, under "More filters") is
  the `nex` or `ex` property search on `property[1]`, so it never takes the links filter's
  `property[0]` and both narrow one list. The field list is the chosen item type's fields, by the
  names the type gives them, or every type's fields. Its chip reads "Date: empty" or "Date: has a
  value". One field filter at a time.
- The item type page states beside each field how many of the type's items have a value for it
  ("412 of 690 items have a value."), from one `GET /api/romeka/field-counts?resource_template_id=<id>`
  (`pages/item-types/field-fill.tsx`). "Open the 278 without a value" opens
  `#/items?template=<id>&field=<property id>:empty`, whose total is that number for the same reader. A
  type with no item the reader may see, or a failed read, states no number. A new type has none.
- The Items screen, and so a set's place and a links place, offers "Download 382 items as CSV"
  (`pages/items/csv-download.tsx`): a plain link to `GET /api/romeka/items.csv` with the screen's
  filters and order, every page. The query comes from `everyMatchQuery` in `pages/items/browse.ts`,
  the function the list page query is built from, so a new filter reaches the file. With ticked rows
  it also offers "Download the 3 ticked items as CSV": those ids, in the screen's order.
- The header's jump-to box (`app/jump-box.tsx`) opens a record from anywhere. `#4021` reads
  `GET /api/resources/4021` and opens the item, the set's place or the media. Other text runs
  property searches over items, item sets and media, and stops at the first step that matches:
  `dcterms:identifier` `eq`, then `dcterms:title` `eq`, then `dcterms:title` `in`
  (`app/jump-lookup.ts`). One match opens; several list under the box, ten at most, with each
  record's kind and the total; none reads "No record that you can see matches". Every read is the
  caller's, so a hidden record answers like a missing one. `/` focuses the box when focus is not in
  a text field; on the Items screen `/` keeps its own search box. Escape clears the box. The answer
  is its own chunk (`app/jump.tsx`).
- On the Items screen, "Save this list" names the screen's filters and order
  (`pages/items/saved-lists.tsx`, its own chunk). The name field starts with the filter chips'
  words ("Class: Book · Description: empty"). The list is `{name, query}` in the user's
  `romeka_saved_lists` setting, where `query` is the screen's URL query without the page, the
  view and `list` (`savedQuery` in `pages/items/browse.ts`), so a new filter is saved with no
  change here. The nav lists the names under Items (`app/saved-list-links.tsx`), from the
  session's user, each a link to `#/items?<query>&list=<name>`. The screen keeps `list` through
  filter and order changes, so it knows which saved list it came from (`app/open-saved-list.ts`);
  the nav marks that list. On a saved list the screen reads "Saved list: <name>" with Rename and
  Delete. Once its filters or order differ from the saved query, it says "Changed since saved" and
  offers "Update <name> with these filters" (the same name and place, the new query), "Save as a
  new list" and a link back to the saved filters. Each change reads `GET /api/me` first, applies
  to that answer and writes the whole list, so a list saved in another browser is kept.
- `#/users` lists users. `#/users/new` invites one: it creates the row and then mints the one-time
  password link, which the page shows with a copy control. `#/users/<id>` edits one and can mint
  another link.
- `#/media` lists media with a preview column. `#/media/<id>` shows one media: the image at full
  width, the parsed fields, the file derivatives, the values, and an "Edit this media" panel for alt
  text, language and visibility. The list title links to it.
- `#/vocabularies`, `#/properties`, `#/resource-classes`, `#/resource-templates`,
  `#/jobs`, `#/assets` list one API resource each.
- `#/vocabularies/<id>`, `#/properties/<id>`, `#/resource-classes/<id>` and
  `#/resource-templates/<id>` show one row of the vocabulary family: every member it carries and a
  link to each row it names. See "Vocabulary family detail views". Each view carries the edit form
  and the delete control for its row; `#/vocabularies/new`, `#/properties/new`,
  `#/resource-classes/new` and `#/resource-templates/new` create one, reached from the list page.
- `#/import` starts an import (import v2, `docs/import-v2.md`) from a folder under one of the
  server's archive roots or from files uploaded from this computer, and lists the earlier imports.
  The nav link shows to the two admin roles, whom the import routes answer; the server folder card shows only when
  a root is set up for the user. `#/import/<id>/simple/<screen>` is the guided pass (`files`,
  `found`, `list`, `where`, `check`, `add`, with the sub-screens `example`, `all-files`, `pair`
  and `all-items`), and `#/import/<id>/advanced/<screen>` the eight tabs (`files`, `lists`,
  `fields`, `problems`, `existing`, `changes`, `items`, `run`, with `all-files` and `pair`). Both modes edit one saved plan
  and share one start path (`src/pages/import/`, wire in `src/api/import-routes.ts`,
  `src/api/import-sections.ts` and `src/api/import-uploads.ts`).
- `#/assets/<id>` shows one asset with its edit and delete controls. `#/jobs/<id>` shows one job with
  its update and delete controls, and its log from `GET /api/romeka/jobs/<id>/log`, read again every
  two seconds while the job runs. There is no create route for either. See "Write requests".
- `#/system` is Admin > Advanced > System, one tab each for what upstream shows on three pages:
  System information (the Omeka S version from the response header and the registered API
  resource names), `#/system/modules` (the modules list) and `#/system/query` (the advanced search
  form, whose `Preview` runs a real search and lists the matches). `#/system-info`, `#/modules` and
  `#/query` open the matching tab. Upstream's Browse columns page is not ported: its choice was
  never saved, so `#/columns` opens System.
- `#/settings` renders the global settings form. The port serves no settings endpoint, so it does
  not persist.
- Items, item sets, users and media are the resources with a write form, plus sites and site pages,
  the vocabulary family, and the asset and job detail views. Every resource the API serves `DELETE`
  for offers a delete control: items, item sets, media, assets and jobs on the show page, users, sites,
  site pages and the vocabulary family on the edit surface. The control sits behind a
  confirmation that names the row, so no single click deletes and no list row carries one.

## Vocabulary family detail views

One row of the vocabulary family per route, reached from the browse table. Each view reads its own
row with `ApiClient::getVocabulary`, `::getProperty`, `::getResourceClass` or
`::getResourceTemplate`, which is `GET /api/<resource>/<id>`; a missing row is the API's 404 and the
shared error surface renders it.

| Route | Upstream action | Members shown | Links out |
| --- | --- | --- | --- |
| `#/vocabularies/<id>` | `Admin/VocabularyController.php showDetailsAction propertiesAction classesAction` | `o:id`, `o:prefix`, `o:namespace_uri`, `o:label`, `o:comment`, `o:owner`, and its properties and classes | owner to `#/users/<id>`, each property to `#/properties/<id>`, each class to `#/resource-classes/<id>` |
| `#/properties/<id>` | `Admin/PropertyController.php showDetailsAction` | `o:id`, `o:label`, `o:term`, `o:local_name`, `o:comment`, `o:vocabulary` | vocabulary to `#/vocabularies/<id>` |
| `#/resource-classes/<id>` | `Admin/ResourceClassController.php showDetailsAction` | `o:id`, `o:label`, `o:term`, `o:local_name`, `o:comment`, `o:vocabulary` | vocabulary to `#/vocabularies/<id>` |
| `#/resource-templates/<id>` | `Admin/ResourceTemplateController.php showAction` | `o:id`, `o:label`, `o:owner`, `o:resource_class`, `o:title_property`, `o:description_property`, and one row per `o:resource_template_property` entry: `o:property`, `o:data_type`, `o:alternate_label`, `o:alternate_comment`, `o:is_required`, `o:is_private`, `o:default_lang` | owner to `#/users/<id>`, class to `#/resource-classes/<id>`, title and description property and each entry's property to `#/properties/<id>` |

A vocabulary's own properties and classes come from one search each, `per_page` large enough to
carry every row, with the API's `vocabulary_id` filter on `properties` and `resource_classes`
(`crates/omeka-api/src/read/vocab.rs`). The template view reads its property labels from the whole
`properties` collection in one page, because a `o:resource_template_property` entry names its
property by id alone.

Not shown, and why: upstream's property and class details link an item count, and the item search
has no property filter yet, so there is nothing to count with. The owner is shown as `#<id>` on the
vocabulary view and by name on the template view, where the user list is read anyway.

## Vocabulary family write forms

One editor per resource, over the members the API's write handlers read
(`crates/omeka-api/src/write/vocab.rs`, `vocab_member.rs`, `templates.rs`).

| Route | Members the form sends | Request |
| --- | --- | --- |
| `#/vocabularies/new` | `o:namespace_uri`, `o:prefix`, `o:label`, `o:comment`, `o:owner` | `POST /api/vocabularies` |
| `#/vocabularies/<id>` edit | `o:namespace_uri`, `o:label`, `o:comment`, `o:owner` | `PATCH /api/vocabularies/<id>` |
| `#/properties/new`, `#/resource-classes/new` | `o:vocabulary`, `o:local_name`, `o:label`, `o:comment`, `o:owner` | `POST /api/properties`, `POST /api/resource_classes` |
| `#/properties/<id>`, `#/resource-classes/<id>` edit | `o:local_name`, `o:label`, `o:comment` | `PUT /api/properties/<id>`, `PUT /api/resource_classes/<id>` |
| `#/resource-templates/new` and edit | `o:label`, `o:owner`, `o:resource_class`, `o:title_property`, `o:description_property`, `o:resource_template_property` | `POST`, `PUT /api/resource_templates/<id>` |

Three choices are worth naming.

The vocabulary edit is a `PATCH`. A full `PUT` must carry `o:class` and `o:property`, and a full write
that omits either removes every member of that kind (`VocabularyAdapter::hydrate`'s `orphanRemoval`
loop). The vocabulary form owns the row's own columns and each class and property owns its own form,
so the vocabulary write is partial and the member lists stay. Upstream's edit posts
`['isPartial' => true]` for the same reason (`VocabularyController::editAction`).

`o:prefix` is create-only, following `VocabularyForm::init`'s `if (!$vocabulary)` guard, and
`o:vocabulary` is create-only because `vocab_member::hydrate` reads it on create only. A property or
class edit does not offer an owner either: `AbstractVocabularyMemberRepresentation::getJsonLd` carries
no `o:owner`, so the form has nothing to seed an owner select with, and omitting the member keeps the
stored owner.

A resource template's `o:resource_template_property` list is the row set the API accepts, and its
order is the stored `position`, so the form moves rows up and down. Each row carries an alternate
label and comment, the data types from `GET /api/data_types`, the required and private flags and a
default language.

A vocabulary whose prefix is `dcterms` or `dctype` is permanent (upstream cannot edit or delete it,
`VocabularyRepresentation::isPermanent`). The port's API has no such check, so the detail view keeps
the rule and hides the edit and delete controls for those two.

## Write requests

`ApiClient` sends one method per endpoint it uses:

| Method | Request | Response |
| --- | --- | --- |
| `createItem` / `updateItem` | `POST /api/items`, `PUT /api/items/<id>` | `Item` |
| `createItemSet` / `updateItemSet` | `POST /api/item_sets`, `PUT /api/item_sets/<id>` | `ItemSet` |
| `createUser` / `updateUser` | `POST /api/users`, `PUT /api/users/<id>` | `User` |
| `updateUserLocale` | `PATCH /api/users/<id>` with `o:user_settings` | nothing |
| `getSettings` / `updateSettings` (`api/settings.ts`) | `GET`, `PATCH /api/settings` | `GlobalSettings` |
| `getTimeZones` (`api/settings.ts`) | `GET /api/romeka/time-zones` | the zone names |
| `getInstallation` | `GET /api/romeka/installation` | `Installation` |
| `updateMedia` | `PUT /api/media/<id>` | `Media` |
| `createVocabulary` / `updateVocabulary` | `POST /api/vocabularies`, `PATCH /api/vocabularies/<id>` | `Vocabulary` |
| `createProperty` / `updateProperty` | `POST /api/properties`, `PUT /api/properties/<id>` | `Property` |
| `createResourceClass` / `updateResourceClass` | `POST /api/resource_classes`, `PUT /api/resource_classes/<id>` | `ResourceClass` |
| `createResourceTemplate` / `updateResourceTemplate` | `POST /api/resource_templates`, `PUT /api/resource_templates/<id>` | `ResourceTemplate` |
| `updateAsset` | `PUT /api/assets/<id>` | `Asset` |
| `updateJob` | `PATCH /api/jobs/<id>` | `Job` |
| `createPasswordLink` | `POST /api/users/<id>/password-link` | `PasswordLink` (`{key, link, sent}`) |
| `deleteItem` / `deleteItemSet` / `deleteMedia` / `deleteUser` / `deleteSite` / `deleteSitePage` / `deleteVocabulary` / `deleteProperty` / `deleteResourceClass` / `deleteResourceTemplate` / `deleteAsset` / `deleteJob` | `DELETE /api/<resource>/<id>` | the detached answer (`o:id` is null) |

Two write endpoints have no form, each for the reason the API documents
(`crates/omeka-api/README.md`, "Two creates are excluded"):

- `POST /api/assets` answers 501. `AssetAdapter::hydrate` reads the uploaded file out of
  `$request->getFileData()`, and the port's write boundary parses JSON only, so a multipart body never
  reaches the handler. The asset create needs a multipart boundary before it can be built.
- `POST /api/jobs` answers 501. `JobAdapter::hydrate` is empty while `job.class` is `NOT NULL`, so
  upstream's create fails at flush for every body and the port leaves the endpoint unbuilt. A job row
  comes from the dispatcher.

A job's update writes no member: `JobAdapter::hydrate` is empty, so `PATCH /api/jobs/<id>` finds the
row, checks the admin grant and answers its representation. The job detail's form therefore carries
no control and names why; the request still runs and the row comes back unchanged.

Every write sends `Content-Type: application/json` and parses its response through `parse.ts`, so a
write returns the same type a read does. A delete sends no body and answers the detached entity
`Api\Manager::finalize` renders once the flush has cleared its identifier, so `o:id` is null and `@id`
is the collection URL. No caller reads that body, so the twelve `delete*` methods return `void` and the
boundary only checks the answer is the detached shape (`assertDetached` in `parse.ts`). An error body
maps to `ApiError` the way a read error does, including the 403 a caller without permission gets and
the 404 a missing row gets; the delete control shows the API's message and leaves the page in place.

A `PUT` replaces every member the adapter hydrates, so the forms send the full resource: visibility,
owner, class, template, thumbnail and every value. A create with no chosen owner omits `o:owner` so
the server assigns the acting user. Property values are grouped under their term, each object naming
`property_id` and `type` (`literal`, `uri` or `resource`). The supported members and their meanings
come from `crates/omeka-api/src/write/` and the upstream adapters at omeka-s 5e413f0.

Not on the wire today: media create is not offered (it needs an ingester and an item), and values are
not editable in the media form. The user password and activation flow is `POST /api/users/<id>/password-link`
plus the client surfaces in "Client auth"; the user create still writes no password.

## Client auth

Two credentials sign a caller in. The session is the person-facing one; the API key stays for
programmatic use and as the second way in.

| Flow | Request | Answer |
| --- | --- | --- |
| Sign in, from the header or a 403 page | `POST /api/login` with `{email, password}` | the user and an `httpOnly` session cookie; 401 wrong credentials, 403 inactive, 429 the login limit |
| Sign out, from the header | `POST /api/logout` | 204, and the session row is revoked |
| Who is signed in, once per load | `GET /api/me` | the user, or 401 when nobody is signed in |
| Set a password, from a mail link | `POST /api/password` with `{key, password}` | 200; 404 an unknown or expired key, 422 a password the rules reject |
| Request a reset, from `#/forgot` | `POST /api/password/forgot` with `{email}` | 204 for a known and an unknown address alike |
| Create a password link, from the users page | `POST /api/users/<id>/password-link` | `{key, link, sent}` |

The session half.

- The cookie is `httpOnly` and `SameSite=Lax`, so no script reads it and `localStorage` holds no
  trace of it. `GET /api/me` is the only way the SPA learns who is signed in, and the shell asks once
  on load. Until it answers, a caller with no key sees a loading line and nothing else; then the
  shell renders the pages or the sign-in panel. An unreachable API leaves the caller signed out.
- `src/api/session.ts` holds the answer, and `useSession` in `src/app/auth.tsx` subscribes to it, so
  the header and every sign-out read one value.
- Sign out posts to the server first, because the session row is the server's to revoke, and then
  clears both client halves: the key in `localStorage` and the user in memory. A refused logout
  renders its message and leaves the caller signed in.
- Every request sends `credentials: 'same-origin'`, so the cookie rides along. A write's same-origin
  marker is the `Origin` header, which the browser sets and JavaScript cannot; the SPA adds nothing,
  and the password sign-in spec asserts the header the browser sent.
- The auth routes carry no API key. A sign-in cannot depend on a credential the caller may not have,
  and a key would be one more thing a login request leaks.

The key half, unchanged.

- The key lives in `localStorage` under `romeka.apiKey`, as `{"identity": "...", "credential": "..."}`.
  A reload reads it back, so a session survives one. The value is re-validated on read, so a tampered
  entry reads as signed out.
- Both halves are 32 alphanumeric characters (`ApiKey::STRING_LENGTH` and `ApiKey::STRING_CHARLIST`
  upstream). `parseApiKey` in `src/api/auth.ts` checks both and returns a message per bad field, so a
  malformed key never reaches the network.
- `ApiClient` appends both parameters to every `/api` URL through one private `apiUrl` helper, and to
  none when signed out. Both come from one validated pair, so a request never carries a lone identity.
- The key form sits behind the `Use an API key` disclosure in the sign-in panel, so the person-facing
  form is the one a reader meets.
- The shell header shows `Signed in as <name>` for a session and `Signed in as <identity>` for a key,
  each with a sign-out, or `Signed out` with a `Sign in` button that opens the panel.
- A 403 from a page load renders the shared `ErrorNotice`, which puts the panel beside the message.
  Any other status shows the message alone.
- The credential half stays in the component only while the user types it: the field is a password
  input and the value is cleared from state on submit.

### Set a password and invite a user

`#/password/<key>` posts the key it read from the URL with the new password. The client checks two
things the server cannot answer cheaply, an empty password and a mistyped confirmation, and renders
the server's 404 or 422 as it came, so the password rules stay in one place.

The users page invites rather than simply creates. `POST /api/users` writes no password, so a row made
there can sign in only once an administrator mints a one-time key for it, and the invite does both:
create, then `POST /api/users/<id>/password-link`, then show `{link, sent}` in a panel with a copy
control. When no SMTP service is configured the answer says so and the link is the only way in. The
edit form offers the same mint on its own, which is the reset path. A minted key is single-use, and
the second use is the API's 404.

`romeka key add` mints a pair. Upstream presents the same two parameters, so a client signed in here
sends exactly what a proxy would have appended.

## Development against a live API

Requests are same-origin by default. `npm run dev` forwards the port's routes (`/api`,
`/api-context`, `/s`, `/files` and the theme's `/assets`) to `ROMEKA_DEV_TARGET`, by default
`http://localhost:8080`, so a running `romeka serve` there backs the dev server and the Theme and
page previews load the real theme sheet. To point the client at another origin instead, set the base
URL before the client module loads:

```js
window.ROMEKA_BASE = 'http://localhost:8080';
```

The base URL is read once, when `src/api/client.ts` is first imported.

## Boundary parsing

`src/api/parse.ts` is the only place raw JSON becomes a type. The parsers take `unknown`, and a
missing or mistyped field throws `ParseError` with the JSON path and the expected kind, for example
`$.dcterms:title[0].property_id: expected number`. There is no schema library, no `as`, and no
compile-time database. The client parses every response body through these parsers before it returns
it.

## Locales

`src/i18n/` holds the message tables, the translator and the locale store. Strings live in two
kinds of table:

- The **shell catalogue**, `catalogue.ts`: the 55 keys the shell renders before any route loads (the
  title, the nav, the sign-in panel, the locale switch, the route boundary). It is in the first
  paint, so nothing else goes here. `locales/<tag>.ts` holds each other locale, typed `Catalogue`.
- A **namespace**, `namespaces/<id>/index.ts`: every other string, grouped by the routes that render
  it (`common`, `resources`, `sites`, `editor`, `standards`, `admin`, `account`, and the page tables
  `theme`, `item-types`, `term-lists`, `lessons`, `publish`). Its English is a chunk its routes
  import; `locales/<tag>.ts` beside it holds each other locale, typed against that English, so a
  missing key is a `tsc` error. `namespaces/registry.ts` lists them for the scripts.

A key id is unique across the shell and every namespace (`tests/i18n-sync.test.ts`). A component
names the namespaces it reads: `const { t } = useTranslator(sites, common)`. The key type follows,
so a key from an undeclared namespace is a `tsc` error. A namespace table that has not arrived
suspends the caller (React 19 `use()`), so the page shows its Suspense fallback and never English
first (`tests/no-english-flash.test.tsx`). English never suspends. `setLocale` fetches the shell
catalogue and the table of every namespace defined so far, then switches once, so no open page
unmounts and no draft is lost. The shell's `RouteBoundary` turns a chunk or table that fails to
load into a reload prompt. A key read from data (a JSON registry) is checked with the namespace's
`has`, as `sites.has(key)`.

`createTranslator(locale)` returns `{ locale, t, number }`. `t(key, params)` fills `{name}`
placeholders; `number(value)` formats a count or a byte size in the locale. There is no read-time
fallback: every table is checked complete when it loads.

`store.ts` keeps the selected locale in `localStorage` under `romeka.locale`. `parseLocale` narrows
the stored string at the boundary, so a region tag (`es_419`, `en_US`) reads as its language and an
unknown tag as English. A sign-in and `GET /api/me` answer `o:admin_locale`, the server's resolution
of `MvcListeners::bootstrapLocale`, and `setSession` switches to it. When a signed-in user changes
the switch, it also writes their `locale` user setting, best-effort. The settings page and the user
edit page carry the global and the user `Locale` select (`src/app/locale-select.tsx`).

### Add a string

1. Find the routes that render it. One route family: its namespace (a site page: `sites`; the page
   editor: `editor`; an admin page: `admin`). Several families: `common`. The shell before any route:
   `catalogue.ts`, and only then. `node scripts/split-catalogue.mjs plan` prints where each shell
   catalogue key belongs by the route graph.
2. Add the key to that namespace's `index.ts` under a new id, and its Spanish to `locales/es.ts` by
   hand. Never reuse an id another table holds.
3. Name the namespace in the component: `useTranslator(admin)`.
4. Run `bun scripts/translate-locales.mjs` for the Indic locales, then `--check`, which makes no model
   call and must print `0 keys a run would change`. A hand correction in a locale file is kept by
   every later run; `sources.json` records the English each translation was made from.

Never edit a namespace locale file to move a key: moving keys between tables is
`scripts/split-catalogue.mjs` (`copy`, `fold`, `prune`), which moves each entry and its
`sources.json` record unchanged. A new namespace is one `registry.ts` row and one
`codeSplitting.groups` entry in `vite.config.ts`; the script writes its files. To add a locale, add
it to `CATALOGUE_LOADERS`, then `tsc` asks for its file in every table and for its `locale.<tag>`
label.

The built-in vocabularies' labels and comments (Dublin Core, Dublin Core Type, BIBO, FOAF) are two
more namespaces over `locales/vocabulary/{labels,comments}/<tag>.json`, upstream's
`$translate($property->label())`. `useTermLabels()` (`termText.ts`) reads the labels and
`useTermText()` (`termComments.ts`) the labels and comments; each suspends like `useTranslator`. The
comments wait for their first read, so a view that shows labels only never fetches them. Neither
module is exported from `src/i18n/index.ts`, which is in the first paint. The text is keyed by term,
and a stored text is translated only while it is still the built-in English, so a user's vocabulary,
an edited label and a template's alternate label show as stored. The public site reads the label
tables too (`crates/omeka-site/src/i18n/vocabulary.rs`). English is the text a fresh install
stores and Spanish is upstream's `es.po` (`scripts/extract-vocabulary-text.mjs`); the other locales
come from `scripts/translate-locales.mjs`. A new locale adds its two files to the loaders in `termText.ts` and
`termComments.ts`, then runs `UPDATE_GENERATED=1 cargo test -p omeka-site vocabulary` and adds the
tag to the site's `TABLES`.

## Wire shapes

These are the upstream details the parsers depend on. They are read from the PHP representations at
`application/src/Api/Representation/*.php` (omeka-s 5e413f0) and the golden fixtures in
`fixtures/api/` at the repository root.

- A value's `type` is the data-type name (`literal`, `uri`, `resource:item`, and so on).
- A resource value carries `value_resource_name` and `value_resource_id`, and its `url` is `null` in
  the API context.
- `o:created` and `o:modified` are `{ "@value", "@type" }` objects for items, media, item sets, sites
  and jobs, but bare ISO 8601 strings for users.
- `@type` may be a string or an array of strings.
- `o:media` on an item, `o:item_set`, `o:site` and `o:page` are lists of `{ "@id", "o:id" }`
  references. Media titles come from `GET /api/media?item_id=<id>`.
- An error body is `{ "errors": { "field": "message" } }` or `{ "errors": { "field": ["message"] } }`.
- Search responses are bare arrays. Pagination comes from the `Link` header and
  `Omeka-S-Total-Results`.
- `o:thumbnail_urls` carries one file URL per thumbnail type when the row has thumbnails, and the
  empty PHP array `[]` when it has none. The parser reads both. The e2e seed uploads its media
  through `POST /api/media`, which makes the three derivatives, so the seeded rows carry the URLs.
- `o:title` is null on every media row of the live collection. The label falls back the way
  `MediaRepresentation::displayTitle` does, to `o:source`, then to `o:filename`.
- `thumbnail_display_urls` has the keys `large`, `medium`, `square`, always present. Each is the
  derivative URL when the row has thumbnails, and the bundled fallback asset otherwise, so it is
  never a signal that a real thumbnail exists.
- Item browse defaults are `sort_by=id` and `sort_order=desc`. Every browse list asks for
  `per_page` from the installation's `pagination_per_page` (`app/installation.ts`), 25 unless stored.
  The shell reads it before the first route renders; the same store gives the header its
  installation title, every date its `time_zone`, and a new item set its starting visibility.
- Item set adds `o:is_open` and an `o:items` search link. `o:items` carries only `@id`, no `o:id`.
- Asset is not a resource entity: it has no `o:is_public` and no dates. Its fields are `o:owner`,
  `o:name`, `o:filename`, `o:media_type`, `o:asset_url`, `o:alt_text`.
- Job carries `o:status` (a string constant), `o:job_class` (a PHP class name), `o:args` (raw JSON),
  `o:owner`, and the `o:started` / `o:ended` value objects. `o:ended` is `null` until the job ends.
- Module `o:id` is the module directory name, a string. `o:state` is a string constant and `o:ini`
  is the parsed `module.ini` object.
- Property and resource class share `o:local_name`, `o:label`, `o:comment`, `o:term` and
  `o:vocabulary`.
- Vocabulary adds `o:namespace_uri`, `o:prefix`, `o:label`, `o:comment` and `o:owner`.
- Resource template carries `o:label`, `o:owner`, `o:resource_class`, `o:title_property`,
  `o:description_property` and `o:resource_template_property`, a list of entries with `o:property`,
  `o:alternate_label`, `o:alternate_comment`, `o:data_type` (a list), `o:is_required`, `o:is_private`
  and `o:default_lang`.
- Site carries `o:slug`, `o:theme`, `o:title`, `o:summary`, `o:thumbnail`, `o:navigation`,
  `o:homepage`, `o:item_pool`, `o:owner`, the dates, `o:is_public`, `o:assign_new_items`, `o:page`,
  `o:site_permission`, `o:site_item_set` and `o:site_item`. `o:site_item` carries only `@id`.
- `GET /api` returns the `o:ApiResource` list: `o:id` is the resource name, a string.

### Corrections to this file

The golden fixtures won where the older notes disagreed.

- `o:renderer` on an upload media is `file` (`Media\Ingester\Upload::getRenderer`). The older
  `tests/fixtures/media-page.json` carried `image`, and `tests/parse.test.ts` asserted it. Both are
  wrong against the golden fixture. They are left as they were because the Items tests that use them
  are frozen; the coordinator should correct them in a follow-up.
- Module and API resource `o:id` are strings. Earlier notes only described numeric ids.
- Item set and site link members (`o:items`, `o:site_item`) carry only `@id`. They are read as URL
  strings, not references.
- `o:thumbnail_urls` was `[]` on every live media row before the thumbnail unit, and the parser
  rejected that array until then. `tests/fixtures/media-page-live.json` and
  `tests/fixtures/media-951.json` are captures of the running port and carry the real shape.

## Tests

`npm test` runs Vitest with jsdom and Testing Library. No Vitest test opens a network connection: the page
tests stub `globalThis.fetch` and serve `tests/fixtures/*.json` through the real client and parsers.
`tests/stub.ts` holds the fetch stub and the request capture helpers; `tests/item-sets-write.test.tsx`,
`tests/items-write.test.tsx`, `tests/users-write.test.tsx` and `tests/media-write.test.tsx` assert the
method, URL and body each form sends; `tests/vocabulary-family-write.test.tsx` and
`tests/assets-jobs-write.test.tsx` do the same for the vocabulary family, assets and jobs, including the
validation messages each form renders; `tests/delete.test.tsx` asserts the method and URL each delete
control sends, the confirmation gate that holds the request back, the API message a 403 or a missing
row renders, and the list a success returns to; `tests/auth.test.tsx` asserts the key shape rules, the
key on every request builder with and without a session, the stored key, the key form and the 403
path; `tests/session.test.tsx` asserts the password sign-in, the server's message for a wrong password,
the `GET /api/me` probe a reload runs, and the sign-out that ends the session on the server;
`tests/password.test.tsx` asserts the URL shapes a password link can take, the `{key, password}` the
set-password page sends, the server's 404, and the forgot request; `tests/users-write.test.tsx` also
asserts the invite: the create, then the mint, the link the panel shows, whether mail went out, and the
copy control; `tests/lists.test.tsx`, `tests/admin.test.tsx`, `tests/items.test.tsx`,
`tests/media.test.tsx` and `tests/parsers.test.ts` cover the browse and show pages and the wire types.
`tests/vocabulary-details.test.tsx` covers the vocabulary family detail views and the browse links that
reach them.

## End-to-end tests

`npm run e2e` runs the built app against the real port in a real browser. The Vitest suite stubs
`fetch`, so nothing there proves a page loads from the server; this suite does.

`e2e/global-setup.ts` is the whole harness for one run:

1. Builds the SPA with `npm run build` into `dist/`.
2. Resolves the workspace's `debug/romeka` (`CARGO_TARGET_DIR` when set) and builds it with
   `cargo build -p omeka-cli`, unless `ROMEKA_BIN` names a binary to use instead.
3. Makes a temporary directory, runs `romeka install` for a fresh database, and mints an API key with
   `romeka key add`.
4. Picks two free loopback ports, starts `e2e/harness/proxy.ts` on one and `romeka serve` on the other
   with the proxy's origin as its base URL, `--store` on the run's store, and `ROMEKA_ARCHIVE_ROOT` on
   the fixture folder `e2e/harness/archive.ts` writes, so the Import page is on and each import job
   is a real `romeka job run` child.
5. Seeds an item set, three items, one imported media file and one site, through that origin, so the
   seeding run also proves the origin forwards `/api` with the key. Before the port opens the database
   it also inserts one asset and one job straight into it, with the asset's bytes in the store: no
   endpoint creates either row, and the write spec edits and deletes both.
6. Writes the origin, the paths and the minted key to a JSON record that the `baseURL` fixture reads,
   so a spec can type the real key into the sign-in form. The record also carries the seeded asset and
   job ids.

`cargo build -p omeka-cli` runs on every setup, so a warm target costs a fraction of a second and a
stale `debug/romeka` never serves the specs. Set `ROMEKA_BIN` to use a prebuilt binary and skip cargo.

The harness origin is a small HTTP server, not the coordinator's proxy. It serves the build, serves the
file store under `/files`, and forwards `/health`, `/api-context`, `/api` and `/s/` to the port. Every
`/api` request leaves it with `key_identity` and `key_credential` appended, which is how a reverse
proxy presents an API key to upstream. The app sends no key of its own, so the origin's pair is the
only one on the request. Nothing binds a fixed port.

The admin shows a signed-out caller only the sign-in panel, so every context starts signed in as the
administrator. `globalSetup` signs in once and writes the session cookie, scoped to `/api`, as the
Playwright storage state (`e2e/harness/session.ts`); the public site under `/s/` stays anonymous.
The origin keeps that shared session out of every sign-in and sign-out, out of any request that
carries a session of its own, and out of a write with no `Origin` (a spec's script), so a spec that
signs in as someone else acts as them and never ends the session the other specs hold. A spec that
needs the signed-out admin starts with `test.use({ storageState: SIGNED_OUT })`. The shared administrator's
lessons are off, so which lesson opens never depends on the specs run before; `lessons.spec.ts`
signs in its own users.

The origin serves a copy of `dist` in the run's temporary directory, made right after the build. A
`npm run build` started while a run is live empties `dist` before it writes it again; a page load in
that gap would answer 404 and fail whichever spec was on it.

The origin also answers the auth routes while the port serves none of them. `globalSetup` probes
`POST /api/login` once, after health: the port answers 404 for an unknown `/api` resource, which is
what it does before those routes land, and a wrong password is 401, which is the route working. When
the probe finds 404 the origin installs the stand-in in `e2e/harness/auth.ts` and says so in the log;
when the port has the routes the probe finds them and every auth request is forwarded as-is. The
stand-in holds the one administrator `romeka install` created, keeps its sessions in memory, and issues
the same `httpOnly`, `SameSite=Lax` cookie the contract describes, so the specs exercise the cookie the
page can never read. A key it mints works once, the way the real route enforces.

The teardown stops both servers and deletes the temporary directory.

### Specs

| Spec | What it drives |
| --- | --- |
| `e2e/specs/archive-import.spec.ts` | Import v2 against the real port: the Simple pass on a BHC_006-like folder (the files, what we found, one example, the list round trip, where they go, the check, the start, the progress, the end), then an undo that keeps the one item edited since and undoes it anyway; an Advanced edit read in Simple, the pairs list round trip, a ZIP sent from this computer, and a delete the server refuses until its count is typed. Every import job is a real `romeka job run` child. |
| `e2e/specs/pages.spec.ts` | Clicks every link in the admin navigation, asserts the shell renders the page title with no console error and no failed request, then asserts the navigation covers every member of the `AdminPageId` union in `src/app/pages.tsx`. |
| `e2e/specs/budget.spec.ts` | Measures the user paths through the Performance API in the browser and writes `perf/summary.json`: a cold first contentful paint, the items list render, an item detail render, an item create round trip and a page navigation, five runs each, with the `/api` span behind each one. |
| `e2e/specs/item-sets.spec.ts` | Creates an item set, renames it, sees it in the browse table, deletes it, and checks a cancelled delete writes nothing. |
| `e2e/specs/items.spec.ts` | Creates an item with an item set membership, reads the membership back in the edit form, renames it and deletes it. |
| `e2e/specs/family-write.spec.ts` | Creates, renames and deletes a vocabulary, a resource class, a property and a resource template; renames and deletes the seeded asset; updates and deletes the seeded job. The seeded job carries no status, so the run covers the null `o:status` the wire sends. |
| `e2e/specs/media.spec.ts` | Asserts the list preview is the square derivative and the show page hero the large one, both under this run's origin and answered as `image/jpeg`, that the browser decodes them, and that the derivative list links the stored original as `image/png`. |
| `e2e/specs/query.spec.ts` | Runs a search on System's Search query tab and checks the matches, then a query that matches nothing. |
| `e2e/specs/system.spec.ts` | Opens each route System replaced and checks it lands on the matching tab, that the nav names System once under Admin > Advanced and none of the old pages, and that each tab works; then follows a dashboard site name to that site. `SYSTEM_SCREENSHOTS` names a folder for the pictures. |
| `e2e/specs/locale.spec.ts` | Switches the shell to Spanish, asserts the labels and the nav change and the choice is stored, reloads to see it kept, then switches back to English. |
| `e2e/specs/public-site.spec.ts` | Reads the site slug off the admin sites table and loads the port's `/s` routes for it. |
| `e2e/specs/access.spec.ts` | Opens your own account and asserts Role, Active and Delete are hidden with the line why, and that a save still works; opens another user, reads the role line for two roles and the site the user has a role on, and follows its link to the site's Users tab; signs a researcher in and asserts the nav offers no Import, Settings, Jobs or Modules. |
| `e2e/specs/sign-in.spec.ts` | Opens five admin addresses signed out and asserts each shows only the sign-in panel and asks the API for nothing but `GET /api/me`; signs in from `#/users` and lands on Users. Signs in through the panel's key form with the run's minted key, asserts the key survives a reload and rides on the next `/api` request, signs out and asserts it is gone, then has a malformed key refused with no request. Also signs in with the administrator's email and password, asserts the browser set the `Origin` header the server checks for a write, that the session cookie is not readable from the page, that the reload keeps the session and that a sign-out revokes it. |
| `e2e/specs/finding.spec.ts` | Jumps from the header box to an identifier, to a `#<id>` set and to a title two items share, and has a miss said and cleared. Saves a searched, title-ordered Items list, opens it from the nav of a second browser, renames it and deletes it. |
| `e2e/specs/password.spec.ts` | Invites a user from the users page, follows the link the mint answered, sets a password, then reuses the link and asserts the one-time key is refused with the server's message. Then opens a keyless password link, follows the reset form and asserts the request answers for an address that has no account. |

### Running one spec

`npm run e2e` passes its arguments to `playwright test`, and the harness starts once for the run:

```
npm run e2e -- e2e/specs/items.spec.ts          # one file
npm run e2e -- --grep "media"                   # one title
npm run e2e -- --headed e2e/specs/query.spec.ts # watch it
```

Paths are given from `web/admin`. `ROMEKA_E2E_SKIP_BUILD=1` keeps the existing `dist`,
`ROMEKA_E2E_STATE_DIR` moves the run record, and `ROMEKA_BIN` and `CARGO_TARGET_DIR` choose the binary.
A failed test leaves its screenshot and error context under `e2e/test-results/`, which is ignored by
git. Traces are off by default; run with `PW_TRACE=1` to record one per failure, which writes the
trace zip into the same directory.

### Accessibility

`e2e/specs/accessibility.spec.ts` runs axe-core on every page the suite reaches. `@axe-core/playwright`
is the only added dependency. `e2e/harness/axe.ts` holds the check and the budget.

The rule set is the WCAG 2.0 and 2.1 A and AA tags plus axe's best-practice tags
(`AXE_TAGS` in `e2e/harness/axe.ts`). The budget is **zero serious and zero critical** violations. The
moderate and minor counts print on every run, pass or fail, so a regression is visible in the log
before it turns serious:

```
[a11y] admin #/items: serious=0 critical=0 moderate=0 minor=0
```

A run that breaks the budget prints each fault with its impact, rule id, node count and selectors.

The spec covers every admin page in the `AdminPageId` table (read from `src/app/pages.tsx` through
`e2e/harness/table.ts`, so a new page is covered with no edit), the item, item set, media, user and
site detail and edit surfaces, the item create form, the header sign-in panel with the key form
disclosed, the set-password and forgot pages, the invite form and the password-link panel it produces,
and the public site pages the theme renders: the index, the welcome page, the item browse, the page
browse, the advanced search and the search results.

Every page load asks `GET /api/me`, and an anonymous caller gets 401 by design. The `diagnostics`
fixture excludes that one answer from the failed-request list and counts every other status on that
route, and every status on any other route, as a failure.

Run one page by its test title, or the whole file:

```
npm run e2e -- e2e/specs/accessibility.spec.ts                 # every page
npm run e2e -- --grep "admin page #/items meets the axe budget" # one admin page
npm run e2e -- --grep "public"                                  # the public site pages
```

The check measures the rendered document, not the source. Fixes live where the markup is built: the
SPA under `src/`, the theme under `crates/omeka-site/src/theme/`.
