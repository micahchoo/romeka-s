/**
 * The time editor in a real browser, on the corpus audio file.
 *
 * The corpus recording is 0.35 s long (measured with ffprobe), so the range
 * here sits inside it: the five seconds **Note from here** proposes are capped
 * at the length, and a note from 0:00.2 to 0:00.3 is the shortest range the
 * file can carry. Clicking its entry seeks the stage's player to its start.
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
  const document: unknown = await answer.json();
  created.push(numberField(record(document, 'item'), 'o:id', 'item'));
  return firstMediaId(document, name);
};

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

test('an editor marks a range of the recording and the entry plays it', async ({
  page,
  baseURL,
  diagnostics,
}) => {
  const media = await uploadItem(page.request, 'audio.mp3', 'audio/mpeg');
  await signIn(page, baseURL);
  await page.goto(`/#/media/${String(media)}`);

  const player = page.locator('audio.media-stage-player');
  await expect(player).toBeVisible();
  await page.getByRole('button', { name: 'Note from here' }).click();

  // The five seconds Note from here proposes are capped at the 0.35 s length.
  const start = page.getByLabel('Start', { exact: true });
  const end = page.getByLabel('End', { exact: true });
  await expect(start).toHaveValue('0:00');
  await expect(end).toHaveValue('0:00.4');

  await start.fill('0:00.2');
  await end.fill('0:00.3');
  const form = page.locator('.note-form');
  await form.getByLabel('Note').fill('A rustle at the start');
  await form.getByRole('button', { name: 'Save' }).click();

  const entries = page.locator('.media-notes-entries');
  await expect(entries).toContainText('0:00.2 to 0:00.3');
  await expect(entries).toContainText('A rustle at the start');
  await expect(page.locator('.media-notes-status')).toHaveText('Saved.');
  await expectClean(diagnostics, 'a saved time note');

  await page.locator('.note-open').first().click();
  await expect
    .poll(async () =>
      player.evaluate((element) => (element instanceof HTMLMediaElement ? element.currentTime : -1)),
    )
    .toBeCloseTo(0.2, 1);

  await page.reload();
  await expect(page.locator('.media-notes-entries')).toContainText('A rustle at the start');
  await expect(page.locator('.media-notes-entries')).toContainText('0:00.2 to 0:00.3');
});
