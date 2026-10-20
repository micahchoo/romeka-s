# Site builder: a proposal for user-friendliness (v3)

Date: 2026-10-19. Status: proposal, nothing built, no calls open. v3 applies the user's second
round of decisions (D6 to D10). Evidence labels: **measured** (run against the demo on 8124),
**read** (seen in code, file:line given) or **guess**.

## 0. What the user decided

| Call | Decision | What it changes |
|---|---|---|
| D1 Should the site keep a record of the curator's decisions? | No | No decision ledger. The removed-items bug (R9) is fixed the simple way: sync joins new items only, as upstream does (§7). |
| D2 Draft and publish? | Save stays live, with version history | §6 |
| D3 Should a new site get every public item? | No. The curator chooses from items and item sets. | §2 |
| D4 IIIF | Add the viewer. Mirador. | §8 |
| D5 Walk the site? | "Edit this page" is enough | §7 |
| D6 Sections or many blocks? | Sections | §5 |
| D7 Where are a site's items chosen? | On the Items tab only. A new site gets no items. | §2 |
| D8 Map block? | In | §5 "Map" |
| D9 IIIF manifests for the site's own items? | Yes | §8 step 4 |
| D10 Sync joins new items only, as upstream does? | Yes | §7 |

New asks: better presets, an editable footer, a thorough block review, clearer reorder controls in
the theme settings, hero backgrounds from site media, and an item picker with paging and metadata.

## 1. Item picker

**What is there (read).** One `ItemPicker` serves the editor's attachments and the Items tab
(`editor/fields/ItemPicker.tsx`). It asks for the 24 newest items (`RESULTS = 24`, `:22`).
It has no paging and no total. Each item shows only a thumbnail and a title. The help text says
"Type part of a title", but the server matches any value (`omeka-api/src/read/query.rs:65-75`).
Picks outside the current 24 results are invisible, and only a count shows. Every list response
already carries values, class, created date, thumbnails and the public flag
(`api/types.ts:84-98`). So a metadata view costs no extra request.

**Design: a browser with columns, a peek pane and a picked tray.**

- **Job.** A curator finds the exact items they mean in an archive of thousands and picks them.
- **Layout.** Three columns. The first holds sources: "All items", then each item set with
  its count. The second holds results as rows, not tiles. The third holds a peek of the focused item.
  - A result row shows the thumbnail, the title, two or three key values (creator, date,
    identifier), the item's sets and its media count. All of it comes from the list response.
  - The peek pane shows the item's metadata, field by field, in a compact form. It also shows its
    media strip and an "Open in admin" link. This is the transcluded metadata the user asked for.
- **Feedforward.**
  - The search box reads "Search titles and values".
  - Filters for class and "Has media" sit above the results. The API supports both.
  - The total shows before any scrolling: "1,284 items match".
- **Paging.** "Show 24 more", with "48 of 1,284" beside it. A curator who picks across pages
  keeps the earlier rows in view. That is why it is "Show more" rather than numbered pages.
- **Picked tray.** A tray at the bottom always shows the picks as chips. A chip can be removed or
  dragged to reorder, because attachment order is the display order. Picks outside the visible
  results stay visible here.
- **Picking a whole set.** A set in the first column has a "Pick the whole set" button.
  The Items tab uses this (§2). The editor's attachments hide it.
- **Keyboard.**
  - Results use a roving tabindex, so Tab enters the list once.
  - Arrows move the focus, Space picks, Right arrow moves focus into the peek, and Left arrow
    goes back to the sources.
- **States.**
  - Over 1 s: skeleton rows that keep the row height.
  - No match: "No item matches. Clear the filters" with a button.
  - Failure: "The items could not be read." with a Retry button. Picks are kept.
  - Leaving and coming back: the tray state lives in the field value, so nothing is lost.
