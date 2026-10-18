/**
 * Every entry of the admin page table, reached the way a user reaches it: by
 * clicking the navigation link.
 *
 * No jsdom test has ever loaded a page through the shell, a router and a real
 * network before. These two specs prove each page renders and reports nothing to
 * the console or the network, and that the navigation covers the whole table.
 */
import { expect, expectClean, test } from '../fixtures';
import { adminPageIds } from '../harness/table';

test('every navigation entry loads with no console error and no failed request', async ({
  page,
  diagnostics,
}) => {
  await page.goto('/');
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
  await page.goto('/');
  const hrefs = await page
    .locator('nav.app-nav a.nav-link')
    .evaluateAll((nodes) => nodes.map((node) => node.getAttribute('href') ?? ''));
  const navigation = hrefs.map((href) => href.replace('#/', '')).sort();
  const table = [...adminPageIds()].sort();
  expect(navigation).toEqual(table);
});
