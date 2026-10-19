/**
 * A resource template drives the item form end to end: its rows, labels,
 * class and required rule reach the real server, and an item that holds values
 * outside its template survives an unchanged save byte for byte.
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

const getJson = async (request: APIRequestContext, path: string): Promise<unknown> => {
  const response = await request.get(path);
  expect(response.status(), `${path}: ${await response.text()}`).toBe(200);
  return response.json();
};

const postJson = async (request: APIRequestContext, path: string, data: Json): Promise<Json> => {
  const response = await request.post(path, { data });
  expect(response.status(), `${path}: ${await response.text()}`).toBe(200);
  const body: unknown = await response.json();
  if (!isJson(body)) {
    throw new Error(`${path}: expected an object`);
  }
  return body;
};

/** The id of the one member a `term` search names. */
const termId = async (request: APIRequestContext, resource: string, term: string): Promise<number> => {
  const found = await getJson(request, `/api/${resource}?term=${encodeURIComponent(term)}`);
  if (!Array.isArray(found) || found.length !== 1) {
    throw new Error(`/api/${resource}?term=${term}: expected one member`);
  }
  return idOf(found[0]);
};

const row = (property: number, extra: Json = {}): Json => ({
  'o:property': { 'o:id': property },
  ...extra,
});

const fieldLabels = (page: Page) => page.locator('.property-field-label');