- **Closest existing systems.** The macOS Finder column view and Zotero's collection pane.
- **Cost (guess).**
  - It is its own lazy chunk of about 5 to 7 KB gzip, under the 10,240 B ceiling.
  - It replaces the 24-tile grid in both places it is used.
  - The admin items list has no search box or thumbnails. The new row component can serve it later.

## 2. A site's items (D3, D7)

- **A new site gets no items.**
  - Site create stays short, with no item step.
  - Remove the "starts with all public items" default and its form note (`sites/create.tsx:23,51-57`,
    i18n sites:361).
  - Sync starts off.
- **The Items tab is the one place to choose.**
  - Its "Pick items" and "By item set" panels become the single picker from §1.
  - A curator can pick whole item sets, single items, or both.
  - "Add items ({count})" keeps its dry-run preview.
  - "Keep in sync" means "new items added to the picked sets join the site" (D10).
- **The empty site points to the Items tab.**
  - The hub's Items count reads "No items yet" and links to the tab.
  - The welcome starter's browse preview renders an empty state in the editor preview: "No items
    on this site yet. Add items on the Items tab."
  - On the public site, the empty state is a short sentence with no admin link.
- **Replace.** "Replace with these" uses the server's existing Replace action (`site_items.rs:45-46`).
  It comes after a confirmation that names how many items leave.

## 3. Reorder controls (theme "Resource pages" and four more lists)

**What is there (read).** `ResourcePageBlocksField.tsx:75-134` shows each row as a raw upstream
layout name ("Media render", "Lightbox gallery") with text buttons "Up", "Down" and "Remove". You add
a row with a "[Choose a block]" select and an "Add" button. Nothing explains what a section
shows. Nothing previews it, because the theme preview renders only a site page. The same
up/down/remove pattern appears in `AttachmentsField`, `RepeatField` and `PageListField`, and with
arrow icons in item-types `field-row`. The `moveAt` helper is copied three times.

**Design.**

- **Rows that say what they show.**
  - Each section gets a one-line description, for example "Values: the item's metadata, field
    by field" and "Media embeds: each file shown in full, one after another". This needs a
    `helpKey` in the generated registry.
  - Lightbox gallery and Resource class render nothing today (`resource_page.rs:189-199`). Drop
    them, or build them, before they get descriptions.
- **Direct manipulation.**
  - Each row gets a drag handle (⠿) with a grab cursor.
  - The keyboard works as in the navigation tree: Space picks a row up, the arrows move it,
    Space drops it and Escape cancels. A live region announces each step.
  - Up and Down stay as small icon buttons with tooltips, for anyone who does not drag.
- **Hide instead of Remove.**
  - Unused sections sit in a "Hidden sections" list under the shown ones, each with a "Show"
    button. Hiding is reversible and in plain view, and the select-plus-Add step goes away.
- **A sample item beside the list.**
  - The preview renders a real item page from the site under the draft order.
  - The preview route needs a resource-page mode for this (guess: small).
- **One shared hook, five lists.**
  - Extract a `useReorderable` hook from the canvas code (`canvas/move.ts`, `CanvasCard.tsx:140-170`).
  - Put all five lists on it and delete the three `moveAt` copies.

## 4. Theme presets and the footer

**Why the presets look poor (measured: the template page rendered under all five presets at
1280 px and 390 px).**

- All five share one skeleton: the same header with a boxed search, the same hero, and the same
  grey card grid. They differ only in colour and font.
- Only system fonts ship (`theme/choices.rs:70-80`), so every preset falls back to the same few
  faces on each operating system.
- A hero with no image renders as a flat coloured slab. With brown on peach (Archive) or blue on
  blue (Civic) it looks unfinished.
- The design assumes good data. Items with no title and no media fill the grid with grey
  mountain glyphs and "[Untitled]". On a phone each card is one full-width grey glyph.
- On a phone the search box is always open and takes the top of the first screen, before the
  hero.

**Design.**

