/**
 * The hero with a background from a media of this site, end to end: the
 * curator finds the item, the media is chosen, the focal point and height are
 * set, the live preview shows the image, the save stores it, and the public
 * page renders it. A private media is offered but cannot be chosen.
 */
import { deflateSync, crc32 } from 'node:zlib';
import type { Page } from '@playwright/test';
import { expect, expectClean, test } from '../fixtures';
import { SITE_SLUG } from '../harness/seed';
import { readHarness } from '../harness/state';

const PAGE_SLUG = 'hero-media';
const HEADING = 'Charts of the harbour';

const idOf = (value: unknown): number => {
  if (typeof value === 'object' && value !== null && 'o:id' in value && typeof value['o:id'] === 'number') {
    return value['o:id'];
  }
  throw new Error(`no o:id in ${JSON.stringify(value)}`);
};

const first = (value: unknown): unknown => (Array.isArray(value) ? value[0] : value);

/** A `width` by `height` PNG: a sky gradient over a dark band of land. */
const png = (width: number, height: number): Buffer => {
  const rows: Buffer[] = [];
  for (let y = 0; y < height; y += 1) {
    const row = Buffer.alloc(1 + width * 3);
    for (let x = 0; x < width; x += 1) {
      const land = y > height * 0.7;
      row.set(land ? [40, 60, 40] : [90 + Math.round((y / height) * 120), 140, 200], 1 + x * 3);
    }
    rows.push(row);
  }
  const chunk = (kind: string, body: Buffer): Buffer => {
    const length = Buffer.alloc(4);
    length.writeUInt32BE(body.length);
    const typed = Buffer.concat([Buffer.from(kind, 'ascii'), body]);
    const sum = Buffer.alloc(4);
    sum.writeUInt32BE(crc32(typed));
    return Buffer.concat([length, typed, sum]);
  };
  const header = Buffer.alloc(13);
  header.writeUInt32BE(width, 0);
  header.writeUInt32BE(height, 4);
  header.set([8, 2, 0, 0, 0], 8);
  return Buffer.concat([
    Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]),
    chunk('IHDR', header),
    chunk('IDAT', deflateSync(Buffer.concat(rows))),
    chunk('IEND', Buffer.alloc(0)),
  ]);
};

/** An item holding one uploaded image, public or private, made through the API. */
const createItem = async (page: Page, title: string, mediaPublic: boolean): Promise<number> => {
  const properties: unknown = await (await page.request.get('/api/properties?term=dcterms:title')).json();
  const property = idOf(first(properties));
  const created = await page.request.post('/api/items', {
    multipart: {
      data: JSON.stringify({
        'o:is_public': true,
        'dcterms:title': [{ type: 'literal', property_id: property, '@value': title }],
        'o:media': [{ 'o:ingester': 'upload', file_index: 0, 'o:is_public': mediaPublic }],
      }),
      file: { name: 'harbour.png', mimeType: 'image/png', buffer: png(1200, 600) },
    },
  });
  expect(created.status(), await created.text()).toBe(200);
  return idOf(await created.json());
};

/** A fresh page with one HTML block, so the spec owns what it edits. */
const createPage = async (page: Page, slug: string): Promise<string> => {
  const site = idOf(first(await (await page.request.get(`/api/sites?slug=${SITE_SLUG}`)).json()));
  const created = await page.request.post('/api/site_pages', {
    data: {
      'o:site': { 'o:id': site },
      'o:title': 'Hero media',
      'o:slug': slug,
      'o:is_public': true,
      'o:block': [{ 'o:layout': 'html', 'o:data': { html: '<p>Below the banner</p>' } }],
    },
  });
  expect(created.status(), await created.text()).toBe(200);
  return `/#/sites/${String(site)}/pages/${String(idOf(await created.json()))}`;
};

const shown = (page: Page) => page.frameLocator('iframe.live-frame-doc:not(.live-frame-doc--back)');

