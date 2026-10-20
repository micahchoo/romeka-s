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
 *
 * The resource page sections reorder by keyboard, hide and show, and the
 * preview renders an item page of the site under the draft order.
 */
import type { FrameLocator, Locator, Page } from '@playwright/test';

import { expect, test } from '../fixtures';
import { readHarness } from '../harness/state';
import { isArray, numberField, parseJson, record } from '../harness/json';
import { ITEM_TITLES, SITE_SLUG, SITE_TITLE, WELCOME_SLUG } from '../harness/seed';

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

  // One click applies Community: its brand band is the header's background.
  await page.getByRole('button', { name: /^Community/ }).click();
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

test('a preset sets the page structure and the Footer section reaches the public page', async ({ page }) => {
  await holdKey(page);
  await openThemePage(page);
  const frame = previewFrame(page);

  await page.getByRole('button', { name: /^Exhibit/ }).click();
  // Exhibit's structure is classes on the root, and its fonts load from the theme.
  await expect
    .poll(() => frame.locator('html').getAttribute('class'), { timeout: 10_000 })
    .toBe('hero-cover cards-caption search-icon footer-columns item-stacked');

  const footer = page.getByRole('group', { name: 'Footer' });
  await footer.getByRole('button', { name: 'Add a column' }).click();
  await footer.getByRole('textbox', { name: 'Column 1' }).fill('Open Tuesday to Sunday.');
  await footer.getByLabel('Licence').selectOption('cc-by');
  await footer.getByRole('checkbox', { name: 'Show "Powered by Romeka S"' }).uncheck();
  await expect(frame.locator('.licence-badge')).toHaveText('ccCC BY 4.0', { timeout: 10_000 });

  await page.getByRole('button', { name: 'Save' }).click();
  await expect(page.getByText('Saved.')).toBeVisible();

  await page.goto(`/s/${SITE_SLUG}/page/${WELCOME_SLUG}`);
  const publicFooter = page.getByRole('contentinfo');
  await expect(publicFooter.getByText('Open Tuesday to Sunday.')).toBeVisible();
  await expect(publicFooter.getByRole('link', { name: 'CC BY 4.0' })).toHaveAttribute(
    'href',
    'https://creativecommons.org/licenses/by/4.0/',
  );
  await expect(publicFooter.getByText('Powered by Romeka S')).toHaveCount(0);
  // The menu's links repeat in the footer until the curator sets their own.
  await expect(publicFooter.getByRole('navigation', { name: 'Footer links' }).getByRole('link').first()).toBeVisible();
  const font = await page.evaluate(async () => {
    await document.fonts.ready;
    return [...document.fonts].filter((face) => face.status === 'loaded').map((face) => face.family);
  });
  expect(font).toContain('Fraunces');
});

test('custom CSS the sanitiser refuses shows its error and is not saved', async ({ page }) => {
  await holdKey(page);
  await openThemePage(page);

  await page.getByRole('tab', { name: 'Custom CSS' }).click();
  const editor = page.getByRole('textbox', { name: 'Custom CSS' });
  await editor.fill('.site-title a { color: blue; }\n@import url(https://evil.example/x.css);');
  await expect(page.getByRole('button', { name: /^Custom CSS: Line 2, column 1: @import is not allowed/ })).toBeVisible({
    timeout: 10_000,
  });
  await expect(page.getByRole('button', { name: 'Save' })).toBeDisabled();

  // The stored CSS is still the last good one.
  await page.goto(`/s/${SITE_SLUG}/page/${WELCOME_SLUG}`);
  const style = await page.locator('#custom-css').textContent();
  expect(style).not.toContain('@import');
  expect(style).not.toContain('blue');
});

/** An API call with the harness key, failing the test on any status but 200. */
const api = async (page: Page, method: 'GET' | 'POST', path: string, body?: unknown): Promise<unknown> => {
  const { baseUrl, keyIdentity, keyCredential } = readHarness();
  const separator = path.includes('?') ? '&' : '?';
  const response = await page.request.fetch(
    `${baseUrl}${path}${separator}key_identity=${keyIdentity}&key_credential=${keyCredential}`,
    { method, ...(body === undefined ? {} : { data: body }) },
  );
  expect(response.status(), `${method} ${path}`).toBe(200);
  return parseJson(await response.text());
};

