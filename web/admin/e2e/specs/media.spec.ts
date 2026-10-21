/**
 * The media pages render the stored files. `romeka import media` writes the
 * original plus the three thumbnails, so the browse cell shows the square
 * derivative and the show page the large one beside the original in the
 * derivative list. These specs prove each URL resolves on this run's origin and
 * that the browser decodes the bytes, which is the check a jsdom test cannot
 * make.
 */
import type { Locator } from '@playwright/test';

import { expect, expectClean, test } from '../fixtures';
import { MEDIA_FILE } from '../harness/seed';
import { readHarness } from '../harness/state';

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
