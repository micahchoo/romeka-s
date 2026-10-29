/**
 * The page editor against the real server and database: add an HTML block,
 * type into it, move it, save, reload and see it persisted, then read the
 * public page the port renders. The live preview is not exercised, because the
 * route may not exist yet.
 */
import { expect, test } from '../fixtures';
import { SITE_SLUG, SITE_TITLE, WELCOME_SLUG } from '../harness/seed';

const TYPED = 'Persisted by the page editor.';

test('the editor adds, moves and saves a block and the public page shows it', async ({ page }) => {
  await page.goto('/#/sites');
  await page.getByRole('link', { name: SITE_TITLE }).click();
  await page.getByRole('link', { name: 'Welcome' }).click();
  await expect(
    page.getByRole('button', { name: /Insert block at position/ }).first(),
  ).toBeVisible();

  // A new HTML block goes in at the end of the page.
  await page.getByRole('button', { name: /Insert block at position/ }).last().click();
  const picker = page.getByRole('dialog', { name: 'Insert a block or template' });
  await picker.getByRole('button', { name: 'Text', exact: true }).click();

  // The shell selects the new block, so its form is the inspector.
  const editor = page.getByRole('textbox', { name: 'Text', exact: true });
  await editor.fill(TYPED);

  // Space picks the card up, ArrowUp moves it one position, Space drops it.
  const card = page.locator('.canvas-card').filter({ hasText: TYPED });
  await expect(card).toHaveCount(1);
  const before = await card.getAttribute('aria-label');
  await card.focus();
  await page.keyboard.press(' ');
  await page.keyboard.press('ArrowUp');
  await page.keyboard.press(' ');
  const moved = await card.getAttribute('aria-label');
  expect(moved).not.toBe(before);

  await page.getByRole('button', { name: 'Save' }).click();
  await expect(page.getByText('Saved.')).toBeVisible();

  // The reload reads the saved page back from the database.
  await page.reload();
  const reloaded = page.locator('.canvas-card').filter({ hasText: TYPED });
  await expect(reloaded).toHaveCount(1);
  expect(await reloaded.getAttribute('aria-label')).toBe(moved);

  const response = await page.goto(`/s/${SITE_SLUG}/page/${WELCOME_SLUG}`);
  expect(response?.status()).toBe(200);
  await expect(page.getByText(TYPED)).toBeVisible();
});
