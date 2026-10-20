/**
 * The map end to end: a site names its location field, an item gets a point
 * through the item form, a map block picks that item, and the public page
 * draws a Leaflet marker for it. The spec makes its own site and deletes it at
 * the end, because a site with a location field adds the place control to
 * every item form the later specs open.
 */
import type { Page } from '@playwright/test';
import { expect, test } from '../fixtures';
import { readHarness } from '../harness/state';

const SITE = 'Map Site';
const SLUG = 'map-site';
const ITEM = 'Tower Bridge';
const PLACE = '51.5055, -0.0754';

const createSite = async (page: Page): Promise<number> => {
  await page.goto('/#/sites/new');
  await page.getByLabel('Title').fill(SITE);
  await expect(page.getByLabel('Slug')).toHaveValue(SLUG);
  await page.getByRole('button', { name: 'Add site' }).click();
  await expect(page).toHaveURL(/#\/sites\/\d+$/);
  const id = /#\/sites\/(\d+)$/.exec(page.url())?.[1];
  if (id === undefined) {
    throw new Error(`no site id in ${page.url()}`);
  }
  return Number(id);
};

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

test('a point set on an item shows as a marker on the public map', async ({ page, baseURL }) => {
  const siteId = await createSite(page);
  try {
    await page.goto(`/#/sites/${String(siteId)}/settings`);
    const maps = page.getByRole('group', { name: 'Maps' });
    await maps.getByRole('combobox', { name: 'Location field' }).selectOption('dcterms:spatial');
    // Tiles from this origin, so the run never asks OpenStreetMap for any.
    await maps.getByLabel('Map tiles').fill(`${baseURL}/files/no-tiles/{z}/{x}/{y}.png`);
    await page.getByRole('button', { name: 'Save' }).click();
    await expect(page.getByText('Saved.')).toBeVisible();

    await page.goto('/#/items/new');
    await page.getByRole('textbox', { name: 'Title', exact: true }).fill(ITEM);
    const place = page.getByRole('group', { name: 'Place on a map: Spatial Coverage' });
    await place.getByRole('button', { name: 'Add a place' }).click();
    await place.getByRole('textbox', { name: 'Place 1' }).fill(PLACE);
    await expect(place.locator('.leaflet-interactive')).toHaveCount(1);
    await page.getByRole('checkbox', { name: SITE }).check();
    await page.getByRole('button', { name: 'Add item' }).click();
    await expect(page).toHaveURL(/#\/items\/\d+$/);

    await page.goto(`/#/sites/${String(siteId)}`);
    await page.getByRole('link', { name: 'Welcome' }).click();
    await page
      .getByRole('button', { name: /Insert block at position/ })
      .last()
      .click();
    await page
      .getByRole('dialog', { name: 'Insert a block or template' })
      .getByRole('button', { name: 'Map', exact: true })
      .click();
    const form = page.getByRole('region', { name: 'Block inspector' });
    await form.getByLabel('Search titles and values').fill(ITEM);
    await form
      .getByRole('listbox', { name: 'Items' })
      .getByRole('option', { name: ITEM, exact: true, selected: false })
      .click();
    await expect(form.getByRole('region', { name: 'Picked' })).toContainText(ITEM);
    await form.getByLabel('Group nearby points').check();

    // The script-free preview shows the poster with the place, not the map.
    const preview = page.frameLocator('iframe.live-frame-doc:not(.live-frame-doc--back)');
    await expect(preview.locator('.romeka-map-poster')).toContainText(`${ITEM} ${PLACE}`);

    await page.getByRole('button', { name: 'Save' }).click();
    await expect(page.getByText('Saved.')).toBeVisible();

    const response = await page.goto(`/s/${SLUG}/page/welcome`);
    expect(response?.status()).toBe(200);
    const marker = page.locator('.romeka-map-canvas .leaflet-marker-icon');
    await expect(marker).toHaveCount(1);
    await marker.click();
    await expect(page.locator('.romeka-map-popup-title')).toHaveText(ITEM);
    await expect(page.locator('.romeka-map-list')).toContainText(`${ITEM} ${PLACE}`);
  } finally {
    const removed = await page.request.delete(`/api/sites/${String(siteId)}`);
    expect(removed.ok(), await removed.text()).toBe(true);
  }
});

test('a page without a map loads no map code', async ({ page }) => {
  const scripts: string[] = [];
  page.on('request', (request) => {
    scripts.push(request.url());
  });
  const response = await page.goto('/s/e2e-site/page/welcome');
  expect(response?.status()).toBe(200);
  expect(scripts.filter((url) => url.includes('leaflet') || url.includes('/js/map.js'))).toEqual(
    [],
  );
});
