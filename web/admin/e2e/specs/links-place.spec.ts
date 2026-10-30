/**
 * A linked record is a place: "Person A" is the Creator of two letters and the Subject of one
 * item set. The item page states each kind and field with its count; the Creator row opens the
 * two letters under Person A's card; a bulk change there reaches exactly those two; the set
 * list states each set's count; and the set card's "Linked from" opens on demand.
 * `LINKS_SCREENSHOTS` names a folder for the pictures.
 */
import { join } from 'node:path';
import type { APIRequestContext, Page } from '@playwright/test';
import { expect, expectClean, test } from '../fixtures';
import { expectAxeBudget } from '../harness/axe';
import { isArray, numberField, record } from '../harness/json';
import { readHarness } from '../harness/state';

const PERSON = 'Person A';
const LETTERS = ['Links e2e letter one', 'Links e2e letter two'] as const;
const UNRELATED = 'Links e2e unrelated letter';
const SUBJECT_SET = 'Links e2e people set';
const LETTER_SET = 'Links e2e letters set';

const shoot = async (page: Page, name: string): Promise<void> => {
  const folder = process.env.LINKS_SCREENSHOTS;
  if (folder !== undefined && folder !== '') {
    // The pointer leaves the control it last pressed, so the picture shows it at rest.
    await page.mouse.move(0, 0);
    await page.screenshot({ path: join(folder, `links-${name}.png`), fullPage: true });
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

const create = async (request: APIRequestContext, kind: 'items' | 'item_sets', body: object): Promise<number> => {
  const response = await request.post(`/api/${kind}`, { data: body });
  expect(response.status(), await response.text()).toBe(200);
  return numberField(record(await response.json(), kind), 'o:id', kind);
};

const literal = (property: number, text: string) => [{ type: 'literal', property_id: property, '@value': text }];
const pointer = (property: number, target: number) => [{ type: 'resource', property_id: property, value_resource_id: target }];

const isPublic = async (request: APIRequestContext, id: number): Promise<boolean> => {
  const item = record(await getJson(request, `/api/items/${String(id)}`), 'item');
  return item['o:is_public'] === true;
};

test('a record others link to opens as a place, and a bulk change there reaches exactly its items', async ({
  page,
  diagnostics,
  baseURL,
}) => {
  const request = page.request;
  const [title, creator, subject, isPartOf] = await Promise.all([
    propertyId(request, 'dcterms:title'),
    propertyId(request, 'dcterms:creator'),
    propertyId(request, 'dcterms:subject'),
    propertyId(request, 'dcterms:isPartOf'),
  ]);
  const letterSet = await create(request, 'item_sets', { 'dcterms:title': literal(title, LETTER_SET), 'o:is_public': true });
  const person = await create(request, 'items', {
    'dcterms:title': literal(title, PERSON),
    'dcterms:isPartOf': pointer(isPartOf, letterSet),
    'o:is_public': true,
  });
  const letters: number[] = [];
  for (const name of LETTERS) {
    letters.push(
      await create(request, 'items', {
        'dcterms:title': literal(title, name),
        'dcterms:creator': pointer(creator, person),
        'o:item_set': [{ 'o:id': letterSet }],
        'o:is_public': true,
      }),
    );
  }
  const unrelated = await create(request, 'items', { 'dcterms:title': literal(title, UNRELATED), 'o:is_public': true });
  const subjectSet = await create(request, 'item_sets', {
    'dcterms:title': literal(title, SUBJECT_SET),
    'dcterms:subject': pointer(subject, person),
    'o:is_public': true,
  });
  await signIn(page, baseURL);

  await page.goto(`/#/items/${String(person)}`);
  await expect(page.getByRole('heading', { name: 'Linked from', level: 2 })).toBeVisible();
  const creatorRow = page.getByRole('link', { name: 'Creator · 2 items' });
  await expect(creatorRow).toHaveAttribute('href', `#/items?links=${String(person)}&via=${String(creator)}`);
  const subjectRow = page.getByRole('button', { name: 'Subject · 1 item set' });
  await subjectRow.click();
  await expect(page.getByRole('link', { name: SUBJECT_SET })).toHaveAttribute('href', `#/items?set=${String(subjectSet)}`);
  await diagnostics.idle();
  await expectAxeBudget(page, 'item page linked from');
  await shoot(page, 'item-page');

  await creatorRow.click();
  await expect(page).toHaveURL(new RegExp(`#/items\\?links=${String(person)}&via=${String(creator)}$`));
  const card = page.locator('.links-card');
  await expect(card.getByRole('heading', { name: `Items that link to ${PERSON}`, level: 2 })).toBeVisible();
  await expect(card.getByText('As Creator')).toBeVisible();
  await expect(card.getByText('2 items', { exact: true })).toBeVisible();
  await expect(card.getByRole('link', { name: 'Open the record' })).toHaveAttribute('href', `#/items/${String(person)}`);
  await expect(page.getByText(`Links to: ${PERSON} (as Creator)`)).toBeVisible();
  const rows = page.locator('.items-results tbody tr');
  await expect(rows).toHaveCount(2);
  for (const name of LETTERS) {
    await expect(page.locator('.items-results').getByRole('link', { name })).toBeVisible();
  }
  await diagnostics.idle();
  await expectAxeBudget(page, 'links place');
  await shoot(page, 'place');

  await page.getByRole('button', { name: 'Change items…' }).click();
  const panel = page.getByRole('region', { name: 'Change items' });
  await expect(panel.getByRole('radio', { name: 'All 2 items that match' })).toBeChecked();
  await panel.getByRole('combobox', { name: 'Visibility' }).selectOption('private');
  await panel.getByRole('button', { name: 'Preview' }).click();
  await expect(panel.getByText('2 items will change.')).toBeVisible();
  await panel.getByRole('button', { name: 'Apply' }).click();
  await expect(panel.getByText('2 items changed.')).toBeVisible({ timeout: 15_000 });
  await expect(page.locator('.items-results').getByRole('button', { name: 'Private' })).toHaveCount(2);
  for (const id of letters) {
    expect(await isPublic(request, id), `letter ${String(id)} went private`).toBe(false);
  }
  expect(await isPublic(request, person), 'the linked record stayed public').toBe(true);
  expect(await isPublic(request, unrelated), 'an item outside the place stayed public').toBe(true);
  await expectClean(diagnostics, 'links place bulk change');

  await page.goto('/#/item-sets');
  await page.getByRole('combobox', { name: 'Sort' }).selectOption('most');
  const letterRow = page.getByRole('row').filter({ has: page.getByRole('link', { name: LETTER_SET, exact: true }) });
  await expect(letterRow.getByRole('link', { name: '2 items' })).toHaveAttribute('href', `#/items?set=${String(letterSet)}`);
  await page.getByRole('combobox', { name: 'Sort' }).selectOption('fewest');
  const subjectSetRow = page.getByRole('row').filter({ has: page.getByRole('link', { name: SUBJECT_SET, exact: true }) });
  await expect(subjectSetRow.getByRole('link', { name: '0 items' })).toHaveAttribute('href', `#/items?set=${String(subjectSet)}`);
  await diagnostics.idle();
  await expectAxeBudget(page, 'set list counts');
  await shoot(page, 'set-list');

  await letterRow.getByRole('link', { name: LETTER_SET, exact: true }).click();
  await expect(page).toHaveURL(new RegExp(`#/items\\?set=${String(letterSet)}$`));
  const setCard = page.locator('.set-card').first();
  await expect(setCard.getByRole('heading', { name: LETTER_SET, level: 2 })).toBeVisible();
  const toggle = setCard.getByRole('button', { name: 'Linked from' });
  await expect(toggle).toHaveAttribute('aria-expanded', 'false');
  await toggle.click();
  await expect(setCard.getByRole('link', { name: 'Part of · 1 item' })).toHaveAttribute(
    'href',
    `#/items?links=${String(letterSet)}&via=${String(isPartOf)}`,
  );
  await diagnostics.idle();
  await expectAxeBudget(page, 'set card linked from');
  await shoot(page, 'set-card');
  await expectClean(diagnostics, 'links place');
});
