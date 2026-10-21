/**
 * The media page's notes panel in a real browser, against the real WAP routes.
 *
 * These specs prove the four interactions A-3 owns: an editor writes a note
 * about the whole file, a reader without the grant sees the text and no write
 * controls, a save refused with 412 shows both versions, and a delete after a
 * colleague's change shows their version before it removes anything.
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

/** Sign the browser in with the seeded administrator's password. */
const signIn = async (page: Page, baseURL: string): Promise<void> => {
  const { adminEmail, adminPassword } = readHarness();
  const response = await page.request.post('/api/login', {
    data: { email: adminEmail, password: adminPassword },
    headers: { origin: new URL(baseURL).origin },
  });
  expect(response.status(), await response.text()).toBe(200);
};

/** Write one note about the whole file and wait until the list shows it. */
const writeNote = async (page: Page, text: string): Promise<void> => {
  await page.getByRole('button', { name: 'Add a note about this file' }).click();
  const form = page.locator('.note-form');
  await form.getByLabel('Note').fill(text);
  await form.getByRole('button', { name: 'Save' }).click();
  await expect(page.locator('.media-notes-entries')).toContainText(text);
};

/** Open the only note's entry and wait for its form. */
const openNote = async (page: Page): Promise<void> => {
  await page.locator('.note-open').first().click();
  await expect(page.locator('.note-form textarea')).toBeVisible();
};

test('an editor writes a note about a PDF, and it survives a reload', async ({
  page,
  baseURL,
  diagnostics,
}) => {
  const media = await uploadItem(page.request, 'doc.pdf', 'application/pdf');
  await signIn(page, baseURL);
  await page.goto(`/#/media/${String(media)}`);

  await expect(page.getByRole('heading', { name: 'Notes', exact: true })).toBeVisible();
  await expect(page.getByText('No notes yet.')).toBeVisible();

  const text = 'The scan is missing page 4.';
  await writeNote(page, text);
  await expect(page.getByText('1 note on this file.')).toBeVisible();
  await expect(page.locator('.media-notes-status')).toHaveText('Saved.');
  await expectClean(diagnostics, 'a written whole-file note');

  await page.reload();
  await expect(page.locator('.media-notes-entries')).toContainText(text);
  await expect(page.getByText('1 note on this file.')).toBeVisible();
  await expectClean(diagnostics, 'the note after a reload');
});

test('a reader without the grant reads the notes and holds no write control', async ({
  page,
  browser,
  baseURL,
}) => {
  const media = await uploadItem(page.request, 'doc.pdf', 'application/pdf');
  await signIn(page, baseURL);
  await page.goto(`/#/media/${String(media)}`);
  const text = 'Foxed throughout.';
  await writeNote(page, text);

  const anonymous = await browser.newContext({ baseURL });
  const reader = await anonymous.newPage();
  await reader.goto(`/#/media/${String(media)}`);

  await expect(reader.getByText('You can read these notes. Changing them needs edit rights on this media.')).toBeVisible();
  await expect(reader.locator('.media-notes-entries')).toContainText(text);
  await expect(reader.getByRole('button', { name: 'Add a note about this file' })).toHaveCount(0);
  await expect(reader.locator('.media-notes textarea, .media-notes input')).toHaveCount(0);
  await expect(reader.locator('.media-notes .note-form, .media-notes .note-delete')).toHaveCount(0);

  // The entry still opens, so a reader can read a text the row truncates.
  await reader.locator('.note-open').first().click();
  await expect(reader.locator('.note-full-text')).toHaveText(text);
  await anonymous.close();
});

test('a second editor’s save shows both texts, and this editor’s text is kept', async ({
  page,
  browser,
  baseURL,
  diagnostics,
}) => {
  const media = await uploadItem(page.request, 'doc.pdf', 'application/pdf');
  await signIn(page, baseURL);
  await page.goto(`/#/media/${String(media)}`);
  await writeNote(page, 'The first text.');

  const elsewhere = await browser.newContext({ baseURL });
  const colleague = await elsewhere.newPage();
  await signIn(colleague, baseURL);
  await colleague.goto(`/#/media/${String(media)}`);

  // Both editors read the note before either writes, so both hold the same
  // revision and the second write must be refused.
  await openNote(colleague);
  await openNote(page);
  const mine = page.locator('.note-form textarea');
  await mine.fill('Mine, after theirs.');
  await page.locator('.note-form').getByRole('button', { name: 'Save' }).click();
  await expect(page.locator('.media-notes-status')).toHaveText('Saved.');

  const theirs = colleague.locator('.note-form textarea');
  await theirs.fill('The colleague’s text.');
  await colleague.locator('.note-form').getByRole('button', { name: 'Save' }).click();

  const conflict = colleague.locator('.note-conflict');
  await expect(conflict).toContainText(
    'Someone else changed this note. Their version is shown. Your text is below it.',
  );
  await expect(conflict).toContainText('Mine, after theirs.');
  await expect(conflict.getByLabel('Your text')).toHaveValue('The colleague’s text.');

  await conflict.getByRole('button', { name: 'Save mine over theirs' }).click();
  await expect(colleague.locator('.media-notes-entries')).toContainText(
    'The colleague’s text.',
  );
  await expect(colleague.locator('.media-notes-status')).toHaveText('Saved.');
  await expectClean(diagnostics, 'the editor whose write was refused first');

  await elsewhere.close();
});

test('a delete after a colleague’s change shows their version before removing it', async ({
  page,
  browser,
  baseURL,
}) => {
  const media = await uploadItem(page.request, 'doc.pdf', 'application/pdf');
  await signIn(page, baseURL);
  await page.goto(`/#/media/${String(media)}`);
  await writeNote(page, 'The first text.');

  const elsewhere = await browser.newContext({ baseURL });
  const colleague = await elsewhere.newPage();
  await signIn(colleague, baseURL);
  await colleague.goto(`/#/media/${String(media)}`);

  await openNote(page);
  await openNote(colleague);
  const form = colleague.locator('.note-form');
  await form.getByLabel('Note').fill('The colleague’s change.');
  await form.getByRole('button', { name: 'Save' }).click();
  await expect(colleague.locator('.media-notes-status')).toHaveText('Saved.');

  // The first editor still holds the revision from before that save.
  const mine = page.locator('.note-form');
  await mine.getByRole('button', { name: 'Delete' }).click();
  await mine.getByRole('button', { name: 'Delete this note' }).click();

  const conflict = page.locator('.note-conflict');
  await expect(conflict).toContainText(
    'Someone changed this note after you opened it. Their version is shown.',
  );
  await expect(conflict).toContainText('The colleague’s change.');

  await conflict.getByRole('button', { name: 'Delete their version' }).click();
  await expect(page.locator('.media-notes-status')).toHaveText('Deleted.');
  await expect(page.locator('.media-notes-entries')).toHaveCount(0);
  await expect(page.getByText('No notes yet.')).toBeVisible();

  await elsewhere.close();
});