1. **Presets that differ in structure, not only in colour.** Add structural tokens to
   `tokens_schema.rs`:
   - a hero style: type only, full-bleed image, or image and text side by side
   - a card style: caption below, caption over the image, or a list row with metadata
   - a header style: masthead, bar, or minimal
   - a footer layout
   
   Each preset is named for a use:
   - **Exhibit**: image-led, full-bleed hero, serif display.
   - **Archive**: dense, metadata-forward list rows.
   - **Journal**: long-form reading with a narrow column.
   - **Gallery**: dark image wall with overlay captions.
   - **Community**: bright, with large buttons.
2. **Webfonts.** Self-host about five OFL families as woff2 variable fonts. Examples are Source
   Serif 4, Source Sans 3, Fraunces, Inter and IBM Plex. Add Noto Sans and Noto Serif for
   Devanagari, Tamil, Telugu, Kannada and Malayalam, because nine locales ship. They load on the
   public site only, so the admin JS budget does not apply. Subset them and load with
   `font-display: swap` (guess: 40 to 90 KB per family).
3. **Design for poor data.**
   - An item with no thumbnail gets a tinted surface with its class icon and its title, not a
     photo glyph.
   - A hero with no image uses the type-only style, so no slab is drawn.
   - Phone cards are two to a row.
   - Search collapses behind an icon in the header on phones. A `<details>` element does this
     without JS, as the menu already does.
4. **Process.**
   - First, prototype three directions as throwaway HTML over the real preview output, using
     the poteto Prototype playbook.
   - The user picks one; then encode it as tokens.
   - Every preset must look finished with the demo's untitled, imageless items. That is the
     acceptance test.
5. **Unfamiliar option.**
   - "From your collection" derives a preset's palette from the site's own item images.
   - Cost: server-side colour extraction over the thumbnails.
   - Closest existing system: Apple Music's artwork colours.

**Footer (read).**

- The footer is one `footer_text` HTML field (`default.rs:292,418-426`). It sits at the end of
  "Header and footer", low on the Theme page.
- "Powered by Romeka S" is fixed and no control removes it (`i18n/messages.rs:89`).
- The help text does not say the text appears above that line.
- The field has no slots for links, columns, address or licence.

**Design.**

- **A Footer section** sits near the top of the Theme page, with these controls:
  - one to three columns, each HTML
  - a links list, edited with the navigation editor
  - contact and address
  - a licence choice (the CC licences render a badge and a link)
  - a "Show 'Powered by Romeka S'" switch
- **Footer and header clickable in the page editor.** The page editor's live frame marks the
  footer and header. A click opens them in the inspector under a banner: "Site-wide. Changes
  every page."
  - This is the presence move from v1, kept small.
  - Cost: preview markers on two regions, plus an inspector mode for site settings.

## 5. Blocks

**What is there (read, full table in the audit).**

- **Layouts.** The 15 upstream layouts plus one Romeka block, the hero.
- **Image left, text right.** A curator can build it only by turning the whole page into a grid,
  or by floating an asset with an undiscoverable maximum width.
- **Missing kinds.** No image-and-text, gallery, carousel, button, columns, accordion, quote,
  timeline, map or search block exists.
- **Thin existing blocks:**
  - The asset block crops every image to 3:2 and links to pages only.
  - Browse preview has one card style and no choice of fields.
  - List of pages renders bare links, with no page cards.
  - Item with metadata always prints the whole record.
- **Groups.** They cannot nest, cannot set their own columns, and on a grid page always span the
  full width.
- **Shared styling.** It has no anchor id, no full-bleed, no text colour, no vertical alignment
  and no theme-colour picker. Colours are hex only.

**Design: sections first, then a few content blocks.**

The unfamiliar move is to make `blockGroup` a real layout container rather than to add ten blocks.

- A **section** has its own layout preset: split 50/50, 40/60 or 60/40, two or three or four
  cards, stacked, or full-bleed band. It also has a vertical alignment and a background, either a
  theme colour or site media.