/** A site of its own holding the seeded first item, so the item preview has a record. */
const siteWithItem = async (page: Page): Promise<number> => {
  const site = numberField(
    record(
      await api(page, 'POST', '/api/sites', {
        'o:title': 'E2E sections site',
        'o:slug': 'e2e-sections-site',
        'o:theme': 'default',
        'o:is_public': true,
      }),
      'site',
    ),
    'o:id',
    'site',
  );
  const [title] = ITEM_TITLES;
  const found = await api(page, 'GET', `/api/items?search=${encodeURIComponent(title ?? '')}`);
  const item = numberField(record(isArray(found) ? found[0] : null, 'item'), 'o:id', 'item');
  await api(page, 'POST', `/api/sites/${String(site)}/items`, { action: 'add', items: [item] });
  return site;
};

/** The names of the shown sections of a resource, in order. */
const shownSections = (page: Page, resource: string): Locator =>
  page.getByRole('list', { name: `Sections shown on ${resource} pages` }).locator('.resource-page-block-label');

/** Where each selector first appears in the preview document, -1 when absent. */
const order = (frame: FrameLocator, selectors: readonly string[]): Promise<number[]> =>
  frame.locator('html').evaluate((root, wanted) => {
    const all = [...root.querySelectorAll('*')];
    return wanted.map((selector) => {
      const found = root.querySelector(selector);
      return found === null ? -1 : all.indexOf(found);
    });
  }, selectors);

test('the resource page sections reorder by keyboard, hide, show and preview on an item', async ({ page }) => {
  await holdKey(page);
  const site = await siteWithItem(page);
  await page.goto(`/#/sites/${String(site)}/theme`);

  // The Items resource starts from the theme's set, each section with its line.
  const items = page.getByRole('group', { name: 'Items' });
  await expect(items.getByText('The metadata, field by field.')).toBeVisible();
  await expect(shownSections(page, 'Items')).toHaveText([
    'Media embeds',
    'Values',
    'Item sets',
    'Site pages',
    'Media list',
    'Linked resources',
  ]);

  // Hide puts a section in the hidden list; Show brings it back at the end.
  await items.getByRole('button', { name: 'Hide Media list' }).click();
  const hidden = items.getByRole('list', { name: 'Hidden sections of Items pages' });
  await expect(hidden.getByText('Media list')).toBeVisible();
  await hidden.getByRole('button', { name: 'Show Media list' }).click();
  await expect(shownSections(page, 'Items').last()).toHaveText('Media list');
  await items.getByRole('button', { name: 'Hide Media list' }).click();
  await items.getByRole('button', { name: 'Hide Media embeds' }).click();

  // Space picks Item sets up, the arrow moves it, Space drops it.
  const handle = items.getByRole('button', { name: 'Move Item sets, 2 of 4' });
  await handle.focus();
  await page.keyboard.press('Space');
  await page.keyboard.press('ArrowUp');
  await expect(items.getByRole('status')).toHaveText('Item sets: position 1 of 4.');
  await page.keyboard.press('Space');
  await expect(items.getByRole('button', { name: 'Move Item sets, 1 of 4' })).toBeFocused();
  await expect(shownSections(page, 'Items')).toHaveText(['Item sets', 'Values', 'Site pages', 'Linked resources']);

  // The preview renders the site's item under the draft order.
  await page.getByRole('combobox', { name: 'Preview' }).selectOption('items');
  const frame = previewFrame(page);
  await expect(frame.locator('dl.item-sets')).toBeVisible({ timeout: 10_000 });
  const [sets, values, embeds, list] = await order(frame, [
    'dl.item-sets',
    'dl.values',
    '.media-embeds',
    'dl.media-list',
  ]);
  expect(sets, 'item sets render').toBeGreaterThan(-1);
  expect(values, 'values follow the item sets').toBeGreaterThan(sets ?? 0);
  expect(embeds, 'the hidden media embeds stay out').toBe(-1);
  expect(list, 'the hidden media list stays out').toBe(-1);

  await page.getByRole('button', { name: 'Save' }).click();
  await expect(page.getByText('Saved.')).toBeVisible();

  // Reloading reads the stored map back through `GET /api/sites/<id>`, so the
  // list the editor shows is what storage holds.
  await page.reload();
  await expect(shownSections(page, 'Items')).toHaveText(['Item sets', 'Values', 'Site pages', 'Linked resources']);
  await expect(
    page.getByRole('group', { name: 'Items' }).getByRole('list', { name: 'Hidden sections of Items pages' }),
  ).toContainText('Media list');
});
