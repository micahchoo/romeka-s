/**
 * The page templates end to end: a new page starts from the "Exhibit landing"
 * built-in template, saves, and the public page renders the hero the template
 * laid down.
 */
import { expect, test } from '../fixtures';
import { SITE_SLUG, SITE_TITLE } from '../harness/seed';

test('a new page from the Exhibit landing template saves and renders its hero', async ({
  page,
}) => {
  await page.goto('/#/sites');
  await page.getByRole('link', { name: SITE_TITLE }).click();
  await page.getByRole('link', { name: 'Add page' }).click();

  const chooser = page.getByRole('region', { name: 'Start from a template' });
  await chooser.getByRole('button', { name: /Exhibit landing/ }).click();

  // The template's four blocks are on the canvas, the hero first.
  await expect(page.locator('.canvas-card')).toHaveCount(4);
  await expect(page.locator('.canvas-card').first()).toContainText('Hero banner');

  await page.getByLabel('Title').fill('Exhibit');
  await page.getByLabel('Slug').fill('exhibit');
  await page.getByRole('button', { name: 'Add page' }).click();

  // The create navigates to the new page's editor, which reloads it from the
  // server; there is no "Saved." status to wait on.
  await expect(page).toHaveURL(/#\/sites\/1\/pages\/\d+/);

  const response = await page.goto(`/s/${SITE_SLUG}/page/exhibit`);
  expect(response?.status()).toBe(200);
  const hero = page.locator('.romeka-hero');
  await expect(hero).toHaveCount(1);
  await expect(hero.locator('.romeka-hero-heading')).toHaveText('Your exhibit title');
});