- Any existing block can sit in a section's slots. "Image left, text right" becomes a Media
  block and an HTML block in a 40/60 split.
- Upstream already renders a group as a wrapper div around its blocks, so a section degrades to
  stacked blocks. The section's settings live in `o:layout_data.romeka`, which upstream's admin
  keeps.
- Section templates in the picker cover the common shapes: "Media and text", "Three cards",
  "Quote band".
- Cost: groups gain their own grid in `layout.rs:185-204`, and the canvas gains slot drop
  targets.
- Closest existing systems: Gutenberg's Columns and Group blocks, and Squarespace sections.

**New field kind: `mediaRef`.** It holds one media from the site, stored in
`o:layout_data.romeka.settings`. The control is the §1 picker with a media choice. The plain
rendering writes `/files/large/<storage>.jpg`. The write must refuse private media, because the
plain HTML is public. The audit found that attachments do not fit the html carrier: the server
loads them only for media and item-with-metadata blocks, and upstream's html form may drop them
(not measured).

**Upgrades to existing blocks, ranked by curator value:**
1. **Hero.**
   - A background from site media or from an asset.
   - Height (short, medium, tall, full screen) and a focal point.
   - An overlay colour from the theme.
   - Up to two buttons, a heading level, and the asset's alt text in the rich render (today it is
     `alt=""`, `hero.rs:121`).
2. **Browse preview.**
   - A card style: grid, list, or carousel.
   - The number of columns.
   - Which fields to show.
   - The sort.
3. **Asset.**
   - No forced crop.
   - Links to URLs as well as pages.
   - A row or grid arrangement.
4. **List of pages.** A card mode with the page's thumbnail and summary.
5. **Item with metadata.** A choice of fields, and a compact card mode with a citation line.
6. **IIIF blocks.** A height choice, and site media as the source (§8).

**New content blocks, as Romeka carriers with a plain rendering:**
- **Gallery.** Items' media or assets in a grid, masonry or carousel, with a lightbox. The
  lightbox needs a small script on public pages.
- **Featured item.** A card with chosen fields.
- **Button.** One or two links styled as buttons.
- **Quote.** Text, attribution and an optional source item.
- **Accordion.** The plain rendering is headings and paragraphs, because `details` is not allowed
  by the purifier.
- **Timeline.** Items placed along a date property. The plain rendering is a list in date order.
- **Search box.** Check first whether the purifier allows `<form>` in the plain rendering.
- **Map (D8).** The design follows this list.

**Map block design.**

- **Where the places come from (measured).** No item on the demo holds coordinates, and Romeka
  has no geographic data type. Upstream needs the Mapping module, which uses its own tables.
- **A point is a value.** Store each point as an ordinary value, "lat, lng", in one property per
  site, for example `dcterms:spatial` or a `geo:` term.
  - A site setting, "Location field", names that property.
  - The values survive an upstream round trip, because they are plain literals.
  - Upstream shows them as text.
- **Entering points.**
  - The item form gets a map-pick control for that property: click the map, or type
    coordinates.
  - This touches the item form (the Describe area), so tell the owner of that area before the
    work starts.
  - Archive import can map a CSV column into the property.
- **The block.**
  - Fields: an item query (§1 picker or a saved search), the basemap, the starting view (fit
    all or a set centre and zoom), the height, and the popup fields (title, thumbnail, a chosen
    value).
  - An optional "Group nearby points" switch clusters close markers.
- **Rendering.**
  - Use Leaflet 1.9 (BSD-2, about 42 KB gzip) on public pages only.
  - The tile URL template and its attribution are a site setting. The default is OpenStreetMap,
    whose usage policy requires the attribution line and limits heavy use. The help text says so.
- **Plain rendering.** For upstream and the script-free editor preview: a list of the items with
  their place values and links.
