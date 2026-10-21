/**
 * The media pages render the stored files. `romeka import media` writes the
 * original plus the three thumbnails, so the browse cell shows the square
 * derivative and the show page the large one beside the original in the
 * derivative list. These specs prove each URL resolves on this run's origin and
 * that the browser decodes the bytes, which is the check a jsdom test cannot
 * make.
 */
import type { APIRequestContext, Locator } from '@playwright/test';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';

import { expect, expectClean, test } from '../fixtures';
import { isArray, numberField, record } from '../harness/json';
import { MEDIA_FILE, SITE_SLUG } from '../harness/seed';
import { readHarness, REPO_ROOT } from '../harness/state';

/** The seeded PNG is 8 by 8. The square thumbnail is cropped and scaled to 200. */
const ORIGINAL_WIDTH = 8;
const SQUARE_WIDTH = 200;

const srcOf = async (locator: Locator, what: string): Promise<string> => {
  const src = await locator.getAttribute('src');
  if (src === null) {
    throw new Error(what);
  }
  return src;
};

/** The decoded size the browser reports, and whether it finished loading. */
const decodedOf = async (
  locator: Locator,
): Promise<{ readonly complete: boolean; readonly width: number }> =>
  locator.evaluate((image: HTMLImageElement) => ({
    complete: image.complete,
    width: image.naturalWidth,
  }));

test('the media list renders the square thumbnail, which resolves and decodes', async ({
  page,
  diagnostics,
}) => {
  await page.goto('/#/media');
  const preview = page.locator('table img.media-image').first();
  await expect(preview).toBeVisible();

  // The importer generated the derivatives, so the cell shows the square one
  // under this run's origin rather than the original file.
  const src = await srcOf(preview, 'the media list preview has no src');
  expect(src.startsWith(`${readHarness().baseUrl}/files/square/`)).toBe(true);

  const response = await page.request.get(src);
  expect(response.status()).toBe(200);
  expect(response.headers()['content-type']).toBe('image/jpeg');

  const decoded = await decodedOf(preview);
  expect(decoded.complete).toBe(true);
  expect(decoded.width).toBe(SQUARE_WIDTH);
  await expectClean(diagnostics, 'media list');
});

