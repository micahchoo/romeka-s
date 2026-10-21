# Admin frontend performance (web/admin)

The frontend half of the speed track. `perf/README.md` documents the instrument.
This file is the decision log: one entry per hypothesis, the measurement that
decided it, and the verdict.

## Method

- Box: 32 cores, load average about 7, one measurement process at a time.
  Measured 2026-10-17.
- The user paths come from `npm run e2e`, which runs
  `e2e/specs/budget.spec.ts`: five runs each of a cold first contentful paint,
  the items list render, an item detail render, an item create round trip and a
  page navigation. Each run resets the page first, so every run pays the same
  reads.
- A number is the median of five runs, with the min and max beside it. A gap
  smaller than the min to max range is not a difference.
- Where a change is compared against the code it replaces, the runs are
  interleaved (A, B, A, B, ...) in one session and both sides are rebuilt from
  source, so machine drift favours neither.
- The API behind every timing is the debug `romeka` the e2e harness starts. The
  bundle numbers are the gzip size of the built initial payload and of each lazy
  chunk, which are deterministic.
- Baseline is commit `eda75f7`.

## Baseline (commit eda75f7, five runs each)

| Path | Median | Min | Max | API span | Requests | Slowest request |
| --- | --- | --- | --- | --- | --- | --- |
| first contentful paint | 56.0 ms | 56.0 | 56.0 | none | 0 | none |
| list render | 125.0 ms | 123.3 | 126.8 | 122.5 ms | 3 | `/api/resource_classes` 122.4 ms |
| detail render | 127.0 ms | 124.5 | 127.5 | 125.1 ms | 4 | `/api/resource_classes` 124.8 ms |
| create round trip | 256.5 ms | 254.0 | 261.6 | 255.5 ms | 5 | `/api/resource_classes` 128.9 ms |
| page navigation | 1.5 ms | 1.2 | 2.4 | none | 0 | none |
| bundle, js gzip | 96668 B | | | | | |
| bundle, css gzip | 2258 B | | | | | |

The list and detail medians re-measured between 123 and 128 ms across the later
sessions in this file, so a few milliseconds of drift sits under every number
below.

## The limiter behind three of the five paths

Measured against a debug `romeka` on a temporary database, seven interleaved
rounds of each shape. One `curl` process per shape with `--parallel`, timed with
a wall clock around the process, so process start-up is paid by every side.

| Requests | Auth | Median | Range |
| --- | --- | --- | --- |
| 1 | yes | 124 ms | 122 to 125 |
| 3 | yes | 127 ms | 125 to 129 |
| 4 | yes | 128 ms | 126 to 130 |
| 1 | no | 9 ms | 8 to 10 |
| 3 | no | 11 ms | 10 to 12 |

`curl` and node start-up is about 6 ms of that, since an unauthenticated request
does about 2 ms of work and reports 9 ms here. So one authenticated request costs
about 118 ms of work and three parallel requests add about 3 ms. The cost is the
key check (`bcrypt::verify` in `crates/omeka-api/src/auth.rs`), not the payload
and not the database.

The browser confirms it. In the two baseline runs of the interleaved session
below, the list render reported an API span of 121.3 ms over 3 requests and
122.3 ms over 3 requests, while the item request alone spans 121.5 ms. Three
parallel requests cost what one costs.

This bounds the frontend. The wall clock of the list render and the detail
render is one API round, and no frontend change removes that round except by not
asking. Request count buys at most about 3 ms, so a cache or a partial render
that removes the label reads is not worth a measured comparison.

An earlier probe with one `curl` process per parallel request reported 128 ms for
three requests against 117 ms for one, an 11 ms gap. That gap was the cost of
three extra process spawns inside the timed region, not the server.

## Decision 1: the create round trip paid two rounds (accepted)

**Hypothesis.** After a create, the show page reads the item back even though the
`POST` already answered with the same representation, and it then waits for that
item together with the media and the label lookups. That makes the round trip two
API rounds.

**Measurement, before.** create round trip median 256.5 ms, API span 255.5 ms
over 5 requests. Two rounds: the `POST`, then the four reads of the show page.

**Change.** `src/app/written.ts` holds the record a write returned, and
`ItemDetail` renders that record. The media and the label lookups are now
separate reads that fill in as they answer, so the record heading no longer waits
for them. `src/pages/items/form.tsx` hands the created item to the slot on the
way to the show page.

**Measurement, after.** create round trip median 125.3 ms (min 123.5, max
125.7), API span 122.0 ms over 1 request, the `POST`. The four interleaved runs
in the session below put it between 121.1 and 123.6 ms. 2.0x, and the range does
not overlap the baseline.

A plain detail visit, where no write precedes the page, is unchanged: median
123.2 to 124.0 ms after against 123.7 to 123.5 ms before, inside the run range.

**Verdict.** Accepted. The user waits one round instead of two after a create.
The media grid and the owner and class labels appear about one round later, which
is when they appeared before, except that the record is on screen while they
load.

**Limitation.** Consuming the slot on read means a development `StrictMode` build
runs the loading effect twice and refetches, so only a production build renders
the seeded record. The e2e harness drives the production build, so every number
here is the production path. The item set, user and site creates have the same
shape and are left alone, because nothing measures those paths.

## Decision 2: render the list rows before the label reads answer (rejected)

**Hypothesis.** The list page waits for `/api/items`, `/api/users` and
`/api/resource_classes` together, but only the items read is needed to draw the
rows. Reading the two label lists beside the rows should drop the list render by
the concurrency cost of 3 requests against 1.

**Measurement.** The change was built: the row read and the label reads became
two states, the labels load once per mount instead of once per page, and the
table draws as soon as the rows are known.

| Round | Rows first (A) | Baseline (B) |
| --- | --- | --- |
| 1 | 123.1 ms (120.9 to 124.1), 2 requests | 123.5 ms (122.9 to 124.3), 3 requests |
| 2 | 124.0 ms (121.2 to 131.8), 2 requests | 124.5 ms (122.4 to 126.4), 3 requests |

A, B, A, B in one session, five runs each, both built from source. The API span
is the same on both sides: 121.5 ms on A against 121.3 and 122.3 ms on B.

**Verdict.** Rejected. The gap is 0.4 to 0.5 ms against a run range of 3 to 4 ms,
so there is no measurable difference, and the reason is in the limiter section:
three parallel authenticated requests cost what one costs. The change was
reverted. The same reasoning rejects a session cache of the label lists.

## Decision 3: cut the first contentful paint by splitting the bundle (not taken)

**Composition.** Measured by building the entry with `react-dom/client` and
nothing else, against the full build, with the same vite config. `npm run build`
already produces the production React.

| Build | Raw | Gzip |
| --- | --- | --- |
| React 19 and its scheduler only | 219557 B | 67863 B |
| The app (baseline commit) | 339820 B | 96668 B |
| The app with the 16 other pages replaced by a stub | 288656 B | 86270 B |

React is 70% of the gzipped bundle. The app's own code is 28805 B gzip, of which
the 16 pages other than items are 10398 B gzip. There is no unused dependency to
drop: the only runtime dependencies are `react`, `react-dom` and their
scheduler.

**Where the first contentful paint goes.** Five cold loads with the HTTP cache
disabled through CDP, against a static server that sends the bundle without
`content-encoding` (so the transfer is the worst case).

| Term | One run |
| --- | --- |
| document responseEnd | 3.3 ms |
| JS responseEnd | 8.9 ms |
| JS responseEnd to first contentful paint | 44 to 47 ms |
| first contentful paint | 52 to 56 ms |

