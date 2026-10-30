/**
 * Shelf order: a set whose items have identifiers offers "Reference number" first in the sort list.
 * Choosing it puts the rows in number order (SB_1, SB_2, SB_10, then the item with none), keeps
 * the order in the address, and the walk follows it: row 2 says "2 of 4" and Next opens SB_10.
 * `SORT_SCREENSHOTS` names a folder for the pictures.
 */
import { join } from 'node:path';
import type { Page } from '@playwright/test';
import { expect, expectClean, test } from '../fixtures';
import { expectAxeBudget } from '../harness/axe';
import { isArray, numberField, record } from '../harness/json';
import { readHarness } from '../harness/state';

const SET = 'Sort e2e shelf box';
/** Created in this order, which no sort of their identifiers gives. */
const ITEMS = [
  { title: 'Sort e2e ledger ten', identifier: 'SB_10' },
  { title: 'Sort e2e ledger none', identifier: null },
  { title: 'Sort e2e ledger two', identifier: 'SB_2' },
  { title: 'Sort e2e ledger one', identifier: 'SB_1' },
] as const;
const SHELF_ORDER = ['Sort e2e ledger one', 'Sort e2e ledger two', 'Sort e2e ledger ten', 'Sort e2e ledger none'] as const;

const shoot = async (page: Page, name: string): Promise<void> => {
  const folder = process.env.SORT_SCREENSHOTS;
  if (folder !== undefined && folder !== '') {
    await page.mouse.move(0, 0);
    await page.screenshot({ path: join(folder, `items-sort-${name}.png`), fullPage: true });
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

const propertyId = async (page: Page, term: string): Promise<number> => {
  const answer: unknown = await (await page.request.get(`/api/properties?term=${term}`)).json();
  return numberField(record(isArray(answer) ? answer[0] : answer, term), 'o:id', term);
};

const create = async (page: Page, path: string, data: Record<string, unknown>): Promise<number> => {
  const response = await page.request.post(path, { data });
  expect(response.status(), await response.text()).toBe(200);
  return numberField(record(await response.json(), path), 'o:id', path);
};

test('a set with identifiers offers shelf order first, and the walk follows it', async ({
  page,
  diagnostics,
  baseURL,
}) => {
  const [title, identifier] = [await propertyId(page, 'dcterms:title'), await propertyId(page, 'dcterms:identifier')];
  const literal = (property: number, text: string) => [{ type: 'literal', property_id: property, '@value': text }];
  const setId = await create(page, '/api/item_sets', { 'dcterms:title': literal(title, SET), 'o:is_public': true });
  for (const item of ITEMS) {
    await create(page, '/api/items', {
      'dcterms:title': literal(title, item.title),
      ...(item.identifier === null ? {} : { 'dcterms:identifier': literal(identifier, item.identifier) }),
      'o:is_public': true,
      'o:item_set': [{ 'o:id': setId }],
    });
  }
  await signIn(page, baseURL);

  await page.goto(`/#/items?set=${String(setId)}`);
  const rows = page.locator('.items-results tbody tr');
  await expect(rows).toHaveCount(4);
  const sort = page.getByRole('combobox', { name: 'Sort', exact: true });
  await expect(sort.locator('option').first()).toHaveText('Reference number');
  await sort.selectOption({ label: 'Reference number' });

  await expect(page).toHaveURL(/sort=dcterms%3Aidentifier/);
  for (const [index, name] of SHELF_ORDER.entries()) {
    await expect(rows.nth(index)).toContainText(name);
  }
  await diagnostics.idle();
  await expectAxeBudget(page, 'items sorted by identifier');
  await shoot(page, 'set-by-identifier');

  await page.locator('.items-results').getByRole('link', { name: SHELF_ORDER[1] }).click();
  const walk = page.getByRole('navigation', { name: 'Place in the list' });
  await expect(walk.getByText(`2 of 4 in ${SET}`)).toBeVisible();
  await expect(walk.getByRole('link', { name: `Previous: ${SHELF_ORDER[0]}` })).toBeVisible();
  await walk.getByRole('link', { name: `Next: ${SHELF_ORDER[2]}` }).click();
  await expect(page.getByRole('heading', { name: SHELF_ORDER[2], level: 2 })).toBeVisible();
  await expect(walk.getByText(`3 of 4 in ${SET}`)).toBeVisible();
  await shoot(page, 'walk-row-3');
  await expectClean(diagnostics, 'items sort');
});
