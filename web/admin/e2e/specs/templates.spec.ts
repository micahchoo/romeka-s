/**
 * The page templates end to end: a new page starts from the "Exhibit landing"
 * built-in template, whose text fields are empty and show hints. The block
 * list says which blocks are not written yet, the preview shows the hint where
 * the heading goes, the saved page shows visitors only what the curator wrote,
 * and the site hub counts what is left.
 */
import type { Page } from '@playwright/test';
import { expect, test } from '../fixtures';
import { readHarness } from '../harness/state';
import { SITE_SLUG, SITE_TITLE } from '../harness/seed';

/** The preview frame on show. */
const shown = (page: Page) => page.frameLocator('iframe.live-frame-doc:not(.live-frame-doc--back)');

test.beforeEach(async ({ page }) => {
  // The preview POST goes to `/s/<slug>/preview`, which the harness proxy does
  // not key, so the browser holds the key the way a signed-in user's does.
  const harness = readHarness();
  await page.addInitScript(
    (key: { identity: string; credential: string }) => {
      window.localStorage.setItem('romeka.apiKey', JSON.stringify(key));
    },
    { identity: harness.keyIdentity, credential: harness.keyCredential },
  );
});

test('a page from the Exhibit landing template shows hints, not filler, until it is written', async ({
  page,
}) => {
  await page.goto('/#/sites');
  await page.getByRole('link', { name: SITE_TITLE }).click();
  await page.getByRole('link', { name: 'Add page' }).click();

  const chooser = page.getByRole('region', { name: 'Start from a template' });
  await chooser.getByRole('button', { name: /Exhibit landing/ }).click();

  // The template's four blocks are on the canvas, the hero first. The hero,
  // the introduction and the list of pages wait for the curator's words.
  const cards = page.locator('.canvas-card');
  await expect(cards).toHaveCount(4);
  await expect(cards.first()).toContainText('Hero banner');
  await expect(page.locator('.canvas-card[data-writing="unwritten"]')).toHaveCount(3);
  await expect(cards.first()).toContainText('Not written yet');
  await expect(cards.nth(3)).toContainText('No pages chosen');

  // The preview shows the hint, greyed, where the heading goes.
  await expect(shown(page).locator('.romeka-hero-heading .romeka-hint')).toHaveText(
    'e.g. Welcome to the collection',
  );

  // Writing the heading makes the hero written, in the list and in the frame.
  await cards.first().click();
  const heading = page.getByLabel('Heading', { exact: true });
  await expect(heading).toHaveAttribute('placeholder', 'e.g. Welcome to the collection');
  await heading.fill('Harbour letters');
  await expect(page.locator('.canvas-card[data-writing="unwritten"]')).toHaveCount(2);
  await expect(shown(page).locator('.romeka-hero-heading')).toHaveText('Harbour letters');

  // The page's own settings come back when the selection is cleared.
  await page.getByRole('button', { name: 'Page', exact: true }).click();
  await page.getByLabel('Title').fill('Exhibit');
  await page.getByLabel('Slug').fill('exhibit');
  await page.getByRole('button', { name: 'Add page' }).click();

  // The create navigates to the new page's editor, which reloads it from the
  // server; there is no "Saved." status to wait on.
  await expect(page).toHaveURL(/#\/sites\/1\/pages\/\d+/);

  // Visitors see the hero the curator wrote, and nothing of the two blocks
  // still empty: no hint, no filler, no empty wrapper.
  const response = await page.goto(`/s/${SITE_SLUG}/page/exhibit`);
  expect(response?.status()).toBe(200);
  const hero = page.locator('.romeka-hero');
  await expect(hero).toHaveCount(1);
  await expect(hero.locator('.romeka-hero-heading')).toHaveText('Harbour letters');
  await expect(page.locator('.romeka-hint')).toHaveCount(0);
  await expect(page.locator('.blocks-inner > .block')).toHaveCount(2);

  // The site hub says how many blocks of the page are left to write.
  await page.goto('/#/sites/1');
  await expect(page.getByRole('row', { name: /Exhibit/ })).toContainText('2 blocks not written');
});
