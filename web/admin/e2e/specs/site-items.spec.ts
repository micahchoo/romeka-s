/**
 * Putting existing items on a site: a new site holds no items, and its Items
 * tab adds them with the item picker, by single items and whole item sets,
 * replaces them, and keeps the site in sync with the picked sets as items are
 * created. The Items list adds picked items to another site in bulk.
 *
 * The run drives the real server, so a count the page shows after a change is
 * the `item_site` membership the server stored.
 */
import type { Page } from '@playwright/test';
import { expect, expectClean, test } from '../fixtures';
import { isArray, numberField, parseJson, record } from '../harness/json';
import { ITEM_SET_TITLE, ITEM_TITLES } from '../harness/seed';
import { readHarness } from '../harness/state';

const SITE = 'E2E items site';
const BULK_SITE = 'E2E bulk site';

const keyed = (path: string): string => {
  const { baseUrl, keyIdentity, keyCredential } = readHarness();
  const separator = path.includes('?') ? '&' : '?';
  return `${baseUrl}${path}${separator}key_identity=${keyIdentity}&key_credential=${keyCredential}`;
};

const apiSend = async (
  page: Page,
  method: 'POST' | 'PATCH',
  path: string,
  body: unknown,
): Promise<Record<string, unknown>> => {
  const response = await page.request.fetch(keyed(path), { method, data: body });
  expect(response.status(), `${method} ${path}`).toBe(200);
  return record(parseJson(await response.text()), path);
};

const apiList = async (page: Page, path: string): Promise<readonly unknown[]> => {
  const response = await page.request.get(keyed(path));
  const doc = parseJson(await response.text());
  if (!isArray(doc)) {
    throw new Error(`${path}: expected a list`);
  }
  return doc;
};

const titleId = async (page: Page): Promise<number> =>
  numberField(record((await apiList(page, '/api/properties?term=dcterms:title'))[0], 'title'), 'o:id', 'title');

/** Create an item through the API, in the item set when one is named. */
const createItem = async (page: Page, title: string, itemSet: number | null): Promise<number> => {
  const made = await apiSend(page, 'POST', '/api/items', {
    'o:is_public': true,
    'dcterms:title': [{ property_id: await titleId(page), type: 'literal', '@value': title }],
    ...(itemSet === null ? {} : { 'o:item_set': [{ 'o:id': itemSet }] }),
  });
  return numberField(made, 'o:id', 'item');
};

const siteItemCount = async (page: Page, siteHash: string): Promise<number> => {
  const id = siteHash.replace('/sites/', '');
  const response = await page.request.get(keyed(`/api/items?site_id=${id}&per_page=1`));
  return Number(response.headers()['omeka-s-total-results']);
};

