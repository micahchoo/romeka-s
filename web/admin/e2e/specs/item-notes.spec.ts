/**
 * The item page's two new ways into a file: Annotate reaches that file's notes
 * panel ready to add, and Preview shows the file over the item page with the
 * media page's stage.
 *
 * The second file of the notes item is a picture because the region editor is
 * what proves the return trip lands on the note itself, not just on the media.
 */
import type { APIRequestContext, Page } from '@playwright/test';
import { mkdirSync, readFileSync } from 'node:fs';
import { join } from 'node:path';

import { expect, expectClean, test } from '../fixtures';
import { isRecord, numberField, record, stringField } from '../harness/json';
import { readHarness, REPO_ROOT } from '../harness/state';

const corpus = (name: string): Buffer =>
  readFileSync(join(REPO_ROOT, 'fixtures', 'media-kinds', name));

const SHOTS = join(REPO_ROOT, '.evidence');
mkdirSync(SHOTS, { recursive: true });

const shot = async (page: Page, name: string): Promise<void> => {
  await page.screenshot({ path: join(SHOTS, `${name}.png`), fullPage: true });
};

/** Sign the browser in with the seeded administrator's password. */
const signIn = async (page: Page, baseURL: string): Promise<void> => {
  const { adminEmail, adminPassword } = readHarness();
  const response = await page.request.post('/api/login', {
    data: { email: adminEmail, password: adminPassword },
    headers: { origin: new URL(baseURL).origin },
  });
  expect(response.status(), await response.text()).toBe(200);
};

/** The dcterms:title property's id, which a literal value names. */
const titleProperty = async (request: APIRequestContext): Promise<number> => {
  const answer = await request.get('/api/properties?term=dcterms:title');
  expect(answer.status(), await answer.text()).toBe(200);
  const page: unknown = await answer.json();
  if (!Array.isArray(page) || page.length === 0) {
    throw new Error('the API has no dcterms:title property');
  }
  const first: unknown = page[0];
  if (!isRecord(first)) {
    throw new Error('the property is not an object');
  }
  return numberField(first, 'o:id', 'property');
};

/** A new public item, and its id. */
const createItem = async (request: APIRequestContext, title: string): Promise<number> => {
  const property = await titleProperty(request);
  const answer = await request.post('/api/items', {
    data: {
      'o:is_public': true,
      'dcterms:title': [{ type: 'literal', property_id: property, '@value': title }],
    },
  });
  expect(answer.status(), await answer.text()).toBe(200);
  return numberField(record(await answer.json(), 'item'), 'o:id', 'item');
};

/** One uploaded corpus file attached to an item, in item order. */
const addMedia = async (
  request: APIRequestContext,
  item: number,
  name: string,
  mediaType: string,
  bytes: Buffer,
): Promise<number> => {
  const answer = await request.post('/api/media', {
    multipart: {
      data: JSON.stringify({
        'o:ingester': 'upload',
        file_index: 0,
        'o:is_public': true,
        'o:item': { 'o:id': item },
      }),
      file: { name, mimeType: mediaType, buffer: bytes },
    },
  });
  expect(answer.status(), await answer.text()).toBe(200);
  return numberField(record(await answer.json(), 'media'), 'o:id', 'media');
};

/** Wait until the derivative worker has settled a media's preview. */
const settled = async (request: APIRequestContext, media: number): Promise<void> => {
  const deadline = Date.now() + 30_000;
  for (;;) {
    const document: unknown = await (await request.get(`/api/media/${String(media)}`)).json();
    const preview = record(document, 'media')['o-module-romeka:preview'];
    const state =
      typeof preview === 'object' && preview !== null && 'state' in preview ? preview.state : null;
    if (state === null || state === 'ready' || state === 'none') {
      return;
    }
    if (Date.now() > deadline) {
      throw new Error(`media ${String(media)} preview stayed ${JSON.stringify(state)}`);
    }
    await new Promise((done) => setTimeout(done, 100));
  }
};