test('a media row opens its show page with the large thumbnail and the original', async ({
  page,
  diagnostics,
}) => {
  await page.goto('/#/media');
  await page.getByRole('link', { name: MEDIA_FILE }).click();
  await expect(page).toHaveURL(/#\/media\/\d+$/);

  const hero = page.locator('img.media-hero');
  await expect(hero).toBeVisible();
  const src = await srcOf(hero, 'the media show page has no hero src');
  expect(src.startsWith(`${readHarness().baseUrl}/files/large/`)).toBe(true);

  // The bytes are a real image, not just a 200 from a file store. The large
  // derivative is not upscaled, so it stays the size of the seeded PNG.
  const response = await page.request.get(src);
  expect(response.status()).toBe(200);
  expect(response.headers()['content-type']).toBe('image/jpeg');

  const decoded = await decodedOf(hero);
  expect(decoded.complete).toBe(true);
  expect(decoded.width).toBe(ORIGINAL_WIDTH);

  // The store kept the original beside the derivatives, and the page links to
  // it with the PNG content type.
  const original = page.locator('.derivative-list a', { hasText: 'original' });
  const originalHref = await original.getAttribute('href');
  expect(originalHref?.startsWith(`${readHarness().baseUrl}/files/original/`)).toBe(true);
  expect(originalHref?.endsWith('.png')).toBe(true);
  const originalResponse = await page.request.get(originalHref ?? '');
  expect(originalResponse.status()).toBe(200);
  expect(originalResponse.headers()['content-type']).toBe('image/png');

  await expectClean(diagnostics, 'media show');
});

/** A 0.2 s silent MP3, so the upload is a file the browser could play. */
const MP3_BASE64 =
  'SUQzBAAAAAAAIlRTU0UAAAAOAAADTGF2ZjYyLjMuMTAwAAAAAAAAAAAAAAD/4zjAAAAAAAAAAAAASW5mbwAAAA8AAAAFAAACQACAgICAgICAgICAgICAgICAgICAoKCgoKCgoKCgoKCgoKCgoKCgoKDAwMDAwMDAwMDAwMDAwMDAwMDAwODg4ODg4ODg4ODg4ODg4ODg4ODg//////////////////////////8AAAAATGF2YzYyLjExAAAAAAAAAAAAAAAAJALAAAAAAAAAAkCzYEFYAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAD/4xjEAAAAA0gAAAAATEFNRTMuMTAxIChiZXRhIDMpVVVVVVVVVVVVVVVVVVVVVVVVVVVVVUxBTUUzLjEwMSAoYmV0YSAzKVX/4xjEOwAAA0gAAAAAVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVX/4xjEdgAAA0gAAAAAVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVX/4xjEsQAAA0gAAAAAVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVX/4xjExAAAA0gAAAAAVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVU=';

test('a media list row for an mp3 shows the audio placeholder and no broken image', async ({
  page,
  diagnostics,
}) => {
  const created = await page.request.post('/api/items', {
    multipart: {
      data: JSON.stringify({
        'o:is_public': true,
        'o:media': [{ 'o:ingester': 'upload', file_index: 0, 'o:is_public': true }],
      }),
      file: { name: 'tone.mp3', mimeType: 'audio/mpeg', buffer: Buffer.from(MP3_BASE64, 'base64') },
    },
  });
  expect(created.status(), await created.text()).toBe(200);

  await page.goto('/#/media');
  const row = page.locator('tbody tr', { hasText: 'tone.mp3' });
  await expect(row).toHaveCount(1);

  // The row has no image at all, and its cell names the kind and the file.
  await expect(row.locator('[data-kind="audio"]')).toBeVisible();
  await expect(row.locator('img')).toHaveCount(0);
  await expect(row).toContainText('Audio');
  await expect(row).toContainText('MP3');

  // Every image the table does show is a decoded picture.
  await expect
    .poll(
      async () =>
        page.locator('table img').evaluateAll(
          (images) =>
            images.filter(
              (image) =>
                !(image instanceof HTMLImageElement) || !image.complete || image.naturalWidth === 0,
            ).length,
        ),
      { message: 'a media list cell shows an image the browser could not decode' },
    )
    .toBe(0);

  await expectClean(diagnostics, 'media list with an mp3');
});

test('the Kind filter lists one kind and keeps the filter in the URL', async ({
  page,
  diagnostics,
}) => {
  const created = await page.request.post('/api/items', {
    multipart: {
      data: JSON.stringify({
        'o:is_public': true,
        'o:media': [{ 'o:ingester': 'upload', file_index: 0, 'o:is_public': true }],
      }),
      file: {
        name: 'filter-tone.mp3',
        mimeType: 'audio/mpeg',
        buffer: Buffer.from(MP3_BASE64, 'base64'),
      },
    },
  });
  expect(created.status(), await created.text()).toBe(200);

  await page.goto('/#/media');
  const audio = page.locator('tbody tr', { hasText: 'filter-tone.mp3' });
  const picture = page.locator('tbody tr', { hasText: MEDIA_FILE });
  await expect(audio).toHaveCount(1);
  await expect(picture).toHaveCount(1);

  await page.getByLabel('Kind').selectOption('audio');
  await expect(audio).toHaveCount(1);
  await expect(picture).toHaveCount(0);
  await expect(page).toHaveURL(/#\/media\?kind=audio$/);

  await page.reload();
  await expect(page.getByLabel('Kind')).toHaveValue('audio');
  await expect(audio).toHaveCount(1);
  await expect(picture).toHaveCount(0);

  await expectClean(diagnostics, 'media kind filter');
});

/** A corpus file the running server turns into derivatives with ffmpeg or
 * poppler. Read from the repository so the matrix and this spec use one copy. */
const corpus = (name: string): Buffer =>
  readFileSync(join(REPO_ROOT, 'fixtures', 'media-kinds', name));

/** Upload one corpus file as the only media of a new public item, and answer
 * the item's id. */
const uploadItem = async (
  request: APIRequestContext,
  name: string,
  mediaType: string,
): Promise<number> => {
  const answer = await request.post('/api/items', {
    multipart: {
      data: JSON.stringify({
        'o:is_public': true,
        'dcterms:title': [{ type: 'literal', '@value': name }],
        'o:media': [{ 'o:ingester': 'upload', file_index: 0, 'o:is_public': true }],
      }),
      file: { name, mimeType: mediaType, buffer: corpus(name) },
    },
  });
  expect(answer.status(), await answer.text()).toBe(200);
  const created: unknown = await answer.json();
  const item = record(created, 'item');
  return numberField(item, 'o:id', 'item');
};

test('a video poster reaches the admin list and the public player', async ({
  page,
  diagnostics,
}) => {
  const item = await uploadItem(page.request, 'video.mp4', 'video/mp4');

  // The admin list shows the square derivative as the row's picture, not the
  // video placeholder.
  await page.goto('/#/media');
  const row = page.locator('tbody tr', { hasText: 'video.mp4' });
  await expect(row).toHaveCount(1);
  const cell = row.locator('img.media-image');
  await expect(cell).toBeVisible();
  const src = await srcOf(cell, 'the video row has no picture');
  expect(src.startsWith(`${readHarness().baseUrl}/files/square/`)).toBe(true);
  const derivative = await page.request.get(src);
  expect(derivative.status()).toBe(200);
  expect(derivative.headers()['content-type']).toBe('image/jpeg');

  // The public item page plays the file with that still as its poster, and
  // asks for the header rather than the whole recording.
  await page.goto(`/s/${SITE_SLUG}/item/${String(item)}`);
  const video = page.locator('.media-render video').first();
  await expect(video).toBeVisible();
  const poster = await video.getAttribute('poster');
  expect(poster?.startsWith(`${readHarness().baseUrl}/files/large/`), String(poster)).toBe(true);
  expect(await video.getAttribute('preload')).toBe('metadata');
  const still = await page.request.get(poster ?? '');
  expect(still.status()).toBe(200);
  expect(still.headers()['content-type']).toBe('image/jpeg');
  await expectClean(diagnostics, 'video poster');
});

test('a PDF first page is the media list thumbnail', async ({ page, diagnostics }) => {
  await uploadItem(page.request, 'doc.pdf', 'application/pdf');

  await page.goto('/#/media');
  const row = page.locator('tbody tr', { hasText: 'doc.pdf' });
  await expect(row).toHaveCount(1);
  const cell = row.locator('img.media-image');
  await expect(cell).toBeVisible();
  const src = await srcOf(cell, 'the PDF row has no picture');
  expect(src.startsWith(`${readHarness().baseUrl}/files/square/`)).toBe(true);
  const pageImage = await page.request.get(src);
  expect(pageImage.status()).toBe(200);
  expect(pageImage.headers()['content-type']).toBe('image/jpeg');
  await expectClean(diagnostics, 'PDF first page');
});

/** The only file's id on the item `item`. */
const mediaIdOf = async (request: APIRequestContext, item: number): Promise<number> => {
  const answer = await request.get(`/api/items/${String(item)}`);
  expect(answer.status(), await answer.text()).toBe(200);
  const doc = record(await answer.json(), 'item');
  const media = doc['o:media'];
  if (!isArray(media) || media.length === 0) {
    throw new Error('the item has no media');
  }
  return numberField(record(media[0], 'media'), 'o:id', 'media');
};

/** A policy with no `unsafe-eval`: the vendored viewer must run under it. */
const PAGE_POLICY =
  "default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; " +
  "img-src 'self' data: blob:; connect-src 'self' blob:; font-src 'self'";

test('a PDF file page draws the browser reader and keeps the card for a phone', async ({
  page,
  diagnostics,
}) => {
  const item = await uploadItem(page.request, 'doc.pdf', 'application/pdf');
  const media = await mediaIdOf(page.request, item);
  await page.goto(`/s/${SITE_SLUG}/media/${String(media)}`);

  const reader = page.locator('.pdf-object');
  await expect(reader).toBeVisible();
  await expect(reader).toHaveAttribute('type', 'application/pdf');
  const source = await reader.getAttribute('data');
  expect(source).toMatch(/\/files\/original\/[\da-f]+\.pdf$/);
  // The harness serves the stored bytes with a generic type, where the deployed
  // origin sends the stored `o:media_type`; the object names the type it needs.
  const answer = await page.request.get(source ?? '');
  expect(answer.status()).toBe(200);

  // A phone cannot draw a scrollable inline PDF, so the card waits for the
  // rule that swaps the two; on a desktop it stays out of the way.
  await expect(page.locator('.pdf-reader-page > .pdf-card')).toBeHidden();
  await expect(page.locator('.pdf-object .pdf-card')).toHaveCount(1);
  await expectClean(diagnostics, 'PDF file page');
});

test.describe('on a phone', () => {
  // Pixel 5: a coarse pointer under 40rem, which is where the card rule starts.
  test.use({ viewport: { width: 393, height: 851 }, deviceScaleFactor: 2.75, isMobile: true, hasTouch: true });

  test('a PDF file page shows the card instead of the reader', async ({ page }) => {
    const item = await uploadItem(page.request, 'doc.pdf', 'application/pdf');
    const media = await mediaIdOf(page.request, item);
    await page.goto(`/s/${SITE_SLUG}/media/${String(media)}`);

    await expect(page.locator('.pdf-object')).toBeHidden();
    const card = page.locator('.pdf-reader-page > .pdf-card');
    await expect(card).toBeVisible();
    await expect(card).toContainText('PDF ·');
    const href = await card.getAttribute('href');
    const answer = await page.request.get(href ?? '');
    expect(answer.status()).toBe(200);
  });
});

test('a 3D file loads its viewer only when the reader asks, under the site policy', async ({
  page,
}) => {
  const item = await uploadItem(page.request, 'model.glb', 'model/gltf-binary');
  const media = await mediaIdOf(page.request, item);

  const library: string[] = [];
  page.on('request', (request) => {
    if (request.url().includes('/model-viewer-umd.min.js')) {
      library.push(request.url());
    }
  });
  // Chromium reports a blocked script or a refused eval as a console error, so
  // the policy's own report is the evidence: no violation may name the viewer.
  const reported: string[] = [];
  page.on('console', (message) => {
    if (message.type() === 'error') {
      reported.push(message.text());
    }
  });
  page.on('pageerror', (error) => {
    reported.push(error.message);
  });
  await page.route('**/s/**', async (route) => {
    if (route.request().resourceType() !== 'document') {
      await route.continue();
      return;
    }
    await route.continue({
      headers: { ...route.request().headers(), 'content-security-policy': PAGE_POLICY },
    });
  });

  await page.goto(`/s/${SITE_SLUG}/media/${String(media)}`);
  const button = page.getByRole('button', { name: /Turn the model \(loads the 3D viewer, \d+ KB\)/ });
  await expect(button).toBeVisible();
  // The poster is on the page and the model's bytes are not.
  await expect(page.locator('model-viewer')).toBeHidden();
  expect(library, 'the viewer loaded before the reader asked').toEqual([]);

  const arriving = page.waitForResponse((response) =>
    response.url().includes('/model-viewer-umd.min.js'),
  );
  await button.click();
  expect((await arriving).status()).toBe(200);
  await expect(page.locator('model-viewer')).toBeVisible();
  // The facade marks the reader once the model itself has loaded, which is the
  // proof that the press downloaded the bytes and they decoded.
  await expect(page.locator('[data-model-reader]')).toHaveAttribute('data-model-loaded', '');
  expect(
    await page.evaluate(() => window.customElements.get('model-viewer') !== undefined),
  ).toBe(true);
  const violations = reported.filter((text) =>
    /content security policy|unsafe-eval|refused to/i.test(text),
  );
  expect(violations).toEqual([]);
});
