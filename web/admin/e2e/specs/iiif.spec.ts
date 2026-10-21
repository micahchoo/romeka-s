/**
 * The IIIF viewer end to end: a curator adds a IIIF presentation block that
 * opens an item of this site, the script-free preview shows its poster, and
 * the public page opens the item's manifest in Mirador, served from this
 * origin under the viewer page's policy.
 */
import type { Page } from '@playwright/test';
import { expect, expectClean, test } from '../fixtures';
import { record } from '../harness/json';
import { png } from '../harness/png';
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

/** An item holding one uploaded image, created through the API. */
const createItem = async (page: Page): Promise<{ item: number; media: number }> => {
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
  const document: unknown = await created.json();
  return { item: idOf(document), media: idOf(first(record(document, 'item')['o:media'])) };
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
  const { item } = await createItem(page);
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

const NOTE_TEXT = 'Watermark: crowned shield';

/** Write one region note on the item's media through the Web Annotation Protocol. */
const addNote = async (page: Page, item: number, media: number): Promise<void> => {
  const harness = readHarness();
  const canvas = `${harness.baseUrl}/iiif/3/item/${String(item)}/canvas/${String(media)}`;
  const answer = await page.request.post(
    `${harness.baseUrl}/annotations/media/${String(media)}/?key_identity=${harness.keyIdentity}&key_credential=${harness.keyCredential}`,
    {
      headers: { 'content-type': 'application/ld+json; profile="http://www.w3.org/ns/anno.jsonld"' },
      data: {
        '@context': 'http://www.w3.org/ns/anno.jsonld',
        type: 'Annotation',
        motivation: 'commenting',
        body: { type: 'TextualBody', value: NOTE_TEXT, format: 'text/plain' },
        target: {
          type: 'SpecificResource',
          source: { id: canvas, type: 'Canvas' },
          selector: {
            type: 'FragmentSelector',
            conformsTo: 'http://www.w3.org/TR/media-frags/',
            value: 'xywh=pixel:120,140,320,200',
          },
        },
      },
    },
  );
  expect(answer.status(), await answer.text()).toBe(201);
};

test('Mirador lists the note the item manifest references from its canvas', async ({ page }) => {
  const { item, media } = await createItem(page);
  const manifest = `${readHarness().baseUrl}/iiif/3/item/${String(item)}/manifest`;
  const openViewer = async (): Promise<void> => {
    const response = await page.goto(`/iiif-viewer?url=${encodeURIComponent(manifest)}`);
    expect(response?.status()).toBe(200);
    await expect(page.locator('#viewer .mirador-viewer')).toBeVisible({ timeout: 15_000 });
    await expect(
      page.locator('#viewer .mirador-osd-container canvas').first(),
    ).toBeVisible({ timeout: 15_000 });
  };
  const evidence = process.env.EVIDENCE_DIR;

  // Before: the canvas references no page, so Mirador offers no annotations panel.
  await openViewer();
  await expect(page.getByRole('tab', { name: /annotations/i })).toHaveCount(0);
  if (evidence !== undefined && evidence !== '') {
    // Let the sidebar settle and the canvas paint before the capture.
    await page.waitForTimeout(1_500);
    await page.screenshot({ path: `${evidence}/iiif-notes-before.png`, fullPage: true });
  }

  await addNote(page, item, media);
  await openViewer();
  await page.getByRole('tab', { name: /annotations/i }).click();
  await expect(page.getByText(NOTE_TEXT)).toBeVisible({ timeout: 15_000 });
  if (evidence !== undefined && evidence !== '') {
    await page.waitForTimeout(1_000);
    await page.screenshot({ path: `${evidence}/iiif-notes-after.png`, fullPage: true });
  }
});
