/**
 * The site Theme page: a curator picks a preset, changes the brand colour,
 * adds custom CSS, sees each change in the live preview, saves, and the public
 * page renders it. CSS the sanitiser refuses shows its error and is not saved.
 *
 * The preview is the sandboxed iframe the page editor also renders, and the
 * public page is the real `/s/<slug>` route. The harness proxy forwards the
 * theme's `/assets/css/style.css` to the port the way `deploy/Caddyfile` does,
 * so both are styled by the real sheet, and the spec asserts that with a
 * computed style only the sheet sets.
 */
import type { FrameLocator, Page } from '@playwright/test';

import { expect, test } from '../fixtures';
import { readHarness } from '../harness/state';
import { SITE_SLUG, SITE_TITLE, WELCOME_SLUG } from '../harness/seed';

/** A brand the curator picks after the preset. */
const NEW_BRAND = '#7c3aed';
/** The colour the custom CSS gives the site title. */
const TITLE_RED = 'rgb(204, 0, 0)';

/** Open the Theme page for the seeded site. */
const openThemePage = async (page: Page): Promise<void> => {
  await page.goto('/#/sites');
  await page.getByRole('row', { name: new RegExp(SITE_TITLE) }).getByRole('link').first().click();
  const match = /#(\/sites\/\d+)$/.exec(page.url());
  if (match?.[1] === undefined) {
    throw new Error(`no site detail hash in ${page.url()}`);
  }
  await page.goto(`/#${match[1]}/theme`);
};

/** The preview POST goes to `/s/<slug>/preview`, which the harness proxy does
 * not key, so the browser holds the key the way a signed-in user's does. */
const holdKey = async (page: Page): Promise<void> => {
  const harness = readHarness();
  await page.addInitScript(
    (key: { identity: string; credential: string }) => {
      window.localStorage.setItem('romeka.apiKey', JSON.stringify(key));
    },
    { identity: harness.keyIdentity, credential: harness.keyCredential },
  );
};

const previewFrame = (page: Page): FrameLocator => page.frameLocator('iframe[title="Preview of the page"]');

const rootProperty = (frame: FrameLocator, name: string): Promise<string> =>
  frame
    .locator('html')
    .evaluate((element, property) => getComputedStyle(element).getPropertyValue(property).trim(), name);

const computed = (frame: FrameLocator, selector: string, property: string): Promise<string> =>
  frame
    .locator(selector)
    .first()
    .evaluate((element, name) => getComputedStyle(element).getPropertyValue(name), property);

test('a preset, a brand and custom CSS reach the preview and the public page', async ({ page }) => {
  await holdKey(page);
  await openThemePage(page);
  const frame = previewFrame(page);

  // The theme sheet loads in the preview: only `style.css` lays the header
  // out as a flex row, and only it hides the skip link off screen.
  await expect.poll(() => computed(frame, '.header-inner', 'display'), { timeout: 10_000 }).toBe('flex');
  expect(await computed(frame, '.skip-link', 'position')).toBe('absolute');

  // One click applies Civic: its brand band is the header's background.
  await page.getByRole('button', { name: /^Civic/ }).click();
  await expect.poll(() => rootProperty(frame, '--color-brand'), { timeout: 10_000 }).toBe('#0b57d0');
  await expect
    .poll(() => computed(frame, 'header', 'background-color'), { timeout: 10_000 })
    .toBe('rgb(11, 87, 208)');

  // A new brand colour derives a new palette, AA by construction.
  await page.locator('#theme-setting-brand_color').fill(NEW_BRAND);
  await expect.poll(() => rootProperty(frame, '--color-brand'), { timeout: 10_000 }).toBe(NEW_BRAND);
  await expect(page.getByText(/Every text and background pair passes/)).toBeVisible();

  // Custom CSS overrides the theme in the preview.
  await page.getByRole('tab', { name: 'Custom CSS' }).click();
  const editor = page.getByRole('textbox', { name: 'Custom CSS' });
  await editor.fill(`.site-title a { color: ${TITLE_RED}; }`);
  await expect.poll(() => computed(frame, '.site-title a', 'color'), { timeout: 10_000 }).toBe(TITLE_RED);

  await page.getByRole('button', { name: 'Save' }).click();
  await expect(page.getByText('Saved.')).toBeVisible();

  await page.goto(`/s/${SITE_SLUG}/page/${WELCOME_SLUG}`);
  const css = (selector: string, property: string): Promise<string> =>
    page.locator(selector).first().evaluate((element, name) => getComputedStyle(element).getPropertyValue(name), property);
  expect(await page.evaluate(() => getComputedStyle(document.documentElement).getPropertyValue('--color-brand').trim())).toBe(
    NEW_BRAND,
  );
  expect(await css('.header-inner', 'display')).toBe('flex');
  expect(await css('.site-title a', 'color')).toBe(TITLE_RED);
  expect(await css('header', 'background-color')).toBe('rgb(124, 58, 237)');
});

test('custom CSS the sanitiser refuses shows its error and is not saved', async ({ page }) => {
  await holdKey(page);
  await openThemePage(page);

  await page.getByRole('tab', { name: 'Custom CSS' }).click();
  const editor = page.getByRole('textbox', { name: 'Custom CSS' });
  await editor.fill('.site-title a { color: blue; }\n@import url(https://evil.example/x.css);');
  await expect(page.getByRole('button', { name: /^Line 2, column 1: @import is not allowed/ })).toBeVisible({
    timeout: 10_000,
  });
  await expect(page.getByRole('button', { name: 'Save' })).toBeDisabled();

  // The stored CSS is still the last good one.
  await page.goto(`/s/${SITE_SLUG}/page/${WELCOME_SLUG}`);
  const style = await page.locator('#custom-css').textContent();
  expect(style).not.toContain('@import');
  expect(style).not.toContain('blue');
});

test('the resource page block editor saves a reordered list', async ({ page }) => {
  await openThemePage(page);

  // The Items resource starts from the theme's set: media embeds, values, item
  // sets, site pages, media list and linked resources.
  const items = page.getByRole('group', { name: 'Items' });
  await expect(items.getByRole('button', { name: 'Remove Media list' })).toBeVisible();
  await items.getByRole('button', { name: 'Remove Media list' }).click();
  await items.getByRole('button', { name: 'Move Values up' }).click();

  await page.getByRole('button', { name: 'Save' }).click();
  await expect(page.getByText('Saved.')).toBeVisible();

  // Reloading reads the stored map back through `GET /api/sites/<id>`, so the
  // list the editor shows is what storage holds.
  await openThemePage(page);
  const reloaded = page.getByRole('group', { name: 'Items' });
  await expect(reloaded.getByRole('button', { name: 'Remove Values' })).toBeVisible();
  await expect(reloaded.getByRole('button', { name: 'Remove Media list' })).toHaveCount(0);
});