/** Insert a hero and point its background at a media of the item `title`. */
const heroFromMedia = async (page: Page, title: string) => {
  await page.getByRole('button', { name: /Insert block at position/ }).first().click();
  await page
    .getByRole('dialog', { name: 'Insert a block or template' })
    .getByRole('button', { name: 'Hero banner' })
    .click();
  const form = page.getByRole('region', { name: 'Block inspector' });
  await form.getByRole('textbox', { name: 'Heading', exact: true }).fill(HEADING);
  await form.getByLabel('Background from').selectOption('media');
  await form.getByLabel('Search titles and values').fill(title);
  await form
    .getByRole('listbox', { name: 'Find the item' })
    .getByRole('option', { name: title, exact: true, selected: false })
    .click();
  return form;
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

test('a hero with a media of this site previews, saves and renders publicly', async ({ page, diagnostics }) => {
  const title = 'E2E hero harbour photograph';
  await createItem(page, title, true);
  await page.goto(await createPage(page, PAGE_SLUG));
  const form = await heroFromMedia(page, title);

  // The item has one image, so it is chosen at once.
  await expect(form.getByRole('radio', { name: 'harbour.png' })).toBeChecked();
  await form.getByLabel('Height', { exact: true }).selectOption('tall');
  await expect(form.getByRole('button', { name: /^Focal point: 50% across, 50% down/ })).toBeVisible();
  const focal = form.getByRole('button', { name: /^Focal point/ });
  await expect(focal.locator('img')).toBeVisible();
  const rendered = page.waitForResponse(
    (response) => response.url().includes(`/s/${SITE_SLUG}/preview`) && response.request().method() === 'POST',
  );
  await focal.press('ArrowDown');
  await focal.press('ArrowDown');
  await expect(form.getByRole('button', { name: /^Focal point: 50% across, 60% down/ })).toBeVisible();
  await rendered;

  const preview = shown(page).locator('.romeka-hero-background');
  await expect(preview).toHaveAttribute('src', /\/files\/(large|original)\//);
  await expect(preview).toHaveAttribute('style', 'object-position: 50% 60%');

  await page.getByRole('button', { name: 'Save' }).click();
  await expect(page.getByText('Saved.')).toBeVisible();

  diagnostics.reset();
  const response = await page.goto(`/s/${SITE_SLUG}/page/${PAGE_SLUG}`);
  expect(response?.status()).toBe(200);
  const hero = page.locator('.romeka-hero');
  await expect(hero).toHaveClass(/romeka-hero-height-tall/);
  await expect(hero.locator('.romeka-hero-heading')).toHaveText(HEADING);
  const image = hero.locator('.romeka-hero-background');
  await expect(image).toHaveAttribute('src', /\/files\/(large|original)\//);
  await expect(image).toHaveCSS('object-position', '50% 60%');
  await expect.poll(() => image.evaluate((element: HTMLImageElement) => element.naturalWidth)).toBeGreaterThan(0);
  await expectClean(diagnostics, 'the public page with a media hero');
});

test('a private media is shown but cannot be chosen', async ({ page }) => {
  const title = 'E2E hero private scan';
  await createItem(page, title, false);
  await page.goto(await createPage(page, `${PAGE_SLUG}-private`));
  const form = await heroFromMedia(page, title);
  const radio = form.getByRole('radio', { name: /Private/ });
  await expect(radio).toBeDisabled();
  await expect(radio).not.toBeChecked();
});

test('a IIIF image from a media of this site opens through the Image API', async ({ page, diagnostics }) => {
  const title = 'E2E IIIF estuary scan';
  await createItem(page, title, true);
  await page.goto(await createPage(page, 'iiif-media'));
  await page.getByRole('button', { name: /Insert block at position/ }).last().click();
  await page
    .getByRole('dialog', { name: 'Insert a block or template' })
    .getByRole('button', { name: 'IIIF image', exact: true })
    .click();
  const form = page.getByRole('region', { name: 'Block inspector' });
  await form.getByLabel('Image from').selectOption('media');
  await form.getByLabel('Search titles and values').fill(title);
  await form
    .getByRole('listbox', { name: 'Find the item' })
    .getByRole('option', { name: title, exact: true, selected: false })
    .click();
  await expect(form.getByRole('radio', { name: 'harbour.png' })).toBeChecked();
  await page.getByRole('button', { name: 'Save' }).click();
  await expect(page.getByText('Saved.')).toBeVisible();

  diagnostics.reset();
  const response = await page.goto(`/s/${SITE_SLUG}/page/iiif-media`);
  expect(response?.status()).toBe(200);
  const source = await page.locator('.block-iiifImage script:not([src])').textContent();
  expect(source).toMatch(/"tileSources":"\/iiif\/image\/3\/\d+\/info\.json"/);
  await expect(page.locator('.block-iiifImage .openseadragon-canvas canvas').first()).toBeVisible({
    timeout: 15_000,
  });
  await expectClean(diagnostics, 'the public page with a site media IIIF image');
});
