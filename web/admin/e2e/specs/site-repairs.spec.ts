/**
 * The site admin repairs in a real browser, on a site of their own so the
 * seeded site stays as the other specs expect it: the theme's name in the
 * sites list, the navigation's link problems, the Users tab's names, the
 * unsaved guard, and a colour's dark value under the automatic scheme.
 */
import type { APIRequestContext } from '@playwright/test';

import { expect, expectClean, test } from '../fixtures';
import { readHarness } from '../harness/state';

const SLUG = 'e2e-repairs';

const makeSite = async (request: APIRequestContext): Promise<number> => {
  const response = await request.post('/api/sites', {
    data: { 'o:title': 'E2E repairs site', 'o:slug': SLUG, 'o:theme': 'default', 'o:is_public': true },
  });
  expect(response.status(), await response.text()).toBe(200);
  const document: unknown = await response.json();
  if (typeof document === 'object' && document !== null && 'o:id' in document && typeof document['o:id'] === 'number') {
    return document['o:id'];
  }
  throw new Error(`no site id in ${JSON.stringify(document)}`);
};

test('the site admin repairs hold in the browser', async ({ page, request, diagnostics }) => {
  test.setTimeout(60_000);
  const site = await makeSite(request);
  // The Theme preview POSTs to `/s/<slug>/preview`, which the harness proxy
  // does not key, so the browser holds the key as a signed-in user's does.
  const { keyIdentity, keyCredential } = readHarness();
  await page.addInitScript(
    (key: { identity: string; credential: string }) => {
      window.localStorage.setItem('romeka.apiKey', JSON.stringify(key));
    },
    { identity: keyIdentity, credential: keyCredential },
  );
  const hub = `/#/sites/${String(site)}`;

  // The sites list names the bundled theme as the hub does.
  await page.goto('/#/sites');
  const row = page.getByRole('row', { name: /E2E repairs site/ });
  await expect(row.getByRole('cell', { name: 'Default', exact: true })).toBeVisible();

  // Navigation: each incomplete link says what it lacks, and a second link to
  // a page already in the menu is caught before the server sees it.
  await page.goto(`${hub}/navigation`);
  await page.getByRole('button', { name: 'Add link' }).click();
  await page.getByLabel('Link type', { exact: true }).selectOption('url');
  await page.getByLabel('URL', { exact: true }).fill('https://example.org/e2e');
  const tree = page.getByRole('list', { name: 'Menu links' });
  await expect(tree.getByText('Needs a label')).toBeVisible();
  await page.getByLabel('Label', { exact: true }).fill('Example');
  await expect(tree.getByText('Needs a label')).toHaveCount(0);
  await page.getByRole('button', { name: 'Add link' }).click();
  await page.getByLabel('Page', { exact: true }).selectOption({ label: 'Welcome' });
  await expect(tree.getByText('Page already in the menu')).toBeVisible();
  await page.getByRole('button', { name: 'Save' }).click();
  await expect(page.getByRole('alert')).toHaveText(
    'Some links are not complete. Fix the marked links, then save.',
  );

  // The unsaved guard: leaving the dirty tab asks, and declining stays.
  await expect(page.getByText('Unsaved changes')).toBeVisible();
  page.once('dialog', (dialog) => {
    expect(dialog.message()).toBe('This tab has unsaved changes. Leave and lose them?');
    void dialog.dismiss();
  });
  await page.getByRole('navigation', { name: 'Site sections' }).getByRole('link', { name: 'Theme' }).click();
  await expect(page).toHaveURL(new RegExp(`/sites/${String(site)}/navigation$`));
  page.once('dialog', (dialog) => {
    void dialog.accept();
  });
  await page.getByRole('navigation', { name: 'Site sections' }).getByRole('link', { name: 'User permissions' }).click();

  // Users: a row names its user; Add user offers only users not listed.
  await expect(page).toHaveURL(new RegExp(`/sites/${String(site)}/users$`));
  const firstUser = page.getByLabel('User', { exact: true }).first();
  await expect(firstUser).toHaveText(/E2E Admin/);
  await expect(page.getByLabel('Add user').getByRole('option', { name: 'E2E Admin' })).toHaveCount(0);
  diagnostics.reset();

  // Theme: under the automatic scheme the Text colour has a dark value of its
  // own, which a save stores and the public page's dark media query carries.
  await page.goto(`${hub}/theme`);
  await page.getByText('Fine-tune').click();
  await page.getByLabel('Colour scheme', { exact: true }).selectOption('auto');
  // The colour picker beside it carries the same name, so the text field is found by id.
  await page.locator('#theme-setting-text_color_dark').fill('#eeeeee');
  await expect(page.getByText('Unsaved changes')).toBeVisible();
  await page.getByRole('button', { name: 'Save' }).click();
  await expect(page.getByText('Saved.')).toBeVisible();
  await expect(page.getByText('Unsaved changes')).toHaveCount(0);
  await page.reload();
  await page.getByText('Fine-tune').click();
  await expect(page.locator('#theme-setting-text_color_dark')).toHaveValue('#eeeeee');
  await expectClean(diagnostics, 'theme dark value');

  const html = await (await request.get(`/s/${SLUG}`)).text();
  const media = html.split('@media (prefers-color-scheme: dark)')[1] ?? '';
  expect(media).toContain('--color-text: #eeeeee;');
});
