/**
 * An item set create, edit and delete round trip through the SPA, against the
 * running port. A new set lands on its place (Items filtered to it, under its
 * card); its edit form is a step away, and the card's delete moves it to the trash.
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
  await page.getByRole('textbox', { name: 'Title', exact: true }).fill(NAME);
  await page.getByRole('button', { name: 'Add item set' }).click();

  // Created: the new set's place reads it back from the API.
  await expect(page).toHaveURL(/#\/items\?set=\d+$/);
  const card = page.locator('.set-card');
  await expect(card.getByRole('heading', { name: NAME, level: 2 })).toBeVisible();
  await expect(card.getByText('0 items', { exact: true })).toBeVisible();
  await expectClean(diagnostics, 'item set create');

  await card.getByRole('link', { name: 'Edit set' }).click();
  await expect(page).toHaveURL(/#\/item-sets\/\d+\/edit$/);
  await expect(page.getByRole('textbox', { name: 'Title', exact: true })).toHaveValue(NAME);
  await page.getByRole('textbox', { name: 'Title', exact: true }).fill(RENAMED);
  await page.getByRole('button', { name: 'Save' }).click();
  await expect(page.locator('[role=status]')).toHaveText('Saved.');
  await expectClean(diagnostics, 'item set rename');

  // The browse table is the user's check that the rename was stored.
  await page.getByRole('link', { name: 'Item sets' }).click();
  await expect(page.getByRole('link', { name: RENAMED })).toBeVisible();
  await expect(page.getByRole('link', { name: NAME, exact: true })).toHaveCount(0);

  await page.getByRole('link', { name: RENAMED }).click();
  await card.getByRole('button', { name: 'Delete this item set' }).click();
  const confirm = page.getByRole('region', { name: 'Move to the trash' });
  await expect(confirm.getByText(`Move ${RENAMED} to the trash?`)).toBeVisible();
  await expect(confirm.getByText('The set has no items.')).toBeVisible();
  await confirm.getByRole('button', { name: 'Move to the trash' }).click();

  // In the trash: back on the list, without the row, with the Undo notice.
  await expect(page).toHaveURL(/#\/item-sets$/);
  await expect(page.locator('.trashed-notice').getByText(`Deleted ${RENAMED}.`)).toBeVisible();
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
  await page.getByRole('textbox', { name: 'Title', exact: true }).fill(`${NAME} never deleted`);
  await page.getByRole('button', { name: 'Add item set' }).click();
  await expect(page).toHaveURL(/#\/items\?set=\d+$/);

  await page.getByRole('button', { name: 'Delete this item set' }).click();
  await expect(page.getByRole('region', { name: 'Move to the trash' })).toBeVisible();
  await page.getByRole('button', { name: 'Cancel' }).click();
  await expect(page.getByRole('button', { name: 'Delete this item set' })).toBeVisible();

  await page.getByRole('link', { name: 'Item sets' }).first().click();
  await expect(page.locator('table tbody tr')).toHaveCount(before + 1);
  await expect(page.getByRole('link', { name: `${NAME} never deleted` })).toBeVisible();
  await expectClean(diagnostics, 'item set delete cancelled');
});
