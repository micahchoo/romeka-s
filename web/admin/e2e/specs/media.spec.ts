/**
 * The media pages render the stored files. `omeka import media` writes the
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
