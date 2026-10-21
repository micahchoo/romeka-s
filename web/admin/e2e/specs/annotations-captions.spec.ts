/**
 * Captions in a real browser: a language added, replaced and removed on the
 * admin media page, a caption file given to a recording beside it, and the
 * switch that hides notes from visitors while captions keep showing.
 *
 * The server owns the cue parser and the counts; every sentence asserted here
 * comes back from a dry run or an import answer.
 */
import type { APIRequestContext, Page } from '@playwright/test';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';

import { expect, expectClean, test } from '../fixtures';
import { numberField, record } from '../harness/json';
import { SITE_SLUG } from '../harness/seed';
import { readHarness, REPO_ROOT } from '../harness/state';

const corpus = (name: string): Buffer =>
  readFileSync(join(REPO_ROOT, 'fixtures', 'media-kinds', name));

interface Uploaded {
  readonly item: number;
  readonly media: number;
}

/** One item with one uploaded corpus file, created through the API. */
const uploadItem = async (
  request: APIRequestContext,
  name: string,
  mediaType: string,
): Promise<Uploaded> => {
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
  const item = record(await answer.json(), 'item');
  created.push(numberField(item, 'o:id', 'item'));
  const media = item['o:media'];
  if (!Array.isArray(media) || media.length === 0) {
    throw new Error('the created item names no media');
  }
  return {
    item: numberField(item, 'o:id', 'item'),
    media: numberField(record(media[0], 'media'), 'o:id', 'media'),
  };
};

/** One more file on an existing item, as a media of its own. */
const attachFile = async (
  request: APIRequestContext,
  item: number,
  name: string,
  mediaType: string,
): Promise<number> => {
  const answer = await request.post('/api/media', {
    multipart: {
      data: JSON.stringify({
        'o:ingester': 'upload',
        'o:item': { 'o:id': item },
        'o:is_public': true,
        'o:lang': 'en',
        file_index: 0,
      }),
      file: { name, mimeType: mediaType, buffer: corpus(name) },
    },
  });
  expect(answer.status(), await answer.text()).toBe(200);
  return numberField(record(await answer.json(), 'media'), 'o:id', 'media');
};

/** The items a test made, removed after it so the shared database stays small. */
const created: number[] = [];

test.afterEach(async ({ page }) => {
  const harness = readHarness();
  for (const item of created.splice(0)) {
    await page.request.delete(
      `/api/items/${String(item)}?key_identity=${harness.keyIdentity}&key_credential=${harness.keyCredential}`,
    );
  }
});

const signIn = async (page: Page, baseURL: string): Promise<void> => {
  const { adminEmail, adminPassword } = readHarness();
  const response = await page.request.post('/api/login', {
    data: { email: adminEmail, password: adminPassword },
    headers: { origin: new URL(baseURL).origin },
  });
  expect(response.status(), await response.text()).toBe(200);
};

/** Choose a caption file, the language, and wait for the dry run's sentence. */
const fillDialog = async (page: Page, file: string, language: string): Promise<void> => {
  const dialog = page.getByRole('dialog');
  await dialog.getByLabel(/Caption file/).setInputFiles(join(REPO_ROOT, 'fixtures', 'media-kinds', file));
  await dialog.getByLabel('Language').selectOption({ label: language });
};

test('an editor adds, replaces with no change, then removes English captions', async ({
  page,
  baseURL,
  diagnostics,
}) => {
  const { media } = await uploadItem(page.request, 'video.mp4', 'video/mp4');
  await signIn(page, baseURL);
  await page.goto(`/#/media/${String(media)}`);

  await expect(page.getByText(/No captions yet/)).toBeVisible();
  await page.getByRole('button', { name: 'Add captions…' }).click();
  await fillDialog(page, 'captions.vtt', 'English');
  await expect(page.getByRole('dialog')).toContainText('This file has 1 cue, 0:00 to 0:00.4.');
  await page.getByRole('button', { name: 'Add English captions' }).click();

  await expect(page.locator('.caption-status')).toHaveText('English captions: 1 cue imported.');
  await expect(page.getByRole('heading', { name: 'Notes', exact: true })).toBeVisible();
  await expect(page.locator('.caption-row')).toContainText('English');
  await expect(page.locator('.caption-row')).toContainText('1 cue from a caption file');
  await expect(page.locator('video.media-stage-player track[srclang="en"]')).toHaveCount(1);
  await expectClean(diagnostics, 'the captions an editor added');

  // The same file again changes nothing, and the server says so.
  await page.getByRole('button', { name: 'Replace…' }).click();
  await fillDialog(page, 'captions.vtt', 'English');
  await page.getByRole('button', { name: 'Replace 1 English cue' }).click();
  await expect(page.locator('.caption-status')).toHaveText(
    'No change. The file matches the current English captions.',
  );

  await page.getByRole('button', { name: 'Remove…' }).click();
  await expect(page.locator('.caption-row')).toContainText(
    'Remove the 1 English cues? Visitors will no longer see English captions. Your own notes stay.',
  );
  await page.getByRole('button', { name: 'Remove English captions' }).click();

  await expect(page.locator('.caption-status')).toHaveText('English captions removed.');
  await expect(page.getByText(/No captions yet/)).toBeVisible();
  await expect(page.locator('video.media-stage-player track')).toHaveCount(0);
});

