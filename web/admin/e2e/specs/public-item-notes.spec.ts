/**
 * The public item page's first file carries its notes list where the theme
 * leaves the `.annotation-list` place: the same server-drawn list the public
 * media page uses, with the notes switch on.
 */
import type { APIRequestContext } from '@playwright/test';
import { mkdirSync, readFileSync } from 'node:fs';
import { join } from 'node:path';

import { expect, expectClean, test } from '../fixtures';
import { isRecord, numberField, record } from '../harness/json';
import { settled } from '../harness/preview';
import { SITE_SLUG } from '../harness/seed';
import { readHarness, REPO_ROOT } from '../harness/state';

const corpus = (name: string): Buffer =>
  readFileSync(join(REPO_ROOT, 'fixtures', 'media-kinds', name));

const SHOTS = join(REPO_ROOT, '.evidence');
mkdirSync(SHOTS, { recursive: true });

const ANNO = 'application/ld+json; profile="http://www.w3.org/ns/anno.jsonld"';

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

/** One uploaded corpus file attached to an item. */
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

/** Post one note on a media's whole file, with the harness key in the query. */
const addNote = async (
  request: APIRequestContext,
  media: number,
  text: string,
): Promise<void> => {
  const { keyIdentity, keyCredential } = readHarness();
  const container = await request.get(
    `/annotations/media/${String(media)}/?key_identity=${keyIdentity}&key_credential=${keyCredential}`,
    { headers: { accept: ANNO } },
  );
  expect(container.status(), await container.text()).toBe(200);
  const canvas = container.headers()['romeka-canvas'];
  expect(canvas, 'the container states its canvas').toBeTruthy();
  const answer = await request.post(
    `/annotations/media/${String(media)}/?key_identity=${keyIdentity}&key_credential=${keyCredential}`,
    {
      headers: { 'content-type': ANNO, accept: ANNO },
      data: {
        '@context': 'http://www.w3.org/ns/anno.jsonld',
        type: 'Annotation',
        motivation: 'commenting',
        body: { type: 'TextualBody', value: text, format: 'text/plain' },
        target: canvas,
      },
    },
  );
  expect(answer.status(), await answer.text()).toBe(201);
};

test('the public item page fills the first file’s notes list', async ({
  page,
  diagnostics,
}) => {
  const item = await createItem(page.request, 'E2E public notes item');
  const first = await addMedia(page.request, item, 'first.jpg', 'image/jpeg', corpus('image.jpg'));
  const second = await addMedia(page.request, item, 'second.jpg', 'image/jpeg', corpus('image.jpg'));
  await settled(page.request, first);
  await settled(page.request, second);
  const text = 'A pencil note on the whole file.';
  await addNote(page.request, first, text);

  await page.goto(`/s/${SITE_SLUG}/item/${String(item)}`);

  const figure = page.locator('.annotated-media');
  await expect(figure).toHaveCount(1);
  const list = figure.locator('section.annotation-list');
  await expect(list).toBeVisible();
  await expect(list).toContainText(text);
  // The empty place the theme leaves after the first file stays, and draws nothing.
  await expect(page.locator('.media-embeds > div.annotation-list:empty')).toHaveCount(1);
  await expect(page.locator('.media-embeds > div.annotation-list:empty')).toBeHidden();
  await page.screenshot({ path: join(SHOTS, 'public-item-notes.png'), fullPage: true });
  await expectClean(diagnostics, 'the public item page and its first file’s notes');
});
