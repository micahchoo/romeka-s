/**
 * System's Search query tab runs a real search against the port and lists the
 * matches. `Admin/QueryController::sidebarPreviewAction` is the upstream action:
 * the form on the sidebar, the table under it.
 */
import { expect, expectClean, test } from '../fixtures';
import { ITEM_TITLES } from '../harness/seed';

test('the search query tab lists the items a search matches, and nothing for a miss', async ({
  page,
  diagnostics,
}) => {
  await page.goto('/#/system/query');

  await page.locator('#query-resource').selectOption('items');
  await page.locator('#query-search').fill('E2E item');
  await page.getByRole('button', { name: 'Preview' }).click();

  const table = page.locator('table');
  await expect(table).toBeVisible();
  for (const title of ITEM_TITLES) {
    await expect(table.getByRole('cell', { name: title, exact: true })).toBeVisible();
  }
  await expectClean(diagnostics, 'query preview');

  // A query that matches nothing says so instead of showing the last result.
  await page.locator('#query-search').fill('E2E no such item');
  await page.getByRole('button', { name: 'Preview' }).click();
  await expect(page.getByText('No resources match the query.')).toBeVisible();
  await expectClean(diagnostics, 'query miss');
});
