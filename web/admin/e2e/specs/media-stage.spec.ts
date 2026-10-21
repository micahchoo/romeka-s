/**
 * The admin media stage: a media's own page shows the control its kind needs.
 *
 * The corpus files come from `fixtures/media-kinds`, so the stage is driven
 * with the same bytes the public matrix uses. Each test asserts the element an
 * editor meets and that no `<img>` on the page holds a file the browser cannot
 * draw, which is the failure the hero `<img>` had for six kinds.
 */
import type { APIRequestContext, Page } from '@playwright/test';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';

import { expect, expectClean, test } from '../fixtures';
import { isRecord, numberField, record } from '../harness/json';
import { readHarness, REPO_ROOT } from '../harness/state';

const corpus = (name: string): Buffer =>
  readFileSync(join(REPO_ROOT, 'fixtures', 'media-kinds', name));

/** Upload one corpus file as the only media of a new public item. */
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
  return firstMediaId(created, name);
};

/** The id of the first media of a created item, from its `o:media` refs. */
const firstMediaId = (item: unknown, what: string): number => {
  const media = record(item, what)['o:media'];
  if (!Array.isArray(media) || media.length === 0) {
    throw new Error(`${what}: the created item names no media`);
  }
  const first: unknown = media[0];
  if (!isRecord(first)) {
    throw new Error(`${what}: the media reference is not an object`);
  }
  return numberField(first, 'o:id', 'media');
};

