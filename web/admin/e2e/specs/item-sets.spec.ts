/**
 * An item set create, edit and delete round trip through the SPA, against the
 * running port. Upstream keeps these three actions on three controllers; here
 * they are one browser journey from the list back to the list.
 */
import { expect, expectClean, test } from '../fixtures';

const NAME = 'E2E round trip set';
const RENAMED = `${NAME} renamed`;

test('an item set is created, renamed and deleted from the admin', async ({
  page,
  diagnostics,
}) => {
  await page.goto('/#/item-sets');
  await page.getByRole('link', { name: 'Add item set' }).click();

  await expect(page).toHaveURL(/#\/item-sets\/new$/);
  await page.locator('#item-set-title').fill(NAME);
  await page.getByRole('button', { name: 'Add item set' }).click();

  // Created: the form moves to the new row and reads it back from the API.
  await expect(page).toHaveURL(/#\/item-sets\/\d+$/);
  await expect(page.locator('#item-set-title')).toHaveValue(NAME);
  await expectClean(diagnostics, 'item set create');

  await page.locator('#item-set-title').fill(RENAMED);
  await page.getByRole('button', { name: 'Save' }).click();
  await expect(page.locator('[role=status]')).toHaveText('Saved.');
  await expectClean(diagnostics, 'item set rename');

  // The browse table is the user's check that the rename was stored.
  await page.getByRole('link', { name: 'Item sets' }).click();
  await expect(page.getByRole('link', { name: RENAMED })).toBeVisible();
  await expect(page.getByRole('link', { name: NAME, exact: true })).toHaveCount(0);

  await page.getByRole('link', { name: RENAMED }).click();
  await page.getByRole('button', { name: 'Delete this item set' }).click();
  await expect(page.getByText(`Delete ${RENAMED}? This cannot be undone.`)).toBeVisible();
  await page.getByRole('button', { name: `Delete ${RENAMED}`, exact: true }).click();

  // Deleted: back on the list, without the row.
  await expect(page).toHaveURL(/#\/item-sets$/);
  await expect(page.getByRole('link', { name: RENAMED })).toHaveCount(0);
  await expectClean(diagnostics, 'item set delete');
});

test('the delete control holds the request back until it is confirmed', async ({
  page,
  diagnostics,
}) => {
  await page.goto('/#/item-sets');
  await expect(page.locator('table')).toBeVisible();
  const before = await page.locator('table tbody tr').count();

  await page.getByRole('link', { name: 'Add item set' }).click();
  await page.locator('#item-set-title').fill(`${NAME} never deleted`);
  await page.getByRole('button', { name: 'Add item set' }).click();
  await expect(page).toHaveURL(/#\/item-sets\/\d+$/);

  await page.getByRole('button', { name: 'Delete this item set' }).click();
  await expect(page.getByText(/This cannot be undone\./)).toBeVisible();
  await page.getByRole('button', { name: 'Cancel' }).click();
  await expect(page.getByRole('button', { name: 'Delete this item set' })).toBeVisible();

  await page.getByRole('link', { name: 'Item sets' }).click();
  await expect(page.locator('table tbody tr')).toHaveCount(before + 1);
  await expect(page.getByRole('link', { name: `${NAME} never deleted` })).toBeVisible();
  await expectClean(diagnostics, 'item set delete cancelled');
});