The limiter is parsing and executing the bundle, not the transfer. The request
starts about 5 ms after navigation and its response ends at 8.9 ms, so the
transfer of the 340585 B is a few milliseconds over loopback, and the browser
then spends 44 to 47 ms between the JS response end and the paint.

**Splitting measurement.** The app built with the 16 non-items pages stubbed is
the best case for code splitting: those bytes are absent from the first parse.
Five interleaved rounds of five cold loads each.

| Build | Median first contentful paint per round |
| --- | --- |
| full | 52, 56, 56, 56, 52 |
| 16 other pages stubbed | 52, 52, 48, 48, 52 |

Median 56 ms against 52 ms, and the ranges overlap at 52 ms. The browser reports
paint in 4 ms steps, so this 4 ms gap is one quantum and inside the run range.
There is no measurable first contentful paint win to take.

**Verdict.** No change to the paint path. Code splitting has no measurable first
contentful paint win, and Decision 5 later took the split for the bundle budget
instead. The chunk for the current route is still a second, sequential request
before that route can render.

Two notes on this number. The harness origin (`e2e/harness/proxy.ts`) serves
`dist` without `content-encoding`, while `crates/omeka-server/src/http.rs` gzips
a body when the client accepts it, so the 340585 B transfer is a harness worst
case and a real deployment would send about 97 KB.

## Decision 4: the vocabulary, asset and job write forms grow the bundle (accepted)

**Change.** The `adminwrite2` unit adds the write forms for the vocabulary family, assets and jobs:
`src/pages/vocabularies/`, `properties/`, `resource-classes/`, `resource-templates/`, `assets/` and
`jobs/`, their client methods and request encoders, and the message ids they render in two catalogues.

**Measurement, before.** bundle js gzip 96668 B at commit `eda75f7`, budget 102400 B, headroom 5732 B.

**Measurement, after.** bundle js gzip 103351 B (raw 378394 B), the same build command and the same
box. The feature costs 6683 B gzip. Eight duplicate message ids were collapsed into the shared
`form.errorEmptyLabel`, `field.addProperty`, `field.selectProperty` and `navigation.moveUpShort` /
`moveDownShort` keys before this number, which took the figure from 104288 B to 103351 B.

**Verdict.** Accepted. React is 67863 B gzip of the 103351 B, so the app's own code is 35488 B and the
write surface is about a fifth of that. The budget moves with the feature: `perf/budget.json`
`jsGzipBytes` 102400 to 109568 (107 KiB), which leaves 6217 B of headroom against the baseline's
5732 B. Timings are unchanged; no measured path goes through the new pages. Decision 5 removes the
raise and takes the bundle back under the original 102400 B.

## Decision 5: split the router so a route loads its own chunk (accepted)

**Premise.** One entry bundle held every admin page. Decision 4's write forms took it from 96668 B
gzip to 103351 B and the budget was raised from 102400 to 109568 to fit. A budget raised to cover a
change stops bounding the axis it measures, so the fix is to move the pages out of the entry rather
than to keep the raise.

**Measurement, before.** One emitted js asset, `dist/assets/index-BlgUSnBM.js`, 378394 B raw and
103351 B gzip. Budget 109568 B, headroom 6217 B. Every route added 0 B, because every page module was
already in the entry and parsed by the first paint.

**Change.** Each row of `src/app/pages.tsx` holds its `element` as a `React.lazy` component, so each
`import('../pages/<area>/page')` becomes a dynamic import and Rollup emits one chunk per route.
`src/app/shell.tsx` wraps the page in a `Suspense` boundary whose fallback is the existing
`loading.page` message. The `AdminPage` contract keeps its `titleKey` and `upstream` fields, so the
nav, the heading and the `e2e/harness/table.ts` reader are untouched. `vite.config.ts` sets
`build.manifest: true`, which `perf/pages.mjs` reads to name each route's chunk.

**Measurement, after.** The same build command on the same box. The initial payload is 85354 B js
gzip and 2258 B css gzip, the two files `dist/index.html` references. 24 lazy chunks sit under
`dist/assets`, the largest 6909 B. The entry drops 17997 B gzip, 17.4% of the 103351 B. A page's own
chunk plus the shared chunks it pulls in, `npm run perf:pages`, is:

| Route | Own chunk | Shared | Total | Before |
| --- | --- | --- | --- | --- |
| assets | 1343 | 4305 | 5648 | 0 |
| columns | 698 | 169 | 867 | 0 |
| dashboard | 619 | 413 | 1032 | 0 |
| item-sets | 1453 | 4305 | 5758 | 0 |
| items | 2436 | 4515 | 6951 | 0 |
| jobs | 1180 | 4305 | 5485 | 0 |
| media | 1715 | 5012 | 6727 | 0 |
| modules | 423 | 1258 | 1681 | 0 |
| properties | 838 | 5767 | 6605 | 0 |
| query | 798 | 1258 | 2056 | 0 |
| resource-classes | 839 | 5767 | 6606 | 0 |
| resource-templates | 3196 | 4305 | 7501 | 0 |
| settings | 676 | 0 | 676 | 0 |
| sites | 6909 | 4126 | 11035 | 0 |
| system-info | 463 | 582 | 1045 | 0 |
| users | 1332 | 4126 | 5458 | 0 |
| vocabularies | 1931 | 4126 | 6057 | 0 |

Sizes are gzip bytes. "Shared" is the chunks a route imports that are not the initial payload, so a
route's total is the first-visit cost when the shared chunks are still cold. `sites` is the largest
route at 11035 B, and its own chunk at 6909 B is the largest single chunk.

**Verdict.** Accepted. A first paint now downloads 85354 B of js instead of 103351 B, and the route a
user opens fetches only its own modules.

**Budget change.** The old budget summed every emitted js asset and compared that to one number. That
sum counted bytes no first paint downloads, so the split made it meaningless: it rose from 103351 B to
118856 B while the payload a user waits on fell. `perf/bundle.mjs` now splits `dist` the same way
`index.html` does, and the gate names an initial payload and a per-chunk ceiling.

| Budget key | Before | After | What it gates |
| --- | --- | --- | --- |
| `bundle.jsGzipBytes` | 109568 | removed | The sum of every emitted js asset, which no user pays at once. |
| `bundle.initialJsGzipBytes` | n/a | 102400 | The js `index.html` references, which the first paint waits on. |
| `bundle.initialCssGzipBytes` | 6144 | 6144 | The css `index.html` references. Unchanged. |
| `bundle.chunkGzipBytes` | n/a | 10240 | Every other emitted js or css asset, each on its own. |

`102400` is the original budget, so the raise Decision 4 made is gone. The per-chunk ceiling is
10240 B (10 KiB), 48% above the largest chunk measured at 6909 B, so a page that grows by half trips
it. A ceiling per chunk, rather than a map keyed by file name, because a chunk's name carries a
content hash.

**First contentful paint.** Decision 3 found no measurable paint win from splitting, because the
browser reports paint in 4 ms steps and the stubbed build's 52 ms sat inside the full build's 52 to
56 ms range. The split now removes 17997 B gzip from the parse. That does not overturn Decision 3:
the FCP samples the budget spec records are not an interleaved A/B against the eager build, so they
cannot separate a 17 KB parse change from machine drift.

## Decision 6: the auth surface costs 3049 B gzip of initial payload (accepted)

The sign-in panel now carries two forms and the shell asks `GET /api/me` once on load, so the entry
chunk grew from 85354 B to 88411 B gzip, 3.6%, and stays 13.7% under the 102400 B budget. The
set-password and forgot pages are their own lazy chunks (620 B and 708 B gzip), so a reader of an
admin page downloads neither.

