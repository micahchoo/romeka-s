/**
 * Every entry of the admin page table, reached the way a user reaches it: by
 * clicking the navigation link.
 *
 * No jsdom test has ever loaded a page through the shell, a router and a real
 * network before. These two specs prove each page renders and reports nothing to
 * the console or the network, and that the navigation covers the whole table.
 */
import type { Page } from '@playwright/test';

import { expect, expectClean, test } from '../fixtures';
import { adminPageIds } from '../harness/table';

/**
 * Open the app and wait for the Import link. The shell shows it only after
 * `GET /api/romeka/archive` answers enabled, which the harness's archive root
 * makes it, so a count taken before that answer misses one page.
 */
const openWithEveryLink = async (page: Page): Promise<void> => {
  await page.goto('/');
  await expect(page.locator('nav.app-nav a.nav-link[href="#/import"]')).toBeVisible();
};

test('every navigation entry loads with no console error and no failed request', async ({
  page,
  diagnostics,
}) => {
  await openWithEveryLink(page);
  const links = page.locator('nav.app-nav a.nav-link');
  const count = await links.count();
  expect(count).toBeGreaterThan(0);

  const visited: string[] = [];
  for (let index = 0; index < count; index += 1) {
    const link = links.nth(index);
    const title = ((await link.textContent()) ?? '').trim();
    const href = (await link.getAttribute('href')) ?? '(no href)';
    diagnostics.reset();
    await link.click();
    // The shell renders the page title from the same table the link came from, so
    // a title that disagrees means the row is wired to the wrong page.
    await expect(page.locator('main h1')).toHaveText(title);
    await expectClean(diagnostics, `${title} (${href})`);
    // A page that fails a read renders an alert instead of its table.
    await expect(page.locator('main [role=alert]')).toHaveCount(0);
    visited.push(href.replace('#/', ''));
  }
  expect(visited.length).toBe(count);
});

test('the navigation covers every page id in the admin page table', async ({ page }) => {
  await openWithEveryLink(page);
  const hrefs = await page
    .locator('nav.app-nav a.nav-link')
    .evaluateAll((nodes) => nodes.map((node) => node.getAttribute('href') ?? ''));
  const navigation = hrefs.map((href) => href.replace('#/', '')).sort();
  const table = [...adminPageIds()].sort();
  expect(navigation).toEqual(table);
});
