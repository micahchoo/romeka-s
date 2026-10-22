/**
 * A box on a video's frames, in a real browser against the real WAP routes:
 * the editor draws it on the stage's own player, the server stores it as one
 * `xywh=…&t=…` fragment, and the public page shows it only while the player is
 * inside its range.
 *
 * The corpus video is 96 by 64 pixels and 0.5 s long (ffprobe), so a box drawn
 * at the start covers the whole file: the five seconds **Box on this frame**
 * proposes are capped at the length.
 */
import type { APIRequestContext, Page } from '@playwright/test';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';

import { expect, expectClean, test } from '../fixtures';
import { isRecord, numberField, record } from '../harness/json';
import { SITE_SLUG } from '../harness/seed';
import { readHarness, REPO_ROOT } from '../harness/state';

const ANNO = 'application/ld+json; profile="http://www.w3.org/ns/anno.jsonld"';

const created: number[] = [];

test.afterEach(async ({ page }) => {
  const harness = readHarness();
  for (const item of created.splice(0)) {
    await page.request.delete(
      `/api/items/${String(item)}?key_identity=${harness.keyIdentity}&key_credential=${harness.keyCredential}`,
    );
  }
});

/** The corpus video as the only media of a new public item, its preview settled. */
const uploadVideo = async (request: APIRequestContext): Promise<number> => {
  const answer = await request.post('/api/items', {
    multipart: {
      data: JSON.stringify({
        'o:is_public': true,
        'dcterms:title': [{ type: 'literal', '@value': 'A boxed video' }],
        'o:media': [{ 'o:ingester': 'upload', file_index: 0, 'o:is_public': true }],
      }),
      file: {
        name: 'video.mp4',
        mimeType: 'video/mp4',
        buffer: readFileSync(join(REPO_ROOT, 'fixtures', 'media-kinds', 'video.mp4')),
      },
    },
  });
  expect(answer.status(), await answer.text()).toBe(200);
  const document: unknown = await answer.json();
  created.push(numberField(record(document, 'item'), 'o:id', 'item'));
  const media = record(document, 'item')['o:media'];
  if (!Array.isArray(media) || !isRecord(media[0])) {
    throw new Error('the created item names no media');
  }
  const id = numberField(media[0], 'o:id', 'media');
  const deadline = Date.now() + 30_000;
  for (;;) {
    const read: unknown = await (await request.get(`/api/media/${String(id)}`)).json();
    const preview = record(read, 'media')['o-module-romeka:preview'];
    const state = isRecord(preview) ? preview['state'] : null;
    if (state === null || state === undefined || state === 'ready' || state === 'none') {
      return id;
    }
    if (Date.now() > deadline) {
      throw new Error(`media ${String(id)} preview stayed ${JSON.stringify(state)}`);
    }
    await new Promise((done) => setTimeout(done, 100));
  }
};

const signIn = async (page: Page, baseURL: string): Promise<void> => {
  const { adminEmail, adminPassword } = readHarness();
  const response = await page.request.post('/api/login', {
    data: { email: adminEmail, password: adminPassword },
    headers: { origin: new URL(baseURL).origin },
  });
  expect(response.status(), await response.text()).toBe(200);
};

/** The selector values of the media's notes, as the WAP page states them. */
const storedValues = async (request: APIRequestContext, media: number): Promise<string[]> => {
  const answer = await request.get(`/annotations/media/${String(media)}/page`, { headers: { accept: ANNO } });
  const page: unknown = await answer.json();
  const items = record(page, 'page')['items'];
  if (!Array.isArray(items)) {
    return [];
  }
  return items.flatMap((item: unknown) => {
    const target = isRecord(item) ? item['target'] : null;
    const selector = isRecord(target) ? target['selector'] : null;
    const value = isRecord(selector) ? selector['value'] : null;
    return typeof value === 'string' ? [value] : [];
  });
};