- **Item pages.** A "Map" resource-page section shows the item's own point.
- **Closest existing system.** The Omeka S Mapping module, but storing points as values
  instead of in its own tables.

**Shared styling additions, all under `o:layout_data.romeka`:**
- An anchor id.
- A spacing scale (none, small, medium, large) beside the raw padding.
- Full-bleed width.
- A text colour.
- Vertical alignment.
- Theme-token colour choices beside hex.

**Cost of each new block (read).** The hero is the model:
- a Rust module, plus a bump to the fixed `ROMKA_BLOCKS` array size (`romeka.rs:128`)
- CSS, and the selector assertion list
- a regenerated `blocks.json`
- strings in nine locales, with the Indic locales filled by `scripts/translate-locales.mjs`
- a carrier test and an e2e spec

## 6. Version history (D2)

**What is there (read).**

- `site_page` already has `created` and `modified`.
- `PUT /api/site_pages/{id}` overwrites the page, deletes its blocks one by one and inserts them
  again, all outside a transaction (`omeka-api/src/write/site_pages.rs:125-178`). A failure
  partway through leaves the page with only some of its blocks.
- Nothing checks for a conflicting save.
- Migrations on disk run to 0005 (0005 is the archive import), so the next one is **0006**. MySQL
  has no 0004 file.
- `romeka_publication_version` (0003) is the model: one row per version, never updated, holding
  a JSON snapshot.

**Design.**
1. **Table.** `romeka_site_page_version` (page id, version, created, user id, snapshot JSON).
   Claim migration 0006 first.
2. **Write.** Take the before-image in `update()` after authorisation, using
   `site_page_document`, the same call `delete` uses. Put the snapshot, the update and the block
   rewrite in one transaction. This also fixes the partial-write hazard.
3. **History panel.**
   - A "History" panel in the editor lists the versions: when, who, and how many blocks.
   - Selecting one renders it in the live frame through the preview route, which already renders
     any page body.
   - "Restore this version" loads it into the draft. The next Save makes a new version.
4. **Conflict check.**
   - The editor sends the `o:modified` it loaded. A mismatch answers 409.
   - The editor then shows "Saved by {name} at {time} since you opened this page", with the choices
     "Show their version" and "Keep mine".
5. **Retention.** Keep the last 100 versions per page (guess). Deleting a page deletes its
   history.

## 7. Smaller items from the decisions

- **"Edit this page" (D5).**
  - The public user bar links to `/#/sites/<id>/pages/<pageId>`, and on item pages to the item's
    admin page.
  - The link shows only to callers who may edit. That needs the caller's site role in the render
    context.
  - The same role lets the admin hide controls a Creator cannot use (R10).
- **R9 without a ledger (D1).** Sync attaches an item only when it is created, as upstream does
  (`compat.md:67`). Then "Items you remove stay removed" is true, unless a new item matches. Say
  that in the help text.
- **The repairs from v1** still stand: R1 to R8 and R10 to R13, and the copy bugs. The worst
  three are below.
  - R1: page settings are lost after selecting a block.
  - R2: the public Logout does not log out.
  - R8: the preview fails on an untitled new page.

## 8. Mirador (D4)

**What is there (measured and read).**

- The IIIF presentation block embeds `/iiif-viewer?...` (`iiif_presentation.rs:90`). That path
  serves the admin shell.
- The IIIF image block loads OpenSeadragon from a path that answers 404.

Upstream's `IiifViewerController` (read in the oracle container) works like this:
- It serves a bare page that loads `vendor/mirador/mirador.min.js`. That is Mirador 4, 2.36 MB
  minified, and the package lists `openseadragon ^5.0.1`.
- It takes `?url=` and an allow-listed `mirador_config`: `window.sideBarOpen` and
  `selectedTheme`.

**Design.**
1. **The viewer page.** Mount `/iiif-viewer` with the same contract and allow-list. Vendor Mirador
   4 (Apache-2.0) and OpenSeadragon 5 (BSD-3) under `/assets/vendor`, and add both to NOTICE.