The session probe is one request per load and the shell does not wait on it: the header shows the
key's state until the user arrives. That keeps the paint path free of it, which is what Decision 3
measured. Measured with `npm run perf:bundle` at this commit.

## Decision 7: the page editor's chunks, and the preview panel on demand (accepted)

The page editor is a lazy route under `src/pages/sites/editor/`. Opening a page fetches the shell and
the shared chunks its static imports name; the canvas, the field controls, the template chooser, the
save-as-template dialog and the preview panel each arrive on demand. Measured with `npm run
perf:bundle` after `npm run build`, gzip bytes, at commit `957fbb2` plus this change:

| Chunk | Module | Raw | Gzip |
| --- | --- | --- | --- |
| shell | `editor/shell/index.ts` | 13290 | 4268 |
| canvas | `editor/canvas/index.ts` | 11715 | 4018 |
| canvas css | `editor/canvas/canvas.css` | 3674 | 992 |
| fields | `editor/fields/index.ts` | 20352 | 5388 |
| fields, shared | the field controls' shared module | 9810 | 2867 |
| registry, shared | the block registry and schema the shell reads | 25081 | 6538 |
| preview panel | `editor/shell/PreviewPanel.tsx` | 1353 | 595 |
| chooser | `editor/shell/TemplateChooser.tsx` | 1455 | 570 |
| dialogs | `editor/shell/SaveTemplateDialog.tsx` | 1330 | 659 |

**Change.** `PageEditor` loads the preview panel with `React.lazy` instead of a static import, behind
the `Suspense` boundary that already wraps the canvas. The panel renders only after a preview mode is
chosen, so a page that is being edited without a preview no longer fetches it. Before, the shell's
static import pulled a shared chunk holding both `editor/preview.ts` and the panel, 6996 B gzip.
After, the panel is its own 595 B chunk and the rest of that shared chunk, 6538 B gzip, stays eager,
because the shell reads the block registry through it.

**Measured.** The shell chunk is 4268 B gzip against the 10240 B per-chunk ceiling, 5972 B of slack,
and the largest lazy chunk is the sites route page at 7643 B, 2597 B of slack. The initial payload is
97732 B js gzip and 3504 B css gzip, the raise over Decision 6's 88411 B being the pages added since.
`npm run perf:check` passes on all three.

## Decision 8: the live preview loads on every editor open (accepted)

The preview is now the editor's main surface, so its chunk loads on every open, beside the Outline,
instead of on a mode switch. It stays lazy: a static import would add about 3 KB to the shell for no
gain in latency, because both lazy chunks start on the first render. Measured with `npm run
perf:bundle` after `vite build`, gzip bytes, against commit `b18c583` built the same way from a
`git archive` copy with the same `node_modules`:

| Chunk | Before | After | Note |
| --- | --- | --- | --- |
| initial js | 99320 | 99512 | the shell's route-level width flag |
| initial css | 3684 | 3698 | the empty live region rule |
| shell (`editor/shell/index.ts`) | 4705 | 5113 | `usePreview`, `selection`; the mode toggle left |
| canvas (`editor/canvas/index.ts`) | 4894 | 4385 | the picker and its template choices left |
| canvas css | 1253 | 943 | the picker styles left |
| preview panel | 851 (`PreviewPanel`) | 3213 (`LivePanel`) | `frame/`: markers, guard, geometry, anchor, two frames and the overlay |
| preview css | none | 650 | `frame/frame.css` |
| picker, shared | none | 1003 js, 517 css | `BlockPicker`, which the Outline and the preview both open |
| device bar, shared | none | 621 | `DeviceBar`, which the editor and the Theme page both show |
| registry, shared (`preview-*`) | 6631 | 6167 | the template registry moved to the picker chunk |

The largest lazy chunk is the shared registry at 6167 B against the 10240 B ceiling. The editor's
first open now fetches about 3.9 KB more gzip than an editor opened without a preview did before,
and nothing on the paint path changes beyond the 192 B of the initial js.

## Decision 9: the site settings help text grows the eager catalogues (accepted)

**Change.** The site settings form gives every setting a plain label, one help sentence, readable
choice names and a group intro, in English and Spanish (`siteSetting*` keys in
`src/i18n/catalogue.ts` and `src/i18n/locales/es.ts`). Both catalogues are eager, so every string
is on the first paint.

**Measured.** `vite build` then `npm run perf:bundle`, gzip bytes, against commit `f3cec5e` built the
same way from a `git archive` copy with the same `node_modules`:

| Asset | Before | After | Note |
| --- | --- | --- | --- |
| initial js | 100672 | 103180 | over the 102400 budget by 780 |
| shared chunk with both catalogues (`jsx-runtime-*`) | 23870 | 26363 | the new strings, 10410 B raw across the two files |
| entry (`index-*`) | 73397 | 73412 | |
| sites route page | 8353 | 9752 | the grouped form, the property and asset pickers |
| initial css | 4445 | 4596 | the group and help styles |

**Verdict.** Accepted, and `bundle.initialJsGzipBytes` moves 102400 to 103424, one KiB, which leaves
244 B. The strings are the feature, and the brief puts every user-facing string in the catalogue.
The limiter is that the Spanish catalogue ships to every English visitor and the reverse. Loading
only the active locale's catalogue would remove roughly half of the shared chunk's strings from the
first paint. That change is in `src/i18n/`, outside this unit, and is the next step before another
wave adds strings.

## Decision 10: the Theme page loads as three chunks and keeps its messages (accepted)

The Theme page redesign (presets, brand derivation, fine-tune, custom CSS) adds a palette module that
repeats the theme's OKLab arithmetic, a generated registry of five presets, and about 150 messages in
two languages. Measured with `npm run perf:bundle` after `npm run build`, gzip bytes:

| Asset | Before | After | Note |
| --- | --- | --- | --- |
| initial js | 100661 | 99806 | the old theme messages left the main catalogues |
| `theme` (the page) | 6318 | 6341 | the page and its controls |
| `theme-model` | none | 5504 | `themeEditor/{colour,palette,registry,draft}.ts` and `generated/themeSettings.json` |
| `theme-messages` | none | 4340 | `themeEditor/messages.ts`, English and Spanish |
| `CustomCssTab` | none | 2030 | the CSS editor and the theme CSS viewer, on the tab's first open |
| theme css | none | 1680 | `themeEditor/theme.css`, out of the initial `styles.css` |

**Messages.** In the main catalogues the new messages took the initial payload to 104014 B, 1614 B over
the 102400 B budget, because `src/i18n/catalogues.ts` loads both languages on the first paint. They
live in `themeEditor/messages.ts` instead, typed the way the main catalogue is (`es` against `en`), and
load with the page.

**Chunks.** One route chunk measured 15542 B, over the 10240 B ceiling. `vite.config.ts` names two
groups (`build.rolldownOptions.output.codeSplitting`), the messages and the model, with
`includeDependenciesRecursively: false` so React stays in the entry. The three chunks load in parallel
when the route opens. A first Theme visit now fetches about 17.9 KB gzip where it fetched 6.3 KB; the
first paint and every other route are unchanged.

## Decision 11: only the active locale's catalogue loads (accepted, supersedes Decision 9's budget)

**Premise.** Decision 9 named the limiter: the Spanish catalogue shipped to every English visitor and
the reverse, in one shared chunk on the first paint.