test('a new site starts empty, and its Items tab picks, syncs and replaces items', async ({
  page,
  diagnostics,
}) => {
  await page.goto('/#/sites/new');
  // One theme is installed, so the form offers no theme choice and points at the Theme tab.
  await expect(page.getByLabel('Theme')).toHaveCount(0);
  await expect(page.getByText('Choose the look after you add the site, on its Theme tab.')).toBeVisible();
  await page.getByLabel('Title').fill(SITE);
  await expect(page.getByText(/starts with all public items/)).toHaveCount(0);
  await page.getByRole('button', { name: 'Add site' }).click();
  await expect(page).toHaveURL(/#\/sites\/\d+$/);
  const siteHash = /#(\/sites\/\d+)$/.exec(page.url())?.[1] ?? '';

  // The hub says the site is empty and points at the Items tab.
  await page.getByRole('link', { name: 'No items yet' }).click();
  await expect(page).toHaveURL(new RegExp(`#${siteHash}/resources$`));
  const total = page.locator('.site-items-total');
  await expect(total).toHaveText('Items on this site: 0');
  const sync = page.getByRole('switch', { name: 'Keep this site in sync' });
  await expect(sync).not.toBeChecked();
  // With no item set, sync would let every new item join, so it waits for a set.
  await expect(sync).toBeDisabled();
  await expect(sync).toHaveAccessibleDescription('Pick an item set to keep this site in sync with it.');
  await expectClean(diagnostics, 'a new site holds no items');

  // The class filter offers only the classes items use, each with its item count.
  const sound = numberField(record((await apiList(page, '/api/resource_classes?term=dctype:Sound'))[0], 'Sound'), 'o:id', 'Sound');
  await apiSend(page, 'POST', '/api/items', {
    'o:is_public': true,
    'o:resource_class': { 'o:id': sound },
    'dcterms:title': [{ property_id: await titleId(page), type: 'literal', '@value': 'E2E classed sound' }],
  });
  await page.reload();
  const classFilter = page.getByRole('combobox', { name: 'Class' });
  await expect(classFilter.getByRole('option', { name: /^Sound · \d+ items?$/ })).toHaveCount(1);
  const classChoices = await classFilter.getByRole('option').allTextContents();
  expect(classChoices[0]).toBe('Any class');
  expect(classChoices.slice(1).filter((choice) => !/ · \d+ items?$/.test(choice)), 'every class names its count').toEqual([]);

  // One item by searching its title.
  const [firstTitle] = ITEM_TITLES;
  if (firstTitle === undefined) {
    throw new Error('the seed names no item');
  }
  const results = page.getByRole('listbox', { name: 'Find items' });
  await page.getByLabel('Search titles and values').fill(firstTitle);
  await results.getByRole('option', { name: firstTitle, exact: true }).click();
  await expect(page.getByText('1 picked, 1 not on this site yet.')).toBeVisible();
  await page.getByRole('button', { name: 'Add items (1)' }).click();
  await expect(page.getByText('Items added: 1.')).toBeVisible();
  await expect(total).toHaveText('Items on this site: 1');
  await expect(page.getByRole('region', { name: 'Picked' })).toContainText('Picked: 0');

  // The whole seeded set: its other items join, and the set becomes the site's.
  await page.getByRole('button', { name: `Pick the whole set ${ITEM_SET_TITLE}` }).click();
  await expect(page.getByText(/^\d+ picked, \d+ not on this site yet\.$/)).toBeVisible();
  await page.getByRole('button', { name: /^Add items \(\d+\)$/ }).click();
  await expect(page.getByText(/^Items added: \d+\.$/)).toBeVisible();
  await expect(page.getByRole('region', { name: 'Item sets on this site' })).toContainText(ITEM_SET_TITLE);
  const afterSet = await siteItemCount(page, siteHash);
  expect(afterSet, 'the set added its other items').toBeGreaterThan(1);
  await expect(total).toHaveText(`Items on this site: ${String(afterSet)}`);

  // Sync: a new item in the set joins; an item made outside it does not, and
  // an edit that puts it in the set does not either.
  await expect(sync).toHaveAccessibleDescription(/^Off\. Turn it on and an item created in one of this site.s item sets joins/);
  await sync.click();
  await expect(sync).toBeChecked();
  const itemSet = numberField(
    record((await apiList(page, `/api/item_sets?search=${encodeURIComponent(ITEM_SET_TITLE)}`))[0], 'set'),
    'o:id',
    'item set',
  );
  await createItem(page, 'E2E synced newcomer', itemSet);
  const outsider = await createItem(page, 'E2E outsider', null);
  await apiSend(page, 'PATCH', `/api/items/${String(outsider)}`, { 'o:item_set': [{ 'o:id': itemSet }] });
  expect(await siteItemCount(page, siteHash), 'only the item created in the set joined').toBe(afterSet + 1);

  // Replace: the confirmation names how many items leave.
  await page.reload();
  await page.getByLabel('Search titles and values').fill('E2E outsider');
  await results.getByRole('option', { name: 'E2E outsider', exact: true }).click();
  await expect(page.getByText('1 picked, 1 not on this site yet.')).toBeVisible();
  await page.getByRole('button', { name: 'Replace with these' }).click();
  await expect(
    page.getByText(
      `Replace the site’s items with these 1? Items that leave the site: ${String(afterSet + 1)}. Items that join: 1.`,
    ),
  ).toBeVisible();
  await page.getByRole('button', { name: 'Replace the items' }).click();
  await expect(page.getByText('Replaced. Items on this site: 1.')).toBeVisible();
  await expect(total).toHaveText('Items on this site: 1');
  await expect(page.getByText('This site has no item sets. Pick a whole set above to add one.')).toBeVisible();
  // The replace left no set, so the same write turned sync off.
  await expect(sync).not.toBeChecked();
  await expect(sync).toBeDisabled();
  await expectClean(diagnostics, 'the Items page changes');

  // Bulk add from the Items list to a site that holds nothing.
  await apiSend(page, 'POST', '/api/sites', {
    'o:title': BULK_SITE,
    'o:slug': 'e2e-bulk-site',
    'o:theme': 'default',
    'o:is_public': true,
  });
  await page.goto('/#/items');
  const rows = page.getByRole('checkbox', { name: /^Select / });
  await rows.nth(1).check();
  await rows.nth(2).check();
  await expect(page.getByText('Selected: 2')).toBeVisible();
  await page.getByRole('button', { name: 'Add to site…' }).click();
  await page.getByLabel('Site', { exact: true }).selectOption({ label: BULK_SITE });
  await page.getByRole('button', { name: `Add 2 to ${BULK_SITE}` }).click();
  await expect(page.getByText(`${BULK_SITE} now has 2 items.`)).toBeVisible();
  await expectClean(diagnostics, 'the Items list bulk add');
});