const overlayBox = async (page: Page): Promise<{ x: number; y: number; width: number; height: number }> => {
  const overlay = page.locator('.region-overlay');
  await expect(overlay).toHaveCount(1);
  await overlay.scrollIntoViewIfNeeded();
  const box = await overlay.boundingBox();
  if (box === null) {
    throw new Error('the overlay has no box');
  }
  return box;
};

/** Draw the rectangle this spec expects, fill it, and save it. */
const drawAndSave = async (page: Page, text: string): Promise<void> => {
  await page.getByRole('button', { name: 'Rectangle' }).click();
  const box = await overlayBox(page);
  await page.mouse.move(box.x + box.width * 0.25, box.y + box.height * 0.05);
  await page.mouse.down();
  await page.mouse.move(box.x + box.width * 0.75, box.y + box.height * 0.3, { steps: 4 });
  await page.mouse.up();
  const form = page.locator('.note-form');
  await expect(form).toBeVisible();
  await form.getByLabel('Note').fill(text);
  await form.getByRole('button', { name: 'Save' }).click();
  await expect(page.locator('.media-notes-entries')).toContainText(text);
};

test('Annotate on the second file saves a region note the item page can reach', async ({
  page,
  baseURL,
  diagnostics,
}) => {
  const item = await createItem(page.request, 'E2E notes item');
  await addMedia(page.request, item, 'first.jpg', 'image/jpeg', corpus('image.jpg'));
  const second = await addMedia(page.request, item, 'second.jpg', 'image/jpeg', corpus('image.jpg'));
  await settled(page.request, second);
  await signIn(page, baseURL);

  await page.goto(`/#/items/${String(item)}`);
  await expect(page.getByRole('heading', { name: 'Media (2)', level: 2 })).toBeVisible();
  await shot(page, 'notes-item-before');

  const secondCard = page.locator('.media-card').nth(1);
  await secondCard.getByRole('link', { name: 'Annotate' }).click();
  await expect(page).toHaveURL(new RegExp(`#/media/${String(second)}\\?notes=add$`, 'u'));
  await expect(page.locator('.note-form')).toBeVisible();
  await expect(page.locator('.region-overlay')).toBeVisible();
  await shot(page, 'notes-annotate-open');
  // The draft form changes the panel's width, so the region editor's first
  // rendition request is superseded by the sized one. The check below covers
  // the save and the return, not that first paint.
  diagnostics.reset();

  const text = 'A printed stamp beside the door.';
  await drawAndSave(page, text);
  await expect(page.locator('.media-notes-status')).toHaveText('Saved.');
  await shot(page, 'notes-region-saved');
  await expectClean(diagnostics, 'a region note saved from the item page');

  // Back to the item through the record's own Part of item link.
  await page.locator('dd a[href^="#/items/"]').click();
  await expect(page).toHaveURL(new RegExp(`#/items/${String(item)}$`, 'u'));
  await expect(page.locator('.media-card').nth(1)).toContainText('1 note');
  const all = page.locator('.item-all-notes');
  await expect(all.getByRole('heading', { name: 'All notes in this item' })).toBeVisible();
  await expect(all).toContainText(text);
  await shot(page, 'notes-item-after');

  await all.locator('.item-note-place').first().click();
  await expect(page).toHaveURL(new RegExp(`#/media/${String(second)}\\?note=\\d+$`, 'u'));
  await expect(page.locator('.note-entry.is-selected')).toContainText('Region 1');
  await expect(page.locator('.note-form textarea')).toBeFocused();
  await expect(page.locator('.region-shapes .is-selected')).toHaveCount(1);
  await shot(page, 'notes-landed');
});

