/**
 * Putting existing items on a site: a new site already holds the public items,
 * and the Items page removes items, adds them back by picking, and adds an item
 * set's items. The Items list adds picked items to another site in bulk.
 *
 * The run drives the real server, so a count the page shows after a change is
 * the `item_site` membership the server stored.
 */
import type { Page } from '@playwright/test';
import { expect, expectClean, test } from '../fixtures';
import { isArray, numberField, parseJson, record } from '../harness/json';
import { ITEM_SET_TITLE } from '../harness/seed';
import { readHarness } from '../harness/state';

const SITE = 'E2E items site';
const BULK_SITE = 'E2E bulk site';
const PRIVATE_IN_SET = 'E2E private item in the set';

const keyed = (path: string): string => {
  const { baseUrl, keyIdentity, keyCredential } = readHarness();
  const separator = path.includes('?') ? '&' : '?';
  return `${baseUrl}${path}${separator}key_identity=${keyIdentity}&key_credential=${keyCredential}`;
};

const apiPost = async (page: Page, path: string, body: unknown): Promise<Record<string, unknown>> => {
  const response = await page.request.post(keyed(path), { data: body });
  expect(response.status(), `POST ${path}`).toBe(200);
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

const publicItemCount = async (page: Page): Promise<number> => {
  const response = await page.request.get(keyed('/api/items?is_public=1&per_page=1'));
  return Number(response.headers()['omeka-s-total-results']);
};

const titleId = async (page: Page): Promise<number> =>
  numberField(record((await apiList(page, '/api/properties?term=dcterms:title'))[0], 'title'), 'o:id', 'title');

test('a site starts with the public items, and the Items page and list change them', async ({
  page,
  diagnostics,
}) => {
  await page.goto('/#/sites/new');
  await page.getByLabel('Title').fill(SITE);
  await expect(page.getByText(/A new site starts with all public items/)).toBeVisible();
  await page.getByRole('button', { name: 'Add site' }).click();
  await expect(page).toHaveURL(/#\/sites\/\d+$/);
  const siteHash = /#(\/sites\/\d+)$/.exec(page.url())?.[1] ?? '';

  const publicItems = await publicItemCount(page);
  await page.goto(`/#${siteHash}/resources`);
  const total = page.locator('.site-items-total');
  await expect(total).toHaveText(`Items on this site: ${String(publicItems)}`);
  await expectClean(diagnostics, 'a new site holds the public items');

  // Remove two items from the site.
  const list = page.getByRole('list', { name: 'On this site now' });
  const titles = await list.getByRole('link').allTextContents();
  const [first, second] = titles;
  if (first === undefined || second === undefined) {
    throw new Error('the site lists fewer than two items');
  }
  for (const title of [first, second]) {
    await page.getByRole('button', { name: `Remove ${title} from this site` }).click();
    await expect(page.getByText(`Removed ${title}.`)).toBeVisible();
  }
  await expect(total).toHaveText(`Items on this site: ${String(publicItems - 2)}`);

  // Add them back by picking.
  await page.getByRole('tab', { name: 'Pick items' }).click();
  const picker = page.getByLabel('Find items');
  for (const title of [first, second]) {
    await picker.fill(title);
    await page.getByRole('button', { name: title, exact: true }).click();
  }
  await expect(page.getByText('2 picked, 2 not on this site yet.')).toBeVisible();
  await page.getByRole('button', { name: 'Add items (2)' }).click();
  await expect(page.getByText('Items added: 2.')).toBeVisible();
  await expect(total).toHaveText(`Items on this site: ${String(publicItems)}`);

  // Add by item set: the set's private item is the one the site lacks.
  const itemSet = record((await apiList(page, '/api/item_sets?search=E2E'))[0], 'item set');
  await apiPost(page, '/api/items', {
    'o:is_public': false,
    'dcterms:title': [{ property_id: await titleId(page), type: 'literal', '@value': PRIVATE_IN_SET }],
    'o:item_set': [{ 'o:id': numberField(itemSet, 'o:id', 'item set') }],
  });
  await page.getByRole('tab', { name: 'By item set' }).click();
  await page.getByLabel('Find an item set').fill(ITEM_SET_TITLE);
  await page.getByRole('button', { name: ITEM_SET_TITLE }).click();
  await expect(page.getByText(/^E2E item set: \d+ items, 1 not on this site yet\.$/)).toBeVisible();
  await page.getByRole('button', { name: 'Attach the set and add items (1)' }).click();
  await expect(page.getByText('Items added: 1.')).toBeVisible();
  await expect(total).toHaveText(`Items on this site: ${String(publicItems + 1)}`);
  await expect(
    page.getByRole('list', { name: 'Item sets' }).getByText('On this site'),
  ).toBeVisible();
  await expectClean(diagnostics, 'the Items page changes');

  // Bulk add from the Items list to a site that holds nothing.
  await apiPost(page, '/api/sites', {
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
