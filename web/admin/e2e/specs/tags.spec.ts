/**
 * Tags end to end: a reader tags an item in its Subject field and the tags are
 * stored as plain `dcterms:subject` text; the Term lists page counts them,
 * merges two spellings of one place after a preview, and promotes two tags
 * into a new list after a preview. The promoted values keep their text on the
 * subject with the list's type, and the item's type now offers the list.
 */
import type { APIRequestContext } from '@playwright/test';
import { expect, expectClean, test } from '../fixtures';
import { expectAxeBudget } from '../harness/axe';
import { isArray, numberField, record, stringField } from '../harness/json';

const TYPE = 'E2E tagged letter';
const TOPICS = 'E2E Topics';

const propertyId = async (request: APIRequestContext, term: string): Promise<number> => {
  const found: unknown = await (await request.get(`/api/properties?term=${term}`)).json();
  return numberField(record(isArray(found) ? found[0] : null, term), 'o:id', term);
};

/** The `(type, @value)` pairs of an item's subject values. */
const subjects = async (
  request: APIRequestContext,
  item: number,
): Promise<readonly (readonly [string, string])[]> => {
  const document = record(await (await request.get(`/api/items/${String(item)}`)).json(), 'item');
  const values = document['dcterms:subject'];
  return (isArray(values) ? values : []).map((entry) => {
    const value = record(entry, 'subject');
    return [stringField(value, 'type', 'subject'), stringField(value, '@value', 'subject')] as const;
  });
};

test('tags are added on the form, merged and promoted into a term list', async ({
  page,
  diagnostics,
}) => {
  const title = await propertyId(page.request, 'dcterms:title');
  const created = await page.request.post('/api/resource_templates', {
    data: { 'o:label': TYPE, 'o:resource_template_property': [{ 'o:property': { 'o:id': title } }] },
  });
  expect(created.status(), await created.text()).toBe(200);
  const typeId = numberField(record(await created.json(), 'template'), 'o:id', 'template');
  const other = await page.request.post('/api/items', {
    data: {
      'o:resource_template': { 'o:id': typeId },
      'dcterms:title': [{ type: 'literal', property_id: 'auto', '@value': 'E2E second letter' }],
      'dcterms:subject': [
        { type: 'literal', property_id: 'auto', '@value': 'E2E emigration' },
        { type: 'literal', property_id: 'auto', '@value': 'E2E Co. Cork' },
      ],
    },
  });
  expect(other.status(), await other.text()).toBe(200);

  // The Subject field's tags: Enter and a comma each add a tag.
  await page.goto('/#/items/new');
  await page.getByRole('combobox', { name: 'Type', exact: true }).selectOption({ label: TYPE });
  await page.getByRole('textbox', { name: 'Title', exact: true }).fill('E2E first letter');
  const tags = page.getByRole('group', { name: 'Subject' });
  const entry = tags.getByLabel('Add a tag');
  await entry.fill('E2E emigration');
  await entry.press('Enter');
  await entry.pressSequentially('E2E wartime,E2E Cork,');
  await expect(tags.getByRole('listitem')).toHaveText([
    'E2E emigration×',
    'E2E wartime×',
    'E2E Cork×',
  ]);
  await expectAxeBudget(page, 'item form with tags');
  await page.getByRole('button', { name: 'Add item' }).click();
  await expect(page).toHaveURL(/#\/items\/\d+$/);
  await expectClean(diagnostics, 'item create with tags');
  const itemId = Number(/#\/items\/(\d+)$/.exec(page.url())?.[1]);
  expect(await subjects(page.request, itemId)).toEqual([
    ['literal', 'E2E emigration'],
    ['literal', 'E2E wartime'],
    ['literal', 'E2E Cork'],
  ]);

  // The Tags panel counts each tag and offers to merge the two spellings of Cork.
  await page.goto('/#/term-lists');
  const panel = page.getByRole('complementary', { name: 'Tags' });
  await expect(panel.getByRole('checkbox', { name: 'E2E emigration 2' })).toBeVisible();
  const prompt = panel.getByRole('region', { name: 'Similar spellings' });
  await expect(prompt.getByRole('radio', { name: /Keep “E2E Cork”/ })).toBeVisible();
  await prompt.getByRole('radio', { name: /Keep “E2E Cork”/ }).check();
  await prompt.getByRole('button', { name: 'Merge into “E2E Cork”' }).click();
  await expect(prompt.locator('.rewrite-counts dd').first()).toHaveText('1');
  await expectAxeBudget(page, 'term lists with a merge preview');
  await prompt.getByRole('button', { name: 'Merge', exact: true }).click();
  await expect(panel.getByRole('status')).toHaveText('Merged into “E2E Cork”. Resources changed: 1.');
  await expect(panel.getByRole('checkbox', { name: 'E2E Cork 2' })).toBeVisible();
  await expectClean(diagnostics, 'merge tags');

  // Promote two tags into a new list, after the preview.
  await panel.getByRole('checkbox', { name: 'E2E emigration 2' }).check();
  await panel.getByRole('checkbox', { name: 'E2E wartime 1' }).check();
  await panel.getByLabel('List name').fill(TOPICS);
  await panel.getByRole('button', { name: 'Promote to list' }).click();
  const preview = panel.getByRole('region', { name: `Promote to “${TOPICS}”` });
  await expect(preview).toContainText('This makes a new list.');
  await expect(preview.locator('dd').first()).toHaveText('2');
  await expect(preview).toContainText(TYPE);
  await expectAxeBudget(page, 'term lists with a promotion preview');
  await preview.getByRole('button', { name: 'Promote to list' }).click();
  await expect(panel.getByRole('status')).toHaveText(
    `Promoted to “${TOPICS}”. Resources changed: 2.`,
  );
  await expect(page.getByRole('row').filter({ hasText: TOPICS })).toContainText('A list of words');
  await expectClean(diagnostics, 'promote tags');

  const lists: unknown = await (await page.request.get('/api/custom_vocabs')).json();
  const listId = (isArray(lists) ? lists : [])
    .map((list) => record(list, 'term list'))
    .filter((list) => stringField(list, 'o:label', 'term list') === TOPICS)
    .map((list) => numberField(list, 'o:id', 'term list'))[0];
  const name = `customvocab:${String(listId)}`;
  expect(await subjects(page.request, itemId)).toEqual([
    [name, 'E2E emigration'],
    [name, 'E2E wartime'],
    ['literal', 'E2E Cork'],
  ]);
  const template = record(
    await (await page.request.get(`/api/resource_templates/${String(typeId)}`)).json(),
    'template',
  );
  const rows = template['o:resource_template_property'];
  expect(isArray(rows) ? rows.map((row) => record(row, 'row')['o:data_type']) : null).toEqual([
    [],
    [name],
  ]);
});
