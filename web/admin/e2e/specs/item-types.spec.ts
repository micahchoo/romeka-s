/**
 * Item types against the real server: a template made raw through the API
 * opens in the editor and saves back byte for byte, a drag reorders the rows,
 * a field no standard names is created in the local vocabulary, and the
 * Fields page finds a term from plain words.
 */
import type { APIRequestContext, Page } from '@playwright/test';
import { expect, expectClean, test } from '../fixtures';

type Json = Record<string, unknown>;

const isJson = (value: unknown): value is Json =>
  typeof value === 'object' && value !== null && !Array.isArray(value);

const idOf = (value: unknown): number => {
  if (isJson(value) && typeof value['o:id'] === 'number') {
    return value['o:id'];
  }
  throw new Error(`no o:id in ${JSON.stringify(value)}`);
};

const getText = async (request: APIRequestContext, path: string): Promise<string> => {
  const response = await request.get(path);
  expect(response.status(), path).toBe(200);
  return response.text();
};

const termId = async (
  request: APIRequestContext,
  resource: string,
  term: string,
): Promise<number> => {
  const found: unknown = JSON.parse(
    await getText(request, `/api/${resource}?term=${encodeURIComponent(term)}`),
  );
  if (!Array.isArray(found) || found.length !== 1) {
    throw new Error(`/api/${resource}?term=${term}: expected one member`);
  }
  return idOf(found[0]);
};

const createTemplate = async (request: APIRequestContext, data: Json): Promise<number> => {
  const response = await request.post('/api/resource_templates', { data });
  expect(response.status(), await response.text()).toBe(200);
  return idOf(await response.json());
};

const rowTerms = async (page: Page, id: number): Promise<readonly number[]> => {
  const stored: unknown = JSON.parse(
    await getText(page.request, `/api/resource_templates/${String(id)}`),
  );
  if (!isJson(stored) || !Array.isArray(stored['o:resource_template_property'])) {
    throw new Error('expected a template');
  }
  return stored['o:resource_template_property'].map((row: unknown) =>
    isJson(row) ? idOf(row['o:property']) : 0,
  );
};

const saveType = async (page: Page): Promise<void> => {
  await page.getByRole('button', { name: 'Save item type' }).click();
  await expect(page.locator('[role=status]').filter({ hasText: 'Saved.' })).toBeVisible();
};

test('a template made raw through the API saves back unchanged from the editor', async ({
  page,
  diagnostics,
}) => {
  const [title, creator, date, created, subject] = await Promise.all(
    ['dcterms:title', 'dcterms:creator', 'dcterms:date', 'dcterms:created', 'dcterms:subject'].map(
      (term) => termId(page.request, 'properties', term),
    ),
  );
  const id = await createTemplate(page.request, {
    'o:label': `Expert ${String(Date.now())}`,
    'o:title_property': { 'o:id': subject },
    'o:resource_template_property': [
      { 'o:property': { 'o:id': title }, 'o:data_type': ['uri', 'literal', 'resource:item'] },
      {
        'o:property': { 'o:id': creator },
        'o:alternate_label': '',
        'o:alternate_comment': 'Who, if known.',
      },
      { 'o:property': { 'o:id': date }, 'o:alternate_label': 'Date', 'o:is_private': true },
      { 'o:property': { 'o:id': created }, 'o:alternate_label': 'Date', 'o:default_lang': 'ga' },
      {
        'o:property': { 'o:id': subject },
        'o:data_type': ['customvocab:999', 'numeric:integer'],
        'o:is_required': true,
      },
    ],
  });
  const before = await getText(page.request, `/api/resource_templates/${String(id)}`);

  await page.goto(`/#/resource-templates/${String(id)}`);
  await expect(page.getByText('No standard class.')).toBeVisible();
  await expect(
    page.getByText('customvocab:999 (not installed), numeric:integer (not installed)'),
  ).toBeVisible();
  await saveType(page);
  await expectClean(diagnostics, 'expert template save');

  expect(await getText(page.request, `/api/resource_templates/${String(id)}`)).toBe(before);
  await page.request.delete(`/api/resource_templates/${String(id)}`);
});

test('dragging a field by its handle reorders the rows', async ({ page, diagnostics }) => {
  const [title, creator, date] = await Promise.all(
    ['dcterms:title', 'dcterms:creator', 'dcterms:date'].map((term) =>
      termId(page.request, 'properties', term),
    ),
  );
  const id = await createTemplate(page.request, {
    'o:label': `Drag ${String(Date.now())}`,
    'o:resource_template_property': [title, creator, date].map((property) => ({
      'o:property': { 'o:id': property },
    })),
  });
  await page.goto(`/#/resource-templates/${String(id)}`);
  const handles = page.locator('.type-field > .reorder-handle');
  await handles.nth(2).dragTo(page.locator('.type-field').first());
  await expect(page.locator('.type-field-name').first()).toHaveAttribute('placeholder', 'Date');
  await saveType(page);
  await expectClean(diagnostics, 'drag reorder');
  expect(await rowTerms(page, id)).toEqual([date, title, creator]);
  await page.request.delete(`/api/resource_templates/${String(id)}`);
});

test('a field no standard names is created in the local vocabulary', async ({
  page,
  diagnostics,
}) => {
  const id = await createTemplate(page.request, { 'o:label': `Local ${String(Date.now())}` });
  const name = `Shelf mark ${String(Date.now())}`;
  await page.goto(`/#/resource-templates/${String(id)}`);
  await page.getByRole('searchbox', { name: 'Add a field' }).fill(name);
  await page.getByRole('button', { name: `Create a new field “${name}”` }).click();
  await expect(page.locator('.type-field-term code').first()).toHaveText(/^local:shelfMark\d+$/);
  await saveType(page);
  await expectClean(diagnostics, 'local field create');

  const vocabularies: unknown = JSON.parse(
    await getText(page.request, '/api/vocabularies?prefix=local'),
  );
  const local: unknown = Array.isArray(vocabularies)
    ? vocabularies.find((entry: unknown) => isJson(entry) && entry['o:prefix'] === 'local')
    : undefined;
  expect(isJson(local) ? local['o:namespace_uri'] : null).toBe(
    `${new URL(page.url()).origin}/vocab/local/`,
  );
  const [property] = await rowTerms(page, id);
  const stored: unknown = JSON.parse(
    await getText(page.request, `/api/properties/${String(property)}`),
  );
  expect(isJson(stored) ? stored['o:label'] : null).toBe(name);
  await page.request.delete(`/api/resource_templates/${String(id)}`);
});

test('the Fields page finds the creator from plain words', async ({ page, diagnostics }) => {
  await page.goto('/#/fields');
  await page.getByLabel('Find a field').fill('who wrote it');
  await expect(page.locator('.fields-entry').first()).toContainText('dcterms:creator');
  await expectClean(diagnostics, 'fields search');
});