2. **Settings.** Wire the latent `iiif_viewer_sidebar` site setting to `sideBarOpen`. Apply
   `iiif_viewer_theme` on item and media pages too. Today it reaches only the block
   (`omeka-server/src/site/load.rs:474-483`).
3. **Preview.** The editor preview is script-free, so it shows a poster card instead of the
   viewer: "IIIF viewer: <title>. It opens on the public page."
4. **Manifests for the site's own items (D9).** Upstream core does not do this; it needs the
   IIIF Server module.
   - **Routes.** `/iiif/3/item/<id>/manifest` and `/iiif/3/item-set/<id>/collection`. Both are
     IIIF Presentation 3 and obey the same visibility rules as `/s/` pages. A private item answers
     404.
   - **Canvases.** One canvas per image media, in media order. Labels and `metadata` come from
     the item's values. `rights` comes from `dcterms:rights` when it holds a URI.
   - **Image size.** Every canvas needs its image's width and height. Measured: the demo's media
     store neither (`data` is `[]`).
     - `omeka-media/src/image_metadata.rs:65-67,190-192` reads them from the file header without
       decoding the image.
     - Read them on the first manifest request and cache them in memory by storage id. This needs
       no migration and never writes upstream's `media.data`.
   - **Phase 1: fast.** The canvas body is the "large" derivative. A demo original is 5,989,397
     bytes (measured), too slow to load whole. The original is linked as a download through
     `rendering`.
   - **Phase 2: deep zoom.** Add an IIIF Image API level 1 endpoint
     (`/iiif/image/3/<media>/...`). It crops and scales on request with the `image` crate,
     keeps a disk cache, and runs under `spawn_blocking`, as the ingesters do.
   - **Media that is already IIIF.** The canvas points at the remote image service.
   - **Where curators meet it.**
     - Item pages gain a "View in the IIIF viewer" link and the manifest link with the IIIF
       logo.
     - The IIIF presentation block gains a "An item from this site" source that uses the §1
       picker, beside "A manifest URL".
     - The IIIF image block gains site media as a source, through `mediaRef` (§5).

## 9. Order of work

The waves are independent where noted, so peer sessions can take them in parallel.

| Wave | Contents | Depends on |
|---|---|---|
| A Repairs | v1 repairs R1 to R13, copy bugs, the R9 change | none |
| B Picker and lists | §1 picker, §2 Items tab and an empty new site, §3 reorder hook in five lists | none |
| C Look | §4 preset prototypes, then the user picks, then structural tokens, webfonts, poor-data design, phone header, footer section | prototypes before tokens |
| D Blocks | §5 `mediaRef` and hero, then sections, then block upgrades, then new blocks | B (the picker) |
| E History | §6 migration 0006, transaction, history panel, conflict check; §7 "Edit this page" and role in the render context | claim 0006 |
| F IIIF | §8 viewer page, vendored files, settings, poster; then local manifests (phase 1), then the Image API (phase 2) | none |
| G Map | §5 location setting, item-form map control, map block, item-page section | B (picker); tell the Describe owner |

Budgets: the admin lazy chunks have a 10,240 B gzip ceiling, and the initial JS has 87,040 B
(`web/admin/perf/budget.json`). The picker, the history panel and each new block form go in
their own chunks. Fonts and Mirador load on public pages only.

## 10. Calls

No product call is open. These defaults are taken, and the user can override any of them:

- The map's default basemap is OpenStreetMap, with its attribution line.
- A map point is a "lat, lng" literal in one property per site.
- Manifests use the "large" derivative until the Image API ships.
- Page history keeps 100 versions per page.
- Three preset directions are prototyped before any tokens are built. Picking one is the
  user's call when the prototypes are ready.

D7 and D10 change documented behaviour. Update `docs/compat.md:57,66-67` and the user guide in
the same waves.