test('choosing a template lays out its rows, applies its class and enforces its required row', async ({
  page,
  diagnostics,
}) => {
  const creator = await termId(page.request, 'properties', 'dcterms:creator');
  const title = await termId(page.request, 'properties', 'dcterms:title');
  const date = await termId(page.request, 'properties', 'dcterms:date');
  const sound = await termId(page.request, 'resource_classes', 'dctype:Sound');
  const label = `Oral history ${String(Date.now())}`;
  await postJson(page.request, '/api/resource_templates', {
    'o:label': label,
    'o:resource_class': { 'o:id': sound },
    'o:resource_template_property': [
      row(creator, { 'o:alternate_label': 'Interviewee', 'o:is_required': true }),
      row(title, { 'o:default_lang': 'fr' }),
      row(date, { 'o:alternate_label': 'Interview notes', 'o:is_private': true }),
    ],
  });

  await page.goto('/#/items/new');
  await page.getByRole('combobox', { name: 'Type', exact: true }).selectOption({ label });
  await expect(fieldLabels(page)).toHaveText(['Interviewee', 'Title', 'Interview notes']);
  await expect(page.getByText('Standard class: Sound recording')).toBeVisible();
  await page.getByText('Advanced').click();
  await expect(page.getByLabel('Class')).toHaveValue(String(sound));
  await expect(page.getByLabel('Title language')).toHaveValue('fr');
  await expect(page.getByRole('checkbox', { name: 'Interview notes value public' })).not.toBeChecked();

  await page.getByRole('textbox', { name: 'Title', exact: true }).fill('Entretien');
  await page.getByRole('button', { name: 'Add item' }).click();
  await expect(page.getByRole('alert')).toHaveText('The following field is required: Interviewee');
  await expect(page).toHaveURL(/#\/items\/new$/);

  await page.getByRole('textbox', { name: 'Interviewee', exact: true }).fill('Rosa Parks');
  await page.getByRole('button', { name: 'Add item' }).click();
  await expect(page).toHaveURL(/#\/items\/\d+$/);
  await expect(page.getByRole('heading', { name: 'Entretien' })).toBeVisible();
  await expectClean(diagnostics, 'template item create');

  const id = /#\/items\/(\d+)$/.exec(page.url())?.[1];
  const stored = await getJson(page.request, `/api/items/${String(id)}`);
  if (!isJson(stored)) {
    throw new Error('expected the created item');
  }
  expect(stored['o:resource_class']).toMatchObject({ 'o:id': sound });
  expect(stored['dcterms:title']).toMatchObject([{ '@value': 'Entretien', '@language': 'fr' }]);
  expect(stored['dcterms:date']).toBeUndefined();
});

test('the server refuses an item that misses a required template value', async ({ page }) => {
  const creator = await termId(page.request, 'properties', 'dcterms:creator');
  const template = await postJson(page.request, '/api/resource_templates', {
    'o:label': `Strict ${String(Date.now())}`,
    'o:resource_template_property': [
      row(creator, { 'o:alternate_label': 'Interviewee', 'o:is_required': true }),
    ],
  });
  const response = await page.request.post('/api/items', {
    data: { 'o:resource_template': { 'o:id': idOf(template) } },
  });
  expect(response.status()).toBe(422);
  expect(await response.json()).toEqual({
    errors: {
      'o:resource_template_property': [
        `The "${String(template['o:label'])}" resource template requires a "Interviewee" value`,
      ],
    },
  });
});

/** The stored document, without the timestamp a save is meant to move. */
const storedItem = async (request: APIRequestContext, id: number): Promise<Json> => {
  const document = await getJson(request, `/api/items/${String(id)}`);
  if (!isJson(document)) {
    throw new Error('expected an item document');
  }
  return Object.fromEntries(Object.entries(document).filter(([key]) => key !== 'o:modified'));
};

test('an item with values outside its template saves unchanged from the admin', async ({
  page,
  diagnostics,
}) => {
  const ids = Object.fromEntries(
    await Promise.all(
      [
        'dcterms:title',
        'dcterms:creator',
        'dcterms:subject',
        'dcterms:relation',
        'dcterms:date',
        'bibo:annotates',
      ].map(async (term) => [term, await termId(page.request, 'properties', term)] as const),
    ),
  );
  const property = (term: string): number => {
    const value = ids[term];
    if (value === undefined) {
      throw new Error(`no id for ${term}`);
    }
    return value;
  };
  const items = await getJson(page.request, '/api/items?sort_by=id&sort_order=asc');
  if (!Array.isArray(items)) {
    throw new Error('expected the seeded items');
  }
  const related = idOf(items[0]);
  const template = await postJson(page.request, '/api/resource_templates', {
    'o:label': `Narrow ${String(Date.now())}`,
    'o:resource_template_property': [
      row(property('dcterms:title')),
      row(property('dcterms:date'), { 'o:is_required': true }),
    ],
  });
  const created = await postJson(page.request, '/api/items', {
    'o:resource_template': { 'o:id': idOf(template) },
    'o:is_public': false,
    'dcterms:title': [
      { type: 'literal', property_id: property('dcterms:title'), '@value': 'Round trip', '@language': 'en' },
      { type: 'literal', property_id: property('dcterms:title'), '@value': 'Second title' },
    ],
    'dcterms:date': [{ type: 'literal', property_id: property('dcterms:date'), '@value': '1972' }],
    'dcterms:creator': [
      { type: 'literal', property_id: property('dcterms:creator'), '@value': 'Hidden', is_public: false },
    ],
    'dcterms:subject': [
      {
        type: 'uri',
        property_id: property('dcterms:subject'),
        '@id': 'http://example.org/oral-history',
        'o:label': 'Oral history',
        'o:lang': 'fr',
      },
    ],
    'dcterms:relation': [
      { type: 'resource:item', property_id: property('dcterms:relation'), value_resource_id: related },
    ],
    'bibo:annotates': [
      { type: 'literal', property_id: property('bibo:annotates'), '@value': 'Outside the template' },
    ],
  });
  const id = idOf(created);
  const before = await storedItem(page.request, id);

  await page.goto(`/#/items/${String(id)}`);
  await page.getByRole('button', { name: 'Edit this item' }).click();
  // The template rows come first, then the values the template does not list.
  await expect(fieldLabels(page)).toHaveText([
    'Title',
    'Date',
    'Creator',
    'Subject',
    'Relation',
    'annotates',
  ]);
  await page.getByRole('button', { name: 'Save' }).click();
  await expect(page.locator('[role=status]')).toHaveText('Saved.');
  await expectClean(diagnostics, 'unchanged save');

  expect(await storedItem(page.request, id)).toEqual(before);
  await page.request.delete(`/api/items/${String(id)}`);
});

test('the template list names a class past the first hundred', async ({ page }) => {
  const classes = await getJson(page.request, '/api/resource_classes?sort_by=id&sort_order=desc&per_page=1');
  const last: unknown = Array.isArray(classes) ? classes[0] : undefined;
  if (!isJson(last) || typeof last['o:label'] !== 'string') {
    throw new Error('expected a labelled class');
  }
  const counted = await page.request.get('/api/resource_classes?per_page=1');
  expect(Number(counted.headers()['omeka-s-total-results'])).toBeGreaterThan(100);
  const label = `Last class ${String(Date.now())}`;
  await postJson(page.request, '/api/resource_templates', {
    'o:label': label,
    'o:resource_class': { 'o:id': idOf(last) },
  });
  await page.goto('/#/resource-templates');
  const templateRow = page.getByRole('row').filter({ has: page.getByRole('link', { name: label }) });
  await expect(templateRow).toContainText(last['o:label']);
});