**Change.** `src/i18n/catalogues.ts` is now the locale registry: each locale maps to a loader. English
resolves from the bundle, because it is the default, the fallback and the type every catalogue is
checked against. Every other locale is a dynamic import, so Vite emits it as its own chunk. The store
switches only after the chosen catalogue arrives, and `main.tsx` waits for the stored locale's
catalogue before the first render, so a reload in Spanish paints Spanish first. Adding a locale is one
file under `src/i18n/locales/` and one registry entry; no locale other than English enters the
initial bundle.

**Measured.** `vite build` then `npm run perf:bundle`, gzip bytes, against commit `17ccde4` built the
same way from a `git archive` copy with the same `node_modules`:

| Asset | Before | After | Note |
| --- | --- | --- | --- |
| initial js | 103180 | 91807 | 11373 B less for every visitor |
| shared chunk (`jsx-runtime-*`) | 26363 | 15511 | the Spanish strings left it |
| entry (`index-*`) | 73412 | 72891 | |
| Spanish catalogue (`es-*`) | none | 12003 | fetched only in Spanish, before the first render |
| largest route chunk | 9752 | 9748 | the sites route page |

A visitor in Spanish now downloads 91807 + 12003 = 103810 B before the first paint, 630 B more than
before, in one more request that starts after the entry runs. The timings do not resolve that
request: a cold load timed in the browser, 15 runs per side interleaved over `vite preview` on
localhost, gave a Spanish first paint of 64 ms median before and after, and every after median sat
inside the before range. The box's load average was 287 during the run, so those timings bound the
cost on localhost and say nothing about a slow network, where the extra request costs one round trip.

**Budget.** `bundle.initialJsGzipBytes` moves 103424 to 93184, which leaves 1377 B. A catalogue chunk
is not a route: a visitor in that locale waits on it before the first paint, and it is about as large
as the English catalogue inside the initial payload. So it gets its own ceiling,
`bundle.catalogueChunkGzipBytes`, 13312 B over the 12003 B Spanish chunk. `perf/bundle.mjs` tells a
catalogue from a route chunk by its source in the Vite manifest (`src/i18n/locales/`). A new locale
whose script needs more bytes per message, such as Devanagari in UTF-8, is measured against that
ceiling when it lands.

## Decision 12: the site's Items page costs 1598 B of initial payload (accepted)

The site's Items page (`src/pages/sites/resources.tsx`, `siteItemsAdd.tsx`), the item picker and the
Items list's bulk site actions add about 60 messages to each of the two catalogues, and both
catalogues sit in the initial payload. Measured with `zlib.gzipSync` over the files `dist/index.html`
references, the gzip `perf/bundle.mjs` uses: main at `f3cec5e` built with `vite build` is 100672 B,
this change is 102270 B, a cost of 1598 B, and 130 B under the 102400 B budget. A first draft cost
about 2200 B and went over; merging messages into existing keys (`field.saved`) and shortening the
help texts took it under. The page's own chunk is 4091 B js and 1125 B css gzip, against the
10240 B per-chunk ceiling.

**Verdict.** Accepted, with almost no headroom left. The next message-heavy feature should load the
non-default locale's catalogue on demand instead of in the entry, which frees roughly half the
catalogue bytes from the first paint.

Decision 11 landed first on main, so only the English half of these messages reaches the first
paint; the Spanish half moves to the lazy `es` chunk.

## Decision 13: seven Indic catalogues raise the catalogue ceiling (accepted)

**Premise.** Decision 11 said a locale whose script needs more bytes per message would be measured
against `bundle.catalogueChunkGzipBytes` when it lands. Hindi, Marathi, Malayalam, Tamil, Kannada,
Telugu and Nepali land here. Their letters take 3 bytes each in UTF-8, where Spanish takes 1 or 2.

**Change.** Seven catalogues under `src/i18n/locales/`, one registry entry each. The Theme page's own
messages (Decision 10) now load the same way: English stays in the page chunk, and every other locale
is a chunk under `src/pages/sites/themeEditor/locales/` that the page waits for with its site. The
Standards page's two messages are imported with its page, every locale, as before.

**Measured.** `npm run build` then `npm run perf:check` on this branch merged with main `2b6ff5e`,
gzip bytes from `zlib.gzipSync` as `perf/bundle.mjs` measures them:

| Asset | Bytes | Note |
| --- | --- | --- |
| initial js | 93157 | 27 B under 93184; English is the only eager catalogue |
| Spanish catalogue | 12642 | for comparison |
| Hindi catalogue | 14598 | |
| Nepali catalogue | 14651 | |
| Marathi catalogue | 14775 | |
| Kannada catalogue | 14975 | |
| Telugu catalogue | 15056 | |
| Tamil catalogue | 15179 | |
| Malayalam catalogue | 15216 | the largest |
| largest route chunk | 9830 | the Theme tables count as route chunks |

These are single builds on one machine, and gzip output moves a few bytes with the input order. Each
new English key adds about 30 B to each Indic chunk.

**Budget.** `bundle.catalogueChunkGzipBytes` moves from 13312 to 16384, 1168 B over Malayalam. That is
about the margin Decision 11 left over Spanish. The route ceiling and the initial ceiling do not move.
The initial payload has 27 B left. These locales put the seven endonyms in the English catalogue and
seven entries in the registry, both eager. The next change that grows the eager path needs its own
decision.

## Decision 14: the admin locale and translated server errors raise the initial ceiling (accepted)

**Premise.** Decision 13 left 27 B of initial payload. Two changes need the eager path. The admin
locale follows the user and global `locale` settings (`src/api/session.ts`, the header switch, the
client calls). Server error text is translated at the fetch boundary (`src/api/client.ts`). Each runs
before any route loads, on sign-in or on the first failed request, so it cannot wait on a route chunk.

**Measured.** `npm run build` then `npm run perf:check`, gzip bytes from `zlib.gzipSync`:

| Build | initial js | Note |
| --- | --- | --- |
| main `6cec165` | 93157 | Decision 13 |
| + admin locale (`admin/user-locale`) | 93418 | 261 B: the session step, the switch write, three client calls |
| + translated server errors (`admin/i18n-errors`) | 93728 | 310 B: the boundary call and the locale's loader table |

These are single builds on one machine. The pattern table and each locale's server catalogue are lazy
chunks, fetched with the first error in a non-English locale. 480 B of the ceiling remain.

**Budget.** `bundle.initialJsGzipBytes` moves from 93184 to 94208, one KiB. This is a stopgap. The
English catalogue holds every page's strings and is eager. Splitting it into per-route namespaces,
with English lazy per route, would free several KB of the first paint. That split is queued as its
own unit, and the ceiling should come back down when it lands.

## Decision 15: the block field messages leave the main catalogue (accepted)

