/**
 * The IIIF viewer end to end: a curator adds a IIIF presentation block that
 * opens an item of this site, the script-free preview shows its poster, and
 * the public page opens the item's manifest in Mirador, served from this
 * origin under the viewer page's policy.
 */
import { crc32, deflateSync } from 'node:zlib';
import type { Page } from '@playwright/test';
import { expect, expectClean, test } from '../fixtures';
import { SITE_SLUG } from '../harness/seed';
import { readHarness } from '../harness/state';

const ITEM_TITLE = 'E2E IIIF harbour chart';
const BLOCK_TITLE = 'The harbour chart';
const PAGE_SLUG = 'iiif-viewer';

const idOf = (value: unknown): number => {
  if (typeof value === 'object' && value !== null && 'o:id' in value && typeof value['o:id'] === 'number') {
    return value['o:id'];
  }
  throw new Error(`no o:id in ${JSON.stringify(value)}`);
};

const first = (value: unknown): unknown => (Array.isArray(value) ? value[0] : value);

/** A `width` by `height` PNG: a blue sea with a red band, so a zoom shows. */
const png = (width: number, height: number): Buffer => {
  const rows: Buffer[] = [];
  for (let y = 0; y < height; y += 1) {
    const row = Buffer.alloc(1 + width * 3);
    for (let x = 0; x < width; x += 1) {
      const band = Math.abs(y - height / 2) < height / 12;
      row.set(band ? [200, 40, 40] : [30, 90, 160 + Math.round((x / width) * 80)], 1 + x * 3);
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

/** An item holding one uploaded image, created through the API. */
const createItem = async (page: Page): Promise<number> => {
  const properties: unknown = await (await page.request.get('/api/properties?term=dcterms:title')).json();
  const title = idOf(first(properties));
  const created = await page.request.post('/api/items', {
    multipart: {
      data: JSON.stringify({
        'o:is_public': true,
        'dcterms:title': [{ type: 'literal', property_id: title, '@value': ITEM_TITLE }],
        'o:media': [{ 'o:ingester': 'upload', file_index: 0 }],
      }),
      file: { name: 'harbour.png', mimeType: 'image/png', buffer: png(1600, 1000) },
    },
  });
  expect(created.status(), await created.text()).toBe(200);
  return idOf(await created.json());
};

/** A fresh page with one HTML block, so the spec owns what it edits. */
const createPage = async (page: Page): Promise<string> => {
  const site = idOf(first(await (await page.request.get(`/api/sites?slug=${SITE_SLUG}`)).json()));
  const created = await page.request.post('/api/site_pages', {
    data: {
      'o:site': { 'o:id': site },
      'o:title': 'IIIF viewer',
      'o:slug': PAGE_SLUG,
      'o:is_public': true,
      'o:block': [{ 'o:layout': 'html', 'o:data': { html: '<p>Above the viewer</p>' } }],
    },
  });
  expect(created.status(), await created.text()).toBe(200);
  return `/#/sites/${String(site)}/pages/${String(idOf(await created.json()))}`;
};

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

test('a IIIF block of a local item previews as a poster and opens Mirador on the public page', async ({
  page,
  diagnostics,
}) => {
  const item = await createItem(page);
  await page.goto(await createPage(page));
  await page.getByRole('button', { name: /Insert block at position/ }).last().click();
  await page
    .getByRole('dialog', { name: 'Insert a block or template' })
    .getByRole('button', { name: 'IIIF presentation', exact: true })
    .click();

  const form = page.getByRole('region', { name: 'Block inspector' });
  await form.getByRole('radio', { name: 'An item from this site' }).check();
  await form.getByLabel('Search titles and values').fill(ITEM_TITLE);
  await form
    .getByRole('listbox', { name: 'Find an item' })
    .getByRole('option', { name: ITEM_TITLE, exact: true, selected: false })
    .click();
  await expect(form.getByText(ITEM_TITLE, { exact: true }).first()).toBeVisible();
  await form.getByRole('textbox', { name: 'Title', exact: true }).fill(BLOCK_TITLE);
  const rendered = page.waitForResponse(
    (response) => response.url().includes(`/s/${SITE_SLUG}/preview`) && response.request().method() === 'POST',
  );
  await form.getByLabel('Viewer height').selectOption('medium');
  await rendered;

  const poster = shown(page).locator('.iiif-poster');
  await expect(poster).toHaveText(`IIIF viewer: ${BLOCK_TITLE}. It opens on the public page.`);
  await expect(poster).toHaveCSS('height', '600px');
  await expect(shown(page).locator('iframe')).toHaveCount(0);

  await page.getByRole('button', { name: 'Save' }).click();
  await expect(page.getByText('Saved.')).toBeVisible();

  diagnostics.reset();
  const response = await page.goto(`/s/${SITE_SLUG}/page/${PAGE_SLUG}`);
  expect(response?.status()).toBe(200);
  const manifest = `${readHarness().baseUrl}/iiif/3/item/${String(item)}/manifest`;
  const frame = page.locator('.block-iiifPresentation iframe');
  await expect(frame).toHaveAttribute('src', new RegExp(`^/iiif-viewer\\?url=${encodeURIComponent(manifest)}&`));
  await expect(frame).toHaveCSS('height', '600px');
  const viewer = page.frameLocator('.block-iiifPresentation iframe');
  await expect(viewer.locator('.mirador-viewer')).toBeVisible({ timeout: 15_000 });
  await expect(viewer.getByText(ITEM_TITLE).first()).toBeVisible({ timeout: 15_000 });
  await expect(viewer.locator('.mirador-osd-container canvas').first()).toBeVisible({ timeout: 15_000 });
  await expectClean(diagnostics, 'the public page with Mirador');
});
