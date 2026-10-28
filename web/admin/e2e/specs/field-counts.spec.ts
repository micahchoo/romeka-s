/**
 * An item type states how complete its fields are: three letters of one type, two with a date.
 * The type's page says "2 of 3 items have a value." beside Date, and its link opens Items filtered
 * to the one letter without a date; the list's total is the number the link stated. The field
 * filter then narrows the links place too. `FIELDS_SCREENSHOTS` names a folder for the pictures.
 */
import { join } from 'node:path';
import type { APIRequestContext, Page } from '@playwright/test';
import { expect, expectClean, test } from '../fixtures';
import { expectAxeBudget } from '../harness/axe';
import { isArray, numberField, record } from '../harness/json';
import { readHarness } from '../harness/state';

const TYPE = 'Fields e2e letter';
const DATED = ['Fields e2e dated one', 'Fields e2e dated two'] as const;
const UNDATED = 'Fields e2e undated';
const PERSON = 'Fields e2e writer';

const shoot = async (page: Page, name: string): Promise<void> => {
  const folder = process.env.FIELDS_SCREENSHOTS;
  if (folder !== undefined && folder !== '') {
    await page.mouse.move(0, 0);
    await page.screenshot({ path: join(folder, `fields-${name}.png`), fullPage: true });
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

const create = async (request: APIRequestContext, kind: string, body: object): Promise<number> => {
  const response = await request.post(`/api/${kind}`, { data: body });
  expect(response.status(), await response.text()).toBe(200);
  return numberField(record(await response.json(), kind), 'o:id', kind);
};

const literal = (property: number, text: string) => [{ type: 'literal', property_id: property, '@value': text }];
const pointer = (property: number, target: number) => [{ type: 'resource', property_id: property, value_resource_id: target }];

test("an item type's field counts open the items that lack the field", async ({ page, diagnostics, baseURL }) => {
  const request = page.request;
  const [title, date, creator] = await Promise.all([
    propertyId(request, 'dcterms:title'),
    propertyId(request, 'dcterms:date'),
    propertyId(request, 'dcterms:creator'),
  ]);
  const type = await create(request, 'resource_templates', {
    'o:label': TYPE,
    'o:resource_template_property': [{ 'o:property': { 'o:id': title } }, { 'o:property': { 'o:id': date } }],
  });
  const person = await create(request, 'items', { 'dcterms:title': literal(title, PERSON), 'o:is_public': true });
  const letter = (name: string, extra: object) =>
    create(request, 'items', {
      'o:resource_template': { 'o:id': type },
      'dcterms:title': literal(title, name),
      'o:is_public': true,
      ...extra,
    });
  await letter(DATED[0], { 'dcterms:date': literal(date, '1901'), 'dcterms:creator': pointer(creator, person) });
  await letter(DATED[1], { 'dcterms:date': literal(date, '1902') });
  await letter(UNDATED, { 'dcterms:creator': pointer(creator, person) });
  // Another type's undated item is not one of this type's.
  await create(request, 'items', { 'dcterms:title': literal(title, 'Fields e2e other type'), 'o:is_public': true });
  await signIn(page, baseURL);

  await page.goto(`/#/resource-templates/${String(type)}`);
  const rows = page.locator('.type-field-list > li');
  const dateRow = rows.filter({ has: page.getByRole('textbox', { name: 'Name on the form for dcterms:date' }) });
  const titleRow = rows.filter({ has: page.getByRole('textbox', { name: 'Name on the form for dcterms:title' }) });
  await expect(dateRow.getByText('2 of 3 items have a value.')).toBeVisible();
  await expect(titleRow.getByText('3 of 3 items have a value.')).toBeVisible();
  await expect(titleRow.getByRole('link', { name: /without a value/ })).toHaveCount(0);
  const open = dateRow.getByRole('link', { name: 'Open the 1 without a value' });
  await diagnostics.idle();
  await expectAxeBudget(page, 'item type field counts');
  await shoot(page, 'item-type');

  await open.click();
  await expect(page).toHaveURL(new RegExp(`#/items\\?template=${String(type)}&field=${String(date)}%3Aempty$`));
  await expect(page.getByText('Date: empty')).toBeVisible();
  await expect(page.getByText('1 item matches')).toBeVisible();
  const results = page.locator('.items-results');
  await expect(results.locator('tbody tr')).toHaveCount(1);
  await expect(results.getByRole('link', { name: UNDATED })).toBeVisible();
  await expect(page.getByRole('combobox', { name: 'Field', exact: true })).toHaveValue(String(date));
  await diagnostics.idle();
  await expectAxeBudget(page, 'items field filter');
  await shoot(page, 'items-empty');

  await page.getByRole('combobox', { name: 'Field is' }).selectOption('filled');
  await expect(page.getByText('Date: has a value')).toBeVisible();
  await expect(page.getByText('2 items match')).toBeVisible();
  for (const name of DATED) {
    await expect(results.getByRole('link', { name })).toBeVisible();
  }

  await page.goto(`/#/items?links=${String(person)}&via=${String(creator)}&field=${String(date)}%3Aempty`);
  await expect(page.getByText(`Links to: ${PERSON} (as Creator)`)).toBeVisible();
  await expect(page.getByText('Date: empty')).toBeVisible();
  await expect(results.locator('tbody tr')).toHaveCount(1);
  await expect(results.getByRole('link', { name: UNDATED })).toBeVisible();
  await diagnostics.idle();
  await shoot(page, 'links-and-field');
  await expectClean(diagnostics, 'field counts');
});
