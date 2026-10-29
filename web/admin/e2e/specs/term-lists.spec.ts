/**
 * Term lists end to end: a list of words is created on the Term lists page, an
 * item type's field accepts it from the Accepts picker, the live preview shows
 * the term picker, a new item of that type picks a word from it, the stored
 * value keeps the list's type, an unchanged save keeps the item byte for byte,
 * the list page counts that item, and the public item page shows the word.
 */
import type { APIRequestContext } from '@playwright/test';
import { expect, expectClean, test } from '../fixtures';
import { SITE_SLUG } from '../harness/seed';
import { isArray, numberField, record, stringField } from '../harness/json';

const LIST = 'E2E places';
const TYPE = 'E2E place type';
const ITEM = 'E2E placed item';

const propertyId = async (
  request: APIRequestContext,
  term: string,
): Promise<number> => {
  const found: unknown = await (await request.get(`/api/properties?term=${term}`)).json();
  return numberField(record(isArray(found) ? found[0] : null, term), 'o:id', term);
};

test('an item type field accepts a term list and its value renders on the site', async ({
  page,
  diagnostics,
}) => {
  await page.goto('/#/term-lists');
  await page.getByRole('link', { name: 'Add term list' }).click();
  await page.getByLabel('Label', { exact: true }).fill(LIST);
  await page.getByLabel('Words, one per line').fill('Cork\nDublin\nDún Laoghaire');
  await page.getByLabel('Language of the words').fill('en');
  await page.getByRole('button', { name: 'Add term list' }).click();

  await expect(page).toHaveURL(/#\/term-lists$/);
  const row = page.getByRole('row').filter({ hasText: LIST });
  await expect(row).toContainText('A list of words');
  await expect(row).toContainText('3');
  await expectClean(diagnostics, 'term list create');

  const lists: unknown = await (await page.request.get('/api/custom_vocabs')).json();
  const listId = (isArray(lists) ? lists : [])
    .map((entry) => record(entry, 'term list'))
    .filter((entry) => stringField(entry, 'o:label', 'term list') === LIST)
    .map((entry) => numberField(entry, 'o:id', 'term list'))[0];
  expect(listId).toBeDefined();

  const [title, spatial] = await Promise.all(
    ['dcterms:title', 'dcterms:spatial'].map((term) => propertyId(page.request, term)),
  );
  const created = await page.request.post('/api/resource_templates', {
    data: {
      'o:label': TYPE,
      'o:resource_template_property': [title, spatial].map((id) => ({ 'o:property': { 'o:id': id } })),
    },
  });
  expect(created.status(), await created.text()).toBe(200);
  const typeId = numberField(record(await created.json(), 'template'), 'o:id', 'template');

  // The Accepts picker groups the term lists under their own legend, by label.
  await page.goto(`/#/resource-templates/${String(typeId)}`);
  await page.getByRole('button', { name: 'Change what Spatial Coverage accepts' }).click();
  const accepted = page
    .getByRole('group', { name: 'What Spatial Coverage accepts' })
    .getByRole('group', { name: 'Term lists' });
  await accepted.getByRole('checkbox', { name: LIST }).check();
  await expect(page.locator('.type-field-accepts').nth(1)).toContainText(`Accepts: Term list: ${LIST}`);

  // The live preview is the item form, so the field already shows the picker.
  const preview = page.getByRole('form', { name: `Preview of the form for a new ${TYPE}` });
  await expect(
    preview.getByRole('combobox', { name: `Spatial Coverage Term list: ${LIST}` }),
  ).toBeVisible();
  await page.getByRole('button', { name: 'Save item type' }).click();
  await expect(page.locator('[role=status]').filter({ hasText: 'Saved.' })).toBeVisible();
  await expectClean(diagnostics, 'item type accepts a term list');

  const template = record(
    await (await page.request.get(`/api/resource_templates/${String(typeId)}`)).json(),
    'template',
  );
  const rows = template['o:resource_template_property'];
  expect(isArray(rows) ? rows.map((entry) => record(entry, 'row')['o:data_type']) : null).toEqual([
    [],
    [`customvocab:${String(listId)}`],
  ]);

  // An item of that type picks its place from the list.
  await page.goto('/#/items/new');
  await page.getByRole('combobox', { name: 'Item type', exact: true }).selectOption({ label: TYPE });
  await page.getByRole('textbox', { name: 'Title', exact: true }).fill(ITEM);
  const picker = page.getByRole('combobox', { name: `Spatial Coverage Term list: ${LIST}` });
  await picker.fill('laog');
  await expect(page.locator('.term-picker').getByRole('option')).toHaveText(['Dún Laoghaire']);
  await picker.press('ArrowDown');
  await picker.press('Enter');
  await expect(picker).toHaveValue('Dún Laoghaire');
  await page.getByRole('button', { name: 'Add item' }).click();
  await expect(page).toHaveURL(/#\/items\/\d+$/);
  await expectClean(diagnostics, 'item create with a term');

  const itemId = Number(/#\/items\/(\d+)$/.exec(page.url())?.[1]);
  const stored = async (): Promise<Record<string, unknown>> => {
    const document = record(await (await page.request.get(`/api/items/${String(itemId)}`)).json(), 'item');
    return Object.fromEntries(Object.entries(document).filter(([key]) => key !== 'o:modified'));
  };
  const before = await stored();
  expect(before['dcterms:spatial']).toMatchObject([
    { type: `customvocab:${String(listId)}`, '@value': 'Dún Laoghaire', '@language': 'en' },
  ]);

  // Reopened, the picker shows the stored word, and an unchanged save keeps the item.
  await page.getByRole('button', { name: 'Edit this item' }).click();
  await expect(picker).toHaveValue('Dún Laoghaire');
  await page.getByRole('button', { name: 'Save' }).click();
  await expect(page.locator('[role=status]')).toHaveText('Saved.');
  await expectClean(diagnostics, 'unchanged save with a term');
  expect(await stored()).toEqual(before);

  // The list page counts the item.
  await page.goto('/#/term-lists');
  await expect(page.getByRole('row').filter({ hasText: LIST }).getByRole('cell').nth(3)).toHaveText('1');
  await expectClean(diagnostics, 'term list usage');

  const response = await page.goto(`/s/${SITE_SLUG}/item/${String(itemId)}`);
  expect(response?.status()).toBe(200);
  await expect(page.locator('dd.value[class*="customvocab-"]')).toContainText('Dún Laoghaire');
});