test('Preview plays a video in place, steps to the PDF, and lands ready to add', async ({
  page,
  baseURL,
  diagnostics,
}) => {
  const item = await createItem(page.request, 'E2E preview item');
  const video = await addMedia(page.request, item, 'clip.mp4', 'video/mp4', corpus('video.mp4'));
  const pdf = await addMedia(page.request, item, 'scan.pdf', 'application/pdf', corpus('doc.pdf'));
  await settled(page.request, video);
  await settled(page.request, pdf);
  await signIn(page, baseURL);

  await page.goto(`/#/items/${String(item)}`);
  await page.locator('.media-card').nth(0).getByRole('button', { name: 'Preview' }).click();
  const viewer = page.getByRole('dialog');
  await expect(viewer).toBeVisible();
  await expect(viewer.getByText('File 1 of 2')).toBeVisible();
  await expect(page).toHaveURL(new RegExp(`#/items/${String(item)}\\?preview=${String(video)}$`, 'u'));

  // A modal dialog keeps Tab inside it, so the page behind cannot be reached.
  for (let step = 0; step < 8; step += 1) {
    await page.keyboard.press('Tab');
    const inside = await page.evaluate(() =>
      document.querySelector('dialog.media-viewer')?.contains(document.activeElement) ?? false,
    );
    expect(inside, `Tab ${String(step + 1)} stayed inside the viewer`).toBe(true);
  }
  await viewer.getByRole('button', { name: 'Close the preview' }).focus();

  // A muted play needs no gesture, which is the same player the media page uses.
  const player = viewer.locator('video.media-stage-player');
  await expect(player).toBeVisible();
  await player.evaluate(async (element: HTMLVideoElement) => {
    element.muted = true;
    await element.play();
  });
  await expect
    .poll(() => player.evaluate((element: HTMLVideoElement) => element.currentTime))
    .toBeGreaterThan(0);
  await shot(page, 'preview-video');

  await viewer.getByRole('button', { name: 'Next' }).click();
  await expect(viewer.getByText('File 2 of 2')).toBeVisible();
  await expect(viewer.locator('[data-stage="pdf"]')).toBeVisible();
  await expect(viewer.locator('.media-stage img').first()).toBeVisible();
  await shot(page, 'preview-pdf');
  await expectClean(diagnostics, 'a video and a PDF in the viewer');

  // Back closes the viewer, since opening pushed the item page's own URL.
  await page.goBack();
  await expect(page.getByRole('dialog')).toHaveCount(0);
  await expect(page).toHaveURL(new RegExp(`#/items/${String(item)}$`, 'u'));

  await page.locator('.media-card').nth(0).getByRole('button', { name: 'Preview' }).click();
  await expect(page.getByRole('dialog').getByText('File 1 of 2')).toBeVisible();
  await page.getByRole('dialog').getByRole('button', { name: 'Next' }).click();
  await page.getByRole('dialog').getByRole('link', { name: 'Annotate' }).click();
  await expect(page).toHaveURL(new RegExp(`#/media/${String(pdf)}\\?notes=add$`, 'u'));
  await expect(page.locator('.note-form')).toBeVisible();
  await shot(page, 'preview-annotate');
});