Every block field, Romeka field and styling field now has a plain label, one help line, readable
choice names and, for visible text, an example placeholder, and the styling form has named
sections. Those strings, and the old block field labels they replace (196 keys in all, plus the
controls' own words), live in `src/pages/sites/editor/messages.ts` with one table per locale under
`editor/locales/`, loaded through `EDITOR_LOADERS` the way the Theme page loads its messages
(Decision 10). `SitePageEditor` waits for the active table before it renders, and
`parseBlockRegistry` checks a field's label, help, example and choice names against that table.
`vite.config.ts` names the English table's chunk `editor-messages`.

Measured with `vite build` then `npm run perf:check` and `perf/bundle.mjs`, gzip bytes, against the
base commit `79af36f` built the same way from a `git archive` copy with the same `node_modules`:

| Asset | Before | After | Note |
| --- | --- | --- | --- |
| initial js | 93419 | 92282 | the block field keys left the English catalogue |
| English catalogue chunk (`jsx-runtime-*`) | 16332 | 15194 | |
| largest catalogue chunk (Malayalam) | 15214 | 13753 | the same keys left every locale |
| `editor-messages` (English table) | none | 3487 | loads with the editor and the site's Items page |
| `editor/fields` chunk | 4134 | 5966 | the colour, padding and link controls, the grouped form |
| fields css | none | 860 | `editor/fields/controls.css` |
| registry, shared (`preview-*`) | 6169 | 7007 | `blocks.json` carries help keys, conditions and groups |
| sites route page | 9860 | 9915 | the largest lazy chunk, under the 10240 B ceiling |

**Verdict.** Accepted. The feature takes 1137 B off the first paint instead of adding to it. The
editor's first open now fetches the English table (3487 B), and a visitor in another locale
also fetches that locale's table.

## Decision 16: the lessons load as one shared chunk (accepted)

The lessons (`src/app/lessons/`) anchor on six routes: Standards, Fields, Item types, the item form
and the user form. The registry, the seen-state, the component and the English table are one named
chunk, `lessons`, in `vite.config.ts`; each other locale's table is its own chunk under
`app/lessons/locales/`. A first build without the group folded them into the shared `plain-terms`
chunk, which then measured 10498 B, over the 10240 B ceiling.

Measured with `vite build` then `perf/bundle.mjs`, gzip bytes, against main `28bd6ba` built the same
way from a `git archive` copy with the same `node_modules`:

| Asset | Before | After | Note |
| --- | --- | --- | --- |
| initial js | 92488 | 92701 | the lesson settings in `parseUser` and `encodeUserSettings` |
| initial css | 4724 | 4724 | |
| `lessons` | none | 3209 | registry, seen-state, component, English table |
| lessons css | none | 469 | named `plain-terms-*.css` by the chunk that imports it first |
| `plain-terms` | 7486 | 7487 | |
| Spanish lessons table | none | 1226 | fetched only in Spanish |

**Verdict.** Accepted. The initial payload grows 213 B, because the seen-state arrives with the
session (`GET /api/me`), so the session parser reads it. That removes a request: the alternative was
one more user read on the first page that shows a lesson. These are single builds on one machine.

## Decision 17: the vocabulary import loads on demand (accepted)

The Vocabularies tab gains "Import a vocabulary" and, on a vocabulary's page, "Update from a file or
web address". Measured with `vite build` then `npm run perf:pages` and `zlib.gzipSync` over the
manifest's chunks, gzip bytes, against the base commit `28bd6ba` built the same way:

| Asset | Before | After | Note |
| --- | --- | --- | --- |
| initial js | 92488 | 92873 | the three client methods, their encoders and the diff parser in `src/api/` |
| `standards` route, own chunk | 5846 | 6366 | the link, the route and the two lazy imports |
| `vocabularies/import` | none | 1014 | the import form, on the import route only |
| `vocabularies/update` | none | 1407 | the update panel and the diff view, with a vocabulary page |
| `vocabularies/locales/es` | none | 1439 | one chunk per non-English locale |

**Verdict.** Accepted. A first build with the two screens imported eagerly put the `standards` chunk
at 9785 B, 455 B under the 10240 B ceiling; loading both with `React.lazy`, and moving the one link
string to the Standards table, leaves 3874 B. The initial payload keeps 1335 B under its 94208 B ceiling. The English table and the shared fields sit in a lazy
chunk both screens import.

## Decision 18: tags cost the forms 2.4 KB and the Term lists page 2.5 KB (accepted)

Tags add a Tags field to the item, item set and media forms and a Tags panel with merge prompts to
the Term lists page. The field's four strings live in `src/app/describe-messages.ts`, which imports
every locale with the forms; the panel's strings live in the Term lists table, one chunk per locale.
Measured with `vite build` and `zlib.gzipSync` (the gzip `perf/bundle.mjs` uses), against the base
`e5c1b75` built the same way from a `git archive` copy with the same `node_modules`:

| Asset | Before | After | Note |
| --- | --- | --- | --- |
| initial js | 92599 | 92641 | `draft.ts` splits the tags out of the values |
| describe strings (`plain-terms-*`) | 7539 | 8599 | four strings in nine locales |
| `tags-field` | none | 1028 | the field, with the forms |
| `tags-field` css | none | 319 | |
| Term lists page (`page-*`) | 2334 | 4340 | the panel, the merge prompt, the similarity rule |
| Term lists English table (`messages-*`) | 1138 | 1651 | |
| largest chunk | 9887 | 9893 | not a chunk this change touches |

**Verdict.** Accepted. The first paint grows 42 B. A form's first open fetches about 2.4 KB more,
and the Term lists page about 2.5 KB more with its table, all under the 10240 B chunk ceiling.

## Decision 19: the built-in vocabularies' text loads with the page that shows it (accepted)

**Hypothesis.** The labels and comments of Dublin Core, Dublin Core Type, BIBO and FOAF can be shown
in the admin's locale without growing the first paint or the locale switch, if each locale's tables
are lazy chunks a page fetches the first time it shows that text.

**Change.** `src/i18n/termText.ts` keys the text by term. Each locale has a labels table and a
comments table under `src/i18n/locales/vocabulary/`, so each is a catalogue chunk. `useTermText()`
loads the active locale's two tables and the two English tables (the guard compares a stored label
with the built-in English), and renders its caller again when they arrive. English loads none,
because the stored text is the English. A first version loaded the tables with the catalogue; it
broke `e2e/specs/locale.spec.ts` ("only the chosen language downloads"), because a switch then
fetched five scripts instead of one.

| Measure (gzip) | Before (28bd6ba) | After | Note |
| --- | --- | --- | --- |
| initial js | 92488 | 93112 | the loader table, the guard and the hook |
| largest lazy chunk (`page-*`) | 9893 | 9942 | |
| largest catalogue chunk | 13415 | 13415 | the vocabulary chunks are smaller |
| labels chunk, English / Spanish / largest Indic (Malayalam) | none | 2893 / 3495 / 4541 | |
| comments chunk, English / Spanish / largest Indic (Malayalam) | none | 7390 / 9263 / 10180 | |

**Verdict.** Accepted. The first paint pays 624 B and a locale switch pays nothing. The first page
in a non-English session that shows vocabulary text fetches four chunks, 25004 B gzip in Malayalam,
and shows the stored English text until they arrive. Splitting the comments out of that first fetch
would take 17570 B off it.