test('an editor draws a box on a video frame and it is stored with its range', async ({
  page,
  baseURL,
  diagnostics,
}) => {
  const media = await uploadVideo(page.request);
  await signIn(page, baseURL ?? '');
  await page.goto(`/#/media/${String(media)}`);
  await page.locator('video.media-stage-player').waitFor();

  await page.getByRole('button', { name: 'Box on this frame' }).click();
  await expect(page.getByRole('button', { name: 'Box on this frame' })).toHaveAttribute('aria-pressed', 'true');
  const overlay = page.locator('.frame-boxes');
  const box = await overlay.boundingBox();
  if (box === null) {
    throw new Error('the frame layer has no box');
  }
  await page.mouse.move(box.x + box.width * 0.25, box.y + box.height * 0.25);
  await page.mouse.down();
  await page.mouse.move(box.x + box.width * 0.75, box.y + box.height * 0.75, { steps: 6 });
  await page.mouse.up();

  await expect(page.locator('.frame-box.is-draft')).toHaveCount(1);
  await expect(page.getByLabel('Start', { exact: true })).toHaveValue('0:00');
  await expect(page.getByLabel('End', { exact: true })).toHaveValue('0:00.5');
  await page.locator('.note-form').getByLabel('Note').fill('The speaker’s hands');
  await page.locator('.note-form').getByRole('button', { name: 'Save' }).click();
  await expect(page.locator('.media-notes-status')).toHaveText('Saved.');

  await expect(page.locator('.note-open').first()).toContainText('Box 1, 0:00 to 0:00.5');
  const [value] = await storedValues(page.request, media);
  expect(value).toMatch(/^xywh=pixel:\d+,\d+,\d+,\d+&t=0,0\.5$/);
  await expectClean(diagnostics, 'a box drawn on a video frame');
});

test('the public page shows a box only while the player is inside its range', async ({
  page,
  baseURL,
}) => {
  const media = await uploadVideo(page.request);
  await signIn(page, baseURL ?? '');
  const container = await page.request.get(`/annotations/media/${String(media)}/`, { headers: { accept: ANNO } });
  const canvas = container.headers()['romeka-canvas'] ?? '';
  const ids: number[] = [];
  for (const [text, value] of [
    ['Early', 'xywh=pixel:4,4,40,30&t=0,0.2'],
    ['Late', 'xywh=pixel:50,30,40,30&t=0.3,0.5'],
  ] as const) {
    const answer = await page.request.post(`/annotations/media/${String(media)}/`, {
      headers: { accept: ANNO, 'content-type': ANNO, origin: new URL(baseURL ?? '').origin },
      data: {
        '@context': 'http://www.w3.org/ns/anno.jsonld',
        type: 'Annotation',
        motivation: 'commenting',
        body: { type: 'TextualBody', value: text, format: 'text/plain' },
        target: {
          type: 'SpecificResource',
          source: { id: canvas, type: 'Canvas' },
          selector: { type: 'FragmentSelector', conformsTo: 'http://www.w3.org/TR/media-frags/', value },
        },
      },
    });
    expect(answer.status(), await answer.text()).toBe(201);
    const iri = record(await answer.json(), 'note')['id'];
    if (typeof iri !== 'string') {
      throw new Error('the created note has no id');
    }
    ids.push(Number(iri.split('/').pop()));
  }

  await page.goto(`/s/${SITE_SLUG}/media/${String(media)}`);
  const player = page.locator('figure.annotated-video video');
  await player.waitFor();
  await player.evaluate((element: HTMLVideoElement) => {
    element.currentTime = 0.1;
  });
  const [early, late] = ids;
  await expect(page.locator(`#region-${String(early)}`)).toBeVisible();
  await expect(page.locator(`#region-${String(late)}`)).toBeHidden();
  await player.evaluate((element: HTMLVideoElement) => {
    element.currentTime = 0.4;
  });
  await expect(page.locator(`#region-${String(late)}`)).toBeVisible();
  await expect(page.locator(`#region-${String(early)}`)).toBeHidden();
});
