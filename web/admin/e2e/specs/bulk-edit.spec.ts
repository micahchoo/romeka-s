/**
 * A bulk change through the real server and its job runner: filter the Items list, make every
 * item that matches Private, watch the change finish, see the list follow, undo it, and see the
 * list back as it was; then add a value to every item that matches, see it on an item, and undo
 * it; then move every item that matches to the trash with its media, see the list empty, undo it,
 * and see the items back with their thumbnails. `BULK_SCREENSHOTS` names a folder for the panel's
 * pictures.
 */
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import type { Locator, Page } from '@playwright/test';
import { expect, expectClean, test } from '../fixtures';
import { expectAxeBudget } from '../harness/axe';
import { REPO_ROOT, readHarness } from '../harness/state';

const TITLE = 'Bulk e2e letter';
const COUNT = 3;

const shoot = async (page: Page, name: string, prefix = 'bulk'): Promise<void> => {
  const folder = process.env.BULK_SCREENSHOTS;
  if (folder !== undefined && folder !== '') {
    await page.screenshot({ path: join(folder, `${prefix}-${name}.png`), fullPage: true });
  }
};

const makeItems = async (page: Page, title: string): Promise<void> => {
  for (let n = 1; n <= COUNT; n += 1) {
    const values = [{ type: 'literal', property_id: 1, '@value': `${title} ${String(n)}` }];
    const created = await page.request.post('/api/items', { data: { 'dcterms:title': values, 'o:is_public': true } });
    expect(created.status(), await created.text()).toBe(200);
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
  await makeItems(page, TITLE);
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

test('a value added to every item that matches shows on the item, and undo takes it away', async ({ page, diagnostics, baseURL }) => {
  const title = 'Bulk e2e value';
  const subject = 'Harbour maps';
  await makeItems(page, title);
  await signIn(page, baseURL);

  await page.goto(`/#/items?q=${encodeURIComponent(title)}`);
  await expect(page.locator('.items-results tbody tr')).toHaveCount(COUNT);
  await page.getByRole('button', { name: 'Change items…' }).click();
  const panel = page.getByRole('region', { name: 'Change items' });
  const values = panel.getByRole('group', { name: 'Values' });
  await values.getByRole('searchbox', { name: 'Change the values of a field' }).fill('subject');
  await values.getByRole('button', { name: /dcterms:subject/ }).click();
  await values.getByRole('textbox', { name: 'Text' }).fill(subject);
  await values.getByRole('textbox', { name: 'Language (optional), like en or es' }).fill('en');
  await diagnostics.idle();
  await expectAxeBudget(page, 'items bulk change values');
  await shoot(page, 'values-form', 'bulk3');
  await values.getByRole('button', { name: 'Add this change' }).click();
  await expect(values.getByRole('listitem')).toHaveText(new RegExp(`add “${subject}” \\(en\\)`));
  await shoot(page, 'values', 'bulk3');

  await panel.getByRole('button', { name: 'Preview' }).click();
  await expect(panel.getByText(`${String(COUNT)} items will change.`)).toBeVisible();
  const example = panel.getByRole('table', { name: 'Examples of the change' }).getByRole('row').nth(1);
  await expect(example).toContainText(`“${subject}” (en)`);
  await expectAxeBudget(page, 'items bulk change value preview');
  await shoot(page, 'value-preview', 'bulk3');

  await panel.getByRole('button', { name: 'Apply' }).click();
  await expect(panel.getByText(`${String(COUNT)} items changed.`)).toBeVisible({ timeout: 15_000 });
  await expectClean(diagnostics, 'bulk value change');

  await page.locator('.items-results').getByRole('link', { name: `${title} 1` }).click();
  await expect(page.getByText(subject)).toBeVisible();
  await page.goBack();

  await page.getByRole('button', { name: 'Recent bulk changes' }).click();
  const recent = page.getByRole('region', { name: 'Recent bulk changes' });
  const row = recent.getByRole('row', { name: /1 value change/ }).first();
  await row.getByRole('button', { name: 'Undo' }).click();
  // The panel names itself after its step, so it is found by its class from here on.
  const undo = page.locator('.bulk-panel');
  await expect(undo.getByText(`Undo puts ${String(COUNT)} items back as they were before this change.`)).toBeVisible();
  await undo.getByRole('button', { name: 'Undo the change' }).click();
  await expect(undo.getByText(`Undone. ${String(COUNT)} items are back as they were.`)).toBeVisible({ timeout: 15_000 });

  await page.locator('.items-results').getByRole('link', { name: `${title} 1` }).click();
  await expect(page.getByText(`${title} 1`).first()).toBeVisible();
  await expect(page.getByText(subject)).toHaveCount(0);
  await expectClean(diagnostics, 'bulk value undo');
});

/** Items titled `title 1`, `title 2`…, each with one uploaded image. */
const makeImageItems = async (page: Page, title: string): Promise<void> => {
  const image = readFileSync(join(REPO_ROOT, 'fixtures', 'media-kinds', 'image.jpg'));
  for (let n = 1; n <= COUNT; n += 1) {
    const created = await page.request.post('/api/items', {
      multipart: {
        data: JSON.stringify({
          'o:is_public': true,
          'dcterms:title': [{ type: 'literal', property_id: 1, '@value': `${title} ${String(n)}` }],
          'o:media': [{ 'o:ingester': 'upload', file_index: 0, 'o:is_public': true }],
        }),
        file: { name: 'image.jpg', mimeType: 'image/jpeg', buffer: image },
      },
    });
    expect(created.status(), await created.text()).toBe(200);
  }
};

/** Every card's thumbnail in the Grid has loaded its bytes. */
const expectThumbnails = async (page: Page, count: number): Promise<void> => {
  const images = page.locator('.items-results img');
  await expect(images).toHaveCount(count);
  for (let at = 0; at < count; at += 1) {
    await expect
      .poll(() => images.nth(at).evaluate((element: HTMLImageElement) => (element.complete ? element.naturalWidth : 0)), {
        timeout: 15_000,
      })
      .toBeGreaterThan(0);
  }
};

test('every item that matches goes to the trash with its media, and undo brings them back with their thumbnails', async ({
  page,
  diagnostics,
  baseURL,
}) => {
  const title = 'Bulk e2e trash';
  await makeImageItems(page, title);
  await signIn(page, baseURL);

  await page.goto(`/#/items?q=${encodeURIComponent(title)}`);
  await page.getByRole('button', { name: 'Grid' }).click();
  await expectThumbnails(page, COUNT);

  await page.getByRole('button', { name: 'Delete items…' }).click();
  const panel = page.getByRole('region', { name: 'Delete items' });
  await expect(panel.getByRole('radio', { name: `All ${String(COUNT)} items that match` })).toBeChecked();
  await panel.getByRole('button', { name: 'Preview' }).click();
  await expect(panel.getByText(`${String(COUNT)} items go to the trash.`)).toBeVisible();
  await expect(panel.getByText(new RegExp(`^${String(COUNT)} media files go too: .+ in all\\.$`))).toBeVisible();
  const move = panel.getByRole('button', { name: `Move ${String(COUNT)} items to the trash` });
  await expect(move).toBeDisabled();
  await panel.getByRole('textbox', { name: `To confirm, type the number of items, ${String(COUNT)}` }).fill(String(COUNT));
  await diagnostics.idle();
  await expectAxeBudget(page, 'items bulk delete confirm');
  await shoot(page, 'confirm', 'bulk4');

  await move.click();
  await expect(panel.getByText(`${String(COUNT)} items moved to the trash.`)).toBeVisible({ timeout: 15_000 });
  await expect(page.locator('.items-results img')).toHaveCount(0);
  await expect(page.getByRole('link', { name: `${title} 1` })).toHaveCount(0);
  await expectAxeBudget(page, 'items bulk delete done');
  await shoot(page, 'done', 'bulk4');
  await expectClean(diagnostics, 'bulk delete');

  await panel.getByRole('button', { name: 'Undo' }).click();
  await expect(panel.getByText(`Undo puts ${String(COUNT)} items back as they were before this change.`)).toBeVisible();
  await panel.getByRole('button', { name: 'Undo the change' }).click();
  await expect(panel.getByText(`Undone. ${String(COUNT)} items are back as they were.`)).toBeVisible({ timeout: 15_000 });
  await expectThumbnails(page, COUNT);
  await expect(page.getByRole('link', { name: `${title} 1` })).toBeVisible();

  await page.getByRole('button', { name: 'Recent bulk changes' }).click();
  const recent = page.getByRole('region', { name: 'Recent bulk changes' });
  await expect(recent.getByRole('row', { name: /Move to the trash/ }).first()).toContainText('Undone');
  await expectClean(diagnostics, 'bulk delete undo');
});
