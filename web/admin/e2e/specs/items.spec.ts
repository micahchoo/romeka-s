/**
 * An item create, edit and delete round trip, including the item set membership
 * the item form sends and reads back.
 */
import { expect, expectClean, test } from '../fixtures';
import { ITEM_SET_TITLE, ITEM_TITLES } from '../harness/seed';

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
  await expect(page.getByText(`Move ${RENAMED} to the trash?`)).toBeVisible();
  await page.getByRole('button', { name: 'Move to the trash', exact: true }).click();

  await expect(page).toHaveURL(/#\/items$/);
  await expect(page.getByText(`Deleted ${RENAMED}.`)).toBeVisible();
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

test('the items list narrows by search, an item set and media, and the URL keeps the slice', async ({
  page,
  diagnostics,
}) => {
  const [first, second] = ITEM_TITLES;
  if (first === undefined || second === undefined) {
    throw new Error('the seed names fewer than two items');
  }
  await page.goto('/#/items');
  const search = page.getByRole('searchbox', { name: 'Search' });
  await expect(page.getByRole('link', { name: first, exact: true })).toBeVisible();

  await page.keyboard.press('/');
  await expect(search).toBeFocused();
  await search.fill(second);
  await expect(page.getByText(`Search: ${second}`)).toBeVisible();
  await expect(page.getByRole('link', { name: second, exact: true })).toBeVisible();
  await expect(page.getByRole('link', { name: first, exact: true })).toHaveCount(0);
  await expect(page.getByText('1 item matches')).toBeVisible();

  // A reload opens the same slice from the URL.
  await page.reload();
  await expect(search).toHaveValue(second);
  await expect(page.getByRole('link', { name: second, exact: true })).toBeVisible();
  await expect(page.getByRole('link', { name: first, exact: true })).toHaveCount(0);

  await page.getByRole('button', { name: `Remove filter: Search: ${second}` }).click();
  await expect(page.getByRole('link', { name: first, exact: true })).toBeVisible();

  // Filter from where the value is: the item set name in a row.
  await page
    .getByRole('row')
    .filter({ has: page.getByRole('link', { name: first, exact: true }) })
    .getByRole('button', { name: ITEM_SET_TITLE })
    .click();
  await expect(page.getByText(`Item set: ${ITEM_SET_TITLE}`)).toBeVisible();
  expect(new URL(page.url()).hash).toMatch(/[?&]set=\d+/);

  await page.getByRole('combobox', { name: 'Media' }).selectOption({ label: 'Image' });
  await expect(page.getByText('Media: Image')).toBeVisible();
  await expect(page.getByRole('link', { name: first, exact: true })).toBeVisible();
  await expect(page.getByRole('link', { name: second, exact: true })).toHaveCount(0);

  await page.getByRole('button', { name: 'Grid' }).click();
  const card = page.getByRole('listitem').filter({ has: page.getByRole('link', { name: first, exact: true }) });
  await expect(card.locator('img')).toBeVisible();
  await page.reload();
  await expect(page.getByRole('button', { name: 'Grid' })).toHaveAttribute('aria-pressed', 'true');
  await expect(page.getByText('Media: Image')).toBeVisible();

  await page.getByRole('button', { name: 'Clear all' }).click();
  await expect(page.getByRole('link', { name: second, exact: true })).toBeVisible();
  await page.getByRole('button', { name: 'Table' }).click();
  await expectClean(diagnostics, 'items browse');
});
