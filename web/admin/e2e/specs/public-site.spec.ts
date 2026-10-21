/**
 * The public site the admin manages renders on the port's `/s` routes.
 *
 * The slug comes off the admin sites table, the way a user copies it, and the
 * page is then requested from the same origin the SPA is served from. The port
 * renders the theme's HTML there, so this is the one spec that does not run the
 * SPA at all.
 */
import { expect, test } from '../fixtures';
import { isArray, numberField, record } from '../harness/json';
import { SITE_SLUG, SITE_TITLE, WELCOME_SLUG } from '../harness/seed';

test('a media page shows the whole file, never a 4:3 crop of it', async ({ page }) => {
  // The seeded items are not attached to the site, so the browse list is empty;
  // the item page still renders, and the whole first file opens its media page.
  const items = await page.request.get('/api/items?per_page=1');
  const listed: unknown = await items.json();
  if (!isArray(listed) || listed.length === 0) {
    throw new Error('/api/items returned no item');
  }
  const itemId = numberField(record(listed[0], 'item'), 'o:id', 'item');
  await page.goto(`/s/${SITE_SLUG}/item/${String(itemId)}`);
  // One file: it is the whole page, with no strip under it.
  await expect(page.locator('.media-embeds .media-render a')).toHaveCount(1);
  await expect(page.locator('.media-files')).toHaveCount(0);
  const file = page.locator('.media-embeds .media-render a').first();
  await expect(file).toBeVisible();
  await file.click();
  await expect(page).toHaveURL(/\/media\/\d+$/);
  // The upload has no title, so the heading and the document title name it by
  // its source file.
  await expect(page.getByRole('heading', { level: 2, name: 'sample.png' })).toBeVisible();
  await expect(page).toHaveTitle(/^sample\.png · /);
  await expect(page.locator('.media-position')).toHaveCount(0);

  const image = page.locator('.media-page .media-render img').first();
  await expect(image).toBeVisible();
  const shown = await image.evaluate((node) => {
    const computed = getComputedStyle(node);
    return {
      fit: computed.objectFit,
      height: node.getBoundingClientRect().height,
    };
  });
  // Contained rather than cover-cropped, and capped at 80% of the viewport so
  // a tall file fits the screen whole.
  expect(shown.fit).toBe('contain');
  expect(shown.height).toBeLessThanOrEqual((page.viewportSize()?.height ?? 720) * 0.8 + 1);
  const overflow = await page.evaluate(
    () => document.documentElement.scrollWidth - document.documentElement.clientWidth,
  );
  expect(overflow).toBe(0);
});

test('the public site renders the page the admin shows', async ({ page }) => {
  await page.goto('/#/sites');
  const row = page.getByRole('row', { name: new RegExp(SITE_TITLE) });
  await expect(row).toBeVisible();
  await expect(row.getByRole('cell', { name: SITE_SLUG, exact: true })).toBeVisible();

  // The site index answers HTML for the origin the admin serves from.
  const index = await page.goto(`/s/${SITE_SLUG}`);
  expect(index?.status()).toBe(200);
  expect(index?.headers()['content-type']).toBe('text/html; charset=utf-8');

  // The welcome page the port builds on create renders its blocks and footer.
  const welcome = await page.goto(`/s/${SITE_SLUG}/page/${WELCOME_SLUG}`);
  expect(welcome?.status()).toBe(200);
  await expect(page).toHaveTitle(/Welcome/);
  await expect(page.getByText('This is the first page of your site.')).toBeVisible();
  await expect(page.getByText('Powered by Romeka S')).toBeVisible();
});
