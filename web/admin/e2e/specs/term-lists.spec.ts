/**
 * Term lists end to end: a list of words is created on the Term lists page, a
 * resource template row allows it, a new item picks a word from it in the
 * item editor, the stored value keeps the list's type, an unchanged save keeps
 * the item byte for byte, the list page counts that item, and the public item
 * page shows the word.
 */
import { expect, expectClean, test } from '../fixtures';
import { SITE_SLUG } from '../harness/seed';
import { isArray, numberField, record, stringField } from '../harness/json';

const LIST = 'E2E places';
const TEMPLATE = 'E2E place template';
const ITEM = 'E2E placed item';

test('a term list limits a template row and its value renders on the site', async ({
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

  // The template form offers the list among the data types, under its label.
  await page.goto('/#/resource-templates/new');
  await page.getByLabel('Label', { exact: true }).fill(TEMPLATE);
  await page.getByLabel('Add property').selectOption({ label: 'dcterms:spatial' });
  await page.getByRole('button', { name: 'Add property' }).click();
  await page.getByRole('checkbox', { name: LIST }).check();
  await page.getByLabel('Add property').selectOption({ label: 'dcterms:title' });
  await page.getByRole('button', { name: 'Add property' }).click();
  await page.getByRole('button', { name: 'Add resource template' }).click();
  await expect(page).toHaveURL(/#\/resource-templates\/\d+$/);
  await expectClean(diagnostics, 'template create');

  const lists: unknown = await (await page.request.get('/api/custom_vocabs')).json();
  const listId = (isArray(lists) ? lists : [])
    .map((entry) => record(entry, 'term list'))
    .filter((entry) => stringField(entry, 'o:label', 'term list') === LIST)
    .map((entry) => numberField(entry, 'o:id', 'term list'))[0];
  expect(listId).toBeDefined();

  // The row allows only the list, so its value is picked from the list.
  await page.goto('/#/items/new');
  await page.getByLabel('Resource template').selectOption({ label: TEMPLATE });
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
