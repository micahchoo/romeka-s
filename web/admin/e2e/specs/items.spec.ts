/**
 * An item create, edit and delete round trip, including the item set membership
 * the item form sends and reads back.
 */
import { expect, expectClean, test } from '../fixtures';
import { ITEM_SET_TITLE } from '../harness/seed';

const TITLE = 'E2E round trip item';
const RENAMED = `${TITLE} renamed`;

test('an item is created with an item set, renamed and deleted', async ({
  page,
  diagnostics,
}) => {
  await page.goto('/#/items');
  await page.getByRole('link', { name: 'Add item' }).click();
  await expect(page).toHaveURL(/#\/items\/new$/);

  await page.getByRole('textbox', { name: 'Title', exact: true }).fill(TITLE);
  await page.getByLabel(ITEM_SET_TITLE).check();
  await page.getByRole('button', { name: 'Add item' }).click();

  // Created: the show page reads the new item back from the API.
  await expect(page).toHaveURL(/#\/items\/\d+$/);
  await expect(page.getByRole('heading', { name: TITLE })).toBeVisible();
  await expectClean(diagnostics, 'item create');

  // The membership went to the server: the edit form reads it back checked.
  await page.getByRole('button', { name: 'Edit this item' }).click();
  await expect(page.getByLabel(ITEM_SET_TITLE)).toBeChecked();

  await page.getByRole('textbox', { name: 'Title', exact: true }).fill(RENAMED);
  await page.getByRole('button', { name: 'Save' }).click();
  await expect(page.locator('[role=status]')).toHaveText('Saved.');
  await expect(page.getByRole('heading', { name: RENAMED })).toBeVisible();
  await expectClean(diagnostics, 'item rename');

  // The browse table is the user's check that the rename was stored.
  await page.getByRole('link', { name: 'Items' }).click();
  await expect(page.getByRole('link', { name: RENAMED })).toBeVisible();

  await page.getByRole('link', { name: RENAMED }).click();
  await page.getByRole('button', { name: 'Delete this item' }).click();
  await expect(page.getByText(`Delete ${RENAMED}? This cannot be undone.`)).toBeVisible();
  await page.getByRole('button', { name: `Delete ${RENAMED}`, exact: true }).click();

  await expect(page).toHaveURL(/#\/items$/);
  await expect(page.getByRole('link', { name: RENAMED })).toHaveCount(0);
  await expectClean(diagnostics, 'item delete');
});

const SITE = 'Sites control check';

const siteIdsOf = async (page: import('@playwright/test').Page, itemId: string): Promise<number[]> => {
  const item: unknown = await (await page.request.get(`/api/items/${itemId}`)).json();
  const sites = typeof item === 'object' && item !== null && 'o:site' in item ? item['o:site'] : [];
  return Array.isArray(sites)
    ? sites.flatMap((ref: unknown) =>
        typeof ref === 'object' && ref !== null && 'o:id' in ref && typeof ref['o:id'] === 'number' ? [ref['o:id']] : [],
      )
    : [];
};

test('the item form puts an item on a chosen site and an untick takes it off', async ({ page, diagnostics }) => {
  const created = await page.request.post('/api/sites', {
    data: { 'o:title': SITE, 'o:slug': 'e2e-sites-control', 'o:theme': 'default', 'o:is_public': true, 'o:assign_new_items': false },
  });
  expect(created.status(), await created.text()).toBe(200);
  const siteBody: unknown = await created.json();
  const siteId = typeof siteBody === 'object' && siteBody !== null && 'o:id' in siteBody ? siteBody['o:id'] : null;

  await page.goto('/#/items/new');
  await page.getByRole('textbox', { name: 'Title', exact: true }).fill('E2E item on a chosen site');
  const sites = page.getByRole('group', { name: 'Sites' });
  await expect(sites.getByLabel(SITE)).not.toBeChecked();
  await sites.getByLabel(SITE).check();
  await page.getByRole('button', { name: 'Add item' }).click();
  await expect(page).toHaveURL(/#\/items\/\d+$/);
  const itemId = /#\/items\/(\d+)$/.exec(page.url())?.[1] ?? '';
  expect(await siteIdsOf(page, itemId)).toContain(siteId);

  await page.getByRole('button', { name: 'Edit this item' }).click();
  await expect(sites.getByLabel(SITE)).toBeChecked();
  await sites.getByLabel(SITE).uncheck();
  await page.getByRole('button', { name: 'Save' }).click();
  await expect(page.locator('[role=status]')).toHaveText('Saved.');
  expect(await siteIdsOf(page, itemId)).not.toContain(siteId);
  await expectClean(diagnostics, 'item sites');
});
