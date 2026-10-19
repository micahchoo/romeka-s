/**
 * Term lists end to end: a list of words is created on the Term lists page, a
 * resource template row allows it, an item stores a word from it, the list
 * page counts that item, and the public item page shows the word.
 */
import { expect, expectClean, test } from '../fixtures';
import { SITE_SLUG } from '../harness/seed';
import { isArray, numberField, record, stringField } from '../harness/json';
import { readHarness } from '../harness/state';

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
  await page.getByRole('button', { name: 'Add resource template' }).click();
  await expect(page).toHaveURL(/#\/resource-templates\/\d+$/);
  await expectClean(diagnostics, 'template create');

  // The item editor is being rebuilt to follow the template, so the item is
  // written through the API with a word from the list.
  const lists: unknown = await (await page.request.get('/api/custom_vocabs')).json();
  const listId = (isArray(lists) ? lists : [])
    .map((entry) => record(entry, 'term list'))
    .filter((entry) => stringField(entry, 'o:label', 'term list') === LIST)
    .map((entry) => numberField(entry, 'o:id', 'term list'))[0];
  expect(listId).toBeDefined();
  const { keyIdentity, keyCredential } = readHarness();
  const created = await page.request.post('/api/items', {
    params: { key_identity: keyIdentity, key_credential: keyCredential },
    data: {
      'dcterms:title': [{ type: 'literal', property_id: 'auto', '@value': ITEM }],
      'dcterms:spatial': [
        { type: `customvocab:${String(listId)}`, property_id: 'auto', '@value': 'Dún Laoghaire' },
      ],
    },
  });
  expect(created.status()).toBe(200);
  const itemId = numberField(record(await created.json(), 'item'), 'o:id', 'item');

  // The list page counts the item.
  await page.goto('/#/term-lists');
  await expect(page.getByRole('row').filter({ hasText: LIST }).getByRole('cell').nth(3)).toHaveText('1');
  await expectClean(diagnostics, 'term list usage');

  const response = await page.goto(`/s/${SITE_SLUG}/item/${String(itemId)}`);
  expect(response?.status()).toBe(200);
  await expect(page.locator('dd.value[class*="customvocab-"]')).toContainText('Dún Laoghaire');
});
