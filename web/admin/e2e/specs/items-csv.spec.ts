/**
 * A set's place downloads as CSV: the file has one row per item the place counts, with the
 * screen's order, and the ticked rows download alone. `EXPORT_SCREENSHOTS` names a folder for the
 * pictures.
 */
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import type { Download, Page } from '@playwright/test';
import { expect, expectClean, test } from '../fixtures';
import { expectAxeBudget } from '../harness/axe';
import { readHarness } from '../harness/state';

const SET = 'CSV e2e ledgers';
/** Title A to Z is the order the place is sorted by below, so the file reads in this order. */
const ITEMS = [
  { title: 'CSV e2e alpha, the first ledger', creator: 'रवि शंकर' },
  { title: 'CSV e2e beta', creator: 'Ada "the clerk" Byron' },
  { title: 'CSV e2e gamma', creator: null },
] as const;

const shoot = async (page: Page, name: string): Promise<void> => {
  const folder = process.env.EXPORT_SCREENSHOTS;
  if (folder !== undefined && folder !== '') {
    await page.screenshot({ path: join(folder, `csv-${name}.png`), fullPage: true });
  }
};

const signIn = async (page: Page, baseURL: string): Promise<void> => {
  const { adminEmail, adminPassword } = readHarness();
  const response = await page.request.post('/api/login', {
    data: { email: adminEmail, password: adminPassword },
    headers: { origin: new URL(baseURL).origin },
  });
  expect(response.status(), await response.text()).toBe(200);
};

const literal = (propertyId: number, text: string) => [{ type: 'literal', property_id: propertyId, '@value': text }];

const idOf = async (response: { json: () => Promise<unknown> }): Promise<number> => {
  const body: unknown = await response.json();
  const id = typeof body === 'object' && body !== null && 'o:id' in body ? body['o:id'] : null;
  expect(typeof id).toBe('number');
  return Number(id);
};

const seed = async (page: Page): Promise<number> => {
  const set = await page.request.post('/api/item_sets', {
    data: { 'dcterms:title': literal(1, SET), 'o:is_public': true, 'o:is_open': true },
  });
  expect(set.status(), await set.text()).toBe(200);
  const setId = await idOf(set);
  // Created last to first, so the default newest-first order is not the title order.
  for (const { title, creator } of [...ITEMS].reverse()) {
    const created = await page.request.post('/api/items', {
      data: {
        'dcterms:title': literal(1, title),
        ...(creator === null ? {} : { 'dcterms:creator': literal(2, creator) }),
        'o:is_public': true,
        'o:item_set': [{ 'o:id': setId }],
      },
    });
    expect(created.status(), await created.text()).toBe(200);
  }
  return setId;
};

const fileOf = async (download: Download): Promise<string> => readFileSync(await download.path(), 'utf8');

const save = async (page: Page, name: string): Promise<string> => {
  const [download] = await Promise.all([page.waitForEvent('download'), page.getByRole('link', { name }).click()]);
  expect(download.suggestedFilename()).toMatch(/^items-\d{4}-\d{2}-\d{2}\.csv$/);
  return fileOf(download);
};

test('a set place downloads every item it counts as CSV, and the ticked rows alone', async ({
  page,
  diagnostics,
  baseURL,
}) => {
  const setId = await seed(page);
  await signIn(page, baseURL);

  await page.goto(`/#/items?set=${String(setId)}&sort=title`);
  const card = page.locator('.set-card');
  await expect(card.getByText('3 items', { exact: true })).toBeVisible();
  await expect(page.locator('.items-results tbody tr')).toHaveCount(3);
  await diagnostics.idle();
  await expectAxeBudget(page, 'items csv');
  await shoot(page, 'place');

  const every = await save(page, 'Download 3 items as CSV');
  expect(every.startsWith('﻿')).toBe(true);
  expect(every.slice(1).split('\r\n')).toEqual([
    'dcterms:title,dcterms:creator',
    '"CSV e2e alpha, the first ledger",रवि शंकर',
    'CSV e2e beta,"Ada ""the clerk"" Byron"',
    'CSV e2e gamma,',
    '',
  ]);

  await page.getByRole('checkbox', { name: `Select ${ITEMS[1].title}` }).check();
  await expect(page.getByRole('link', { name: 'Download the ticked item as CSV' })).toBeVisible();
  await shoot(page, 'ticked');
  const ticked = await save(page, 'Download the ticked item as CSV');
  expect(ticked.slice(1).split('\r\n')).toEqual([
    'dcterms:title,dcterms:creator',
    'CSV e2e beta,"Ada ""the clerk"" Byron"',
    '',
  ]);

  await page.getByRole('searchbox', { name: 'Search' }).fill('gamma');
  await expect(page.locator('.items-results tbody tr')).toHaveCount(1);
  const narrowed = await save(page, 'Download 1 item as CSV');
  expect(narrowed.slice(1).split('\r\n')).toEqual(['dcterms:title', 'CSV e2e gamma', '']);
  await expectClean(diagnostics, 'items csv');
});
