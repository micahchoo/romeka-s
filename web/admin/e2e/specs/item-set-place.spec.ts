/**
 * A set is a place: open it from the set list, see its card with the count over its items, read
 * the set on an item's page, narrow the place with a search, bulk remove the items from the set,
 * and see the count fall to 0 with the empty state that says how items get in.
 * `SET_SCREENSHOTS` names a folder for the pictures.
 */
import { join } from 'node:path';
import type { Page } from '@playwright/test';
import { expect, expectClean, test } from '../fixtures';
import { expectAxeBudget } from '../harness/axe';
import { readHarness } from '../harness/state';

const SET = 'Place e2e harbour letters';
const ITEMS = ['Place e2e alpha', 'Place e2e beta', 'Place e2e gamma'] as const;

const shoot = async (page: Page, name: string): Promise<void> => {
  const folder = process.env.SET_SCREENSHOTS;
  if (folder !== undefined && folder !== '') {
    await page.screenshot({ path: join(folder, `set-${name}.png`), fullPage: true });
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

const makeSet = async (page: Page): Promise<number> => {
  const created = await page.request.post('/api/item_sets', {
    data: {
      'dcterms:title': literal(1, SET),
      'dcterms:description': literal(
        4,
        'Letters between the harbour office and the shipping agents, sorted by year. '.repeat(6),
      ),
      'o:is_public': true,
      'o:is_open': true,
    },
  });
  expect(created.status(), await created.text()).toBe(200);
  const body: unknown = await created.json();
  const id = typeof body === 'object' && body !== null && 'o:id' in body ? body['o:id'] : null;
  expect(typeof id).toBe('number');
  return Number(id);
};

const makeItems = async (page: Page, setId: number): Promise<void> => {
  for (const title of ITEMS) {
    const created = await page.request.post('/api/items', {
      data: { 'dcterms:title': literal(1, title), 'o:is_public': true, 'o:item_set': [{ 'o:id': setId }] },
    });
    expect(created.status(), await created.text()).toBe(200);
  }
};

test('a set opens as a place, its items leave it in bulk, and the place says how to fill it again', async ({
  page,
  diagnostics,
  baseURL,
}) => {
  const setId = await makeSet(page);
  await makeItems(page, setId);
  await signIn(page, baseURL);

  await page.goto('/#/item-sets');
  await page.getByRole('link', { name: SET }).click();
  await expect(page).toHaveURL(new RegExp(`#/items\\?set=${String(setId)}$`));

  const card = page.locator('.set-card');
  await expect(card.getByRole('heading', { name: SET, level: 2 })).toBeVisible();
  await expect(card.getByText('3 items', { exact: true })).toBeVisible();
  await expect(card.getByText('Open to additions')).toBeVisible();
  await expect(card.getByRole('button', { name: 'Show all' })).toBeVisible();
  await expect(page.locator('.items-results tbody tr')).toHaveCount(3);
  await diagnostics.idle();
  await expectAxeBudget(page, 'set place');
  await shoot(page, 'card-table');

  // Back leaves the place for the set list: the set's address replaced its own entry.
  await page.goBack();
  await expect(page).toHaveURL(/#\/item-sets$/);
  await page.goForward();
  await expect(card.getByText('3 items', { exact: true })).toBeVisible();

  await page.locator('.items-results').getByRole('link', { name: ITEMS[0] }).click();
  const setLink = page.getByRole('link', { name: SET });
  await expect(setLink).toHaveAttribute('href', `#/items?set=${String(setId)}`);
  await shoot(page, 'item-page-rows');
  await setLink.click();
  await expect(card.getByText('3 items', { exact: true })).toBeVisible();

  await page.getByRole('searchbox', { name: 'Search' }).fill('alpha');
  await expect(card.getByText('1 matches the other filters')).toBeVisible();
  await expect(page.locator('.items-results tbody tr')).toHaveCount(1);
  await shoot(page, 'card-narrowed');
  await page.getByRole('button', { name: 'Remove filter: Search: alpha' }).click();
  await expect(page.locator('.items-results tbody tr')).toHaveCount(3);

  await page.getByRole('button', { name: 'Change items…' }).click();
  const panel = page.getByRole('region', { name: 'Change items' });
  await expect(panel.getByRole('radio', { name: 'All 3 items that match' })).toBeChecked();
  await panel.getByRole('combobox', { name: 'Remove from item set' }).selectOption({ label: SET });
  await panel.getByRole('button', { name: 'Preview' }).click();
  await expect(panel.getByText('3 items will change.')).toBeVisible();
  await panel.getByRole('button', { name: 'Apply' }).click();
  await expect(panel.getByText('3 items changed.')).toBeVisible({ timeout: 15_000 });

  await expect(card.getByText('0 items', { exact: true })).toBeVisible();
  await expect(page.getByText('No items in this set yet.')).toBeVisible();
  await expect(page.getByRole('link', { name: 'Go to Items' })).toHaveAttribute('href', '#/items');
  await diagnostics.idle();
  await expectAxeBudget(page, 'set place empty');
  await shoot(page, 'card-empty');
  await expectClean(diagnostics, 'set place');
});