**Re-measured after merging main `19edebf`** (lessons, term lists, tags, the Indic site locales): initial
js 94051 B of 94208, largest lazy chunk 10044 B of 10240. These are single builds. The 157 B left is the
reason the catalogue split is next (Decision 14's stopgap).

## Decision 20: the English catalogue splits into route namespaces (accepted, ends Decision 14's stopgap)

**Change.** `src/i18n/catalogue.ts` keeps the 55 keys the shell renders before any route loads. Every
other string moved, unchanged and with its `sources.json` record, into a message namespace under
`src/i18n/namespaces/<id>/` (`scripts/split-catalogue.mjs`): `common`, `resources`, `sites`,
`editor`, `standards`, `admin` and `account`, plus the page tables that already lived beside their
pages (Theme, item types, Term lists, lessons, Publish). A namespace's English is a named chunk its
routes import; each other locale is its own chunk. A component names its namespaces,
`useTranslator(sites, common)`, and suspends until the active locale's tables arrive, so a page in
Spanish shows its fallback and never English. The vocabulary labels and comments are two more
namespaces; the comments wait for their first read. 20 keys nothing rendered were deleted.

**Measured.** `vite build` then `perf/bundle.mjs` (`zlib.gzipSync`), gzip bytes, against main
`29a77af` built the same way from a `git archive` copy with the same `node_modules`:

| Asset | Before (`29a77af`) | After | Note |
| --- | --- | --- | --- |
| initial js | 94056 | 84680 | 9376 B off the first paint |
| shared React and catalogue chunk (`jsx-runtime-*`) | 15929 | 6232 | the English catalogue left it |
| entry chunk (`index-*`) | 74231 | 74552 | the namespace store and the route boundary |
| Spanish shell catalogue | 11271 | 988 | |
| largest shell catalogue (Tamil) | 13503 | 1156 | |
| largest catalogue chunk | 13503 (Tamil catalogue) | 10180 (Malayalam vocabulary comments) | every locale table now counts as one |
| largest namespace English (`sites-messages`) | none | 5570 | the sites and site settings strings |
| largest namespace locale (Malayalam `sites`) | none | 7149 | |
| largest lazy chunk (`page-*`) | 10048 | 10092 | the sites route, 148 B under the ceiling |

These are single builds on one machine. A route's first open in a non-English locale now fetches
its namespaces' tables beside its own chunk; English fetches no table, because English is static.

**Budget.** `bundle.initialJsGzipBytes` moves from 94208 to 87040, the measured 84680 plus about
2.5 KB. `perf/bundle.mjs` now counts every locale table, the shell's and each namespace's, as a
catalogue chunk, and `bundle.catalogueChunkGzipBytes` moves from 16384 to 11264, the largest one
plus about 1 KB.

## Decision 21: the site Settings and Users tabs load on demand (accepted)

**Premise.** After the catalogue split (Decision 20) the sites route chunk was the largest lazy chunk,
10097 B of the 10240 B ceiling, because `src/pages/sites/page.tsx` imported the Settings and Users tabs
statically, while Navigation, Site resources, Theme and the editor were already lazy.

**Change.** Settings and Users become `lazy()` imports behind the same `loading.site` Suspense fallback
as the other tabs.

**Measured.** `npm run build`, gzip bytes from `zlib.gzipSync` over the Vite manifest's files:

| Asset | Before | After |
| --- | --- | --- |
| sites route (`pages/sites/page.tsx`) | 10097 | 5588 |
| Settings tab (`pages/sites/settings.tsx`) | in the route | 4320 |
| Users tab (`pages/sites/users.tsx`) | in the route | 1255 |
| largest lazy chunk | 10097 | 6873 |
| initial js | 84764 | 84764 |

A single build on one machine. Opening Settings or Users now costs one more request after the route.

## Decision 22: page history loads on demand, its API calls ride the initial payload (accepted)

**Change.** The page editor gains a History panel, a version view and a refused-save dialog. The
panel and the version view are one lazy chunk (`editor/shell/HistoryPanel.tsx`), fetched on the
first press of History; the dialog is another (`editor/shell/ConflictDialog.tsx`), fetched only when
a save is refused. The client methods and their parsers (`listPageVersions`, `getPageVersion`,
`parsePageVersionSummary`, `parseStalePage`) live in `src/api/`, where every boundary parse lives, so
they ride the initial payload.

**Measured.** `npm run build`, then gzip bytes from `zlib.gzipSync` per emitted asset, against main
`d0775a7` built the same way from a `git archive` copy with the same `node_modules`:

| Asset | Before (`d0775a7`) | After | Note |
| --- | --- | --- | --- |
| initial js | 85708 | 85976 | the save result union, the two reads and their parsers |
| editor shell (`shell-*`) | 7274 | 7924 | the History toggle, the lazy imports, `usePageSave` |
| history panel and version view | none | 1189 js, 446 css | |
| refused-save dialog | none | 617 | |
| version time format | none | 265 | shared by the two chunks above |
| shared editor chunks (`draft`, `blockSchema`, `groups`, `romeka`, `preview`) | 7616 in five | 6672 in one | the history chunk imports `preview` and `wire`, so Rollup merged them |
| English editor messages | 5129 | 5483 | 22 `history.*` keys |

A single build on one machine. The largest lazy chunk is the editor shell at 7924 B of the 10240 B
ceiling, and the initial payload has 1064 B left under 87040.

## Decision 23: the site region inspector loads on demand (accepted)

**Change.** The page editor selects the site header and the site footer. The site-wide inspector
(`editor/shell/SiteRegionInspector.tsx`) is a lazy chunk, fetched on the first region selection. It
reuses the Theme page's Header and Footer sections and its token rows, so Rollup moves the code the
two screens share (`FooterSection`, `HeaderSection`, `TokenField`, `siteTheme`) out of the Theme
page's chunk into one shared chunk. The selection union, the region buttons in the page settings,
the toolbar's unsaved note and the lazy import ride the editor shell.

**Measured.** `npm run build`, then gzip bytes from `zlib.gzipSync` per emitted asset, against main
`b817f94` built the same way from a `git archive` copy with the same `node_modules`:

| Asset | Before (`b817f94`) | After | Note |
| --- | --- | --- | --- |
| initial js (`perf:check`) | 86073 | 86082 | |
| editor shell (`shell-*`) | 8399 | 9013 | the selection union, the region buttons, the unsaved note |
| live panel (`LivePanel-*`) | 3290 js, 672 css | 3579 js, 711 css | region outlines and their style |
| site region inspector | none | 1140 | |
| Theme page (`theme-*`) | 8333 | 5558, plus 3584 shared | the shared sections moved out |
| English editor messages | 6970 | 7148 | 12 `region.*` keys |
| largest editor locale (`ml`) | 8680 | 8911 | under the 11264 catalogue ceiling |

A single build on one machine. The largest lazy chunk is still `fields-*` (9018 B by `perf:check`);
the editor shell is next at 9013 B of the 10240 B ceiling.

## Decision 24: the media reference and focal point controls join the editor fields chunk (accepted)

**Change.** A block setting may name a media of the site. Its control (`editor/fields/MediaRefField.tsx`)
finds the item with the item picker and chooses one of its images; the hero's focal point control
(`editor/fields/FocalPointField.tsx`) sets a point on that image. Both are in the `fields` chunk with
the other field controls; the item picker stays its own chunk. 40 `editor` keys cover the new hero,
IIIF image and control words.

**Measured.** `npm run build`, then `npm run perf:check` and gzip bytes from `zlib.gzipSync` per
emitted asset, against main `509560d` built the same way from a `git archive` copy with the same
`node_modules`:

| Asset | Before (`509560d`) | After | Note |
| --- | --- | --- | --- |
| initial js | 86043 | 86049 | the two new field kinds in the registry parser |
| `fields` chunk | 8046 | 9140 | the two controls; now the largest lazy chunk |
| `fields` css | 1266 | 1437 | the focal frame and the disabled media choice |
| English editor messages | 6451 | 7000 | |

A single build on one machine. The `fields` chunk has 1100 B left under the 10240 B ceiling, so the
next control that grows it should move a rarely used control (the page list, the item query) into a
chunk of its own first.

## Decision 25: the page list and property list controls load on demand (accepted)

**Change.** The merge of `site/block-upgrades` with main put the property list control
(`editor/fields/PropertyListField.tsx`, the item-with-metadata block's fields) and the browse query's
property sort into the `fields` chunk beside Decision 24's media controls, and the chunk went over the
10240 B ceiling. As Decision 24 asked, a rarely used control moved out first: the page list control
(`LazyPageListField.tsx`, the list-of-pages block only) and the property list control
(`propertyListControl.tsx`, item with metadata only) each load as their own chunk the first time their
block opens, behind the `common` table's `loading.form` line. The item query control did not move: it
already shares a chunk with the navigation link form and the site's Items page, so a lazy import of it
saved nothing (measured: the `fields` chunk grew 169 B).

**Measured.** `npm run build`, then `npm run perf:check`, on the merge tree:

| Asset | Before | After | Note |
| --- | --- | --- | --- |
| `fields` chunk | 10771 | 9925 | 315 B left under the ceiling |
| page list | none | 1477 | `PageListField-*` |
| property list | none | 802 | `PropertyListField-*` |
| initial js | 86072 | 86072 | |

A single build on one machine. The editor shell (9047 B) is now the next largest lazy chunk.

## Decision 26: the content blocks' strings and controls load on their own (accepted)

**Change.** Seven content blocks (gallery, featured item, button, quote, accordion, timeline, search
box) bring two field kinds and 88 field strings. Put in the page editor's table, the strings took the
Tamil, Malayalam, Hindi and Telugu `editor` chunks over the 11264 B catalogue ceiling, and the two
controls took the `fields` chunk over 10240 B. Three moves:

- The strings are their own namespace, `content-blocks`. The page editor reads both tables
  (`editor/messages.ts`, `EDITOR_NAMESPACES`), so a curator's first editor visit fetches one more
  locale chunk, in parallel with the editor's.
- The item and item-media controls (`editor/fields/contentControls.tsx`) load as one chunk behind
  `LazyControls.tsx` and the `common` table's `loading.form` line.
- The IIIF manifest source control, which only the IIIF presentation block uses, moved behind the
  same file. On this branch the chunk had 31 B left before the content blocks, so a rarely used
  control had to leave first, as Decision 24 said.

**Measured.** `npm run build`, then `npm run perf:check`, and gzip bytes from `zlib.gzipSync` per
emitted asset, against this branch's base (`6a788a1`) built the same way from a `git archive` copy
with the same `node_modules`:

| Asset | Before (`6a788a1`) | After | Note |
| --- | --- | --- | --- |
| initial js | 86074 | 86065 | the registry parser's two new kinds, less what the moves took out (inferred) |
| `fields` chunk | 10208 | 10173 | 67 B left under the ceiling |
| content controls | none | 618 | `contentControls-*` |
| manifest source | none | 1058 | `ManifestSourceField-*` |
| largest catalogue chunk | 10176 | 10176 | the Tamil `editor` table, unchanged |
| Tamil `content-blocks` table | none | 2372 | the largest of the new table's chunks |

A single build on one machine. With the strings in the `editor` table and the controls eager, the
same build measured the Tamil `editor` chunk at 11506 B and the `fields` chunk at 10418 B. Main
after Decision 25 measured its `fields` chunk at 9925 B and its largest catalogue at 10585 B (a
`git archive` build of main `8c48349`); the merge adds this decision's moves to that.

**Measured after the merge with main** (`8c48349`, Decision 25), same steps on the merge tree. The
three lazy controls now share one loading line, `editor/fields/ControlLoading.tsx`.

| Asset | Main (`8c48349`) | Merge | Note |
| --- | --- | --- | --- |
| initial js | 86072 | 86064 | |
| `fields` chunk | 9925 | 9792 | 448 B left under the ceiling; no further control moved |
| content controls | none | 618 | `contentControls-*` |
| manifest source | none | 1058 | `ManifestSourceField-*` |
| page list | 1477 | 1477 | `PageListField-*` |
| property list | 802 | 805 | `PropertyListField-*` |
| largest catalogue chunk | 10585 | 10640 | the Malayalam `ml-*` table, 624 B under its ceiling |

## Decision 27: the notes panel's table loads as its own chunk (accepted)

**Change.** The media notes panel gains the region editor (A-4), whose strings
live in the `annotations` namespace, and that table sat inside the panel chunk
for both to share. With the new keys the panel chunk measured 10143 B of the
10240 B per-chunk ceiling, 97 B of slack, before the region editor's own chunk
existed. `vite.config.ts` names `i18n/namespaces/annotations/index.ts`
`annotations-messages`, the way the other namespaces already load, so the panel,
the region editor and the coming time editor read one shared table.

**Measured.** `npm run build` then `npm run perf:check`, gzip bytes, single
builds on one machine:

| Asset | Before | After |
| --- | --- | --- |
| panel chunk | 10143 | 8394 |
| `annotations-messages` | none | 1958 |
| region editor | none | 4124 js, 850 css |
| largest chunk | 10143 (panel) | 9909 (`import-routes`) |
| initial js | 86170 | 86178 |

**Verdict.** Accepted. A media page fetches the table beside the panel instead
of inside it, so the notes panel downloads about 1.7 KB less, and a later editor
chunk does not carry the table a second time. The 10 B on the initial payload is
the panel's lazy import of the region editor.

## Decision 28: the asset picker, its upload control and the focal point load on demand (accepted)

**Change.** The asset field's picker and the shared upload/description control
move out of the editor's `fields` chunk, and the hero's focal point control
moves out after them. `fields` had 57 B of headroom, and the lazy wrapper alone
took it 41 B over the 10240 B ceiling. The focal point control, which only the
hero uses, left first, as Decision 24 asks. The asset picker and the upload
control are shared by the assets table and the editor, so each is its own
chunk.

**Measured.** `npm run build` then `npm run perf:check` and gzip bytes from
`zlib.gzipSync` over the manifest's files, against main `b5f40d23` built the
same way:

| Asset | Before | After | Note |
| --- | --- | --- | --- |
| initial js | 86211 | 86243 | the two new lazy boundaries in `FieldControl` |
| initial css | 5172 | 5360 | the upload, drop and kind-tile rules |
| `fields` chunk | 10183 | 9669 | 571 B under the ceiling |
| asset picker | in a shared chunk | 920 | `AssetPicker-*` |
| upload control | none | 1259 | `upload-*`, shared with the assets table |
| focal point | in `fields` | 1256 | `focalPointControl-*`, hero only |
| largest chunk | 10183 (`fields`) | 9905 (`import-routes`) | 335 B under the ceiling |

**Verdict.** Accepted. No budget moved. The editor's first asset block and its
first hero each fetch one more chunk; the assets table fetches the upload
control with its route.

## Decision 29: the initial ceiling moves to 88064 for the media wire members (accepted)

**Measured.** After D3 (av-access) merged with main on 2026-10-21, `npm run perf:check` read
`initial js 87041 B`, 1 B over the 87040 ceiling Decision 20 set when the payload was 84680 B. The
2.4 KB between them went to the media run: the server media kind, the preview state and now the
access-copy state on every media document (`src/api/parse.ts`, eager because every list parses
media), and the shared upload client.

**Tried first.** Moving `playerUrl` into the lazy media stage measured 87037 B once and 87041 B
again with the helper copied exactly: gzip noise, not a saving. Sharing one parser for the preview
and access members measured 87098 B: the shared helper cost more than the duplicate it removed,
because gzip already compresses the repeat.

**Budget.** `bundle.initialJsGzipBytes` moves from 87040 to 88064, which leaves 1023 B.

**To win it back.** The media parse could load with the first route that shows media instead of the
shell, if the list pages that do not show media stop parsing it eagerly. That is a structural
change for its own unit, measured before and after.

## Decision 30: the item page's notes and viewer load as their own chunks (accepted)

**Change.** The item page gains each card's note count and Annotate action, the
first file's notes summary, "All notes in this item", and a file viewer that
shows the media page's stage over the page. None of it is eager: the summary and
the grouped list are the `item-notes` chunk, the WAP container reader they share
with the cards is `read`, and the viewer is `viewer`. The item route chunk keeps
the card actions and the state that ties the three together. The actions and the
summary read the existing `annotations` namespace, so an item page also fetches
its `annotations-messages` chunk, which the media route already shares.

**Measured.** `npm run build` then `npm run perf:check` and `npm run perf:pages`,
gzip bytes, one build:

| Asset | Before | After | Note |
| --- | --- | --- | --- |
| initial js | 86990 | 87006 | the route's new wiring, 34 B under 87040 |
| initial css | 5447 | 5513 | the card action row, the aligned cards |
| items route, own chunk | 5580 | 6649 | the actions and the viewer state |
| `item-notes` | none | 1223 js, 443 css | the summary and the grouped list |
| `read` | none | 296 | the container reader |
| `viewer` | none | 1409 js, 499 css | the dialog and its camera |
| largest chunk | 9785 | 9785 (`preview-*`) | 455 B under the ceiling |

**Read cost.** The notes read is one WAP container request per file, in parallel,
not one per page: `container(mediaId, { pages: 'first' })` stops after the first
page, so a file with 300 notes still costs one request. The e2e item-notes spec
measures 6 requests for 6 files, 846 ms from the navigation to the last group on
the harness (debug server, one run, a warm machine). PERF.md's earlier
measurement stands: parallel authenticated requests cost about one round, because
`bcrypt::verify` dominates.

**Verdict.** Accepted. No budget moved; the initial payload has 33 B of room, so
the next eager addition has to move or split something.

## Decision 31: the item page's note counts come from one read (accepted)

**Premise.** Decision 30 left the item page reading one WAP container per visible file, because a
card's count had no other source. That is 100 session-authenticated requests on a 100-file item.
`GET /annotations/item/{id}/counts` now answers every file's count and write right in one document
(`docs/api.md`, "Web Annotation Protocol"), and the page reads a container only for a file whose
notes it draws.

**Change.** `src/pages/items/notes/read.ts` fetches the counts, then one container for each file with
a count above zero; a file with none costs an entry in the counts document and no request. The cards,
the first file's summary and the grouped list all read the counts document, so the page holds one
source for a count. The counts parse is in the same lazy `read` chunk, not in the eager media parse,
so no list page pays for it.

**Measured.** `npm run build` then `npm run perf:check` and `npm run perf:pages`, gzip bytes, against
this branch's base (`802e978b`) built the same way from a `git archive` copy with the same
`node_modules`:

| Asset | Before | After | Note |
| --- | --- | --- | --- |
| initial js | 87054 | 87047 | inside the run-to-run gzip difference; no member was added to the eager parse |
| initial css | 5513 | 5513 | |
| items route, own chunk | 6654 | 6654 | |
| items route, total with shared chunks | 56150 | 56139 | |
| `read` | 295 | 625 | the counts fetch and its parser |
| `item-notes` | 1222 js, 443 css | 1257 js, 443 css | counts replace the container's totals |
| largest chunk | 9787 (`preview-*`) | 9786 (`preview-*`) | 454 B under the 10240 B ceiling |

**Read cost.** `item-notes.spec.ts` creates a 6-file item with notes on 2 files and counts the
requests the page makes: 1 counts read and 2 container reads, 837 ms from navigation to the last
group on the harness (debug server, one run, a warm machine). Decision 30 measured 6 containers for
the same path, 846 ms, so the wall clock did not move: one authenticated round is `bcrypt::verify`,
and requests in parallel cost about what one costs. What changed is the request count, which is the
part that grows with the item.

**Verdict.** Accepted. No budget moved and the initial payload does not grow. A 100-file item now
costs one counts read plus one container per file that has notes, instead of one per file.

## Budgets

`timings.createRoundTrip` moved 700 ms to 400 ms, which is one API round plus room for a slow box,
matching `listRender` and `detailRender` (Decision 1). `bundle.jsGzipBytes` moved 102400 to 109568
for the write surface Decision 4 measures. Decision 5 then removed that key, put the initial js
payload back on 102400 and added a 10240 B ceiling on each lazy chunk.

The other budgets stay as `uibudget` set them. Their measured medians moved with
the machine, not with a change in the code.

## What a hillclimb should do next

The list render, the detail render and the create round trip are all one API
round of about 118 ms now, and the round is `bcrypt::verify` per authenticated
request on the server. The next step is a server-side change in
`crates/omeka-api/src/auth.rs`, which is outside this unit and outside
`web/admin`. On the frontend the initial payload is 88411 B gzip (Decision 6) and the rest of
the parse on the paint path is React, which Decision 3 measures.

## Decision 31: the document parsers and the write encoders load with their routes (accepted, ends Decision 29's raise)

**Measured, before.** `npm run build` then `npm run perf:check` on this branch: initial js 87043 B
gzip against the 88064 B ceiling Decision 29 set. Decision 29 recorded 87041 B for the same code;
the 2 B is gzip noise between builds.

**Change.** Three moves, each of which takes a module out of the shell without adding a request to a
route's first paint:

- `src/api/media.ts` owns the media document (`parseMedia`), the player URL, `mediaIdFromParam` and
  the five media calls. The media routes and the editor's media fields import it, so it arrives with
  their chunk. `src/api/parse.ts` no longer names a media field, and `client.ts` has no media
  method.
- `src/api/vocabularies.ts` owns the vocabulary document, the diff parser and the eight vocabulary
  calls. The vocabularies, properties, resource classes and resource templates routes import it.
  Classes, properties and templates keep their calls in `client.ts`, because every form's option
  lists read them and they share `parseVocabularyMember`.
- `client.ts` fetches `src/api/write.ts` with the first save (`writeEncoders()`), not with the
  shell. The sites pages import the encoders themselves, so the module arrives with their route as
  it did before, from the entry instead of from the route. Every other form fetches it once, after
  the curator has filled the form and pressed save.

Both new modules follow the pattern `src/api/term-lists.ts` set: a resource family whose pages are
lazy keeps its parser and its calls in its own module, and the shared parse helpers stay in
`parse.ts`.

**Measured, after each move.** The same tree, `npm run build` then `npm run perf:check`, gzip bytes:

| Step | initial js | Cut |
| --- | --- | --- |
| before | 87043 | |
| `src/api/media.ts` | 86538 | 505 |
| `write.ts` behind `writeEncoders()` | 85435 | 1103 |
| `src/api/vocabularies.ts` | 85088 | 347 |

These are single builds on one machine. The total is 1955 B, 2.2% of the 87043 B. The split between
the steps is what each build reported; a gzip difference of a few bytes between two builds of the
same code is noise, so the 505 B is the least certain of the three.

**First paint.** The media and vocabulary modules ride the chunk of the route that imports them, so
no route waits on a new request. The shell's first paint drops 1955 B of parsing.

**Limitation.** A form outside the sites route fetches the 1695 B encoder chunk on its first save,
between the press and the `POST`. Only the first save of a session pays it; the sites routes fetch
it with their own chunk. The budget spec's create round trip is a median of five runs, so its median
does not include that one request. The same run on this tree passed every timing budget: create
round trip median 10.3 ms of 400 ms, first contentful paint 76.0 ms of 250 ms.

**Budget.** `bundle.initialJsGzipBytes` moves from 88064 to 87552, the measured 85088 plus 2464 B.
That is 104 B more room than Decision 20 left (2360 B) and 96 B less than the 2.5 KB this unit was
asked to leave. The per-chunk ceilings do not move; the largest lazy chunk is 9804 B.


