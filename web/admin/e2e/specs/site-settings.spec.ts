/**
 * The site settings form, group by group, against the public site.
 *
 * The spec makes its own site and two items in it, so the seeded site the other
 * specs read stays as seeded. It changes one setting in each group, saves, and
 * then reads the public page each setting changes, which is the only proof that
 * the label and help say what a visitor sees.
 */
import type { APIRequestContext, Page } from '@playwright/test';
import { expect, expectClean, test } from '../fixtures';
import { TITLE_TERM } from '../harness/seed';

const SLUG = 'settings-site';

const createSite = async (page: Page): Promise<number> => {
  await page.goto('/#/sites/new');
  await page.getByLabel('Title').fill('Settings Site');
  await expect(page.getByLabel('Slug')).toHaveValue(SLUG);
  await page.getByRole('button', { name: 'Add site' }).click();
  await expect(page).toHaveURL(/#\/sites\/\d+$/);
  const id = /#\/sites\/(\d+)$/.exec(page.url())?.[1];
  if (id === undefined) {
    throw new Error(`no site id in ${page.url()}`);
  }
  return Number(id);
};

const json = async (request: APIRequestContext, path: string): Promise<unknown> => {
  const response = await request.get(path);
  expect(response.ok(), path).toBe(true);
  return response.json();
};

/** A public item in the site, through the API the harness proxy signs. */
const addItem = async (
  request: APIRequestContext,
  siteId: number,
  title: string,
): Promise<number> => {
  const properties = await json(request, `/api/properties?term=${TITLE_TERM}`);
  const property: unknown = Array.isArray(properties) ? properties[0] : undefined;
  if (typeof property !== 'object' || property === null || !('o:id' in property)) {
    throw new Error(`no ${TITLE_TERM} property`);
  }
  const response = await request.post('/api/items', {
    data: {
      'o:is_public': true,
      'o:site': [{ 'o:id': siteId }],
      [TITLE_TERM]: [
        { type: 'literal', property_id: property['o:id'], is_public: true, '@value': title },
      ],
    },
  });
  expect(response.ok(), await response.text()).toBe(true);
  const item: unknown = await response.json();
  if (typeof item !== 'object' || item === null || !('o:id' in item)) {
    throw new Error('the item create answered no id');
  }
  return Number(item['o:id']);
};

test('a setting in each group changes the public site', async ({ page, diagnostics }) => {
  const siteId = await createSite(page);
  const itemId = await addItem(page.request, siteId, 'Settings item one');
  await addItem(page.request, siteId, 'Settings item two');

  // Before: the first browse page lists both items.
  await page.goto(`/s/${SLUG}/item`);
  await expect(page.getByRole('link', { name: 'Settings item one' })).toBeVisible();
  await expect(page.getByRole('link', { name: 'Settings item two' })).toBeVisible();

  diagnostics.reset();
  await page.goto(`/#/sites/${String(siteId)}/settings`);
  const browsing = page.getByRole('group', { name: 'Browsing' });
  await browsing.getByLabel('Items per page').fill('1');
  await page.getByRole('group', { name: 'Search' }).getByLabel('Search scope').selectOption({
    label: 'All sites',
  });
  await page
    .getByRole('group', { name: 'Item and media pages' })
    .getByLabel('Extra detail on field names')
    .selectOption({ label: 'Technical term, such as dcterms:title' });
  await page.getByRole('group', { name: 'Language' }).getByLabel('Site language', { exact: true }).selectOption({
    label: 'Spanish',
  });
  await page
    .getByRole('group', { name: 'Navigation and chrome' })
    .getByLabel('Name in the browser tab')
    .fill('Settings Archive');
  await page.locator('summary', { hasText: 'Advanced' }).click();
  await page.getByLabel('Leave out linked data').check();
  await page.getByRole('button', { name: 'Save' }).click();
  await expect(page.getByText('Saved.')).toBeVisible();
  await page.reload();
  await expect(page.getByLabel('Items per page')).toHaveValue('1');
  await expect(page.getByLabel('Site language', { exact: true })).toHaveValue('es');
  await expectClean(diagnostics, 'site settings');

  // Browsing: one item per page.
  await page.goto(`/s/${SLUG}/item`);
  await expect(page.getByRole('link', { name: 'Settings item two' })).toBeVisible();
  await expect(page.getByRole('link', { name: 'Settings item one' })).toHaveCount(0);
  await expect(page.locator(`a[href="/s/${SLUG}/item?page=2"]`).first()).toBeVisible();
  // Search: the form sends the visitor to the cross-site search.
  await expect(page.locator('form[action*="cross-site"]').first()).toBeAttached();
  // Language: the page renders in Spanish.
  await expect(page.locator('html')).toHaveAttribute('lang', 'es');
  // Navigation and chrome: the tab name follows the page title.
  await expect(page).toHaveTitle(/Settings Archive/);

  const item = await page.request.get(`/s/${SLUG}/item/${String(itemId)}`);
  expect(item.ok()).toBe(true);
  const body = await item.text();
  // Item and media pages: the technical term sits beside the field name.
  expect(body).toContain('<span class="field-term">(dcterms:title)</span>');
  // Advanced: no embedded JSON-LD.
  expect(body).not.toContain('application/ld+json');
});
