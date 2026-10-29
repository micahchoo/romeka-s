/**
 * Reading and changing an item's values in one place. The item page lists the values once, each row
 * named as the item type names the field ("Name on the label"), with the term as quiet text; a value's
 * kind reads as the form says it ("Text", "URI"). Edit at the top swaps that list for the form in the
 * same place, above the media; Cancel and Save each bring the list back there.
 * `ITEMREAD_SCREENSHOTS` names a folder for the pictures.
 */
import { join } from 'node:path';
import type { APIRequestContext, Locator, Page } from '@playwright/test';
import { expect, expectClean, test } from '../fixtures';
import { expectAxeBudget } from '../harness/axe';
import { isArray, numberField, record } from '../harness/json';
import { readHarness } from '../harness/state';

const TYPE = 'Item read e2e object';
const TITLE = 'Item read e2e brass lamp';
const RENAMED = 'Item read e2e brass lamp, cleaned';
const LABEL = 'Name on the label';

const shoot = async (page: Page, name: string): Promise<void> => {
  const folder = process.env.ITEMREAD_SCREENSHOTS;
  if (folder !== undefined && folder !== '') {
    await page.mouse.move(0, 0);
    await page.screenshot({ path: join(folder, `itemread-${name}.png`), fullPage: true });
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

/** Whether `first` comes before `second` in the document. */
const isBefore = async (first: Locator, second: Locator): Promise<boolean> => {
  const handle = await second.elementHandle();
  return first.evaluate(
    (node, other) => (node.compareDocumentPosition(other) & Node.DOCUMENT_POSITION_FOLLOWING) !== 0,
    handle,
  );
};

test('the item page lists its values once by field name and edits them in the same place', async ({
  page,
  diagnostics,
  baseURL,
}) => {
  const request = page.request;
  const [title, subject, source] = await Promise.all([
    propertyId(request, 'dcterms:title'),
    propertyId(request, 'dcterms:subject'),
    propertyId(request, 'dcterms:source'),
  ]);
  const type = await create(request, 'resource_templates', {
    'o:label': TYPE,
    'o:resource_template_property': [{ 'o:property': { 'o:id': title }, 'o:alternate_label': LABEL }],
  });
  const id = await create(request, 'items', {
    'o:resource_template': { 'o:id': type },
    'o:is_public': true,
    'dcterms:subject': [{ type: 'literal', property_id: subject, '@value': 'Lighting', is_public: false }],
    'dcterms:source': [{ type: 'uri', property_id: source, '@id': 'https://example.org/lamp', 'o:label': 'Catalogue entry' }],
    'dcterms:title': [{ type: 'literal', property_id: title, '@value': TITLE }],
  });
  await signIn(page, baseURL);

  await page.goto(`/#/items/${String(id)}`);
  const values = page.locator('.value-list');
  const titleRow = values.locator('.value-field').filter({ hasText: LABEL });
  await expect(titleRow).toBeVisible();
  // The item type's name for the field leads, the term follows as quiet text, and no table is drawn.
  await expect(titleRow.locator('.value-field-name')).toHaveText(LABEL);
  await expect(titleRow.locator('.value-field-term')).toHaveText('dcterms:title');
  await expect(titleRow.getByText(TITLE, { exact: true })).toBeVisible();
  await expect(titleRow.getByText('Text', { exact: true })).toBeVisible();
  await expect(values.locator('table')).toHaveCount(0);
  await expect(page.getByText('literal', { exact: true })).toHaveCount(0);
  // The item type lists its fields first; the others follow under their own labels.
  await expect(values.locator('.value-field-name')).toHaveText([LABEL, 'Subject', 'Source']);
  const subjectRow = values.locator('.value-field').filter({ hasText: 'Subject' });
  await expect(subjectRow.getByText('Private', { exact: true })).toBeVisible();
  const sourceRow = values.locator('.value-field').filter({ hasText: 'Source' });
  await expect(sourceRow.getByRole('link', { name: 'Catalogue entry' })).toHaveAttribute('href', 'https://example.org/lamp');
  await expect(sourceRow.getByText('URI', { exact: true })).toBeVisible();
  await expect(sourceRow.getByText('Private', { exact: true })).toHaveCount(0);

  // Edit sits at the top of the page, and the values come before the media.
  const edit = page.getByRole('button', { name: 'Edit this item' });
  const facts = page.locator('.item-facts');
  expect(await isBefore(edit, facts)).toBe(true);
  const mediaHeading = page.getByRole('heading', { name: /^Media/ });
  expect(await isBefore(values, mediaHeading)).toBe(true);
  await diagnostics.idle();
  await expectAxeBudget(page, 'item read');
  await shoot(page, 'read');

  await edit.click();
  await expect(edit).toHaveAttribute('aria-expanded', 'true');
  const titleBox = page.getByRole('textbox', { name: LABEL, exact: true });
  await expect(titleBox).toHaveValue(TITLE);
  await expect(values).toHaveCount(0);
  expect(await isBefore(titleBox, mediaHeading)).toBe(true);
  await shoot(page, 'edit');

  await titleBox.fill('Not kept');
  await page.getByRole('button', { name: 'Cancel', exact: true }).click();
  await expect(titleRow.getByText(TITLE, { exact: true })).toBeVisible();
  await expect(edit).toHaveAttribute('aria-expanded', 'false');
  await expect(edit).toBeFocused();

  await edit.click();
  await page.getByRole('textbox', { name: LABEL, exact: true }).fill(RENAMED);
  await page.getByRole('button', { name: 'Save', exact: true }).click();
  await expect(page.locator('[role=status]')).toHaveText('Saved.');
  await expect(titleRow.getByText(RENAMED, { exact: true })).toBeVisible();
  await expect(page.getByRole('heading', { name: RENAMED, level: 2 })).toBeVisible();
  await expect(page.getByRole('textbox', { name: LABEL, exact: true })).toHaveCount(0);
  await shoot(page, 'saved');

  const saved = record(await (await request.get(`/api/items/${String(id)}`)).json(), 'item');
  expect(saved['o:title']).toBe(RENAMED);
  await expectClean(diagnostics, 'item read');
});