test('reads one counts answer and one container only for a file with notes', async ({
  page,
  baseURL,
}) => {
  const item = await createItem(page.request, 'E2E notes cost item');
  const media: number[] = [];
  for (let index = 0; index < 6; index += 1) {
    media.push(
      await addMedia(page.request, item, `page-${String(index)}.jpg`, 'image/jpeg', corpus('image.jpg')),
    );
  }
  await signIn(page, baseURL);
  // Two of the six files have a note: the first file has none, so its summary
  // is drawn from the counts read alone, without a container.
  const annotated = [media[1], media[4]].filter((id): id is number => id !== undefined);
  for (const id of annotated) {
    const container = await page.request.get(`/annotations/media/${String(id)}/`, {
      headers: { accept: 'application/ld+json; profile="http://www.w3.org/ns/anno.jsonld"' },
    });
    expect(container.status(), await container.text()).toBe(200);
    const canvas = container.headers()['romeka-canvas'];
    const answer = await page.request.post(`/annotations/media/${String(id)}/`, {
      headers: {
        accept: 'application/ld+json; profile="http://www.w3.org/ns/anno.jsonld"',
        'content-type': 'application/ld+json; profile="http://www.w3.org/ns/anno.jsonld"',
        origin: new URL(baseURL).origin,
      },
      data: {
        '@context': 'http://www.w3.org/ns/anno.jsonld',
        type: 'Annotation',
        motivation: 'commenting',
        body: { type: 'TextualBody', value: `A note on file ${String(id)}.`, format: 'text/plain' },
        target: canvas,
      },
    });
    expect(answer.status(), await answer.text()).toBe(201);
  }

  const counts: string[] = [];
  const containers: string[] = [];
  page.on('request', (request) => {
    const path = new URL(request.url()).pathname;
    if (path.startsWith('/annotations/item/') && path.endsWith('/counts')) {
      counts.push(path);
    }
    if (path.startsWith('/annotations/media/')) {
      containers.push(path);
    }
  });
  const started = Date.now();
  await page.goto(`/#/items/${String(item)}`);
  await expect(page.locator('.item-notes-file')).toHaveCount(annotated.length);
  const elapsed = Date.now() - started;
  console.log(
    `notes read: ${String(counts.length)} counts request and ${String(containers.length)} container requests ` +
      `for ${String(media.length)} files, ${String(annotated.length)} with notes, ${String(elapsed)} ms to the last group`,
  );

  expect(counts).toHaveLength(1);
  expect(containers).toHaveLength(annotated.length);
  expect(new Set(containers).size).toBe(annotated.length);
  for (const id of annotated) {
    expect(containers, `file ${String(id)}`).toContain(`/annotations/media/${String(id)}/`);
  }
  // The cards and the grouped list agree, from the counts read.
  await expect(page.locator('.media-card .media-card-notes')).toHaveCount(annotated.length);
  await page.screenshot({ path: join(SHOTS, 'note-counts-item.png'), fullPage: true });
});

test('a time note link seeks the recording to its start', async ({ page, baseURL }) => {
  const item = await createItem(page.request, 'E2E time note item');
  const video = await addMedia(page.request, item, 'talk.mp4', 'video/mp4', corpus('video.mp4'));
  await settled(page.request, video);
  await signIn(page, baseURL);
  const container = await page.request.get(`/annotations/media/${String(video)}/`, {
    headers: { accept: 'application/ld+json; profile="http://www.w3.org/ns/anno.jsonld"' },
  });
  expect(container.status(), await container.text()).toBe(200);
  const canvas = container.headers()['romeka-canvas'];
  const answer = await page.request.post(`/annotations/media/${String(video)}/`, {
    headers: {
      accept: 'application/ld+json; profile="http://www.w3.org/ns/anno.jsonld"',
      'content-type': 'application/ld+json; profile="http://www.w3.org/ns/anno.jsonld"',
      origin: new URL(baseURL).origin,
    },
    data: {
      '@context': 'http://www.w3.org/ns/anno.jsonld',
      type: 'Annotation',
      motivation: 'commenting',
      body: { type: 'TextualBody', value: 'The second frame.', format: 'text/plain' },
      target: {
        type: 'SpecificResource',
        source: { id: canvas },
        selector: {
          type: 'FragmentSelector',
          conformsTo: 'http://www.w3.org/TR/media-frags/',
          value: 't=0.2,0.3',
        },
      },
    },
  });
  expect(answer.status(), await answer.text()).toBe(201);
  const noteIri = stringField(record(await answer.json(), 'annotation'), 'id', 'note');
  const noteId = noteIri.slice(noteIri.lastIndexOf('/') + 1);

  await page.goto(`/#/media/${String(video)}?note=${noteId}`);
  await expect(page.locator('.note-entry.is-selected')).toContainText('The second frame.');
  const player = page.locator('video.media-stage-player');
  await expect
    .poll(() => player.evaluate((element: HTMLVideoElement) => element.currentTime))
    .toBeGreaterThan(0.15);
});
