/**
 * The item page's History. A letter dated "1950" is redated "c. 1950" through the API; History
 * opens on the page and states "Date: 1950 → c. 1950" under the date and the name of the person who made it.
 * "Put this value back" saves the item with the old date, the page shows "1950" again, and the
 * put-back is the newest entry. `HISTORY_SCREENSHOTS` names a folder for the pictures.
 */
import { join } from 'node:path';
import type { APIRequestContext, Page } from '@playwright/test';
import { expect, expectClean, test } from '../fixtures';
import { expectAxeBudget } from '../harness/axe';
import { isArray, numberField, record } from '../harness/json';
import { readHarness } from '../harness/state';

const TITLE = 'History e2e letter';

const shoot = async (page: Page, name: string): Promise<void> => {
  const folder = process.env.HISTORY_SCREENSHOTS;
  if (folder !== undefined && folder !== '') {
    await page.mouse.move(0, 0);
    await page.screenshot({ path: join(folder, `history-${name}.png`), fullPage: true });
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

const propertyId = async (request: APIRequestContext, term: string): Promise<number> => {
  const response = await request.get(`/api/properties?term=${encodeURIComponent(term)}`);
  expect(response.status(), await response.text()).toBe(200);
  const found: unknown = await response.json();
  if (!isArray(found) || found.length !== 1) {
    throw new Error(`/api/properties?term=${term}: expected one member`);
  }
  return numberField(record(found[0], term), 'o:id', term);
};

const literal = (property: number, text: string) => [{ type: 'literal', property_id: property, '@value': text }];

test('History states a change and puts the old value back as an edit of its own', async ({
  page,
  diagnostics,
  baseURL,
}) => {
  const request = page.request;
  const [title, date] = await Promise.all([propertyId(request, 'dcterms:title'), propertyId(request, 'dcterms:date')]);
  const made = await request.post('/api/items', {
    data: { 'dcterms:title': literal(title, TITLE), 'dcterms:date': literal(date, '1950'), 'o:is_public': true },
  });
  expect(made.status(), await made.text()).toBe(200);
  const id = numberField(record(await made.json(), 'item'), 'o:id', 'item');
  const redated = await request.patch(`/api/items/${String(id)}`, {
    data: { 'dcterms:title': literal(title, TITLE), 'dcterms:date': literal(date, 'c. 1950') },
  });
  expect(redated.status(), await redated.text()).toBe(200);
  await signIn(page, baseURL);

  await page.goto(`/#/items/${String(id)}`);
  await page.locator('summary', { hasText: 'History' }).click();
  const history = page.locator('.item-history');
  const newest = history.locator('.item-history-entries > li').first();
  await expect(newest.getByText('Date: 1950 → c. 1950')).toBeVisible();
  await expect(newest.locator('.item-history-when')).toHaveText(/ · E2E Admin$/);
  await diagnostics.idle();
  await expectAxeBudget(page, 'item history');
  await shoot(page, 'open');

  await newest.getByRole('button', { name: 'Put this value back' }).click();
  await expect(history.getByRole('status')).toHaveText('The value is back. The newest change at the top says so.');
  await expect(newest.getByText('Date: c. 1950 → 1950')).toBeVisible();
  await expect(page.locator('.value-list').getByText('1950', { exact: true })).toBeVisible();
  const item = record(await (await request.get(`/api/items/${String(id)}`)).json(), 'item');
  const dates = item['dcterms:date'];
  expect(isArray(dates) ? dates.map((value) => record(value, 'date')['@value']) : []).toEqual(['1950']);
  await diagnostics.idle();
  await shoot(page, 'put-back');
  await expectClean(diagnostics, 'item history');
});

/** Run a bulk delete of one set into the trash and wait until it is done. */
const trashSet = async (request: APIRequestContext, set: number): Promise<void> => {
  const made = await request.post('/api/romeka/bulk', {
    data: { query: { id: [set] }, change: { kind: 'delete', of: 'item_sets' } },
  });
  expect(made.status(), await made.text()).toBe(201);
  const run = numberField(record(await made.json(), 'bulk'), 'id', 'bulk');
  const started = await request.post(`/api/romeka/bulk/${String(run)}/run`);
  expect(started.status(), await started.text()).toBe(200);
  await expect
    .poll(async () => record(await (await request.get(`/api/romeka/bulk/${String(run)}`)).json(), 'bulk')['status'])
    .toBe('done');
};

test('History names a set in the trash and a set deleted for good', async ({ page, diagnostics, baseURL }) => {
  const request = page.request;
  const title = await propertyId(request, 'dcterms:title');
  const set = async (name: string): Promise<number> => {
    const made = await request.post('/api/item_sets', { data: { 'dcterms:title': literal(title, name) } });
    expect(made.status(), await made.text()).toBe(200);
    return numberField(record(await made.json(), 'set'), 'o:id', 'set');
  };
  const [trashed, gone] = [await set('History e2e trashed set'), await set('History e2e gone set')];
  const made = await request.post('/api/items', {
    data: {
      'dcterms:title': literal(title, 'History e2e member'),
      'o:item_set': [{ 'o:id': trashed }, { 'o:id': gone }],
      'o:is_public': true,
    },
  });
  expect(made.status(), await made.text()).toBe(200);
  const id = numberField(record(await made.json(), 'item'), 'o:id', 'item');
  await trashSet(request, trashed);
  const deleted = await request.delete(`/api/item_sets/${String(gone)}`);
  expect(deleted.status(), await deleted.text()).toBe(200);
  await signIn(page, baseURL);

  await page.goto(`/#/items/${String(id)}`);
  await page.locator('summary', { hasText: 'History' }).click();
  const entries = page.locator('.item-history .item-history-entries > li');
  const goneName = `Deleted set ${String(gone)}`;
  await expect(entries.nth(0).getByText(`Item sets: ${goneName} → (none)`)).toBeVisible();
  await expect(
    entries.nth(1).getByText(`Item sets: History e2e trashed set (in the trash), ${goneName} → ${goneName}`),
  ).toBeVisible();
  await expect(entries.nth(1).locator('.item-history-when')).toHaveText(/ · E2E Admin · Bulk change \d+$/);
  for (const index of [0, 1]) {
    await expect(
      entries.nth(index).getByRole('button', { name: 'Put this value back' }),
      'a set that is gone cannot be joined again',
    ).toHaveCount(0);
  }
  await diagnostics.idle();
  await expectAxeBudget(page, 'item history gone sets');
  await shoot(page, 'gone-sets');
  await expectClean(diagnostics, 'item history gone sets');
});
