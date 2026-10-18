/**
 * The public site the admin manages renders on the port's `/s` routes.
 *
 * The slug comes off the admin sites table, the way a user copies it, and the
 * page is then requested from the same origin the SPA is served from. The port
 * renders the theme's HTML there, so this is the one spec that does not run the
 * SPA at all.
 */
import { expect, test } from '../fixtures';
import { SITE_SLUG, SITE_TITLE, WELCOME_SLUG } from '../harness/seed';

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
  await expect(page.getByText('Welcome to your new site.')).toBeVisible();
  await expect(page.getByText('Powered by Omeka S')).toBeVisible();
});
