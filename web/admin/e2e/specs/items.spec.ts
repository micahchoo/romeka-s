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

  await page.locator('#item-title').fill(TITLE);
  await page.getByLabel(ITEM_SET_TITLE).check();
  await page.getByRole('button', { name: 'Add item' }).click();

  // Created: the show page reads the new item back from the API.
  await expect(page).toHaveURL(/#\/items\/\d+$/);
  await expect(page.getByRole('heading', { name: TITLE })).toBeVisible();
  await expectClean(diagnostics, 'item create');

  // The membership went to the server: the edit form reads it back checked.
  await page.getByRole('button', { name: 'Edit this item' }).click();
  await expect(page.getByLabel(ITEM_SET_TITLE)).toBeChecked();

  await page.locator('#item-title').fill(RENAMED);
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