test('an editor replaces captions and brings the old ones back with one click', async ({
  page,
  baseURL,
  diagnostics,
}) => {
  const { media } = await uploadItem(page.request, 'video.mp4', 'video/mp4');
  await signIn(page, baseURL);
  await page.goto(`/#/media/${String(media)}`);

  await page.getByRole('button', { name: 'Add captions…' }).click();
  await fillDialog(page, 'captions.vtt', 'English');
  await page.getByRole('button', { name: 'Add English captions' }).click();
  await expect(page.locator('.caption-status')).toHaveText('English captions: 1 cue imported.');
  await expect(page.locator('video.media-stage-player track[srclang="en"]')).toHaveCount(1);

  const replaced = Buffer.from(
    'WEBVTT\n\n00:00:00.000 --> 00:00:00.400\nA green square, then a blue bar.\n',
  );
  await page.getByRole('button', { name: 'Replace…' }).click();
  const dialog = page.getByRole('dialog');
  await dialog.getByLabel(/Caption file/).setInputFiles({
    name: 'second.vtt',
    mimeType: 'text/vtt',
    buffer: replaced,
  });
  await dialog.getByRole('button', { name: 'Replace 1 English cue' }).click();
  await expect(page.locator('.caption-status')).toHaveText('English captions: 1 cue imported.');
  await expect(page.locator('.caption-row')).toContainText(/Bring back the 1 English cue from/);

  // The player reads the replaced cue.
  const track = page.locator('video.media-stage-player track[srclang="en"]');
  const source = await track.getAttribute('src');
  if (source === null) {
    throw new Error('the player has no English track');
  }
  const afterReplace = await (await page.request.get(source)).text();
  expect(afterReplace).toContain('A green square, then a blue bar.');

  // One click, no confirm, and the row and the player carry the old edition.
  await page.getByRole('button', { name: 'Undo replace' }).click();
  await expect(page.locator('.caption-status')).toHaveText('English captions: 1 cue brought back.');
  await expect(page.locator('.caption-row')).toContainText('1 cue from a caption file');
  await expect(page.getByRole('button', { name: 'Undo replace' })).toHaveCount(0);

  const restored = await (await page.request.get(source)).text();
  expect(restored).toContain('A red square, then a blue bar.');
  expect(restored).not.toContain('A green square');
  await expectClean(diagnostics, 'the captions an editor replaced and brought back');
});

test('a caption file on the item is posted to the recording beside it', async ({
  page,
  baseURL,
}) => {
  const { item, media } = await uploadItem(page.request, 'video.mp4', 'video/mp4');
  const captions = await attachFile(page.request, item, 'captions.vtt', 'text/vtt');
  await signIn(page, baseURL);
  await page.goto(`/#/media/${String(captions)}`);

  await expect(page.getByRole('heading', { name: 'Use this file as captions' })).toBeVisible();
  await page.getByRole('button', { name: /video\.mp4/ }).click();
  const dialog = page.getByRole('dialog');
  await expect(dialog).toContainText('captions.vtt');
  await expect(dialog.getByLabel('Language')).toHaveValue('en');
  await expect(dialog).toContainText('This file has 1 cue, 0:00 to 0:00.4.');
  await dialog.getByRole('button', { name: 'Add English captions' }).click();
  await expect(page.locator('.caption-status')).toHaveText('English captions: 1 cue imported.');

  await page.goto(`/#/media/${String(media)}`);
  await expect(page.locator('.caption-row')).toContainText('English');
  await expect(page.locator('video.media-stage-player track[srclang="en"]')).toHaveCount(1);
});

test('the switch hides a note from visitors and keeps the caption track', async ({
  page,
  browser,
  baseURL,
}) => {
  const harness = readHarness();
  const { item, media } = await uploadItem(page.request, 'video.mp4', 'video/mp4');
  const key = `key_identity=${harness.keyIdentity}&key_credential=${harness.keyCredential}`;
  const tracks = await page.request.post(`/annotations/media/${String(media)}/tracks?lang=en&${key}`, {
    headers: { 'content-type': 'text/vtt' },
    data: corpus('captions.vtt'),
  });
  expect(tracks.status(), await tracks.text()).toBe(200);

  const note = await page.request.post(`/annotations/media/${String(media)}/?${key}`, {
    headers: { 'content-type': 'application/ld+json; profile="http://www.w3.org/ns/anno.jsonld"' },
    data: {
      '@context': 'http://www.w3.org/ns/anno.jsonld',
      type: 'Annotation',
      motivation: 'commenting',
      body: { type: 'TextualBody', value: 'A note only editors see', format: 'text/plain' },
      target: `${harness.baseUrl}/iiif/3/item/${String(item)}/canvas/${String(media)}`,
    },
  });
  expect(note.status(), await note.text()).toBe(201);

  const publicPage = `/s/${SITE_SLUG}/media/${String(media)}`;
  const anonymous = await browser.newContext({ baseURL });
  const reader = await anonymous.newPage();

  await reader.goto(publicPage);
  await expect(reader.getByText('A note only editors see')).toBeVisible();
  await expect(reader.locator('video track[srclang="en"]')).toHaveCount(1);

  await signIn(page, baseURL);
  await page.goto('/#/settings');
  const toggle = page.getByLabel('Show notes to visitors');
  await expect(toggle).toBeChecked();
  await toggle.uncheck();
  await page.getByRole('button', { name: 'Save' }).click();
  await expect(page.getByRole('status')).toHaveText('Settings updated.');

  await reader.reload();
  await expect(reader.getByText('A note only editors see')).toHaveCount(0);
  await expect(reader.locator('video track[srclang="en"]')).toHaveCount(1);

  // Leave the shared database as the next spec expects it.
  await toggle.check();
  await page.getByRole('button', { name: 'Save' }).click();
  await expect(page.getByRole('status')).toHaveText('Settings updated.');
  await anonymous.close();
});
