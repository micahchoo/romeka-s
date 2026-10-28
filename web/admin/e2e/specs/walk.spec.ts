/**
 * Walking a result: from a set's place open row 2, read "2 of 4" with the set's title, Next opens
 * row 3 and says "3 of 4", and Save and next saves that item and opens row 4 at its edit form.
 * `WALK_SCREENSHOTS` names a folder for the pictures.
 */
import { join } from 'node:path';
import type { Page } from '@playwright/test';
import { expect, expectClean, test } from '../fixtures';
import { expectAxeBudget } from '../harness/axe';
import { numberField, record, stringField } from '../harness/json';
import { readHarness } from '../harness/state';

const SET = 'Walk e2e ledger box';
/** Created in this order; the place lists newest first, so row 1 is the last of them. */
const ITEMS = ['Walk e2e ledger 1901', 'Walk e2e ledger 1902', 'Walk e2e ledger 1903', 'Walk e2e ledger 1904'] as const;

const shoot = async (page: Page, name: string): Promise<void> => {
  const folder = process.env.WALK_SCREENSHOTS;
  if (folder !== undefined && folder !== '') {
    await page.mouse.move(0, 0);
    await page.screenshot({ path: join(folder, `walk-${name}.png`), fullPage: true });
  }
};

const signIn = async (page: Page, baseURL: string): Promise<void> => {
  const { adminEmail, adminPassword } = readHarness();
  const response = await page.request.post('/api/login', {
    data: { email: adminEmail, password: adminPassword },
    headers: { origin: new URL(baseURL).origin },
  });
  expect(response.status(), await response.text()).toBe(200);
};

const literal = (text: string) => [{ type: 'literal', property_id: 1, '@value': text }];

const create = async (page: Page, path: string, data: Record<string, unknown>): Promise<number> => {
  const response = await page.request.post(path, { data });
  expect(response.status(), await response.text()).toBe(200);
  return numberField(record(await response.json(), path), 'o:id', path);
};

test('a set place walks item by item, and Save and next opens the next item to edit', async ({
  page,
  diagnostics,
  baseURL,
}) => {
  const setId = await create(page, '/api/item_sets', { 'dcterms:title': literal(SET), 'o:is_public': true });
  const ids: number[] = [];
  for (const title of ITEMS) {
    ids.push(
      await create(page, '/api/items', {
        'dcterms:title': literal(title),
        'o:is_public': true,
        'o:item_set': [{ 'o:id': setId }],
      }),
    );
  }
  await signIn(page, baseURL);

  await page.goto('/#/item-sets');
  await page.getByRole('link', { name: SET }).click();
  await expect(page.locator('.items-results tbody tr')).toHaveCount(4);

  // Row 2 of the newest-first place is the third item made.
  await page.locator('.items-results').getByRole('link', { name: ITEMS[2] }).click();
  const walk = page.getByRole('navigation', { name: 'Place in the list' });
  await expect(walk.getByText(`2 of 4 in ${SET}`)).toBeVisible();
  await expect(walk.getByRole('link', { name: `Previous: ${ITEMS[3]}` })).toBeVisible();
  await diagnostics.idle();
  await expectAxeBudget(page, 'item page walk');
  await shoot(page, 'row-2');

  await walk.getByRole('link', { name: `Next: ${ITEMS[1]}` }).click();
  await expect(page.getByRole('heading', { name: ITEMS[1], level: 2 })).toBeVisible();
  await expect(walk.getByText(`3 of 4 in ${SET}`)).toBeVisible();
  await shoot(page, 'row-3');

  await page.getByRole('button', { name: 'Edit this item' }).click();
  const title = page.getByRole('textbox', { name: 'Title', exact: true });
  await title.fill(`${ITEMS[1]} checked`);
  await shoot(page, 'save-and-next');
  await page.getByRole('button', { name: 'Save and next' }).click();

  await expect(page.getByRole('heading', { name: ITEMS[0], level: 2 })).toBeVisible();
  await expect(page).toHaveURL(new RegExp(`#/items/${String(ids[0])}\\?.*edit=1`));
  await expect(walk.getByText(`4 of 4 in ${SET}`)).toBeVisible();
  // The edit form is open on arrival, and the last item has nowhere next to go.
  await expect(page.getByRole('button', { name: 'Edit this item' })).toHaveAttribute('aria-expanded', 'true');
  await expect(page.getByRole('button', { name: 'Save', exact: true })).toBeVisible();
  await expect(page.getByRole('button', { name: 'Save and next' })).toHaveCount(0);
  await expect(walk.getByRole('link', { name: /^Next/ })).toHaveCount(0);
  await shoot(page, 'row-4-edit');

  const saved = await page.request.get(`/api/items/${String(ids[1])}`);
  expect(stringField(record(await saved.json(), 'item'), 'o:title', 'item')).toBe(`${ITEMS[1]} checked`);
  await expectClean(diagnostics, 'walk');
});
