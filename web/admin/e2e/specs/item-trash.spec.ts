/**
 * The item page's delete moves the item to the trash. "Trash person" is the Creator of two
 * letters: the confirm states "Creator · 2 items", the move lands on Items with "Deleted Trash
 * person." and Undo, and Undo puts the item back with both letters' Creator links.
 * `TRASH_SCREENSHOTS` names a folder for the pictures.
 */
import { join } from 'node:path';
import type { APIRequestContext, Page } from '@playwright/test';
import { expect, expectClean, test } from '../fixtures';
import { expectAxeBudget } from '../harness/axe';
import { isArray, numberField, record } from '../harness/json';
import { readHarness } from '../harness/state';

const PERSON = 'Trash person';
const LETTERS = ['Trash e2e letter one', 'Trash e2e letter two'] as const;

const shoot = async (page: Page, name: string): Promise<void> => {
  const folder = process.env.TRASH_SCREENSHOTS;
  if (folder !== undefined && folder !== '') {
    await page.mouse.move(0, 0);
    await page.screenshot({ path: join(folder, `trash-${name}.png`), fullPage: true });
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

const getJson = async (request: APIRequestContext, path: string): Promise<unknown> => {
  const response = await request.get(path);
  expect(response.status(), await response.text()).toBe(200);
  const body: unknown = await response.json();
  return body;
};

const propertyId = async (request: APIRequestContext, term: string): Promise<number> => {
  const found = await getJson(request, `/api/properties?term=${encodeURIComponent(term)}`);
  if (!isArray(found) || found.length !== 1) {
    throw new Error(`/api/properties?term=${term}: expected one member`);
  }
  return numberField(record(found[0], term), 'o:id', term);
};

const createItem = async (request: APIRequestContext, body: object): Promise<number> => {
  const response = await request.post('/api/items', { data: body });
  expect(response.status(), await response.text()).toBe(200);
  return numberField(record(await response.json(), 'item'), 'o:id', 'item');
};

const literal = (property: number, text: string) => [{ type: 'literal', property_id: property, '@value': text }];
const pointer = (property: number, target: number) => [{ type: 'resource', property_id: property, value_resource_id: target }];

/** The ids an item's Creator values point at. */
const creatorsOf = async (request: APIRequestContext, id: number): Promise<readonly number[]> => {
  const item = record(await getJson(request, `/api/items/${String(id)}`), 'item');
  const values = item['dcterms:creator'];
  return isArray(values) ? values.map((value) => numberField(record(value, 'creator'), 'value_resource_id', 'creator')) : [];
};

test('an item deleted from its page goes to the trash, and Undo brings back the links to it', async ({
  page,
  diagnostics,
  baseURL,
}) => {
  const request = page.request;
  const [title, creator] = await Promise.all([propertyId(request, 'dcterms:title'), propertyId(request, 'dcterms:creator')]);
  const person = await createItem(request, { 'dcterms:title': literal(title, PERSON), 'o:is_public': true });
  const letters: number[] = [];
  for (const name of LETTERS) {
    letters.push(
      await createItem(request, {
        'dcterms:title': literal(title, name),
        'dcterms:creator': pointer(creator, person),
        'o:is_public': true,
      }),
    );
  }
  await signIn(page, baseURL);

  await page.goto(`/#/items/${String(person)}`);
  await page.getByRole('button', { name: 'Delete this item' }).click();
  const confirm = page.getByRole('region', { name: 'Move to the trash' });
  await expect(confirm.getByText(`Move ${PERSON} to the trash?`)).toBeVisible();
  await expect(confirm.getByRole('listitem')).toHaveText('Creator · 2 items');
  await expect(confirm.getByText('Their links come back if you restore it.')).toBeVisible();
  await expect(confirm.getByText(/The trash keeps it for 30 days\./)).toBeVisible();
  await expect(confirm.getByRole('button', { name: 'Move to the trash' })).toBeFocused();
  await diagnostics.idle();
  await expectAxeBudget(page, 'item trash confirm');
  await shoot(page, 'confirm');

  await confirm.getByRole('button', { name: 'Move to the trash' }).click();
  await expect(page).toHaveURL(/#\/items$/);
  await expect(page.getByText(`Deleted ${PERSON}.`)).toBeVisible();
  expect((await request.get(`/api/items/${String(person)}`)).status(), 'the item is gone').toBe(404);
  for (const id of letters) {
    expect(await creatorsOf(request, id), `letter ${String(id)} lost its Creator link`).toEqual([]);
  }
  await diagnostics.idle();
  await expectAxeBudget(page, 'items trashed notice');
  await shoot(page, 'notice');

  await page.getByRole('button', { name: 'Undo' }).click();
  await expect(page.getByRole('link', { name: `${PERSON} is back.` })).toHaveAttribute('href', `#/items/${String(person)}`);
  await expect(page.locator('.items-results').getByRole('link', { name: PERSON, exact: true })).toBeVisible();
  for (const id of letters) {
    expect(await creatorsOf(request, id), `letter ${String(id)} links to ${PERSON} again`).toEqual([person]);
  }
  await diagnostics.idle();
  await shoot(page, 'back');
  await expectClean(diagnostics, 'item trash');
});
