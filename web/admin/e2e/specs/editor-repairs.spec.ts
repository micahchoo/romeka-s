/**
 * The page editor repairs in a real browser against the real server: a new
 * blank page previews at once, the page settings come back after a selection,
 * Undo and Redo work from the toolbar and the keyboard, the Keys popover opens
 * and closes, and a removed block comes back from its toast.
 */
import type { Page } from '@playwright/test';
import { expect, test } from '../fixtures';
import { readHarness } from '../harness/state';
import { SITE_SLUG } from '../harness/seed';

const PREVIEW = `/s/${SITE_SLUG}/preview`;

const siteId = async (page: Page): Promise<number> => {
  const sites: unknown = await (await page.request.get(`/api/sites?slug=${SITE_SLUG}`)).json();
  const site: unknown = Array.isArray(sites) ? sites[0] : sites;
  if (
    typeof site === 'object' &&
    site !== null &&
    'o:id' in site &&
    typeof site['o:id'] === 'number'
  ) {
    return site['o:id'];
  }
  throw new Error(`no site ${SITE_SLUG}`);
};

const shown = (page: Page) => page.frameLocator('iframe.live-frame-doc:not(.live-frame-doc--back)');

const isPreviewPost = (url: string, method: string): boolean =>
  url.includes(PREVIEW) && method === 'POST';

test.beforeEach(async ({ page }) => {
  // The preview POST is not keyed by the harness proxy, so the browser holds
  // the key the way a signed-in user's does.
  const harness = readHarness();
  await page.addInitScript(
    (key: { identity: string; credential: string }) => {
      window.localStorage.setItem('romeka.apiKey', JSON.stringify(key));
    },
    { identity: harness.keyIdentity, credential: harness.keyCredential },
  );
});

test('a new blank page previews under a placeholder title, with no preview error', async ({
  page,
}) => {
  const id = await siteId(page);
  await page.goto(`/#/sites/${String(id)}/pages/new`);
  const rendered = page.waitForResponse((response) =>
    isPreviewPost(response.url(), response.request().method()),
  );
  await page.getByRole('button', { name: 'Start blank' }).click();

  expect((await rendered).status()).toBe(200);
  await expect(shown(page).getByText('[Untitled]').first()).toBeVisible();
  await expect(page.getByText(/The preview could not be rendered/)).toHaveCount(0);
});

test('the editor deselects, undoes, lists its keys and restores a removed block', async ({
  page,
}) => {
  const id = await siteId(page);
  await page.goto(`/#/sites/${String(id)}/pages/new`);
  await page.getByRole('button', { name: 'Start blank' }).click();
  const toolbar = page.getByRole('region', { name: 'Editor actions' });
  const title = page.getByLabel('Title', { exact: true });
  const cards = page.locator('li.canvas-card');

  await page.getByRole('button', { name: 'Add your first block' }).click();
  await expect(cards).toHaveCount(1);
  await expect(title).toHaveCount(0);

  // The crumb names the selection, and its Page button brings the settings back.
  const crumb = page.getByRole('navigation', { name: 'Selection' });
  await expect(crumb).toContainText('Text');
  await crumb.getByRole('button', { name: 'Page' }).click();
  await expect(title).toBeVisible();

  // Escape on a focused card does the same.
  await cards.first().click();
  await expect(title).toHaveCount(0);
  await page.keyboard.press('Escape');
  await expect(title).toBeVisible();

  // A click on the preview outside every block and the site header and
  // footer does the same: the content area's left margin.
  await cards.first().click();
  await expect(title).toHaveCount(0);
  await shown(page)
    .locator('#content')
    .click({ position: { x: 5, y: 5 } });
  await expect(title).toBeVisible();

  // Undo and Redo, from the keyboard and from the toolbar.
  await cards.first().focus();
  await page.keyboard.press('ControlOrMeta+z');
  await expect(cards).toHaveCount(0);
  await page.keyboard.press('ControlOrMeta+Shift+z');
  await expect(cards).toHaveCount(1);
  await toolbar.getByRole('button', { name: 'Undo' }).click();
  await expect(cards).toHaveCount(0);
  await toolbar.getByRole('button', { name: 'Redo' }).click();
  await expect(cards).toHaveCount(1);

  // The Keys popover opens from the toolbar and Escape closes it.
  await toolbar.getByRole('button', { name: 'Keys' }).click();
  const keys = page.getByRole('dialog', { name: 'Keyboard shortcuts' });
  await expect(keys).toContainText('Save the page');
  await page.keyboard.press('Escape');
  await expect(keys).toHaveCount(0);

  // Delete removes the focused card at once; the toast's Undo brings it back.
  await cards.first().focus();
  await page.keyboard.press('Delete');
  await expect(cards).toHaveCount(0);
  const toast = page.locator('.editor-toast');
  await expect(toast).toContainText('Removed Text.');
  await toast.getByRole('button', { name: 'Undo' }).click();
  await expect(cards).toHaveCount(1);
  await expect(toast).toBeEmpty();
});
