/**
 * A bulk change through the real server and its job runner: filter the Items list, make every
 * item that matches Private, watch the change finish, see the list follow, undo it, and see the
 * list back as it was. `BULK_SCREENSHOTS` names a folder for the panel's pictures.
 */
import { join } from 'node:path';
import type { Locator, Page } from '@playwright/test';
import { expect, expectClean, test } from '../fixtures';
import { expectAxeBudget } from '../harness/axe';
import { readHarness } from '../harness/state';

const TITLE = 'Bulk e2e letter';
const COUNT = 3;

const shoot = async (page: Page, name: string): Promise<void> => {
  const folder = process.env.BULK_SCREENSHOTS;
  if (folder !== undefined && folder !== '') {
    await page.screenshot({ path: join(folder, `bulk-${name}.png`), fullPage: true });
  }
};

/** Sign the browser in as the seeded administrator: the bulk buttons follow the signed-in role. */
const signIn = async (page: Page, baseURL: string): Promise<void> => {
  const { adminEmail, adminPassword } = readHarness();
  const response = await page.request.post('/api/login', {
    data: { email: adminEmail, password: adminPassword },
    headers: { origin: new URL(baseURL).origin },
  });
  expect(response.status(), await response.text()).toBe(200);
};

const privateCues = (page: Page): Locator => page.locator('.items-results').getByRole('button', { name: 'Private' });

test('every item that matches goes Private, the list follows, and undo puts it back', async ({ page, diagnostics, baseURL }) => {
  for (let n = 1; n <= COUNT; n += 1) {
    const title = [{ type: 'literal', property_id: 1, '@value': `${TITLE} ${String(n)}` }];
    const created = await page.request.post('/api/items', { data: { 'dcterms:title': title, 'o:is_public': true } });
    expect(created.status(), await created.text()).toBe(200);
  }
  await signIn(page, baseURL);

  await page.goto(`/#/items?q=${encodeURIComponent(TITLE)}`);
  await expect(page.locator('.items-results tbody tr')).toHaveCount(COUNT);
  await expect(privateCues(page)).toHaveCount(0);

  await page.getByRole('button', { name: 'Change items…' }).click();
  const panel = page.getByRole('region', { name: 'Change items' });
  await expect(panel.getByRole('radio', { name: `All ${String(COUNT)} items that match` })).toBeChecked();
  await panel.getByRole('combobox', { name: 'Visibility' }).selectOption('private');
  await diagnostics.idle();
  await expectAxeBudget(page, 'items bulk change panel');
  await shoot(page, 'panel');

  await panel.getByRole('button', { name: 'Preview' }).click();
  await expect(panel.getByText(`${String(COUNT)} items will change.`)).toBeVisible();
  await expect(panel.getByRole('table', { name: 'Examples of the change' }).getByRole('row')).toHaveCount(COUNT + 1);
  await expectAxeBudget(page, 'items bulk change preview');
  await shoot(page, 'preview');

  await panel.getByRole('button', { name: 'Apply' }).click();
  // The job is its own process, so the first read finds the change waiting or under way.
  const progress = panel.getByRole('progressbar', { name: /^Changing items: \d of 3$/ });
  const ended = panel.getByText(`${String(COUNT)} items changed.`);
  await expect(progress.or(ended)).toBeVisible();
  if (await progress.isVisible()) {
    await expect(panel.getByText('You can leave this page; the change keeps running.')).toBeVisible();
    await shoot(page, 'running');
  }
  await expect(ended).toBeVisible({ timeout: 15_000 });
  await expect(privateCues(page)).toHaveCount(COUNT);
  await expectAxeBudget(page, 'items bulk change done');
  await shoot(page, 'done');
  await expectClean(diagnostics, 'bulk change');

  await panel.getByRole('button', { name: 'Undo' }).click();
  await expect(panel.getByText(`Undo puts ${String(COUNT)} items back as they were before this change.`)).toBeVisible();
  await panel.getByRole('button', { name: 'Undo the change' }).click();
  await expect(panel.getByText(`Undone. ${String(COUNT)} items are back as they were.`)).toBeVisible({ timeout: 15_000 });
  await expect(privateCues(page)).toHaveCount(0);

  await page.getByRole('button', { name: 'Recent bulk changes' }).click();
  const recent = page.getByRole('region', { name: 'Recent bulk changes' });
  await expect(recent.getByRole('row', { name: /Private/ }).first()).toContainText('Undone');
  await expectClean(diagnostics, 'bulk undo');
});