/** Every picture on the page answers with image bytes, not a 404 or a document. */
const expectPicturesDecode = async (page: Page, what: string): Promise<void> => {
  const sources = await page.locator('img').evaluateAll((images) =>
    images.map((image) => (image instanceof HTMLImageElement ? image.getAttribute('src') : null)),
  );
  for (const src of sources) {
    if (src === null) {
      continue;
    }
    const response = await page.request.get(src);
    expect(response.status(), `${what}: ${src}`).toBe(200);
    expect(response.headers()['content-type'], `${what}: ${src}`).toMatch(/^image\//);
  }
};

test('an audio file plays in a native player', async ({ page, diagnostics }) => {
  const media = await uploadItem(page.request, 'audio.mp3', 'audio/mpeg');
  await page.goto(`/#/media/${String(media)}`);

  const player = page.locator('audio.media-stage-player');
  await expect(player).toBeVisible();
  await expect(player).toHaveAttribute('controls', '');
  await expect(player).toHaveAttribute('preload', 'metadata');
  const src = await player.getAttribute('src');
  expect(src?.endsWith('.mp3'), String(src)).toBe(true);
  await expect(page.locator('.media-stage img')).toHaveCount(0);
  await expectPicturesDecode(page, 'audio page');
  await expectClean(diagnostics, 'audio stage');
});

test('a video shows its poster in a native player', async ({ page, diagnostics }) => {
  const media = await uploadItem(page.request, 'video.mp4', 'video/mp4');
  await page.goto(`/#/media/${String(media)}`);

  const player = page.locator('video.media-stage-player');
  await expect(player).toBeVisible();
  await expect(player).toHaveAttribute('controls', '');
  await expect(player).toHaveAttribute('preload', 'metadata');
  const poster = await player.getAttribute('poster');
  expect(poster?.startsWith(`${readHarness().baseUrl}/files/large/`), String(poster)).toBe(true);
  const still = await page.request.get(poster ?? '');
  expect(still.status()).toBe(200);
  expect(still.headers()['content-type']).toBe('image/jpeg');
  await expect(page.locator('.media-stage img')).toHaveCount(0);
  await expectClean(diagnostics, 'video stage');
});

test('a PDF shows its first page, and the viewer only after the press', async ({
  page,
  diagnostics,
}) => {
  const media = await uploadItem(page.request, 'doc.pdf', 'application/pdf');
  await page.goto(`/#/media/${String(media)}`);

  const firstPage = page.locator('.media-stage img');
  await expect(firstPage).toHaveCount(1);
  const src = await firstPage.getAttribute('src');
  expect(src?.startsWith(`${readHarness().baseUrl}/files/large/`), String(src)).toBe(true);
  await expect(page.locator('.media-stage object')).toHaveCount(0);
  await expectPicturesDecode(page, 'PDF first page');

  await page.getByRole('button', { name: 'Read the PDF here' }).click();
  const viewer = page.locator('.media-stage object[type="application/pdf"]');
  await expect(viewer).toHaveCount(1);
  const data = await viewer.getAttribute('data');
  expect(data?.endsWith('.pdf'), String(data)).toBe(true);
  await expect(page.getByRole('button', { name: 'Read the PDF here' })).toHaveCount(0);
  await expectClean(diagnostics, 'PDF stage');
});

test('an SVG shows as a picture, on the original file', async ({ page, diagnostics }) => {
  const media = await uploadItem(page.request, 'image.svg', 'image/svg+xml');
  await page.goto(`/#/media/${String(media)}`);

  const picture = page.locator('.media-stage img');
  await expect(picture).toHaveCount(1);
  await expect(picture).toHaveClass(/is-svg/);
  const src = await picture.getAttribute('src');
  expect(src?.endsWith('.svg'), String(src)).toBe(true);
  const decoded = await picture.evaluate((image: HTMLImageElement) => ({
    complete: image.complete,
    width: image.naturalWidth,
  }));
  expect(decoded.complete).toBe(true);
  expect(decoded.width).toBeGreaterThan(0);
  await expectPicturesDecode(page, 'SVG stage');
  await expectClean(diagnostics, 'SVG stage');
});

test('a document is a file card with its size and a download', async ({ page, diagnostics }) => {
  const media = await uploadItem(
    page.request,
    'doc.docx',
    'application/vnd.openxmlformats-officedocument.wordprocessingml.document',
  );
  await page.goto(`/#/media/${String(media)}`);

  const stage = page.locator('.media-stage');
  const card = stage.locator('.media-stage-file');
  await expect(card).toBeVisible();
  await expect(card).toContainText('doc.docx');
  await expect(stage).toContainText('Document');
  await expect(stage).toContainText('DOCX');
  await expect(card.locator('a', { hasText: 'Download the file' })).toHaveAttribute(
    'href',
    /\/files\/original\/.*\.docx$/,
  );
  await expect(page.locator('.media-stage img')).toHaveCount(0);
  await expect(page.locator('.media-stage video, .media-stage audio')).toHaveCount(0);
  await expectClean(diagnostics, 'document stage');
});

test('a 3D model is a file card, not a viewer', async ({ page, diagnostics }) => {
  const media = await uploadItem(page.request, 'model.glb', 'model/gltf-binary');
  await page.goto(`/#/media/${String(media)}`);

  await expect(page.locator('.media-stage-file')).toContainText('model.glb');
  await expect(page.locator('.media-stage')).toContainText('3D model');
  await expect(page.locator('.media-stage')).toContainText('GLB');
  await expect(page.locator('.media-stage model-viewer')).toHaveCount(0);
  await expect(page.locator('.media-stage img')).toHaveCount(0);
  await expectClean(diagnostics, 'model stage');
});

test('a caption file shows its first lines as text', async ({ page, diagnostics }) => {
  const media = await uploadItem(page.request, 'captions.vtt', 'text/vtt');
  await page.goto(`/#/media/${String(media)}`);

  const text = page.locator('.media-stage pre');
  await expect(text).toContainText('WEBVTT');
  await expect(page.locator('.media-stage img')).toHaveCount(0);
  await expectClean(diagnostics, 'captions stage');
});

test('an HTML media loads the server render in a sandboxed frame after the press', async ({
  page,
  diagnostics,
}) => {
  const created = await page.request.post('/api/items', {
    multipart: {
      data: JSON.stringify({
        'o:is_public': true,
        'dcterms:title': [{ type: 'literal', '@value': 'An embed' }],
        'o:media': [
          {
            'o:ingester': 'html',
            'o:is_public': true,
            html: '<p class="embed-body">Hello from the embed</p>',
          },
        ],
      }),
    },
  });
  expect(created.status(), await created.text()).toBe(200);
  const item: unknown = await created.json();
  const media = firstMediaId(item, 'html');

  await page.goto(`/#/media/${String(media)}`);
  const stage = page.locator('.media-stage');
  await expect(stage.locator('iframe')).toHaveCount(0);
  await expect(stage).toContainText('HTML that the server renders');

  await stage.getByRole('button', { name: 'Load the embed' }).click();

  const frame = stage.locator('iframe');
  await expect(frame).toHaveAttribute(
    'src',
    new RegExp(`/api/romeka/media/${String(media)}/render$`),
  );
  await expect(frame).toHaveAttribute('sandbox', 'allow-scripts allow-presentation');
  const sandbox = await frame.getAttribute('sandbox');
  expect(sandbox).not.toContain('allow-same-origin');
  await expect(frame.contentFrame().locator('.embed-body')).toHaveText('Hello from the embed');
  await expect(stage.getByRole('button', { name: 'Load the embed' })).toHaveCount(0);
  await expectClean(diagnostics, 'embed frame stage');
});

test('a YouTube media loads the provider player directly, after the press', async ({
  page,
  diagnostics,
}) => {
  const created = await page.request.post('/api/items', {
    multipart: {
      data: JSON.stringify({
        'o:is_public': true,
        'dcterms:title': [{ type: 'literal', '@value': 'A YouTube media' }],
        'o:media': [
          {
            'o:ingester': 'youtube',
            'o:source': 'https://www.youtube.com/watch?v=dQw4w9WgXcQ',
            'o:is_public': true,
          },
        ],
      }),
    },
  });
  expect(created.status(), await created.text()).toBe(200);
  const item: unknown = await created.json();
  const media = firstMediaId(item, 'youtube');

  // The provider's own document is served from here, so this spec needs no
  // network to prove which frame the stage opens.
  await page.route('https://www.youtube-nocookie.com/**', (route) =>
    route.fulfill({
      contentType: 'text/html; charset=utf-8',
      body: '<!doctype html><p class="player-body">The YouTube player</p>',
    }),
  );

  await page.goto(`/#/media/${String(media)}`);
  const stage = page.locator('.media-stage');
  await expect(stage.locator('iframe')).toHaveCount(0);
  await expect(stage).toContainText('is a page on www.youtube.com');
  const link = stage.getByRole('link', { name: 'Open on www.youtube.com' });
  await expect(link).toHaveAttribute('href', 'https://www.youtube.com/watch?v=dQw4w9WgXcQ');

  await stage.getByRole('button', { name: 'Load the embed' }).click();

  const frame = stage.locator('iframe');
  await expect(frame).toHaveAttribute(
    'src',
    'https://www.youtube-nocookie.com/embed/dQw4w9WgXcQ',
  );
  await expect(frame).toHaveAttribute(
    'sandbox',
    'allow-scripts allow-same-origin allow-presentation allow-popups',
  );
  await expect(frame).toHaveAttribute('allow', 'fullscreen; picture-in-picture');
  await expect(frame).toHaveAttribute('referrerpolicy', 'strict-origin-when-cross-origin');
  await expect(stage).toContainText('Playing from www.youtube-nocookie.com');
  await expect(frame.contentFrame().locator('.player-body')).toHaveText('The YouTube player');
  await expect(stage.getByRole('button', { name: 'Load the embed' })).toHaveCount(0);
  await expect(link).toBeVisible();
  await expectClean(diagnostics, 'YouTube player stage');
});

test('a YouTube media whose id names no player keeps its card and link', async ({
  page,
  diagnostics,
}) => {
  const created = await page.request.post('/api/items', {
    multipart: {
      data: JSON.stringify({
        'o:is_public': true,
        'dcterms:title': [{ type: 'literal', '@value': 'A broken YouTube media' }],
        'o:media': [
          {
            'o:ingester': 'youtube',
            'o:source': 'https://youtu.be/',
            'o:is_public': true,
          },
        ],
      }),
    },
  });
  expect(created.status(), await created.text()).toBe(200);
  const item: unknown = await created.json();
  const media = firstMediaId(item, 'youtube');

  await page.goto(`/#/media/${String(media)}`);
  const stage = page.locator('.media-stage');
  await stage.getByRole('button', { name: 'Load the embed' }).click();

  await expect(stage.getByRole('button', { name: 'Load the embed' })).toHaveCount(0);
  await expect(stage.locator('iframe')).toHaveCount(0);
  await expect(stage).toContainText('is a page on youtu.be');
  await expect(stage.getByRole('link', { name: 'Open on youtu.be' })).toHaveAttribute(
    'href',
    'https://youtu.be/',
  );
  await expectClean(diagnostics, 'YouTube card without a player');
});
