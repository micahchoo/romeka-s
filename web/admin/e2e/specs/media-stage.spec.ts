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

test('a video no browser opens plays its access copy, and the file stays the original', async ({
  page,
  diagnostics,
}) => {
  test.setTimeout(120_000);
  const media = await uploadItem(page.request, 'mjpeg.avi', 'video/x-msvideo');
  // The player opens the copy only once the background worker has made it. The
  // page does not poll, so wait through the API and then reload, as an editor
  // returning to the page does.
  await expect
    .poll(
      async () => {
        const answer = await page.request.get(`/api/media/${String(media)}`);
        expect(answer.status(), await answer.text()).toBe(200);
        const document: unknown = await answer.json();
        const access = record(document, 'media')['o-module-romeka:access'];
        return isRecord(access) ? access['state'] : null;
      },
      { timeout: 60_000 },
    )
    .toBe('ready');

  await page.goto(`/#/media/${String(media)}`);
  const player = page.locator('video.media-stage-player');
  await expect(player).toBeVisible();
  const src = await player.getAttribute('src');
  expect(src?.startsWith(`${readHarness().baseUrl}/files/access/`), String(src)).toBe(true);
  expect(src?.endsWith('.mp4'), String(src)).toBe(true);
  const body = await page.request.get(src ?? '');
  expect(body.status()).toBe(200);
  await expect(player.locator('a')).toHaveAttribute('href', /\/files\/original\/.*\.avi$/);
  await expectClean(diagnostics, 'access-copy video stage');
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

test('an outside player is a card that names its host and loads no frame', async ({
  page,
  diagnostics,
}) => {
  const created = await page.request.post('/api/items', {
    multipart: {
      data: JSON.stringify({
        'o:is_public': true,
        'dcterms:title': [{ type: 'literal', '@value': 'Outside player' }],
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

  await page.goto(`/#/media/${String(media)}`);
  const card = page.locator('.media-stage-outside');
  await expect(card).toContainText('youtube.com');
  await expect(page.locator('.media-stage iframe, .media-stage object, .media-stage embed')).toHaveCount(
    0,
  );
  const link = card.getByRole('link', { name: /Open on/ });
  await expect(link).toHaveAttribute(
    'href',
    'https://www.youtube.com/watch?v=dQw4w9WgXcQ',
  );
  await expectClean(diagnostics, 'outside player stage');
});
