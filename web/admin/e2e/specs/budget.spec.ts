/**
 * The UI budget numbers, measured in a real browser through the Performance API.
 *
 * Five samples each, taken against the harness `globalSetup` already started, so
 * the paths are the ones a user waits on: a cold first contentful paint, the
 * items list render, an item detail render, an item create round trip, and a
 * page navigation through the shell. The last step folds in the gzip bundle
 * measurement and writes `perf/summary.json` with the commit, which is the
 * artifact `npm run perf:check` gates.
 *
 * The spec records and always passes when the paths render. The budget gate is
 * `perf/check.mjs`, kept out of this run so a slow box does not fail the e2e
 * suite; see `perf/README.md`.
 */
import { expect, expectClean, test } from '../fixtures';
import {
  RUNS,
  measureBundleFromDist,
  measureFcp,
  measureInPage,
  repeat,
  summarize,
  summarizeProbes,
  writeSummary,
} from '../harness/perf';

/** The seeded item links, one row per item, sorted the way the list loads them. */
const ITEM_ROW = 'main table tbody a[href^="#/items/"]';
const NAV_ITEMS = 'nav.app-nav a.nav-link[href="#/items"]';
const NAV_SYSTEM_INFO = 'nav.app-nav a.nav-link[href="#/system-info"]';
const SUBMIT = 'main form button[type="submit"]';
/** The item title on the show page (`ItemDetail`). */
const DETAIL_TITLE = 'main section h2';
const PAGE_HEADING = 'main h1';
const CREATE_PREFIX = 'UI budget item';

test('records the bundle and the user-path timings', async ({ page, baseURL, diagnostics }) => {
  const gotoPage = async (pageId: string, title: string): Promise<void> => {
    await page.goto(`/#/${pageId}`);
    await expect(page.locator(PAGE_HEADING)).toHaveText(title);
  };

  // Warm the app so the first measured step is not the initial document load.
  await gotoPage('items', 'Items');
  await page.locator(ITEM_ROW).first().waitFor();

  const fcp = summarize(await measureFcp(page, baseURL, RUNS));

  // The reset page makes no API request of its own, so the only requests inside
  // the measured span are the ones the list path makes.
  const listRender = summarizeProbes(
    await repeat(
      RUNS,
      async () => {
        await gotoPage('columns', 'Browse columns');
      },
      () => page.evaluate(measureInPage, { action: NAV_ITEMS, ready: ITEM_ROW }),
    ),
  );

  const detailRender = summarizeProbes(
    await repeat(
      RUNS,
      async () => {
        await page.goto('/#/items');
        await page.locator(ITEM_ROW).first().waitFor();
      },
      async () => {
        const title = ((await page.locator(ITEM_ROW).first().textContent()) ?? '').trim();
        return page.evaluate(measureInPage, {
          action: ITEM_ROW,
          ready: DETAIL_TITLE,
          text: title,
        });
      },
    ),
  );

  // A create round trip: submit the form, wait for the show page to render it.
  const createRoundTrip = summarizeProbes(
    await repeat(
      RUNS,
      async () => {
        await page.goto('/#/items/new');
        await page.getByRole('textbox', { name: 'Title', exact: true }).waitFor();
      },
      async (index) => {
        const title = `${CREATE_PREFIX} ${String(index)}`;
        await page.getByRole('textbox', { name: 'Title', exact: true }).fill(title);
        const result = await page.evaluate(measureInPage, {
          action: SUBMIT,
          ready: DETAIL_TITLE,
          text: title,
        });
        // The create left the browser on the show page. Remove the row so the
        // list the next sample reads is the seeded three.
        const id = /#\/items\/(\d+)$/.exec(page.url())?.[1];
        expect(id, 'created item id in the URL').toBeDefined();
        const removed = await page.request.delete(`${baseURL}/api/items/${String(id)}`);
        expect(removed.ok(), 'the created item is deleted').toBe(true);
        return result;
      },
    ),
  );

  // A route change through the shell. The page's own data load is the list
  // render above, so this waits only on the heading the router renders.
  const navigation = summarizeProbes(
    await repeat(
      RUNS,
      async () => {
        await gotoPage('columns', 'Browse columns');
      },
      () =>
        page.evaluate(measureInPage, {
          action: NAV_SYSTEM_INFO,
          ready: PAGE_HEADING,
          text: 'System information',
        }),
    ),
  );

  const summary = writeSummary(
    { fcp, listRender, detailRender, createRoundTrip, navigation },
    measureBundleFromDist(),
  );

  for (const [name, stat] of Object.entries(summary.timings)) {
    let api = 'no api request';
    if (stat.apiMedian !== null && stat.apiMaxMedian !== null) {
      api = `api ${stat.apiMedian.toFixed(1)} ms over ${String(stat.apiRequests)} request(s), slowest ${stat.apiMaxMedian.toFixed(1)} ms at ${String(stat.apiSlowest)}`;
    }
    console.log(
      `${name}: median ${stat.median.toFixed(1)} ms (min ${stat.min.toFixed(1)}, max ${stat.max.toFixed(1)}, ${String(stat.runs)} runs), ${api}`,
    );
  }
  console.log(
    `bundle: initial js gzip ${String(summary.bundle.initialJsGzipBytes)} bytes, initial css gzip ${String(summary.bundle.initialCssGzipBytes)} bytes, ${String(summary.bundle.chunks.length)} lazy chunks`,
  );
  console.log(`summary: commit ${summary.commit}`);

  expect(fcp.median).toBeGreaterThan(0);
  expect(navigation.median).toBeGreaterThan(0);
  expect(summary.runs).toBe(RUNS);
  await expectClean(diagnostics, 'ui budget');
});
