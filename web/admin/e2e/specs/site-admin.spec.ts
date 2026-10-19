/**
 * The site-admin workflows the SPA reaches through sub-pages: navigation,
 * resources, user permissions and site settings.
 *
 * The run drives the real server and the real database, so a reload that shows
 * a saved value proves the write reached storage and the read served it back,
 * which no stubbed vitest can show.
 */
import { expect, expectClean, test } from '../fixtures';
import { SITE_TITLE } from '../harness/seed';

/** The hash of the seeded site's detail page, e.g. `#/sites/1`. */
const siteHash = (url: string): string => {
  const match = /#(\/sites\/\d+)$/.exec(url);
  if (match?.[1] === undefined) {
    throw new Error(`no site detail hash in ${url}`);
  }
  return match[1];
};

test('the site admin saves site settings, navigation and permissions', async ({
  page,
  diagnostics,
}) => {
  await page.goto('/#/sites');
  await page.getByRole('link', { name: SITE_TITLE }).click();
  const detail = siteHash(page.url());
  diagnostics.reset();

  // Site settings: each key is stored as a `site_setting` row and read back.
  await page.goto(`/#${detail}/settings`);
  const installationTitle = page.getByLabel('Name in the browser tab');
  await expect(installationTitle).toBeVisible();
  await installationTitle.fill('E2E Library');
  await page.getByRole('button', { name: 'Save' }).click();
  await expect(page.getByText('Saved.')).toBeVisible();
  await page.reload();
  await expect(page.getByLabel('Name in the browser tab')).toHaveValue('E2E Library');

  // Navigation: a URL link is added and survives a reload.
  await page.goto(`/#${detail}/navigation`);
  await page.getByRole('button', { name: 'Add link' }).click();
  await page.getByLabel('Link type', { exact: true }).selectOption('url');
  await page.getByLabel('URL', { exact: true }).fill('https://example.org/e2e');
  await page.getByLabel('Label', { exact: true }).fill('E2E link');
  await page.getByRole('button', { name: 'Save' }).click();
  await expect(page.getByText('Saved.')).toBeVisible();
  await page.reload();
  await expect(page.getByRole('button', { name: 'Move E2E link', exact: true })).toBeVisible();

  // User permissions: the owner's role changes and survives a reload.
  await page.goto(`/#${detail}/users`);
  await page.getByLabel('Role').first().selectOption('editor');
  await page.getByRole('button', { name: 'Save' }).click();
  await expect(page.getByText('Saved.')).toBeVisible();
  await page.reload();
  await expect(page.getByLabel('Role').first()).toHaveValue('editor');

  // Site resources: the item count the site currently holds renders.
  await page.goto(`/#${detail}/resources`);
  await expect(page.getByText(/Items in this site:/)).toBeVisible();

  await expectClean(diagnostics, 'site admin');
});
