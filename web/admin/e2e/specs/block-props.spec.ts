/**
 * Block properties against the real server: a media block shows its thumbnail
 * size once the display is Thumbnail and picks an item through the shared
 * picker, a padding preset reaches the live preview, and a browse preview of
 * item sets counts item sets.
 */
import type { Page } from '@playwright/test';
import { expect, test } from '../fixtures';
import { ITEM_SET_TITLE, ITEM_TITLES, SITE_SLUG } from '../harness/seed';
import { readHarness } from '../harness/state';

const idOf = (value: unknown): number => {
  if (typeof value === 'object' && value !== null && 'o:id' in value && typeof value['o:id'] === 'number') {
    return value['o:id'];
  }
  throw new Error(`no o:id in ${JSON.stringify(value)}`);
};

const first = (value: unknown): unknown => (Array.isArray(value) ? value[0] : value);

const siteId = async (page: Page): Promise<number> =>
  idOf(first(await (await page.request.get(`/api/sites?slug=${SITE_SLUG}`)).json()));

/** A fresh page holding one HTML block, so the spec owns what it edits. */
const createPage = async (page: Page, slug: string): Promise<string> => {
  const site = await siteId(page);
  const created = await page.request.post('/api/site_pages', {
    data: {
      'o:site': { 'o:id': site },
      'o:title': 'Block properties',
      'o:slug': slug,
      'o:is_public': true,
      'o:block': [{ 'o:layout': 'html', 'o:data': { html: '<p>Padded block</p>' } }],
    },
  });
  expect(created.status(), await created.text()).toBe(200);
  return `/#/sites/${String(site)}/pages/${String(idOf(await created.json()))}`;
};

const insert = async (page: Page, name: string): Promise<void> => {
  await page.getByRole('button', { name: /Insert block at position/ }).last().click();
  await page.getByRole('dialog', { name: 'Insert a block or template' }).getByRole('button', { name, exact: true }).click();
};

const inspector = (page: Page) => page.getByRole('region', { name: 'Block inspector' });

const shown = (page: Page) => page.frameLocator('iframe.live-frame-doc:not(.live-frame-doc--back)');

test.beforeEach(async ({ page }) => {
  // The preview POST goes to `/s/<slug>/preview`, which the harness proxy does
  // not key, so the browser holds the key the way a signed-in user's does.
  const harness = readHarness();
  await page.addInitScript(
    (key: { identity: string; credential: string }) => {
      window.localStorage.setItem('romeka.apiKey', JSON.stringify(key));
    },
    { identity: harness.keyIdentity, credential: harness.keyCredential },
  );
});

test('a media block reveals its thumbnail size and picks an item through the picker', async ({ page }) => {
  await page.goto(await createPage(page, 'block-props-media'));
  await insert(page, 'Media embed');
  const form = inspector(page);

  await expect(form.getByLabel('Show media as')).toBeVisible();
  await expect(form.getByLabel('Thumbnail size')).toHaveCount(0);
  await form.getByLabel('Show media as').selectOption('thumbnail');
  await expect(form.getByLabel('Thumbnail size')).toHaveValue('square');

  const item = ITEM_TITLES[0] ?? 'E2E item 1';
  await form.getByLabel('Add or remove items').fill(item);
  await form.getByRole('button', { name: item, exact: true, pressed: false }).click();
  const rows = form.getByRole('list', { name: 'Items to show' });
  await expect(rows.getByText(item, { exact: true })).toBeVisible();
  await expect(rows.locator('img.attachment-thumb').first()).toBeVisible();
  await expect(rows.getByRole('radio', { name: 'Main media' })).toBeChecked();
  // A picked result's name carries the picker's check mark after the title.
  await expect(form.getByRole('button', { name: new RegExp(`^${item}`), pressed: true })).toBeVisible();
});

test('a padding preset reaches the block on the live preview', async ({ page }) => {
  await page.goto(await createPage(page, 'block-props-padding'));
  await shown(page).getByText('Padded block').click();
  const form = inspector(page);

  const rendered = page.waitForResponse(
    (response) => response.url().includes(`/s/${SITE_SLUG}/preview`) && response.request().method() === 'POST',
  );
  await form.getByRole('group', { name: 'Spacing' }).getByRole('button', { name: 'Large' }).click();
  await rendered;

  const block = shown(page).locator('.block-html').filter({ hasText: 'Padded block' });
  await expect(block).toHaveAttribute('style', /padding-top: 2rem/);
  await expect(block).toHaveCSS('padding-left', '32px');
});

test('a browse preview of item sets counts the item sets in the site', async ({ page }) => {
  const site = await siteId(page);
  const sets: unknown = await (await page.request.get('/api/item_sets')).json();
  const list: readonly unknown[] = Array.isArray(sets) ? sets : [];
  const set = list.find((entry) => JSON.stringify(entry).includes(ITEM_SET_TITLE));
  const assign = (ids: readonly number[]) =>
    page.request.patch(`/api/sites/${String(site)}`, {
      data: { 'o:site_item_set': ids.map((id) => ({ 'o:item_set': { 'o:id': id } })) },
    });
  const assigned = await assign([idOf(set)]);
  expect(assigned.status(), await assigned.text()).toBe(200);
  try {
    await page.goto(await createPage(page, 'block-props-browse'));
    await insert(page, 'Browse preview');
    const form = inspector(page);

    await expect(form.getByLabel('Item set')).toBeVisible();
    await form.getByLabel('What to list').selectOption('item_sets');
    await expect(form.getByText('Item sets in this site that match: 1.')).toBeVisible();
    await expect(form.getByLabel('Item set')).toHaveCount(0);
  } finally {
    await assign([]);
  }
});