## Evidence trail

- Demo screenshots, preset renders and contact sheets:
  `/tmp/claude-1000/-mnt-Ghar-2TA-DevStuff-notebook-omeka-research-romeka-s/944b9f71-fa12-4ab1-a52f-df0b7a82623c/scratchpad/walk/`
  (`presets-desk.png`, `presets-phone.png`).
- Upstream IIIF viewer: `omeka-oracle-app-1:/var/www/html/application/src/Controller/IiifViewerController.php`.
- Untested UI, where regressions can hide: the Desktop, Tablet and Mobile buttons, "View public
  page", the theme logo, banner and footer, theme import and export, saved templates in e2e, the
  editor's undo in a browser, and the site title after a rename.
- Stale docs to fix alongside: compat.md:64, omeka-site README:302, README:96 and :100,
  user-guide.md:79-82.

## Appendix: the v1 repair list (R1 to R13)

| # | Defect | Evidence | Fix |
|---|---|---|---|
| R1 | After a block is selected, Page settings (title, slug, Public, Save as template, Delete) never return until that block is deleted or the page reloads. | read: `editor/shell/PageEditor.tsx:103,310,341`; nothing passes `null` to `onSelect` | Escape, a click on empty canvas, and a "Page" crumb in the inspector header deselect. |
| R2 | The public user bar's Logout opens the admin and does not end the session. | measured: `GET /logout` serves the admin shell; `theme/user_bar.rs:28` | A POST form, or a server `/logout` that clears the cookie and redirects back. |
| R3 | IIIF image and presentation blocks render an empty box. | measured: OpenSeadragon 404; `/iiif-viewer` is the admin shell | Wave F (§8). |
| R4 | Resource-page blocks Lightbox gallery and Resource class render nothing. The linked-resources Class column is always empty. | read: `resource_page.rs:189-199,365` | Build them or drop them from the list, and fill or drop the column. |
| R5 | oEmbed "Reloads the embed from the provider on save" never fetches. | read: compat.md | Remove the option, or say it is not available. |
| R6 | A group's "Blocks in the group" number snaps back. | read: `editor/blockContent.ts:33` | Read-only, with the group move actions as the way to change it. Sections (§5) replace it. |
| R7 | Undo and Redo vanish when the Outline is closed. | read: `canvas/PageCanvas.tsx:171-181,292-313` | Toolbar buttons and shortcuts beside Save. |
| R8 | A new page cannot preview until it has a title and slug (422). | measured | The preview fills a placeholder title and slug. |
| R9 | "Items you remove stay removed" is false: an edit re-attaches. | read: `site_items.rs:229-231` | D10: sync attaches on create only. |
| R10 | The admin ignores site roles; a Creator sees Manager controls that 403. | read: `omeka-api/src/read/mod.rs:328-358` | Read the caller's site role in SiteFrame; hide or disable with a reason. |
| R11 | Users tab shows a user's number, and lets the same user be added twice. | read: `sites/users.tsx:126-128,89-93` | Drop the bare-id option; filter listed users out. |
| R12 | Theme, Site settings and Users lose edits on navigation without a warning. | read: `theme.tsx:224`, `settings.tsx:110-134`, `users.tsx:66-97` | Reuse `useUnsavedGuard`. |
| R13 | A hand-set colour under the "auto" scheme never reaches dark mode. | read: `theme/tokens.rs:177-207` | Keep a dark override per token, or say so beside the field. |

Hidden editor keys to surface (one "Keys" popover): Ctrl/Cmd+S, Ctrl/Cmd+Z, Delete/Backspace removes a block
(make it undoable with a toast), drag a card (add a grab handle), Shift+arrows resize a grid span.
Copy bugs: "1 issues" (`editor/index.ts:226`); the sites list shows "default" where the hub shows "Default";
the navigation error "Needs a target" fires when the label is missing (`navigationTree.ts:294-307`).
