# UI budget (web/admin)

The frontend half of the speed track. Two things are measured: the gzip size of
the built bundles, which is deterministic, and the time the user paths take in a
real browser, recorded by the Playwright harness. `perf/budget.json` holds the
budget, `npm run perf:check` is the gate.

## Commands

Run these from `web/admin`.

| Command | What it does |
| --- | --- |
| `npm run build` | Writes `dist/`. The bundle measurement needs it. |
| `npm run perf:bundle` | Gzips every `dist/assets` `.js` and `.css` and fails over budget. |
| `npm run perf:pages` | Prints the initial payload and each route's chunk from the built `dist`. |
| `npm run e2e` | Builds, starts the harness and drives chromium. The `budget` spec writes `perf/summary.json`. |
| `npm run perf:check` | The gate. Checks the bundle, and the timings when `summary.json` exists. |

`perf:check` exits non-zero on any exceeded budget. It checks the bundle alone
when `perf/summary.json` is absent, so it works after a build without an e2e run.

## What is measured

- **Bundle.** `perf/bundle.mjs` reads `dist/assets` and gzips each emitted `.js`
  and `.css` with node's zlib, per file, because the browser downloads one
  response per file. It divides them the way `dist/index.html` does: the initial
  payload is the entry script, its modulepreload links and the stylesheet. A
  file the Vite manifest builds from `src/i18n/locales/` is a catalogue chunk,
  which a visitor in that locale fetches before the first render. Every other
  file is a lazy chunk a route fetches on its first visit.
  Deterministic, so it runs in any gate.
- **Per-route payload.** `perf/pages.mjs` reads `dist/.vite/manifest.json` (the
  `build.manifest` option in `vite.config.ts`) and prints each route's own chunk
  and the shared chunks it pulls in, beside the initial payload. This is the
  table `PERF.md` quotes.
- **First contentful paint.** `e2e/specs/budget.spec.ts` loads the app five times
  with the HTTP cache disabled through CDP and reads the browser's
  `first-contentful-paint` paint-timing entry. A cold load: document, entry,
  parse, first paint.
- **List render, detail render, create round trip, page navigation.** Five runs
  each. Each run resets the page, then the measured span starts on the click and
  ends on the DOM that finishes the step, timed with `performance.now` inside the
  page (`e2e/harness/perf.ts`), so the worker-to-browser round trip is excluded.
- **API attribution.** The same probe reads resource timing on the same clock and
  records the wall span of the `/api` requests inside the span, the slowest single
  request and its pathname. `summary.json` carries them, so a number names its
  limiter instead of leaving it to guesswork.

The median of the five runs is what `perf:check` compares. `summary.json` also
carries the min, the max, the run count and the commit.

## What is not measured

- **Paint after the DOM step.** The probe ends on the DOM mutation, not on the
  next frame. List, detail, create and navigation are render-readiness numbers;
  first contentful paint is the one paint number.
- **A release build.** The harness drives the debug `romeka` the e2e suite already
  starts, so the API share of every timing is a debug number. The list path is
  API-bound (see below), which means those budgets shift when the server build
  changes.
- **Upstream.** There is no comparison against the upstream admin here. The
  Playwright harness has no upstream in it. `verification/perf` owns the
  backend comparison; this file answers the frontend only.
- **Per-component cost, layout shifts, long tasks, memory.** These need a profile,
  not a wall-clock span. The ROADMAP pairs this instrument with one.
- **The public site.** The timing paths are admin SPA paths.
- **Cache-hit runs other than the paint.** The five repeated runs hit a warm HTTP
  cache. Only the first contentful paint is measured cold.
- **Browsers other than Chromium.** The cold-load measurement uses CDP.

## Budgets

Provisional, set from the first measured run on this box and given room for a
slower one. A reviewer should tighten them once a second machine agrees.
`createRoundTrip` moved from 700 ms to 400 ms when the item create stopped
paying a second API round. The bundle moved when the router split: the gate now
measures the initial payload and each lazy chunk, and the old
`bundle.jsGzipBytes` sum is gone. See `PERF.md` Decision 5. The bundle column is
the 2026-10-17 split build, the timing column the 2026-10-17 run.

| Metric | Budget | Measured |
| --- | --- | --- |
| initial js gzip | 102400 B | 85354 B |
| initial css gzip | 6144 B | 2258 B |
| each lazy chunk | 10240 B | largest 6909 B |
| each catalogue chunk | 13312 B | Spanish 12003 B (`PERF.md` Decision 11) |
| first contentful paint | 250 ms | 52-60 ms |
| list render | 400 ms | 123-128 ms |
| detail render | 400 ms | 123-128 ms |
| create round trip | 400 ms | 121-125 ms |
| page navigation | 100 ms | 0.5-1.5 ms |

## The numbers and their limiter

Measured on 2026-10-17, five runs each, one process, warm page cache, a debug
`romeka` behind the harness origin.

| Path | Median | API span | Slowest request | Limiter |
| --- | --- | --- | --- | --- |
| first contentful paint | 52-56 ms | none | none | parsing and running the bundle |
| list render | 123-128 ms | 121-123 ms over 3 requests | `/api/items`, `/api/users`, `/api/resource_classes` | the API |
| detail render | 123-128 ms | 121-123 ms over 2 to 4 requests | the item read, then `/api/media` | the API |
| create round trip | 121-125 ms | 119-122 ms over 1 request | the POST | the API, once |
| page navigation | 0.5-1.5 ms | none | none | the shell render |

**Why the API bounds three of the four paths.** One authenticated `/api` request
costs about 118 ms against the debugging server. Measured directly on a freshly
installed database: an authenticated `GET /api/users` answers in 122 to 125 ms
and an unauthenticated `GET /api/items` in 8 to 10 ms, with about 6 ms of that
being the `curl` process. The cost is per request and independent of the payload,
since `per_page=100` on `/api/resource_classes` (37 KB) and `per_page=1` on
`/api/users` both land near 120 ms. `crates/omeka-api/src/auth.rs` verifies the
key credential with `bcrypt::verify` on every request, the port of upstream's
`password_verify`, and that verification is the cost. Three or four parallel
requests cost about 3 ms more than one, in the browser as well as in `curl`, so
they overlap and the span stays near one request.

**What that means for a hillclimb.** The render itself is 1-3 ms on every path
and the navigation is under 2 ms, so the wall clock of the list render, the
detail render and the create round trip is one API round and nothing on the
frontend removes it. An earlier version of this file said reducing the request
count moves the list render and the detail render by their whole span. That is
wrong: three parallel requests cost what one costs, measured at 121.3 ms for
three against 121.5 ms for one, so the request count buys about 3 ms.
`PERF.md` records the change that tested it and the rejection. The two remaining
frontend levers are the bundle parse on the paint path and the request count on a
real network, and both are small next to the 118 ms round.

## Reading the artifact

`perf/summary.json` is written by every `npm run e2e` and ignored by git. It holds
the commit, the run count, the bundle measurement with one entry per file, and one
entry per timing with `min`, `median`, `max`, `apiMedian`, `apiMaxMedian`,
`apiSlowest` and `apiRequests`.
